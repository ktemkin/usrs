//! Abstraction over the OS/host's USB functionality.

use std::rc::Rc;

use crate::backend::{create_default_backend, Backend};
use crate::device::{Device, DeviceInformation, DeviceSelector};
use crate::error::{self, UsbResult};

/// Representation of a USB host: that is, the thing (e.g. the OS) that talks to
/// USB devices. This is typically an encapsulation of your OS connection.
pub struct Host {
    /// The backend used to provide the functions for this Host.
    backend: Rc<dyn Backend>,
}

impl Host {
    /// Creates a new Host, using the backend appropriate for the current platform.
    pub fn new() -> UsbResult<Self> {
        let backend = create_default_backend()?;
        Self::new_from_backend(backend)
    }

    /// Creates a new Host, from a custom backend; this allows the library to be
    /// used in contexts we don't yet support. (If you're nice, you might consider PR'ing
    /// your backend -- that'll make it our problem, rather than yours~.)
    ///
    /// Most of the time, you want [new].
    pub fn new_from_backend(backend: Rc<dyn Backend>) -> UsbResult<Self> {
        Ok(Host { backend })
    }

    /// Helper for [device] and [devices]; enumerates one or more devices matching a selector.
    fn enumerate_devices(
        &mut self,
        selector: &DeviceSelector,
        single_device: bool,
    ) -> UsbResult<Vec<DeviceInformation>> {
        let mut matching_devices: Vec<DeviceInformation> = vec![];

        // Get a list of all devices...
        let all_devices = self.backend.get_devices()?;

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
    pub fn device(&mut self, selector: &DeviceSelector) -> UsbResult<DeviceInformation> {
        let mut candidates = self.enumerate_devices(selector, true)?;
        candidates.pop().ok_or(error::Error::DeviceNotFound)
    }

    /// Finds devices attached to the system, filtering by one or more criteria.
    pub fn devices(&mut self, selector: &DeviceSelector) -> UsbResult<Vec<DeviceInformation>> {
        self.enumerate_devices(selector, false)
    }

    /// Returns all devices currently connected to the system.
    pub fn all_devices(&mut self) -> UsbResult<Vec<DeviceInformation>> {
        self.devices(&Default::default())
    }

    /// Opens a device given its device information.
    pub fn open(&mut self, information: &DeviceInformation) -> UsbResult<Device> {
        // Ask our backend to open a device for us...
        let backend_device = self.backend.open(information)?;

        // FIXME: actually open the device, here, instead of having the backend do it?
        Ok(Device::from_backend_device(
            backend_device,
            Rc::clone(&self.backend),
        ))
    }
}

/// Returns the first device matching the given selector.
/// Convenience form that implicitly constructs (and destroys) a Host object.
pub fn device(selector: &DeviceSelector) -> UsbResult<DeviceInformation> {
    Host::new()?.device(selector)
}

/// Finds devices matching the given selector.
/// Convenience form that implicitly constructs (and destroys) a Host object.
pub fn devices(selector: &DeviceSelector) -> UsbResult<Vec<DeviceInformation>> {
    Host::new()?.devices(selector)
}

/// Returns all devices currently connected to the system.
/// Convenience form that implicitly constructs (and destroys) a Host object.
pub fn all_devices() -> UsbResult<Vec<DeviceInformation>> {
    Host::new()?.all_devices()
}

/// Opens a device given its device information.
/// Convenience form that implicitly constructs (and destroys) a Host object.
pub fn open(info: &DeviceInformation) -> UsbResult<Device> {
    Host::new()?.open(info)
}
