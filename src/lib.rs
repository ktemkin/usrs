//! Universal Serial Rust -- tools for working with USB from Rust.
use backend::{create_default_backend, Backend};

pub use device::{DeviceInformation, DeviceSelector};
use error::UsbResult;

pub mod backend;
pub mod device;
pub mod error;

/// Representation of a USB host: that is, the thing (e.g. the OS) that talks to
/// USB devices. This is typically an encapsulation of your OS connection.
pub struct UsbHost {
    /// The backend used to provide the functions for this UsbHost.
    backend: Box<dyn Backend>,
}

impl UsbHost {
    /// Creates a new UsbHost, using the backend appropriate for the current platform.
    pub fn new() -> UsbResult<Self> {
        let backend = create_default_backend()?;
        Ok(Self::new_from_backend(backend)?)
    }

    /// Creates a new UsbHost, from a custom backend; this allows the library to be
    /// used in contexts we don't yet support. (If you're nice, you might consider PR'ing
    /// your backend -- that'll make it our problem, rather than yours~.)
    ///
    /// Most of the time, you want [new].
    pub fn new_from_backend(backend: Box<dyn Backend>) -> UsbResult<Self> {
        return Ok(UsbHost { backend });
    }

    /// Helper for [device] and [devices]; enumerates one or more devices matching a selector.
    fn enumerate_devices(
        &self,
        selector: &DeviceSelector,
        single_device: bool,
    ) -> UsbResult<Vec<DeviceInformation>> {
        let mut matching_devices: Vec<DeviceInformation> = vec![];

        // Get a list of all devices...
        let all_devices = self.backend.get_devices();

        // .... and then filter it down.
        for device in all_devices {
            if selector.matches(&device) {
                matching_devices.push(device);

                // If we're only returning a single device, end here.
                if single_device {
                    return Ok(matching_devices);
                }
            }
        }

        Ok(matching_devices)
    }

    /// Returns the first device matching the given selector.
    pub fn device(&self, selector: &DeviceSelector) -> UsbResult<DeviceInformation> {
        let mut candidates = self.enumerate_devices(selector, true)?;
        candidates.pop().ok_or(error::Error::DeviceNotFound)
    }

    /// Finds devices attached to the system, filtering by one or more criteria.
    pub fn devices(&self, selector: &DeviceSelector) -> UsbResult<Vec<DeviceInformation>> {
        self.enumerate_devices(selector, false)
    }

    /// Returns all devices currently connected to the system.
    pub fn all_devices(&self) -> UsbResult<Vec<DeviceInformation>> {
        return self.devices(&Default::default());
    }
}
