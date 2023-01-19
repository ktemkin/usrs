//! Tools for working with low-level macOS device interfaces.

use std::ffi::c_void;

use core_foundation_sys::base::SInt32;
use log::error;

use crate::{Error, UsbResult};

use super::{
    iokit::{
        self, usb_interface_type_id, IOKitEmptyResultExtension, IoService, OsInterface,
        PluginInterface,
    },
    iokit_c::{
        kIOCFPlugInInterfaceID, kIOUsbInterfaceUserClientTypeID, IOCFPlugInInterface,
        IOCreatePlugInInterfaceForService,
    },
};

// Fetches the next OsInterface from a given interface iterator.
pub(crate) fn interface_from_service(
    interface_service: IoService,
    interface_index: u8,
) -> UsbResult<OsInterface> {
    unsafe {
        // Promote an "invalid iterator" error from undefined behavior to
        // the much more defined behavior of panic'ing.
        if interface_service.is_invalid() {
            panic!("interface_from_service in the macOS backend got an invalid service!");
        }

        // Ask macOS to give us the interface plugin, which is capable of creating our actual USB
        // interface. Whee, indirection.~
        let mut _score: SInt32 = 0;
        let mut raw_interface_plugin: *mut *mut IOCFPlugInInterface = std::ptr::null_mut();
        UsbResult::from_io_return(IOCreatePlugInInterfaceForService(
            interface_service.get(),
            kIOUsbInterfaceUserClientTypeID(),
            kIOCFPlugInInterfaceID(),
            &mut raw_interface_plugin,
            &mut _score,
        ))?;

        // If we didn't actually get the interface plugin, despite our apparent success,
        // convert this to an _unspecified_ IO error. T_T
        if raw_interface_plugin.is_null() {
            error!("IOKit indicated it successfully created a Interface PlugInInterface, but the pointer was NULL");
            return Err(Error::UnspecifiedOsError);
        }

        // Handle scoping/dropping for our device interface.
        let interface_plugin = PluginInterface::new(raw_interface_plugin);

        // Finally, get the actual UsbInterface we care about.
        let mut raw_interface: *mut *mut iokit::UsbInterface = std::ptr::null_mut();
        let query_interface = (**interface_plugin.get()).QueryInterface.unwrap();

        // We need to pass &raw_device into a **void, which will let it populate the **UsbDevice.
        // This API is _wild_.
        let raw_interface_ptr = &mut raw_interface as *mut *mut *mut iokit::UsbInterface;
        query_interface(
            interface_plugin.get() as *mut c_void,
            usb_interface_type_id(),
            raw_interface_ptr as *mut *mut c_void,
        );

        // macOS claims that call will never fail, and will always produce a valid pointer.
        // We don't trust it, so we're going to panic if it's lied to us.
        if raw_interface.is_null() {
            panic!("query_interface for interface returned a null pointer, which Apple says is impossible");
        }

        // Finally, package up the raw interface into its wrapper, and return it.
        Ok(OsInterface::new(raw_interface, interface_index))
    }
}
