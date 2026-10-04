//! IOKit declarations used by the GPU-usage and SMC readers.

use std::ffi::{c_char, c_void};

use core_foundation_sys::base::{CFAllocatorRef, CFTypeRef};
use core_foundation_sys::dictionary::CFMutableDictionaryRef;
use core_foundation_sys::string::CFStringRef;

pub type MachPort = u32;
pub type IoObject = u32;
pub type KernReturn = i32;

pub const KERN_SUCCESS: KernReturn = 0;
/// `kIOMainPortDefault`
pub const MAIN_PORT_DEFAULT: MachPort = 0;

#[link(name = "IOKit", kind = "framework")]
unsafe extern "C" {
    pub fn IOServiceMatching(name: *const c_char) -> CFMutableDictionaryRef;
    /// Consumes `matching`.
    pub fn IOServiceGetMatchingServices(main_port: MachPort, matching: CFMutableDictionaryRef, existing: *mut IoObject) -> KernReturn;
    /// Consumes `matching`.
    pub fn IOServiceGetMatchingService(main_port: MachPort, matching: CFMutableDictionaryRef) -> IoObject;
    pub fn IOIteratorNext(iterator: IoObject) -> IoObject;
    pub fn IOObjectRelease(object: IoObject) -> KernReturn;
    pub fn IORegistryEntryGetChildIterator(entry: IoObject, plane: *const c_char, iterator: *mut IoObject) -> KernReturn;
    pub fn IORegistryEntryCreateCFProperty(entry: IoObject, key: CFStringRef, allocator: CFAllocatorRef, options: u32) -> CFTypeRef;
    pub fn IOServiceOpen(service: IoObject, owning_task: MachPort, kind: u32, connect: *mut IoObject) -> KernReturn;
    pub fn IOServiceClose(connect: IoObject) -> KernReturn;
    pub fn IOConnectCallStructMethod(
        connection: IoObject,
        selector: u32,
        input: *const c_void,
        input_size: usize,
        output: *mut c_void,
        output_size: *mut usize,
    ) -> KernReturn;
}

unsafe extern "C" {
    static mach_task_self_: MachPort;
}

pub fn mach_task_self() -> MachPort {
    unsafe { mach_task_self_ }
}

/// An IOKit object released on drop.
pub struct Object(pub IoObject);

impl Drop for Object {
    fn drop(&mut self) {
        if self.0 != 0 {
            unsafe { IOObjectRelease(self.0) };
        }
    }
}

/// Iterates an IOKit iterator, yielding owned objects.
pub fn iterate(iterator: &Object) -> impl Iterator<Item = Object> + '_ {
    std::iter::from_fn(move || {
        let next = unsafe { IOIteratorNext(iterator.0) };
        (next != 0).then_some(Object(next))
    })
}
