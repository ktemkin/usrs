//! Backend tools for opening and working with devices.

use std::{
    collections::HashMap,
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time,
};

use core_foundation_sys::base::SInt32;
use io_kit_sys::{
    ret::{kIOReturnNoResources, kIOReturnSuccess},
    IOIteratorNext,
};
use log::{debug, error};

use crate::{
    backend::macos::enumeration::get_device_iterator, backend::BackendDevice, DeviceInformation,
    Error, UsbResult,
};

use super::{
    endpoint::{address_for_in_endpoint, address_for_out_endpoint},
    interface::interface_from_service,
    iokit::{
        self, get_iokit_numeric_device_property, usb_device_type_id, IoObject, NotificationSource,
        OsDevice, OsInterface, PluginInterface,
    },
    iokit_c::{
        kIOCFPlugInInterfaceID, kIOUsbDeviceUserClientTypeID, IOCFPlugInInterface,
        IOCreatePlugInInterfaceForService,
    },
};

/// Type alias to make it clear when our u32 handle is an IoService.
type IoService = IoObject;

/// Metadata for a given endpoint; used for working with the endpoint
/// in a macOS interface context.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct EndpointInformation {
    /// The interface number to which the interface belongs.
    pub interface_number: u8,

    /// The macOS pipe reference, which encodes the endpoint's position
    /// in macOS's per-interface endpoint array.
    pub pipe_ref: u8,
}

/// Internal type storing the state for our raw USB device.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct MacOsDevice {
    /// The core, low-level macOS device.
    /// Things targeting the whole device (e.g. EP0) use this object.
    pub(crate) device: OsDevice,

    /// The interfaces associated with the given device,
    /// indexed by their interface number.
    pub(crate) interfaces: HashMap<u8, OsInterface>,

    /// Metadata associated with each endpoint _address_.
    /// Contains the information necessary to work with an endpoint.
    pub(crate) endpoint_metadata: HashMap<u8, EndpointInformation>,

    /// Flag used to indicate when this device is being dropped, and thus its thread should die.
    pub(crate) termination_flag: Arc<AtomicBool>,
}

unsafe impl Send for MacOsDevice {}

impl MacOsDevice {
    /// Populates the internal list of interfaces. Each interface provides the object we'll need
    /// to perform an operation on its associated non-EP0 endpoint(s).
    fn populate_interfaces(
        &mut self,
        notification_sources: &mut Vec<NotificationSource>,
    ) -> UsbResult<()> {
        unsafe {
            // Get an interface iterator, which will allow use to walk the device's interfaces...
            let interface_iterator = self.device.create_interface_iterator()?;
            let mut interface_index = 0;

            // ... and iterate over the full list of services.
            let mut interface_service;
            while {
                interface_service = IoService::new(IOIteratorNext(interface_iterator.get()));
                !interface_service.is_invalid()
            } {
                // Get the macOS representation of the current interface...
                let interface = interface_from_service(interface_service, interface_index);
                let mut interface = match interface {
                    // If we get a permission denied error fetching a given interface, we won't want
                    // to immediately fail out, as it's possible the device has other interfaces
                    // that are of interest and which we can readily access -- such as a device
                    // that has a vendor specific interface and a USB Audio Class interface.
                    //
                    // Instead, we'll replace that interface with an interface that denies every
                    // actual request, ensuring that we only error if someone actually tries to
                    // _use_ the interface. This is the way e.g. Linux behaves, and that works
                    // well for them, so... :shrug:
                    Err(Error::PermissionDenied) => {
                        debug!("note: interface {interface_index} can't be opened; generating a permission-deny placeholder");
                        OsInterface::new_denying_placeholder(interface_index)
                    }
                    Err(e) => return Err(e),
                    Ok(interface) => interface,
                };

                // ... subscribe to per-interface events...
                notification_sources.push(interface.notification_source()?);

                // ... and populate the associated endpoint data...
                _ = self.populate_endpoint_metadata(&mut interface);

                // ... and store the interface internally, for later access.
                self.interfaces
                    .insert(interface.interface_number()?, interface);

                // Increment our interface index.
                interface_index += 1;
            }

            Ok(())
        }
    }

    /// Populates the endpoint metadata associated with the given interface.
    fn populate_endpoint_metadata(&mut self, interface: &mut OsInterface) -> UsbResult<()> {
        // First, we'll need to figure out how many endpoints this interface has,
        // which also happens to be the number of pipe references -- because a single pipe
        // reference exists per endpoint. Pipe references are an odd macOS concept -- they're
        // basically the index into an internal array of endpoints associated with the interface.
        // Except they're one indexed, ostensibly because the endpoint array actually contains an
        // internal metadata field for the control endpoint. Fun.
        let pipe_ref_count = interface.endpoint_count()?;

        // Next, we'll need to iterate over the pipe refs.
        // Remember, they're one indexed. Yes. One indexed.
        for pipe_ref in 1..=pipe_ref_count {
            // We'll temporarily open the interface, in order to get its endpoint data,
            // as MacOS won't let us get that information without it. We'll then close the
            // interface until we're ready to actually use it.
            interface.open()?;
            let endpoint_metadata = interface.endpoint_properties(pipe_ref)?;
            interface.close();

            // Once we know the endpoint number, we can construct the part we really want:
            // the endpoint address.
            let address = if endpoint_metadata.direction == 0 {
                address_for_out_endpoint(endpoint_metadata.number)
            } else {
                address_for_in_endpoint(endpoint_metadata.number)
            };

            // Use it to squish the information we do know into our metadata hash-map.
            self.endpoint_metadata.insert(
                address,
                EndpointInformation {
                    interface_number: interface.interface_number()?,
                    pipe_ref,
                },
            );
        }

        Ok(())
    }
}

impl BackendDevice for MacOsDevice {
    //
    // Any types -- allow us to be converted back into MacOsDevices in
    // other parts of the backend. This is the nasty Rust equivalent of a void*.
    //
    fn as_mut_any(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl Drop for MacOsDevice {
    fn drop(&mut self) {
        // Let our event thread know it can stop running, as we're no longer sending it events.
        self.termination_flag.store(true, Ordering::Relaxed);
    }
}

/// Converts an IOIteratorNext result into a backend USB device.
fn open_usb_device_from_io_device(device_service: IoService) -> UsbResult<Box<dyn BackendDevice>> {
    if device_service.is_invalid() {
        panic!("internal inconsistency: got a 0 io-object-handle");
    }

    unsafe {
        // Get the raw USB device associated with the service.
        //
        // NOTE(ktemkin): According to the libusb maintainers, this will sometimes spuriously
        // return `kIOReturnNoResources` for reasons Apple won't explain, usually
        // when a device is freshly plugged in. We'll allow this a few retries,
        // accordingly.
        //
        // [This behavior actually makes sense to me -- when the device is first plugged
        // in, it exists to IOKit, but hasn't been enumerated, yet. Accordingly, the device
        // interface doesn't actually yet exist for us to grab, and/or doesn't yet have the
        // right permissions for us to grab it. MacOS needs to see if a kernel driver binds
        // to it; as its security model won't allow the userland to grab a device that the
        // kernel owns.]
        //
        // If the kIOReturnNoResources persists, it's typically an indication that
        // macOS is preventing us from touching the relevant device due to its security
        // model. This happens when the device has a kernel-mode driver bound to the
        // whole device -- the kernel owns it, and it's unwilling to give it to us.
        //
        for _ in 0..5 {
            let mut _score: SInt32 = 0;
            let mut raw_device_plugin: *mut *mut IOCFPlugInInterface = std::ptr::null_mut();

            // Ask macOS to give us the device plugin, which is capable of creating our actual USB
            // device. Whee, indirection.~
            let rc = IOCreatePlugInInterfaceForService(
                device_service.get(),
                kIOUsbDeviceUserClientTypeID(),
                kIOCFPlugInInterfaceID(),
                &mut raw_device_plugin,
                &mut _score,
            );

            // If we got "no resources", it's possible this is the spurious case above.
            if rc == kIOReturnNoResources {
                std::thread::sleep(time::Duration::from_millis(1));
                continue;
            }

            // For any other error, translate this to a USBResult.
            if rc != kIOReturnSuccess {
                return Err(Error::OsError(rc as i64));
            }

            // If we didn't actually get the device plugin, despite our apparent success,
            // convert this to an _unspecified_ IO error. G'damn.
            if raw_device_plugin.is_null() {
                error!("IOKit indicated it successfully created a PlugInInterface, but the pointer was NULL");
                return Err(Error::UnspecifiedOsError);
            }

            // Handle scoping/dropping for our device interface.
            let device_plugin = PluginInterface::new(raw_device_plugin);

            // Finally, get the actual UsbDevice we care about.
            let mut raw_device: *mut *mut iokit::UsbDevice = std::ptr::null_mut();
            let query_interface = (**device_plugin.get()).QueryInterface.unwrap();

            // We need to pass &raw_device into a **void, which will let it populate the **UsbDevice.
            // This API is _wild_.
            let raw_device_ptr = &mut raw_device as *mut *mut *mut iokit::UsbDevice;
            query_interface(
                device_plugin.get() as *mut c_void,
                usb_device_type_id(),
                raw_device_ptr as *mut *mut c_void,
            );

            // macOS claims that call will never fail, and will always produce a valid pointer.
            // We don't trust it, so we're going to panic if it's lied to us.
            if raw_device.is_null() {
                panic!("query_interface returned a null pointer, which Apple says is impossible");
            }

            // Finally, package up the device we've created as a backend device...
            let mut backend_device = Box::new(MacOsDevice {
                device: OsDevice::new(raw_device),
                interfaces: HashMap::new(),
                endpoint_metadata: HashMap::new(),
                termination_flag: Arc::new(AtomicBool::new(false)),
            });

            // .. open the device, since we said we'd do so...
            backend_device.device.open()?;

            // .. subscribe to per-device asynchronous events ...
            let mut notification_sources: Vec<NotificationSource> = vec![];
            notification_sources.push(backend_device.device.notification_source()?);

            // ... ask it to populate its interfaces, and endpoint metadata ...
            backend_device.populate_interfaces(&mut notification_sources)?;

            // ... spin up a thread to handle its events ...
            let termination_condition = Arc::clone(&backend_device.termination_flag);
            std::thread::spawn(move || {
                NotificationSource::run_event_loop(notification_sources, termination_condition)
            });

            // ... and return it.
            return Ok(backend_device);
        }
    }

    Err(Error::DeviceNotFound)
}

/// Opens a device given the information acquired during enumeration.
pub(crate) fn open_usb_device(
    information: &DeviceInformation,
) -> UsbResult<Box<dyn BackendDevice>> {
    let target_location_id = information
        .backend_numeric_location
        .expect("invalid device_id; did you make this yourself?");

    // NOTE(ktemkin): this process is -strictly- more than is necessary;
    // as macOS offers an ability to open a device by its LocationID. However,
    // at this point, it seems more valuable to me to interface with the least
    // amount of the unsafe-pointer'y/handle'y IOKit code possible; so this is
    // just re-using the iteration method that we've already needed to expose.
    unsafe {
        // Fetch an IOKit iterator over all devices.
        let device_iterator = get_device_iterator()?;

        let mut device;
        while {
            device = IOIteratorNext(device_iterator.get());
            device != 0
        } {
            // Find the macOS location ID for the given device, which uniquely identifies a given
            // device...
            let location_id: UsbResult<u32> =
                get_iokit_numeric_device_property(device, "locationID");

            // Skip any devices that don't have location IDs; as they're not real devices macOS
            // will let us work with -- they're e.g. root hubs or internal-only devices.
            if location_id.is_err() {
                continue;
            }

            // If this isn't the device we're interested in, keep looking.
            if location_id.unwrap() as u64 != target_location_id {
                continue;
            }

            return open_usb_device_from_io_device(IoService::new(device));
        }

        Err(Error::DeviceNotFound)
    }
}
