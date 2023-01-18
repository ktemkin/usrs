//! FFI types we're including here, as they're missing from io-kit-sys.
//! You may not want to stare too closely at this; it's tweaked bindgen output.

#![allow(non_camel_case_types, non_snake_case, dead_code)]

use std::ffi::c_void;

use core_foundation_sys::{
    base::{kCFAllocatorSystemDefault, mach_port_t, SInt32},
    dictionary::CFDictionaryRef,
    mach_port::CFAllocatorRef,
    runloop::CFRunLoopSourceRef,
    uuid::{CFUUIDBytes, CFUUIDRef},
};
use io_kit_sys::{
    ret::IOReturn,
    types::{io_iterator_t, io_service_t},
    IOAsyncCallback1,
};

type REFIID = CFUUIDBytes;
type LPVOID = *mut c_void;
type HRESULT = SInt32;
type UInt8 = ::std::os::raw::c_uchar;
type UInt16 = ::std::os::raw::c_ushort;
type UInt32 = ::std::os::raw::c_uint;
type UInt64 = ::std::os::raw::c_ulonglong;
type ULONG = ::std::os::raw::c_ulong;
type kern_return_t = ::std::os::raw::c_int;
type USBDeviceAddress = UInt16;
type AbsoluteTime = UnsignedWide;
type Boolean = std::os::raw::c_uchar;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct NumVersion {
    pub nonRelRev: UInt8,
    pub stage: UInt8,
    pub minorAndBugRev: UInt8,
    pub majorRev: UInt8,
}

#[repr(C, packed(2))]
#[derive(Debug, Copy, Clone)]
pub struct UnsignedWide {
    pub lo: UInt32,
    pub hi: UInt32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct IOUSBDevRequest {
    pub bmRequestType: UInt8,
    pub bRequest: UInt8,
    pub wValue: UInt16,
    pub wIndex: UInt16,
    pub wLength: UInt16,
    pub pData: *mut ::std::os::raw::c_void,
    pub wLenDone: UInt32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct IOUSBFindInterfaceRequest {
    pub bInterfaceClass: UInt16,
    pub bInterfaceSubClass: UInt16,
    pub bInterfaceProtocol: UInt16,
    pub bAlternateSetting: UInt16,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct IOUSBDevRequestTO {
    pub bmRequestType: UInt8,
    pub bRequest: UInt8,
    pub wValue: UInt16,
    pub wIndex: UInt16,
    pub wLength: UInt16,
    pub pData: *mut ::std::os::raw::c_void,
    pub wLenDone: UInt32,
    pub noDataTimeout: UInt32,
    pub completionTimeout: UInt32,
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct IOCFPlugInInterfaceStruct {
    pub _reserved: *mut ::std::os::raw::c_void,
    pub QueryInterface: ::std::option::Option<
        unsafe extern "C" fn(
            thisPointer: *mut ::std::os::raw::c_void,
            iid: REFIID,
            ppv: *mut LPVOID,
        ) -> HRESULT,
    >,
    pub AddRef: ::std::option::Option<
        unsafe extern "C" fn(thisPointer: *mut ::std::os::raw::c_void) -> ULONG,
    >,
    pub Release: ::std::option::Option<
        unsafe extern "C" fn(thisPointer: *mut ::std::os::raw::c_void) -> ULONG,
    >,
    pub version: UInt16,
    pub revision: UInt16,
    pub Probe: ::std::option::Option<
        unsafe extern "C" fn(
            thisPointer: *mut ::std::os::raw::c_void,
            propertyTable: CFDictionaryRef,
            service: io_service_t,
            order: *mut SInt32,
        ) -> IOReturn,
    >,
    pub Start: ::std::option::Option<
        unsafe extern "C" fn(
            thisPointer: *mut ::std::os::raw::c_void,
            propertyTable: CFDictionaryRef,
            service: io_service_t,
        ) -> IOReturn,
    >,
    pub Stop: ::std::option::Option<
        unsafe extern "C" fn(thisPointer: *mut ::std::os::raw::c_void) -> IOReturn,
    >,
}
pub type IOCFPlugInInterface = IOCFPlugInInterfaceStruct;

extern "C" {
    pub fn CFUUIDGetUUIDBytes(uuid: CFUUIDRef) -> CFUUIDBytes;

    pub fn IOCreatePlugInInterfaceForService(
        service: io_service_t,
        pluginType: CFUUIDRef,
        interfaceType: CFUUIDRef,
        theInterface: *mut *mut *mut IOCFPlugInInterface,
        theScore: *mut SInt32,
    ) -> kern_return_t;

    pub fn CFUUIDGetConstantUUIDWithBytes(
        alloc: CFAllocatorRef,
        byte0: UInt8,
        byte1: UInt8,
        byte2: UInt8,
        byte3: UInt8,
        byte4: UInt8,
        byte5: UInt8,
        byte6: UInt8,
        byte7: UInt8,
        byte8: UInt8,
        byte9: UInt8,
        byte10: UInt8,
        byte11: UInt8,
        byte12: UInt8,
        byte13: UInt8,
        byte14: UInt8,
        byte15: UInt8,
    ) -> CFUUIDRef;

}

pub fn kIOUsbDeviceUserClientTypeID() -> CFUUIDRef {
    unsafe {
        CFUUIDGetConstantUUIDWithBytes(
            std::ptr::null(),
            0x9d,
            0xc7,
            0xb7,
            0x80,
            0x9e,
            0xc0,
            0x11,
            0xD4,
            0xa5,
            0x4f,
            0x00,
            0x0a,
            0x27,
            0x05,
            0x28,
            0x61,
        )
    }
}

pub fn kIOCFPlugInInterfaceID() -> CFUUIDRef {
    unsafe {
        CFUUIDGetConstantUUIDWithBytes(
            std::ptr::null(),
            0xC2,
            0x44,
            0xE8,
            0x58,
            0x10,
            0x9C,
            0x11,
            0xD4,
            0x91,
            0xD4,
            0x00,
            0x50,
            0xE4,
            0xC6,
            0x42,
            0x6F,
        )
    }
}

pub fn kIOUSBDeviceInterfaceID500() -> CFUUIDRef {
    unsafe {
        CFUUIDGetConstantUUIDWithBytes(
            kCFAllocatorSystemDefault,
            0xA3,
            0x3C,
            0xF0,
            0x47,
            0x4B,
            0x5B,
            0x48,
            0xE2,
            0xB5,
            0x7D,
            0x02,
            0x07,
            0xFC,
            0xEA,
            0xE1,
            0x3B,
        )
    }
}

#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct IOUSBConfigurationDescriptor {
    pub bLength: u8,
    pub bDescriptorType: u8,
    pub wTotalLength: u16,
    pub bNumInterfaces: u8,
    pub bConfigurationValue: u8,
    pub iConfiguration: u8,
    pub bmAttributes: u8,
    pub MaxPower: u8,
}
pub type IOUSBConfigurationDescriptorPtr = *mut IOUSBConfigurationDescriptor;

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct IOUSBDeviceStruct500 {
    pub _reserved: *mut ::std::os::raw::c_void,
    pub QueryInterface: ::std::option::Option<
        unsafe extern "C" fn(
            thisPointer: *mut ::std::os::raw::c_void,
            iid: REFIID,
            ppv: *mut LPVOID,
        ) -> HRESULT,
    >,
    pub AddRef: ::std::option::Option<
        unsafe extern "C" fn(thisPointer: *mut ::std::os::raw::c_void) -> ULONG,
    >,
    pub Release: ::std::option::Option<
        unsafe extern "C" fn(thisPointer: *mut ::std::os::raw::c_void) -> ULONG,
    >,
    pub CreateDeviceAsyncEventSource: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            source: *mut CFRunLoopSourceRef,
        ) -> IOReturn,
    >,
    pub GetDeviceAsyncEventSource: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void) -> CFRunLoopSourceRef,
    >,
    pub CreateDeviceAsyncPort: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            port: *mut mach_port_t,
        ) -> IOReturn,
    >,
    pub GetDeviceAsyncPort: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void) -> mach_port_t,
    >,
    pub USBDeviceOpen:
        ::std::option::Option<unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void) -> IOReturn>,
    pub USBDeviceClose:
        ::std::option::Option<unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void) -> IOReturn>,
    pub GetDeviceClass: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void, devClass: *mut UInt8) -> IOReturn,
    >,
    pub GetDeviceSubClass: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            devSubClass: *mut UInt8,
        ) -> IOReturn,
    >,
    pub GetDeviceProtocol: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            devProtocol: *mut UInt8,
        ) -> IOReturn,
    >,
    pub GetDeviceVendor: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            devVendor: *mut UInt16,
        ) -> IOReturn,
    >,
    pub GetDeviceProduct: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            devProduct: *mut UInt16,
        ) -> IOReturn,
    >,
    pub GetDeviceReleaseNumber: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            devRelNum: *mut UInt16,
        ) -> IOReturn,
    >,
    pub GetDeviceAddress: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            addr: *mut USBDeviceAddress,
        ) -> IOReturn,
    >,
    pub GetDeviceBusPowerAvailable: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            powerAvailable: *mut UInt32,
        ) -> IOReturn,
    >,
    pub GetDeviceSpeed: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void, devSpeed: *mut UInt8) -> IOReturn,
    >,
    pub GetNumberOfConfigurations: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void, numConfig: *mut UInt8) -> IOReturn,
    >,
    pub GetLocationID: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            locationID: *mut UInt32,
        ) -> IOReturn,
    >,
    pub GetConfigurationDescriptorPtr: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            configIndex: UInt8,
            desc: *mut IOUSBConfigurationDescriptorPtr,
        ) -> IOReturn,
    >,
    pub GetConfiguration: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void, configNum: *mut UInt8) -> IOReturn,
    >,
    pub SetConfiguration: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void, configNum: UInt8) -> IOReturn,
    >,
    pub GetBusFrameNumber: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            frame: *mut UInt64,
            atTime: *mut AbsoluteTime,
        ) -> IOReturn,
    >,
    pub ResetDevice:
        ::std::option::Option<unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void) -> IOReturn>,
    pub DeviceRequest: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            req: *mut IOUSBDevRequest,
        ) -> IOReturn,
    >,
    pub DeviceRequestAsync: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            req: *mut IOUSBDevRequest,
            callback: IOAsyncCallback1,
            refCon: *mut ::std::os::raw::c_void,
        ) -> IOReturn,
    >,
    pub CreateInterfaceIterator: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            req: *mut IOUSBFindInterfaceRequest,
            iter: *mut io_iterator_t,
        ) -> IOReturn,
    >,
    pub USBDeviceOpenSeize:
        ::std::option::Option<unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void) -> IOReturn>,
    pub DeviceRequestTO: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            req: *mut IOUSBDevRequestTO,
        ) -> IOReturn,
    >,
    pub DeviceRequestAsyncTO: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            req: *mut IOUSBDevRequestTO,
            callback: IOAsyncCallback1,
            refCon: *mut ::std::os::raw::c_void,
        ) -> IOReturn,
    >,
    pub USBDeviceSuspend: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void, suspend: Boolean) -> IOReturn,
    >,
    pub USBDeviceAbortPipeZero:
        ::std::option::Option<unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void) -> IOReturn>,
    pub USBGetManufacturerStringIndex: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void, msi: *mut UInt8) -> IOReturn,
    >,
    pub USBGetProductStringIndex: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void, psi: *mut UInt8) -> IOReturn,
    >,
    pub USBGetSerialNumberStringIndex: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void, snsi: *mut UInt8) -> IOReturn,
    >,
    pub USBDeviceReEnumerate: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void, options: UInt32) -> IOReturn,
    >,
    pub GetBusMicroFrameNumber: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            microFrame: *mut UInt64,
            atTime: *mut AbsoluteTime,
        ) -> IOReturn,
    >,
    pub GetIOUSBLibVersion: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            ioUSBLibVersion: *mut NumVersion,
            usbFamilyVersion: *mut NumVersion,
        ) -> IOReturn,
    >,
    pub GetBusFrameNumberWithTime: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            frame: *mut UInt64,
            atTime: *mut AbsoluteTime,
        ) -> IOReturn,
    >,
    pub GetUSBDeviceInformation: ::std::option::Option<
        unsafe extern "C" fn(self_: *mut ::std::os::raw::c_void, info: *mut UInt32) -> IOReturn,
    >,
    pub RequestExtraPower: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            type_: UInt32,
            requestedPower: UInt32,
            powerAvailable: *mut UInt32,
        ) -> IOReturn,
    >,
    pub ReturnExtraPower: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            type_: UInt32,
            powerReturned: UInt32,
        ) -> IOReturn,
    >,
    pub GetExtraPowerAllocated: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            type_: UInt32,
            powerAllocated: *mut UInt32,
        ) -> IOReturn,
    >,
    pub GetBandwidthAvailableForDevice: ::std::option::Option<
        unsafe extern "C" fn(
            self_: *mut ::std::os::raw::c_void,
            bandwidth: *mut UInt32,
        ) -> IOReturn,
    >,
}
pub type IOUSBDeviceInterface500 = IOUSBDeviceStruct500;
