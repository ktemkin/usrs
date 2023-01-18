//! Helpers for working with IOKit.

use std::ffi::{c_void, CStr, CString};

use core_foundation_sys::{
    number::{kCFNumberSInt64Type, CFNumberGetValue, CFNumberRef},
    string::{kCFStringEncodingUTF8, CFStringGetCStringPtr, CFStringRef},
    uuid::CFUUIDBytes,
};
use io_kit_sys::{
    kIORegistryIterateParents, kIORegistryIterateRecursively, keys::kIOServicePlane,
    types::io_iterator_t, IOObjectRelease, IORegistryEntrySearchCFProperty, CFSTR,
};
use log::error;

use super::iokit_c::{self, CFUUIDGetUUIDBytes, IOCFPlugInInterface};
use crate::error::{Error, UsbResult};

//
// Support declarations.
// These determine which versions of macOS we support, so they should be chosen carefully.
//

/// Alias that select the "version 500" (IOKit 5.0.0) version of UsbDevice, which means
/// that we support macOS versions back to 10.7.3, which is currently every version that Rust
/// supports. Use this instead of touching the iokit_c structure; this may be bumped to
/// (compatible) newer versions of the struct as Rust's support changes.
pub(crate) type UsbDevice = iokit_c::IOUSBDeviceStruct500;

pub(crate) fn usb_device_type_id() -> CFUUIDBytes {
    unsafe { CFUUIDGetUUIDBytes(iokit_c::kIOUSBDeviceInterfaceID500()) }
}

//
// Wrappers around IOKit types.
//

/// Wrapper for an IOKit IO-object that automatically drops it.
#[derive(Debug)]
pub(crate) struct IoObject {
    object: u32,
}

impl IoObject {
    pub(crate) fn new(object: u32) -> Self {
        return IoObject { object };
    }

    /// Fetches the inner handle for passing to IOKit functions.
    pub(crate) fn get(&self) -> u32 {
        return self.object;
    }

    /// Returns true iff the object has been created incorrectly.
    /// Use to maintain internal consistency.
    pub(crate) fn is_invalid(&self) -> bool {
        return self.object == 0;
    }
}

impl Drop for IoObject {
    fn drop(&mut self) {
        unsafe {
            IOObjectRelease(self.object);
        }
    }
}

/// Wrapper around a **IOCFPluginInterface that automatically releases it.
#[derive(Debug)]
pub(crate) struct PluginInterface {
    interface: *mut *mut IOCFPlugInInterface,
}

impl PluginInterface {
    pub(crate) fn new(interface: *mut *mut IOCFPlugInInterface) -> Self {
        return Self { interface };
    }

    /// Fetches the inner pointer for passing to IOKit functions.
    pub(crate) fn get(&self) -> *mut *mut IOCFPlugInInterface {
        return self.interface;
    }
}

impl Drop for PluginInterface {
    fn drop(&mut self) {
        unsafe {
            (*(*self.interface)).Release.unwrap()(self.interface as *mut c_void);
        }
    }
}

// Wrapper around a **UsbDevice that helps us poke at its innards.
#[derive(Debug)]
pub(crate) struct OsDevice {
    device: *mut *mut UsbDevice,

    /// True iff the device is currently open.
    is_open: bool,
}

#[allow(dead_code)]
impl OsDevice {
    pub(crate) fn new(device: *mut *mut UsbDevice) -> Self {
        return Self {
            device,
            is_open: false,
        };
    }

    /// Fetches the inner pointer for passing to IOKit functions.
    /// You probably should use one of the methods below, instead.
    pub(crate) fn get(&self) -> *mut *mut UsbDevice {
        return self.device;
    }

    /// Closes the active USB device.
    pub fn close(&mut self) {
        // If we're already closed, we're done!
        if !self.is_open {
            return;
        }

        // Otherwise, close ourselves.
        unsafe {
            let close = (**self.device).USBDeviceClose.unwrap();
            close(self.device as *mut c_void);

            self.is_open = false;
        }
    }
}

impl Drop for OsDevice {
    fn drop(&mut self) {
        unsafe {
            // If the device is still open, close it...
            self.close();

            // ... and decrease macOS's refcount, so the device can be dealloc'd.
            let release = (**self.device).Release.unwrap();
            release(self.device as *mut c_void);
        }
    }
}

//
// Helpers for working with CoreFoundation / IOKit types.
//

/// Rustified version of the CFSTR C macro.
macro_rules! cfstr {
    ($string:expr) => {{
        let cstr = CString::new($string).unwrap();
        CFSTR(cstr.as_ptr())
    }};
}
pub(crate) use cfstr;

/// Converts a CFNumberRef to a Rust integer.
pub(crate) fn number_from_cf_number<T: TryFrom<u64>>(number_ref: CFNumberRef) -> UsbResult<T> {
    unsafe {
        let mut result: u64 = 0;

        // Promote a null pointer error to a slightly nicer panic.
        if number_ref.is_null() {
            panic!("something passed a null pointer to number_from_cf_number T_T");
        }

        let succeeded = CFNumberGetValue(
            number_ref,
            kCFNumberSInt64Type,
            &mut result as *mut u64 as *mut c_void,
        );
        if !succeeded {
            error!("Failed to convert a NumberRef into a CFNumber!");
            return Err(Error::UnspecifiedOsError);
        }

        result.try_into().map_err(|_| Error::UnspecifiedOsError)
    }
}

/// Converts a raw CFString into a Rust string.
pub(crate) fn string_from_cf_string(string_ref: CFStringRef) -> UsbResult<Option<String>> {
    unsafe {
        // Promote a null pointer error to a slightly nicer panic.
        if string_ref.is_null() {
            panic!("something passed a null pointer to string_from_cf_string T_T");
        }

        let c_string = CFStringGetCStringPtr(string_ref, kCFStringEncodingUTF8);
        if c_string.is_null() {
            return Ok(None);
        }

        Ok(Some(CStr::from_ptr(c_string).to_string_lossy().to_string()))
    }
}

/// Queries IOKit and fetches a device property from the IORegistry.
/// Accepts a usb_device_iterator and the property name.
pub(crate) fn get_iokit_numeric_device_property<T: TryFrom<u64>>(
    device: io_iterator_t,
    property: &str,
) -> UsbResult<T> {
    unsafe {
        let service_plane: *mut i8 = kIOServicePlane as *mut i8;

        let raw_value = IORegistryEntrySearchCFProperty(
            device,
            service_plane,
            cfstr!(property),
            std::ptr::null(),
            kIORegistryIterateRecursively | kIORegistryIterateParents,
        ) as CFNumberRef;
        if raw_value.is_null() {
            error!("Failed to read numeric device property {}!", property);
            return Err(Error::UnspecifiedOsError);
        }
        number_from_cf_number::<T>(raw_value)
    }
}

/// Queries IOKit and fetches a device property from the IORegistry.
/// Accepts a usb_device_iterator and the property name.
pub(crate) fn get_iokit_string_device_property(
    device: io_iterator_t,
    property: &str,
) -> UsbResult<Option<String>> {
    unsafe {
        let service_plane: *mut i8 = kIOServicePlane as *mut i8;

        let raw_value = IORegistryEntrySearchCFProperty(
            device,
            service_plane,
            cfstr!(property),
            std::ptr::null(),
            kIORegistryIterateRecursively | kIORegistryIterateParents,
        ) as CFStringRef;
        if raw_value.is_null() {
            return Ok(None);
        }

        Ok(string_from_cf_string(raw_value)?)
    }
}
