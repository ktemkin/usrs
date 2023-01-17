//! Helpers for working with IOKit.

use std::ffi::{c_void, CStr, CString};

use core_foundation_sys::{
    number::{kCFNumberSInt64Type, CFNumberGetValue, CFNumberRef},
    string::{kCFStringEncodingUTF8, CFStringGetCStringPtr, CFStringRef},
};
use io_kit_sys::{
    kIORegistryIterateParents, kIORegistryIterateRecursively, keys::kIOServicePlane,
    types::io_iterator_t, IOObjectRelease, IORegistryEntrySearchCFProperty, CFSTR,
};
use log::error;

use crate::error::{Error, UsbResult};

// Rustified version of the CFSTR C macro.
macro_rules! cfstr {
    ($string:expr) => {{
        let cstr = CString::new($string).unwrap();
        CFSTR(cstr.as_ptr())
    }};
}
pub(crate) use cfstr;

// Wrapper for an IOKit IO-object that automatically drops it.
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
}

impl Drop for IoObject {
    fn drop(&mut self) {
        unsafe {
            IOObjectRelease(self.object);
        }
    }
}

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
pub(crate) fn string_from_cf_string(string_ref: CFStringRef) -> UsbResult<String> {
    unsafe {
        // Promote a null pointer error to a slightly nicer panic.
        if string_ref.is_null() {
            panic!("something passed a null pointer to string_from_cf_string T_T");
        }

        let c_string = CFStringGetCStringPtr(string_ref, kCFStringEncodingUTF8);
        if c_string.is_null() {
            error!("Could not get a string value out of a CFStringRef!");
            return Err(Error::UnspecifiedOsError);
        }

        Ok(CStr::from_ptr(c_string).to_string_lossy().to_string())
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
        Ok(Some(string_from_cf_string(raw_value)?))
    }
}

// Wrapper for an IOKit IO-pointer that automatically drops it.
/*
pub(crate) struct IoReference<T> {
    reference: T,
}

impl<T> IoReference<T> {
    pub(crate) fn new(reference: T) -> Self {
        return IoReference { reference };
    }

    /// Fetches the inner pointer for passing to IOKit functions.
    pub(crate) fn get(&self) -> T {
        return self.reference;
    }
}

impl<T> Drop for IoReference {
    fn drop(&mut self) {
        unsafe {}
    }
}
*/
