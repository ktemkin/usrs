//! Helper for working with C callbacks for async USB functions.

use std::ffi::c_void;

use io_kit_sys::ret::IOReturn;

use crate::{backend::macos::iokit::unleak_from_iokit, UsbResult};

use super::iokit::IOKitResultExtension;

pub(crate) type CallbackRefconType = dyn FnOnce(UsbResult<usize>);

/// Terrifying bridge helper that allows IOKit to call a Rust callback.
pub(crate) unsafe extern "C" fn delegate_iousb_callback(
    callback: *mut c_void, // Actually a Box<CallbackRefconType>.
    result: IOReturn,
    total_length: *mut c_void,
) {
    // Demangle our type information, since IOKit's mangled it real nicely for us.
    let total_length = total_length as usize;
    let callback: Box<CallbackRefconType> = unleak_from_iokit(callback);

    let callback_raw = Box::into_raw(callback);
    let callback = Box::from_raw(callback_raw);

    // Finally, call back the callback we were passed.
    callback(UsbResult::from_io_return_and_value(result, total_length));
}
