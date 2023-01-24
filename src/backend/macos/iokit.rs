//! Helpers for working with IOKit.

use std::{
    ffi::{c_void, CStr, CString},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use core_foundation_sys::{
    number::{kCFNumberSInt64Type, CFNumberGetValue, CFNumberRef},
    runloop::{
        kCFRunLoopDefaultMode, CFRunLoopAddSource, CFRunLoopGetCurrent, CFRunLoopRunInMode,
        CFRunLoopSourceRef,
    },
    string::{kCFStringEncodingUTF8, CFStringGetCStringPtr, CFStringRef},
    uuid::CFUUIDBytes,
};
use io_kit_sys::{
    kIORegistryIterateParents, kIORegistryIterateRecursively, keys::kIOServicePlane, ret::*,
    types::io_iterator_t, IOAsyncCallback1, IOObjectRelease, IORegistryEntrySearchCFProperty,
    CFSTR,
};
use log::{error, warn};

use super::iokit_c::{
    self, kIOUSBFindInterfaceDontCare, kIOUSBNoAsyncPortErr, kIOUSBPipeStalled,
    kIOUSBTransactionTimeout, kIOUSBUnknownPipeErr, AbsoluteTime, CFUUIDGetUUIDBytes,
    IOCFPlugInInterface, IOUSBDevRequest, IOUSBDevRequestTO, IOUSBFindInterfaceRequest, UInt16,
    UInt32, UInt64, UInt8,
};
use crate::error::{self, Error, UsbResult};

//
// Support declarations.
// These determine which versions of macOS we support, so they should be chosen carefully.
//

/// Alias that select the "version 500" (IOKit 5.0.0) version of UsbDevice, which means
/// that we support macOS versions back to 10.7.3, which is currently every version that Rust
/// supports. Use this instead of touching the iokit_c structure; this may be bumped to
/// (compatible) newer versions of the struct as Rust's support changes.
pub(crate) type UsbDevice = iokit_c::IOUSBDeviceStruct500;
pub(crate) type UsbInterface = iokit_c::IOUSBInterfaceStruct500;

pub(crate) fn usb_device_type_id() -> CFUUIDBytes {
    unsafe { CFUUIDGetUUIDBytes(iokit_c::kIOUSBDeviceInterfaceID500()) }
}

pub(crate) fn usb_interface_type_id() -> CFUUIDBytes {
    unsafe { CFUUIDGetUUIDBytes(iokit_c::kIOUSBInterfaceInterfaceID500()) }
}

//
// Helpers for working with IOKit types.
//

/// Helper for calling IOKit function pointers.
macro_rules! call_unsafe_iokit_function {
    ($ptr:expr, $function:ident) => {{
        unsafe {
            let func = (**$ptr).$function.expect("function pointer from IOKit was null");
            func($ptr as *mut c_void)
        }
    }};
    ($ptr:expr, $function:ident, $($args:expr),*) => {{
        unsafe {
            let func = (**$ptr).$function.expect("function pointer from IOKit was null");
            func($ptr as *mut c_void, $($args),*)
        }
    }};
}

//
// Wrappers around IOKit types.
//

/// Type alias to make it clear when our u32 handle is an IoIterator. It's clear, right?
pub(crate) type IoIterator = IoObject;

/// Type alias to make it clear(er) when our u32 handle is a handle to an IO service.
pub(crate) type IoService = IoObject;

/// Wrapper for an IOKit IO-object that automatically drops it.
#[derive(Debug)]
pub(crate) struct IoObject {
    object: u32,
}

impl IoObject {
    pub(crate) fn new(object: u32) -> Self {
        IoObject { object }
    }

    /// Fetches the inner handle for passing to IOKit functions.
    pub(crate) fn get(&self) -> u32 {
        self.object
    }

    /// Returns true iff the object has been created incorrectly.
    /// Use to maintain internal consistency.
    pub(crate) fn is_invalid(&self) -> bool {
        self.object == 0
    }
}

impl Drop for IoObject {
    fn drop(&mut self) {
        unsafe {
            IOObjectRelease(self.object);
        }
    }
}

/// Wrapper around a **IOCFPluginInterface that automatically releases it.
#[derive(Debug)]
pub(crate) struct PluginInterface {
    interface: *mut *mut IOCFPlugInInterface,
}

impl PluginInterface {
    pub(crate) fn new(interface: *mut *mut IOCFPlugInInterface) -> Self {
        Self { interface }
    }

    /// Fetches the inner pointer for passing to IOKit functions.
    pub(crate) fn get(&self) -> *mut *mut IOCFPlugInInterface {
        self.interface
    }
}

impl Drop for PluginInterface {
    fn drop(&mut self) {
        call_unsafe_iokit_function!(self.interface, Release);
    }
}

/// Wrapper around a Notification Port types that automatically close them
/// when necessary.
#[derive(Debug)]
pub(crate) struct NotificationSource {
    /// The notification source used as an async event target, and to pass to
    /// event loops, so we can await async events.
    source: CFRunLoopSourceRef,
}

impl NotificationSource {
    pub(crate) fn new(source: CFRunLoopSourceRef) -> Self {
        Self { source }
    }

    pub(crate) fn source(&self) -> CFRunLoopSourceRef {
        self.source
    }

    /// Creates a run-loop that will run call-backs for this notification-source.
    pub(crate) fn run_event_loop(
        notification_sources: Vec<NotificationSource>,
        termination_flag: Arc<AtomicBool>,
    ) -> UsbResult<()> {
        unsafe {
            // Add each of our notification sources to our event loop...
            let runloop = CFRunLoopGetCurrent();
            for source in notification_sources {
                CFRunLoopAddSource(runloop, source.source(), kCFRunLoopDefaultMode);
            }

            // ... and run it.
            loop {
                // Let the runloop run for our specified "stop granularity", after which it'll
                // pop back here to  check the termination condition.
                const RUNLOOP_STOP_GRANULARITY: Duration = Duration::from_secs(1);
                CFRunLoopRunInMode(
                    kCFRunLoopDefaultMode,
                    RUNLOOP_STOP_GRANULARITY.as_secs_f64(),
                    false as u8,
                );

                // If our device is no longer around, we won't be getting any events -- so we can
                if termination_flag.load(Ordering::Relaxed) {
                    return Ok(());
                }
            }
        }
    }
}

unsafe impl Send for NotificationSource {}

// Wrapper around a **UsbDevice that helps us poke at its innards.
#[derive(Debug)]
pub(crate) struct OsDevice {
    device: *mut *mut UsbDevice,

    /// True iff the device is currently open.
    is_open: bool,
}

// We really only have a pointer to something that's already Send,
// so override Rust's unwillingness to have Send pointers.
unsafe impl Send for OsDevice {}
unsafe impl Sync for OsDevice {}

#[allow(dead_code)]
impl OsDevice {
    pub(crate) fn new(device: *mut *mut UsbDevice) -> Self {
        Self {
            device,
            is_open: false,
        }
    }

    /// Opens the device, allowing the other functions on this type to be used.
    pub fn open(&mut self) -> UsbResult<()> {
        // If we're already open, we're done!
        if self.is_open {
            return Ok(());
        }

        // Otherwise, open the device.
        UsbResult::from_io_return(call_unsafe_iokit_function!(self.device, USBDeviceOpen))?;

        self.is_open = true;
        Ok(())
    }

    /// Applies a configuration to the device.
    pub fn get_configuration(&self) -> UsbResult<u8> {
        let mut configuration: UInt8 = 0;

        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.device,
            GetConfiguration,
            &mut configuration
        ))?;

        Ok(configuration)
    }

    /// Applies a configuration to the device.
    pub fn set_configuration(&self, index: u8) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.device,
            SetConfiguration,
            index
        ))
    }

    /// Attempts to retrieve the current bus-frame number, and a time relative to Jan 1 2001 (00:00 GMT).
    /// Returns (frame, timestamp).
    pub fn get_frame_number(&self) -> UsbResult<(u64, u64)> {
        let mut frame: UInt64 = 0;
        let mut time: AbsoluteTime = AbsoluteTime { lo: 0, hi: 0 };

        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.device,
            GetBusFrameNumber,
            &mut frame,
            &mut time
        ))?;

        let timestamp = (time.hi as u64) << 32 | (time.lo as u64);
        Ok((frame, timestamp))
    }

    /// Attempts to perform a Bus Reset on the device.
    pub fn reset(&self) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(self.device, ResetDevice))
    }

    /// Performs a control request on the device, without wrapping the unsafe behavior of
    /// the contained IOUSbDevRequest. See also [device_request_with_timeout].
    pub fn device_request(&self, request: &mut IOUSBDevRequest) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.device,
            DeviceRequest,
            request
        ))
    }

    /// Performs an async control request on the device, without wrapping the unsafe behavior of
    /// the contained IOUSbDevRequest. See also [device_request_nonblocking_with_timeout].
    pub fn device_request_nonblocking(
        &self,
        request: &mut IOUSBDevRequest,
        callback: IOAsyncCallback1,
        callback_arg: *mut c_void,
    ) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.device,
            DeviceRequestAsync,
            request,
            callback,
            callback_arg
        ))
    }

    /// Performs an async control request on the device, without wrapping the unsafe behavior of
    /// the contained IOUSbDevRequest. See also [device_request_nonblocking].
    pub fn device_request_nonblocking_with_timeout(
        &self,
        request: &mut IOUSBDevRequestTO,
        callback: IOAsyncCallback1,
        callback_arg: *mut c_void,
    ) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.device,
            DeviceRequestAsyncTO,
            request,
            callback,
            callback_arg
        ))
    }

    /// Performs a control request on the device, without wrapping the unsafe behavior of
    /// the contained IOUSbDevRequest. See also [[device_request]].
    pub fn device_request_with_timeout(&self, request: &mut IOUSBDevRequestTO) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.device,
            DeviceRequestTO,
            request
        ))
    }

    /// Aborts any active transfer on EP0.
    pub fn abort_ep0(&mut self) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.device,
            USBDeviceAbortPipeZero
        ))
    }

    /// Places the device into power-save mode, or takes it out.
    /// A value of true places the device into suspend.
    pub fn suspend(&mut self, suspend: bool) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.device,
            USBDeviceSuspend,
            suspend as u8
        ))
    }

    /// Returns an IOKit iterator that can be used to iterate over all interfaces on this device.
    pub fn create_interface_iterator(&mut self) -> UsbResult<IoObject> {
        let mut iterator: io_iterator_t = 0;

        // For our purposes, we don't want macOS to filter the interface list
        // by anything in particular (e.g. by device class), so we'll just construct
        // a big ol' list of Don't Cares.
        let mut dont_care = IOUSBFindInterfaceRequest {
            bInterfaceClass: kIOUSBFindInterfaceDontCare,
            bInterfaceSubClass: kIOUSBFindInterfaceDontCare,
            bInterfaceProtocol: kIOUSBFindInterfaceDontCare,
            bAlternateSetting: kIOUSBFindInterfaceDontCare,
        };

        // Finally, actually ask macOS to give us that iterator...
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.device,
            CreateInterfaceIterator,
            &mut dont_care,
            &mut iterator
        ))?;

        // ... and pack it all up nicely in an IoObject for our user.
        Ok(IoObject::new(iterator))
    }

    /// Attaches whole-device asynchronous events to the provided event source,
    /// which can be then later attached to a CFRunLoop to run event callbacks.
    pub(crate) fn attach_async_events(
        &self,
        notification_source: &mut NotificationSource,
    ) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.device,
            CreateDeviceAsyncEventSource,
            &mut notification_source.source()
        ))
    }

    pub(crate) fn notification_source(&self) -> UsbResult<NotificationSource> {
        let mut raw_source: CFRunLoopSourceRef = std::ptr::null_mut();

        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.device,
            CreateDeviceAsyncEventSource,
            &mut raw_source
        ))?;

        Ok(NotificationSource::new(raw_source))
    }

    /// Closes the active USB device.
    pub fn close(&mut self) {
        if !self.is_open {
            return;
        }

        if call_unsafe_iokit_function!(self.device, USBDeviceClose) == kIOReturnSuccess {
            self.is_open = false;
        }
    }
}

impl Drop for OsDevice {
    fn drop(&mut self) {
        // If the device is still open, close it...
        self.close();

        // ... and decrease macOS's refcount, so the device can be dealloc'd.
        call_unsafe_iokit_function!(self.device, Release);
    }
}

/// Helper for fetching endpoint metadata from our OsInterface.
/// At some point, a caller will convert this up into OS-agnostic metadata.
#[allow(dead_code)]
pub(crate) struct EndpointMetadata {
    pub(crate) direction: u8,
    pub(crate) number: u8,
    pub(crate) transfer_type: u8,
    pub(crate) max_packet_size: u16,
    pub(crate) interval: u8,
    pub(crate) max_burst: u8,
    pub(crate) mult: u8,
    pub(crate) bytes_per_interval: u16,
}

/// Wrapper around a **UsbInterface that helps us poke at its contained function pointers.
#[derive(Debug)]
pub(crate) struct OsInterface {
    interface: *mut *mut UsbInterface,

    /// The interface number associated with the given OS interface.
    interface_number: u8,

    /// If set, all function calls on this interface will return PermissionDenied.
    ///
    /// This allows us to act like permission is being denied on the individual calls,
    /// rather than on creating the interface object. This makes the macOS backend have
    /// the same behavior as other backends, which don't try to "open" interfaces in an early step.
    deny_all: bool,

    /// True iff the interface is currently open.
    is_open: bool,
}

// We really only have a pointer to something that's already Send,
// so override Rust's unwillingness to have Send pointers.
unsafe impl Send for OsInterface {}
unsafe impl Sync for OsInterface {}

#[allow(dead_code)]
impl OsInterface {
    pub(crate) fn new(interface: *mut *mut UsbInterface, interface_number: u8) -> Self {
        Self {
            interface,
            interface_number,
            deny_all: false,
            is_open: false,
        }
    }

    pub(crate) fn new_denying_placeholder(interface_number: u8) -> Self {
        Self {
            interface: std::ptr::null_mut(),
            interface_number,
            deny_all: true,
            is_open: false,
        }
    }

    pub fn interface_number(&self) -> UsbResult<u8> {
        Ok(self.interface_number)
    }

    /// Opens the interface, allowing the other functions on this type to be used.
    pub fn open(&mut self) -> UsbResult<()> {
        if self.deny_all {
            return Err(Error::PermissionDenied);
        }

        // If we're already open, we're done!
        if self.is_open {
            return Ok(());
        }

        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            USBInterfaceOpen
        ))
    }

    /// Returns the number of endpoints associated with the interface.
    pub fn endpoint_count(&mut self) -> UsbResult<u8> {
        let mut count: UInt8 = 0;

        // If we won't allow access to any actual functions,
        // lie that we have no endpoints, for lack of a better thing to do.
        if self.deny_all {
            return Ok(0);
        }

        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            GetNumEndpoints,
            &mut count
        ))?;

        Ok(count as u8)
    }

    pub fn endpoint_properties(&mut self, pipe_ref: u8) -> UsbResult<EndpointMetadata> {
        if self.deny_all {
            return Err(Error::PermissionDenied);
        }

        let mut direction: UInt8 = 0;
        let mut number: UInt8 = 0;
        let mut transfer_type: UInt8 = 0;
        let mut max_packet_size: UInt16 = 0;
        let mut interval: UInt8 = 0;
        let mut max_burst: UInt8 = 0;
        let mut mult: UInt8 = 0;
        let mut bytes_per_interval: UInt16 = 0;

        // We have entered hell, it's real, and it is this IOKit function signature.
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            GetPipePropertiesV2,
            pipe_ref,
            &mut direction,
            &mut number,
            &mut transfer_type,
            &mut max_packet_size,
            &mut interval,
            &mut max_burst,
            &mut mult,
            &mut bytes_per_interval
        ))?;

        Ok(EndpointMetadata {
            direction,
            number,
            transfer_type,
            max_packet_size,
            interval,
            max_burst,
            mult,
            bytes_per_interval,
        })
    }

    /// Performs a write.
    pub fn write(&self, pipe_ref: u8, data: &[u8]) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            WritePipe,
            pipe_ref,
            data.as_ptr() as *mut c_void,
            data.len() as u32
        ))
    }

    /// Performs an async write.
    pub fn write_nonblocking(
        &self,
        pipe_ref: u8,
        data: *mut c_void,
        data_length: u32,
        callback: IOAsyncCallback1,
        callback_arg: *mut c_void,
    ) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            WritePipeAsync,
            pipe_ref,
            data,
            data_length,
            callback,
            callback_arg
        ))
    }

    /// Performs a write, with an associated timeout.
    pub fn write_with_timeout(&self, pipe_ref: u8, data: &[u8], timeout: u32) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            WritePipeTO,
            pipe_ref,
            data.as_ptr() as *mut c_void,
            data.len() as u32,
            timeout,
            timeout
        ))
    }

    /// Performs an async write.
    pub fn write_with_timeout_nonblocking(
        &self,
        pipe_ref: u8,
        data: *mut c_void,
        data_length: u32,
        callback: IOAsyncCallback1,
        callback_arg: *mut c_void,
        timeout: u32,
    ) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            WritePipeAsyncTO,
            pipe_ref,
            data,
            data_length,
            timeout,
            timeout,
            callback,
            callback_arg
        ))
    }

    /// Performs a read.
    pub fn read(&self, pipe_ref: u8, buffer: &mut [u8]) -> UsbResult<usize> {
        let mut size: UInt32 = buffer.len() as u32;

        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            ReadPipe,
            pipe_ref,
            buffer.as_mut_ptr() as *mut c_void,
            &mut size
        ))?;

        Ok(size as usize)
    }

    /// Performs an async read.
    pub fn read_nonblocking(
        &self,
        pipe_ref: u8,
        data: *mut c_void,
        data_length: u32,
        callback: IOAsyncCallback1,
        callback_arg: *mut c_void,
    ) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            ReadPipeAsync,
            pipe_ref,
            data,
            data_length,
            callback,
            callback_arg
        ))
    }

    /// Performs a write, with an associated timeout.
    pub fn read_with_timeout(
        &self,
        pipe_ref: u8,
        buffer: &mut [u8],
        timeout: u32,
    ) -> UsbResult<usize> {
        let mut size: UInt32 = buffer.len() as u32;

        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            ReadPipeTO,
            pipe_ref,
            buffer.as_mut_ptr() as *mut c_void,
            &mut size,
            timeout,
            timeout
        ))?;

        Ok(size as usize)
    }

    /// Performs an async read.
    pub fn read_with_timeout_nonblocking(
        &self,
        pipe_ref: u8,
        data: *mut c_void,
        data_length: u32,
        callback: IOAsyncCallback1,
        callback_arg: *mut c_void,
        timeout: u32,
    ) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            ReadPipeAsyncTO,
            pipe_ref,
            data,
            data_length,
            timeout,
            timeout,
            callback,
            callback_arg
        ))
    }

    /// Clears the stall condition on the provided PipeRef.
    pub fn clear_stall(&self, pipe_ref: u8) -> UsbResult<()> {
        if self.deny_all {
            return Err(Error::PermissionDenied);
        }

        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            ClearPipeStall,
            pipe_ref
        ))
    }

    /// Clears the stall condition on the provided PipeRef.
    pub fn set_alternate_setting(&self, setting: u8) -> UsbResult<()> {
        if self.deny_all {
            return Err(Error::PermissionDenied);
        }
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            SetAlternateInterface,
            setting
        ))
    }

    /// Attaches per-interface asynchronous events to the provided event source,
    /// which can be then later attached to a CFRunLoop to run event callbacks.
    pub(crate) fn attach_async_events(
        &self,
        notification_source: &mut NotificationSource,
    ) -> UsbResult<()> {
        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            CreateInterfaceAsyncEventSource,
            &mut notification_source.source()
        ))
    }

    pub(crate) fn notification_source(&self) -> UsbResult<NotificationSource> {
        let mut raw_source: CFRunLoopSourceRef = std::ptr::null_mut();

        UsbResult::from_io_return(call_unsafe_iokit_function!(
            self.interface,
            CreateInterfaceAsyncEventSource,
            &mut raw_source
        ))?;

        Ok(NotificationSource::new(raw_source))
    }

    /// Closes the active USB interface.
    pub fn close(&mut self) {
        if !self.is_open {
            return;
        }

        if self.deny_all {
            panic!("internal consistency: somehow, we have an open deny_all interface? what have we _done_");
        }

        if call_unsafe_iokit_function!(self.interface, USBInterfaceClose) == kIOReturnSuccess {
            self.is_open = false;
        }
    }
}

impl Drop for OsInterface {
    fn drop(&mut self) {
        // If the device is still open, close it...
        self.close();

        // ... and decrease macOS's refcount, so the device can be dealloc'd.
        call_unsafe_iokit_function!(self.interface, Release);
    }
}

//
// Helpers for working with CoreFoundation / IOKit types.
//

/// Rustified version of the CFSTR C macro.
macro_rules! cfstr {
    ($string:expr) => {{
        let cstr = CString::new($string).unwrap();
        CFSTR(cstr.as_ptr())
    }};
}
pub(crate) use cfstr;

/// Translates an IOReturn error to its USRs equivalent.
#[allow(non_upper_case_globals, non_snake_case)]
fn io_return_to_error(rc: IOReturn) -> error::Error {
    match rc {
        // Substitute IOKit messages for our equivalent...
        kIOReturnNotOpen => Error::DeviceNotOpen,
        kIOReturnNoDevice => Error::DeviceNotFound,
        kIOReturnExclusiveAccess => Error::DeviceReserved,
        kIOReturnBadArgument => Error::InvalidArgument,
        kIOReturnAborted => Error::Aborted,
        kIOReturnOverrun => Error::Overrun,
        kIOReturnNoResources => Error::PermissionDenied,
        kIOUSBNoAsyncPortErr => Error::DeviceNotOpen,
        kIOUSBUnknownPipeErr => Error::InvalidEndpoint,
        kIOUSBPipeStalled => Error::Stalled,
        kIOUSBTransactionTimeout => Error::TimedOut,
        _ => Error::OsError(rc as i64),
    }
}

// Extend UsbResult with IOKit conversions.
pub(crate) trait IOKitEmptyResultExtension {
    fn from_io_return(io_return: IOReturn) -> UsbResult<()>;
}

pub(crate) trait IOKitResultExtension<T> {
    fn from_io_return_and_value(io_return: IOReturn, ok_value: T) -> UsbResult<T>;
}

impl IOKitEmptyResultExtension for UsbResult<()> {
    /// Creates s UsbResult from an IOKit return code.
    fn from_io_return(io_return: IOReturn) -> UsbResult<()> {
        // If this wasn't a success, translate our error.
        if io_return != kIOReturnSuccess {
            Err(io_return_to_error(io_return))
        } else {
            Ok(())
        }
    }
}

impl<T> IOKitResultExtension<T> for UsbResult<T> {
    /// Creates s UsbResult from an IOKit return code.
    fn from_io_return_and_value(io_return: IOReturn, ok_value: T) -> UsbResult<T> {
        // If this wasn't a success, translate our error.
        if io_return != kIOReturnSuccess {
            Err(io_return_to_error(io_return))
        }
        // Otherwise, package up our Ok value.
        else {
            Ok(ok_value)
        }
    }
}

/// Converts a CFNumberRef to a Rust integer.
pub(crate) fn number_from_cf_number<T: TryFrom<u64>>(number_ref: CFNumberRef) -> UsbResult<T> {
    unsafe {
        let mut result: u64 = 0;

        // Promote a null pointer error to a slightly nicer panic.
        if number_ref.is_null() {
            panic!("something passed a null pointer to number_from_cf_number T_T");
        }

        let succeeded = CFNumberGetValue(
            number_ref,
            kCFNumberSInt64Type,
            &mut result as *mut u64 as *mut c_void,
        );
        if !succeeded {
            error!("Failed to convert a NumberRef into a CFNumber!");
            return Err(Error::UnspecifiedOsError);
        }

        result.try_into().map_err(|_| Error::UnspecifiedOsError)
    }
}

/// Converts a raw CFString into a Rust string.
pub(crate) fn string_from_cf_string(string_ref: CFStringRef) -> UsbResult<Option<String>> {
    unsafe {
        // Promote a null pointer error to a slightly nicer panic.
        if string_ref.is_null() {
            panic!("something passed a null pointer to string_from_cf_string T_T");
        }

        let c_string = CFStringGetCStringPtr(string_ref, kCFStringEncodingUTF8);
        if c_string.is_null() {
            return Ok(None);
        }

        Ok(Some(CStr::from_ptr(c_string).to_string_lossy().to_string()))
    }
}

/// Queries IOKit and fetches a device property from the IORegistry.
/// Accepts a usb_device_iterator and the property name.
pub(crate) fn get_iokit_numeric_device_property<T: TryFrom<u64>>(
    device: io_iterator_t,
    property: &str,
) -> UsbResult<T> {
    unsafe {
        let service_plane: *mut i8 = kIOServicePlane as *mut i8;

        let raw_value = IORegistryEntrySearchCFProperty(
            device,
            service_plane,
            cfstr!(property),
            std::ptr::null(),
            kIORegistryIterateRecursively | kIORegistryIterateParents,
        ) as CFNumberRef;
        if raw_value.is_null() {
            error!("Failed to read numeric device property {}!", property);
            return Err(Error::UnspecifiedOsError);
        }
        number_from_cf_number::<T>(raw_value)
    }
}

/// Queries IOKit and fetches a device property from the IORegistry.
/// Accepts a usb_device_iterator and the property name.
pub(crate) fn get_iokit_string_device_property(
    device: io_iterator_t,
    property: &str,
) -> UsbResult<Option<String>> {
    unsafe {
        let service_plane: *mut i8 = kIOServicePlane as *mut i8;

        let raw_value = IORegistryEntrySearchCFProperty(
            device,
            service_plane,
            cfstr!(property),
            std::ptr::null(),
            kIORegistryIterateRecursively | kIORegistryIterateParents,
        ) as CFStringRef;
        if raw_value.is_null() {
            return Ok(None);
        }

        string_from_cf_string(raw_value)
    }
}

// Helper function that converts timeouts into the IOKit representation.
pub(crate) fn to_iokit_timeout(timeout: Duration) -> u32 {
    let mut timeout_ms = timeout.as_millis() as u32;

    // Truncate this to a u32, since more would be a heckuva long time anyway.
    if timeout.as_millis() > (u32::MAX as u128) {
        warn!(
            "A wildly long timeout ({}s) was truncated to u32::MAX ({}s).",
            timeout.as_secs_f64(),
            Duration::from_millis(u32::MAX as u64).as_secs_f64()
        );
        timeout_ms = u32::MAX;
    }

    timeout_ms
}

/// Helper function that moves an object out of Rust's memory model, for use by IOKit.
pub(crate) fn leak_to_iokit<T>(object: T) -> *mut c_void {
    Box::into_raw(Box::new(object)) as *mut c_void
}

/// Helper function that recovers an object that was leaked with `leak_to_iokit`.
pub(crate) fn unleak_from_iokit<T>(pointer: *mut c_void) -> T {
    unsafe {
        let typed = pointer as *mut T;
        let boxed = Box::from_raw(typed);

        *boxed
    }
}
