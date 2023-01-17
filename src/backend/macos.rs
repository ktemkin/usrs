//! Core, low-level functionality for macOS.

use crate::error::UsbResult;
use super::{Backend, DeviceInformation};

/// Per-OS data for the MacOS backend.
pub struct MacOsBackend {

}

impl MacOsBackend {

    pub fn new() -> UsbResult<MacOsBackend> {
        return Ok(MacOsBackend{});
    }
}


impl Backend for MacOsBackend {

    fn get_devices(&self) -> Vec<DeviceInformation> {
        todo!();
    }

}
