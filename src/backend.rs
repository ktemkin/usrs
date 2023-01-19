//! Trait and factory for our per-OS backends.
//! Backends can (and will) contain unsafe code, but they expose a safe interface here.

use std::any::Any;
use std::rc::Rc;
use std::time::Duration;

use crate::device::{Device, DeviceInformation};
use crate::error::UsbResult;

#[cfg(target_os = "macos")]
mod macos;

/// Trait that collects methods provided by backend USB-device information.
pub trait BackendDevice: std::fmt::Debug {
    fn as_mut_any(&mut self) -> &mut dyn Any;
    fn as_any(&self) -> &dyn Any;
}

/// Trait that unifies all of our OS-specific backends.
pub trait Backend: std::fmt::Debug {
    /// Returns a collection of device information for all devices present on the system.
    fn get_devices(&self) -> UsbResult<Vec<DeviceInformation>>;

    /// Opens a raw USB device, and returns a backend-specific wrapper around the device.
    fn open(&self, information: &DeviceInformation) -> UsbResult<Box<dyn BackendDevice>>;

    /// Releases the kernel driver associated with the given device, if possible.
    fn release_kernel_driver(&self, interface: u8) -> UsbResult<()>;

    /// Attempts to claim an interface on the given device.
    fn claim_interface(&self, interface: u8) -> UsbResult<()>;

    /// Performs an IN control request.
    /// Returns the amount actually read.
    fn control_read(
        &self,
        device: &Device,
        request_type: u8,
        request_number: u8,
        value: u16,
        index: u16,
        target: &mut [u8],
        timeout: Option<Duration>,
    ) -> UsbResult<usize>;

    // Performs an OUT control request.
    fn control_write(
        &self,
        device: &Device,
        request_type: u8,
        request_number: u8,
        value: u16,
        index: u16,
        data: &[u8],
        timeout: Option<Duration>,
    ) -> UsbResult<()>;

    // TODO:
    // - Non-control read.
    // - Non-control write.
}

/// Creates a default backend implementation for MacOS machines.
#[cfg(target_os = "macos")]
pub fn create_default_backend() -> UsbResult<Rc<dyn Backend>> {
    Ok(Rc::new(macos::MacOsBackend::new()?))
}
