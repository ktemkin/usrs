//! Routines for querying IOKit for USB devices.

use super::iokit::{get_iokit_numeric_device_property, get_iokit_string_device_property, IoObject};
use crate::{
    error::{Error, UsbResult},
    DeviceInformation,
};

use io_kit_sys::{kIOMasterPortDefault, IOIteratorNext, IOServiceMatching};
use io_kit_sys::{ret::kIOReturnSuccess, usb::lib::kIOUSBDeviceClassName};
use io_kit_sys::{types::io_iterator_t, IOServiceGetMatchingServices};

/// Type alias to make it clear when our u32 handle is an IoIterator. It's clear, right?
type IoIterator = IoObject;

/// Type alias to make it clear when our u32 handle is an IoService.
type IoService = IoObject;

/// IOKit iterator object that walks all connected USB devices.
fn get_device_iterator() -> UsbResult<IoIterator> {
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

    // ... its serial string ...
    let serial = get_iokit_string_device_property(device, "USB Serial Number")?;

    Ok(DeviceInformation {
        vendor_id,
        product_id,
        serial: Some(serial),
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
            let device_info = get_device_information(device)?;
            devices.push(device_info)
        }
    }

    Ok(devices)
}
