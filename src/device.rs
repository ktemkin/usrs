//! Interface for working with USB devices.

/// Contains known information for an unopened device.
#[derive(Debug)]
pub struct DeviceInformation {
    /// The Vendor ID (idVendor) assigned to the device.
    pub vendor_id: u16,

    /// The Product ID (idProduct) associated with the device.
    pub product_id: u16,

    /// The serial string associated with the device, if we were able to get one.
    pub serial: Option<String>,
}

/// Information used to find a specific device.
#[derive(Debug, Default)]
pub struct DeviceSelector {
    /// If specified, searches for a device with the given VID.
    pub vendor_id: Option<u16>,

    /// If specified, searches for a device with the given PID.
    pub product_id: Option<u16>,
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

        return true;
    }
}
