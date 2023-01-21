//! Trait and factory for our per-OS backends.
//! Backends can (and will) contain unsafe code, but they expose a safe interface here.

use std::any::Any;
use std::rc::Rc;
use std::time::{Duration, SystemTime};

use crate::device::{Device, DeviceInformation};
use crate::error::UsbResult;
use crate::{ReadBuffer, WriteBuffer};

#[cfg(target_os = "macos")]
mod macos;

/// Trait that collects methods provided by backend USB-device information.
pub trait BackendDevice: std::fmt::Debug {
    fn as_mut_any(&mut self) -> &mut dyn Any;
    fn as_any(&self) -> &dyn Any;
}

/// Trait that unifies all of our OS-specific backends.
///
/// See [Device] for more detailed documentation for many of these methods,
/// as their signatures are very close to the same.
pub trait Backend: std::fmt::Debug {
    /// Returns a collection of device information for all devices present on the system.
    fn get_devices(&self) -> UsbResult<Vec<DeviceInformation>>;

    /// Opens a raw USB device, and returns a backend-specific wrapper around the device.
    fn open(&self, information: &DeviceInformation) -> UsbResult<Box<dyn BackendDevice>>;

    /// Releases the kernel driver associated with the given device, if possible.
    fn release_kernel_driver(&self, device: &mut Device, interface: u8) -> UsbResult<()>;

    /// Attempts to claim an interface on the given device.
    fn claim_interface(&self, device: &mut Device, interface: u8) -> UsbResult<()>;

    /// Attempts to release the claim held over a given interface.
    fn unclaim_interface(&self, device: &mut Device, interface: u8) -> UsbResult<()>;

    /// Returns the index of the active configuration, or 0 if the device is unconfigured.
    fn active_configuration(&self, device: &Device) -> UsbResult<u8>;

    /// Attempts to select the active configuration for the device.
    fn set_active_configuration(&self, device: &Device, configuration_index: u8) -> UsbResult<()>;

    /// Attempts to bus reset the given device.
    fn reset_device(&self, device: &Device) -> UsbResult<()>;

    /// Attempts to clear the halt condition on a given endpoint address.
    fn clear_stall(&self, device: &Device, endpoint_address: u8) -> UsbResult<()>;

    /// Configures an interface into an alternate setting.
    fn set_alternate_setting(&self, device: &Device, interface: u8, setting: u8) -> UsbResult<()>;

    /// Returns the current USB frame number, and time at which it occurred.
    /// Precision will vary between backends.
    fn current_bus_frame(&self, device: &Device) -> UsbResult<(u64, SystemTime)>;

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

    /// Performs an IN control request.
    fn control_read_nonblocking(
        &self,
        device: &Device,
        request_type: u8,
        request_number: u8,
        value: u16,
        index: u16,
        target: ReadBuffer,
        callback: Box<dyn FnOnce(UsbResult<usize>)>,
        timeout: Option<Duration>,
    ) -> UsbResult<()>;

    /// Performs an OUT control request.
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

    /// Performs an IN control request.
    fn control_write_nonblocking(
        &self,
        device: &Device,
        request_type: u8,
        request_number: u8,
        value: u16,
        index: u16,
        data: WriteBuffer,
        callback: Box<dyn FnOnce(UsbResult<usize>)>,
        timeout: Option<Duration>,
    ) -> UsbResult<()>;

    /// Reads from an endpoint, for e.g. bulk reads.
    fn read(
        &self,
        device: &Device,
        endpoint: u8,
        buffer: &mut [u8],
        timeout: Option<Duration>,
    ) -> UsbResult<usize>;

    /// Writes to an endpoint, for e.g. bulk writes.
    fn write(
        &self,
        device: &Device,
        endpoint: u8,
        data: &[u8],
        timeout: Option<Duration>,
    ) -> UsbResult<()>;

    /// Reads from an endpoint, for e.g. bulk reads. Async.
    fn read_nonblocking(
        &self,
        device: &Device,
        endpoint: u8,
        buffer: ReadBuffer,
        callback: Box<dyn FnOnce(UsbResult<usize>)>,
        timeout: Option<Duration>,
    ) -> UsbResult<()>;

    /// Writes to an endpoint, for e.g. bulk writes. Async.
    fn write_nonblocking(
        &self,
        device: &Device,
        endpoint: u8,
        data: WriteBuffer,
        callback: Box<dyn FnOnce(UsbResult<usize>)>,
        timeout: Option<Duration>,
    ) -> UsbResult<()>;

    // TODO:
    // - Isochronous???
}

/// Creates a default backend implementation for MacOS machines.
#[cfg(target_os = "macos")]
pub fn create_default_backend() -> UsbResult<Rc<dyn Backend>> {
    Ok(Rc::new(macos::MacOsBackend::new()?))
}
