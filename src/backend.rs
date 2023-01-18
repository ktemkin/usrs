//! Trait and factory for our per-OS backends.
//! Backends can (and will) contain unsafe code, but they expose a safe interface here.

use crate::device::DeviceInformation;
use crate::error::UsbResult;

#[cfg(target_os = "macos")]
mod macos;

/// Trait that collects methods provided by backend USB-device information.
pub trait BackendDevice: std::fmt::Debug {}

/// Trait that unifies all of our OS-specific backends.
pub trait Backend: std::fmt::Debug {
    /// Returns a collection of device information for all devices present on the system.
    fn get_devices(&mut self) -> UsbResult<Vec<DeviceInformation>>;

    /// Opens a raw USB device, and returns a backend-specific wrapper around the device.
    fn open(&mut self, information: &DeviceInformation) -> UsbResult<Box<dyn BackendDevice>>;

    // TODO:
    // - Control read.
    // - Control write.
    // - Non-control read.
    // - Non-control write.
}

/// Creates a default backend implementation for MacOS machines.
#[cfg(target_os = "macos")]
pub fn create_default_backend() -> UsbResult<Box<dyn Backend>> {
    return Ok(Box::new(macos::MacOsBackend::new()?));
}
