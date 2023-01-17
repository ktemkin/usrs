//! Trait and factory for our per-OS backends.
//! Backends can (and will) contain unsafe code, but they expose a safe interface here.

use crate::device::DeviceInformation;
use crate::error::UsbResult;

#[cfg(target_os = "macos")]
mod macos;

/// Trait that unifies all of our OS-specific backends.
pub trait Backend {
    /// Returns a collection of device information for all devices present on the system.
    fn get_devices(&self) -> UsbResult<Vec<DeviceInformation>>;

    // TODO:
    // - Method to open a device given its DeviceInformation.
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
