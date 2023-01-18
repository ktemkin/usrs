//! Interface for working with USB devices.

use crate::backend::BackendDevice;

/// Contains known information for an unopened device.
#[derive(Debug, Default)]
pub struct DeviceInformation {
    /// The Vendor ID (idVendor) assigned to the device.
    pub vendor_id: u16,

    /// The Product ID (idProduct) associated with the device.
    pub product_id: u16,

    /// The serial string associated with the device, if we were able to get one.
    pub serial: Option<String>,

    /// The vendor string associated with the device, if and only if the OS has read it.
    pub vendor: Option<String>,

    /// The product string associated with the device, if and only if the OS has read it.
    pub product: Option<String>,

    /// Numeric field for backend use; can be used to contain a hint used to re-find the device for opening.
    pub(crate) backend_numeric_location: Option<u64>,

    /// String field for backend use; can be used to contain a hint used to re-find the device for opening.
    pub(crate) backend_string_location: Option<String>,
}

impl DeviceInformation {
    /// Allows external backend implementers to create a DeviceInformation object.
    ///
    /// This should only be used if you're implementing your own backend; otherwise, you should
    /// use the DeviceInformation you get from enumeration. The internal backends *will* panic if
    /// you pass them self-constructed device information.
    ///
    /// (Of course, if you're familiar enough with our internals, you're going to ignore me,
    /// right? I'm just a docstring; what the hell do I know?)
    pub fn new(
        vendor_id: u16,
        product_id: u16,
        serial: Option<String>,
        vendor: Option<String>,
        product: Option<String>,
    ) -> DeviceInformation {
        DeviceInformation {
            vendor_id,
            product_id,
            serial,
            vendor,
            product,
            ..Default::default()
        }
    }
}

/// Information used to find a specific device.
#[derive(Debug, Default)]
pub struct DeviceSelector {
    /// If specified, searches for a device with the given VID.
    pub vendor_id: Option<u16>,

    /// If specified, searches for a device with the given PID.
    pub product_id: Option<u16>,

    /// The serial string associated with the device.
    pub serial: Option<String>,
}

impl DeviceSelector {
    pub fn matches(&self, device: &DeviceInformation) -> bool {
        // Oh, gods.
        //
        // This could be made so much tinier if we wanted to commit terrible sins.
        // We don't, so enjoy this wonderful boilerplate.

        // Check VID.
        if let Some(vid) = self.vendor_id {
            if vid != device.vendor_id {
                return false;
            }
        }

        // Check PID.
        if let Some(pid) = self.product_id {
            if pid != device.product_id {
                return false;
            }
        }

        // Check serial.
        if self.serial.is_some() {
            if self.serial != device.serial {
                return false;
            }
        }

        return true;
    }
}

/// Object for working with an -opened- USB device.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Device {
    /// The per-backend inner device interface.
    backend_device: Box<dyn BackendDevice>,
}

impl Device {
    /// Wraps a backend device abstraction with our user-facing device.
    ///
    /// This should only be used if you're implementing a custom device backend; otherwise,
    /// you should get your Device from Host::open().
    pub fn from_backend_device(backend_device: Box<dyn BackendDevice>) -> Device {
        Device { backend_device }
    }
}
