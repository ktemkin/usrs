//! Universal Serial Rust -- tools for working with USB from Rust.

use std::{cell::RefCell, sync::Arc};

pub use device::{DeviceInformation, DeviceSelector};
pub use error::{Error, UsbResult};
pub use host::{all_devices, device, devices, open, Host};

#[cfg(feature = "async")]
pub use convenience::create_read_buffer;

pub mod backend;
pub mod convenience;
pub mod device;
pub mod error;
pub mod host;
pub mod request;

#[cfg(feature = "async")]
pub mod futures;

/// Type used for asynchronous read operations.
#[cfg(feature = "async")]
pub type ReadBuffer = Arc<RefCell<dyn AsMut<[u8]>>>;

/// Type used for asynchronous write operations.
#[cfg(feature = "async")]
pub type WriteBuffer = Arc<dyn AsRef<[u8]>>;

/// Type used for callbacks in the callback-model async functions.
#[cfg(feature = "callbacks")]
pub type AsyncCallback = Box<dyn FnOnce(UsbResult<usize>)>;
