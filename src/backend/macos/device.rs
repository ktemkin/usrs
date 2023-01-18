//! Backend tools for opening and working with devices.

use std::{ffi::c_void, time};

use core_foundation_sys::base::SInt32;
use io_kit_sys::{
    ret::{kIOReturnNoResources, kIOReturnSuccess},
    IOIteratorNext,
};
use log::error;

use crate::{
    backend::macos::enumeration::get_device_iterator, backend::BackendDevice, DeviceInformation,
    Error, UsbResult,
};

use super::{
    iokit::{
        self, get_iokit_numeric_device_property, usb_device_type_id, IoObject, OsDevice,
        PluginInterface,
    },
    iokit_c::{
        kIOCFPlugInInterfaceID, kIOUsbDeviceUserClientTypeID, IOCFPlugInInterface,
        IOCreatePlugInInterfaceForService,
    },
};

/// Type alias to make it clear when our u32 handle is an IoService.
type IoService = IoObject;

/// Internal type storing the state for our raw USB device.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct MacOsDevice {
    /// Our low-level MacOS USB device.
    pub(crate) device: OsDevice,
}

impl BackendDevice for MacOsDevice {}

/// Converts an IOIteratorNext result into a backend USB device.
fn open_usb_device_from_io_device(device_service: IoService) -> UsbResult<Box<dyn BackendDevice>> {
    if device_service.is_invalid() {
        panic!("internal inconsistency: got a 0 io-object-handle");
    }

    unsafe {
        // Get the raw USB device associated with the service.
        //
        // NOTE(ktemkin): According to the libusb maintainers, this will sometimes spuriously
        // return `kIOReturnNoResources` for reasons Apple won't explain, usually
        // when a device is freshly plugged in. We'll allow this a few retries,
        // accordingly.
        //
        // [This behavior actually makes sense to me -- when the device is first plugged
        // in, it exists to IOKit, but hasn't been enumerated, yet. Accordingly, the device
        // interface doesn't actually yet exist for us to grab, and/or doesn't yet have the
        // right permissions for us to grab it. MacOS needs to see if a kernel driver binds
        // to it; as its security model won't allow the userland to grab a device that the
        // kernel owns.]
        //
        // If the kIOReturnNoResources persists, it's typically an indication that
        // macOS is preventing us from touching the relevant device due to its security
        // model. This happens when the device has a kernel-mode driver bound to the
        // whole device -- the kernel owns it, and it's unwilling to give it to us.
        //
        for _ in 0..5 {
            let mut _score: SInt32 = 0;
            let mut raw_device_plugin: *mut *mut IOCFPlugInInterface = std::ptr::null_mut();

            // Ask macOS to give us the device plugin, which is capable of creating our actual USB
            // device. Whee, indirection.~
            let rc = IOCreatePlugInInterfaceForService(
                device_service.get(),
                kIOUsbDeviceUserClientTypeID(),
                kIOCFPlugInInterfaceID(),
                &mut raw_device_plugin,
                &mut _score,
            );

            // If we got "no resources", it's possible this is the spurious case above.
            if rc == kIOReturnNoResources {
                std::thread::sleep(time::Duration::from_millis(1));
                continue;
            }

            // For any other error, translate this to a USBResult.
            if rc != kIOReturnSuccess {
                return Err(Error::OsError(rc as i64));
            }

            // If we didn't actually get the device plugin, despite our apparent success,
            // convert this to an _unspecified_ IO error. G'damn.
            if raw_device_plugin.is_null() {
                error!("IOKit indicated it successfully created a PlugInInterface, but the pointer was NULL");
                return Err(Error::UnspecifiedOsError);
            }

            // Handle scoping/dropping for our device interface.
            let device_plugin = PluginInterface::new(raw_device_plugin);

            // Finally, get the actual UsbDevice we care about.
            let mut raw_device: *mut *mut iokit::UsbDevice = std::ptr::null_mut();
            let query_interface = (**device_plugin.get()).QueryInterface.unwrap();

            // We need to pass &raw_device into a **void, which will let it populate the **UsbDevice.
            // This API is _wild_.
            let raw_device_ptr = &mut raw_device as *mut *mut *mut iokit::UsbDevice;
            query_interface(
                device_plugin.get() as *mut c_void,
                usb_device_type_id(),
                raw_device_ptr as *mut *mut c_void,
            );

            // macOS claims that call will never fail, and will always produce a valid pointer.
            // We don't trust it, so we're going to panic if it's lied to us.
            if raw_device.is_null() {
                panic!("query_interface returned a null pointer, which Apple says is impossible");
            }

            // Finally, package up the device we've created as a backend device...
            let backend_device = Box::new(MacOsDevice {
                device: OsDevice::new(raw_device),
            });

            // ... and return it.
            return Ok(backend_device);
        }
    }

    Err(Error::DeviceNotFound)
}

/// Opens a device given the information acquired during enumeration.
pub(crate) fn open_usb_device(
    information: &DeviceInformation,
) -> UsbResult<Box<dyn BackendDevice>> {
    let target_location_id = information
        .backend_numeric_location
        .expect("invalid device_id; did you make this yourself?");

    // NOTE(ktemkin): this process is -strictly- more than is necessary;
    // as macOS offers an ability to open a device by its LocationID. However,
    // at this point, it seems more valuable to me to interface with the least
    // amount of the unsafe-pointer'y/handle'y IOKit code possible; so this is
    // just re-using the iteration method that we've already needed to expose.
    unsafe {
        // Fetch an IOKit iterator over all devices.
        let device_iterator = get_device_iterator()?;

        let mut device;
        while {
            device = IOIteratorNext(device_iterator.get());
            device != 0
        } {
            // Find the macOS location ID for the given device, which uniquely identifies a given
            // device...
            let location_id: UsbResult<u32> =
                get_iokit_numeric_device_property(device, "locationID");

            // Skip any devices that don't have location IDs; as they're not real devices macOS
            // will let us work with -- they're e.g. root hubs or internal-only devices.
            if location_id.is_err() {
                continue;
            }

            // If this isn't the device we're interested in, keep looking.
            if location_id.unwrap() as u64 != target_location_id {
                continue;
            }

            return open_usb_device_from_io_device(IoService::new(device));
        }

        Err(Error::DeviceNotFound)
    }
}
