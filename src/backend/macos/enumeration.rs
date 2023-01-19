//! Routines for querying IOKit for USB devices.

use super::iokit::{
    get_iokit_numeric_device_property, get_iokit_string_device_property, IoIterator, IoObject,
};
use crate::{
    error::{Error, UsbResult},
    DeviceInformation,
};

use io_kit_sys::{kIOMasterPortDefault, IOIteratorNext, IOServiceMatching};
use io_kit_sys::{ret::kIOReturnSuccess, usb::lib::kIOUSBDeviceClassName};
use io_kit_sys::{types::io_iterator_t, IOServiceGetMatchingServices};
use log::debug;

/// IOKit iterator object that walks all connected USB devices.
pub(crate) fn get_device_iterator() -> UsbResult<IoIterator> {
    unsafe {
        // Create a dictionary containing the object-type we want to match...
        let matcher = IOServiceMatching(kIOUSBDeviceClassName);
        if matcher.is_null() {
            panic!("could not allocate an IOKit object; OOM");
        }

        // ... and convert that dictionary into a match-iterator.
        let mut raw_device_iterator: io_iterator_t = 0;
        let rc =
            IOServiceGetMatchingServices(kIOMasterPortDefault, matcher, &mut raw_device_iterator);
        if rc != kIOReturnSuccess {
            return Err(Error::OsError(rc as i64));
        }
        if raw_device_iterator == 0 {
            return Err(Error::DeviceNotFound);
        }

        Ok(IoObject::new(raw_device_iterator))
    }
}

/// Fetches the IOKit information for a given device without opening it.
fn get_device_information(device: io_iterator_t) -> UsbResult<DeviceInformation> {
    // NOTE(ktemkin): While generically, we should only use Official (TM) macOS
    // documented properties, you can get a general idea of what properties are
    // available on each device by running `ioreg -p IOUSB -l`; `ioreg` being the
    // tool that iterates over the IORegistry.

    // Fetch the device's VID / PID...
    let vendor_id: u16 = get_iokit_numeric_device_property(device, "idVendor")?;
    let product_id: u16 = get_iokit_numeric_device_property(device, "idProduct")?;

    // ... its string properties, where we can get them ...
    let serial = get_iokit_string_device_property(device, "USB Serial Number")?;
    let vendor = get_iokit_string_device_property(device, "USB Vendor Name")?;
    let product = get_iokit_string_device_property(device, "USB Product Name")?;

    // ... and its internal identifier, for easy opening.
    let location_id: UsbResult<u32> = get_iokit_numeric_device_property(device, "locationID");

    // If we don't have a location ID, this isn't a real device to macOS.
    //
    // We can query its properties, but otherwise can't touch it.
    // This is the case for e.g. root hubs.
    if location_id.is_err() {
        debug!(
            "Skipping device {:04x}:{:04x} ({:?}/{:?}), as it has no location ID, and thus isn't real to us.",
            vendor_id, product_id, vendor, product
        );
        return Err(Error::DeviceNotReal);
    }

    Ok(DeviceInformation {
        vendor_id,
        product_id,
        serial,
        vendor,
        product,
        backend_numeric_location: Some(location_id.unwrap() as u64),
        ..Default::default()
    })
}

/// Attempts to gather device information from all devices connected to the system.
pub(crate) fn enumerate_devices() -> UsbResult<Vec<DeviceInformation>> {
    let mut devices: Vec<DeviceInformation> = vec![];

    unsafe {
        // Fetch an IOKit iterator over all devices.
        let device_iterator = get_device_iterator();
        if device_iterator.as_ref().err() == Some(&Error::DeviceNotFound) {
            return Ok(devices);
        }
        let device_iterator = device_iterator?;

        let mut device;
        while {
            device = IOIteratorNext(device_iterator.get());
            device != 0
        } {
            let device_info = get_device_information(device);
            match device_info {
                // If the device isn't real to the operating system, we won't consider it.
                // (Root) hub devices, in particular, wind up enumerated to macOS, but aren't
                // accessible in any other way. We'll skip them.
                Err(Error::DeviceNotReal) => (),

                // Otherwise, either capture the device, or propagate the error.
                Ok(device_info) => devices.push(device_info),
                Err(other) => return Err(other),
            }
        }
    }

    Ok(devices)
}
