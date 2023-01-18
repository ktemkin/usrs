//! Core, low-level functionality for macOS.

use self::device::open_usb_device;

use super::{Backend, BackendDevice, DeviceInformation};
use crate::error::UsbResult;

mod device;
mod enumeration;
mod iokit;
mod iokit_c;

/// Per-OS data for the MacOS backend.
#[derive(Debug)]
pub struct MacOsBackend {}

impl MacOsBackend {
    pub fn new() -> UsbResult<MacOsBackend> {
        return Ok(MacOsBackend {});
    }
}

impl Backend for MacOsBackend {
    fn get_devices(&mut self) -> UsbResult<Vec<DeviceInformation>> {
        enumeration::enumerate_devices()
    }

    fn open(&mut self, information: &DeviceInformation) -> UsbResult<Box<dyn BackendDevice>> {
        open_usb_device(information)
    }
}
