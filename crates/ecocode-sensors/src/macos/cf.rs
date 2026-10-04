//! Minimal Core Foundation helpers for reading IOKit / IOReport data.

use std::ffi::c_void;

use core_foundation_sys::array::{CFArrayGetCount, CFArrayGetTypeID, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation_sys::base::{CFGetTypeID, CFRelease, CFTypeRef, kCFAllocatorDefault};
use core_foundation_sys::dictionary::{CFDictionaryGetTypeID, CFDictionaryGetValue, CFDictionaryRef};
use core_foundation_sys::number::{CFNumberGetTypeID, CFNumberGetValue, CFNumberRef, kCFNumberSInt64Type};
use core_foundation_sys::string::{
    CFStringCreateWithBytes, CFStringGetCString, CFStringGetTypeID, CFStringRef, kCFStringEncodingUTF8,
};

/// A Core Foundation object we own (from a `Create`/`Copy` call), released on drop.
pub struct Owned(CFTypeRef);

impl Owned {
    /// Takes ownership of `ptr`; `None` if it is null.
    ///
    /// # Safety
    /// `ptr` must be a valid CF object returned with +1 retain count.
    pub unsafe fn new(ptr: *const c_void) -> Option<Self> {
        (!ptr.is_null()).then_some(Owned(ptr))
    }

    pub fn as_ptr(&self) -> CFTypeRef {
        self.0
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) }
    }
}

// CF objects are immutable here or used from one thread at a time.
unsafe impl Send for Owned {}

/// A new CFString.
pub fn cfstr(s: &str) -> Owned {
    unsafe {
        let p = CFStringCreateWithBytes(
            kCFAllocatorDefault,
            s.as_ptr(),
            s.len() as _,
            kCFStringEncodingUTF8,
            0,
        );
        Owned::new(p as CFTypeRef).expect("CFStringCreateWithBytes failed")
    }
}

/// Contents of a CFString, or `None` if `s` is null or not a string.
///
/// # Safety
/// `s` must be null or a valid CF object.
pub unsafe fn string(s: CFTypeRef) -> Option<String> {
    unsafe {
        if s.is_null() || CFGetTypeID(s) != CFStringGetTypeID() {
            return None;
        }
        let mut buf = [0 as std::ffi::c_char; 512];
        if CFStringGetCString(s as CFStringRef, buf.as_mut_ptr(), buf.len() as _, kCFStringEncodingUTF8) == 0 {
            return None;
        }
        Some(std::ffi::CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned())
    }
}

/// Value for `key` in a CFDictionary (borrowed), or null.
///
/// # Safety
/// `dict` must be null or a valid CF object.
pub unsafe fn dict_get(dict: CFTypeRef, key: &str) -> CFTypeRef {
    unsafe {
        if dict.is_null() || CFGetTypeID(dict) != CFDictionaryGetTypeID() {
            return std::ptr::null();
        }
        let k = cfstr(key);
        CFDictionaryGetValue(dict as CFDictionaryRef, k.as_ptr())
    }
}

/// Integer value of a CFNumber.
///
/// # Safety
/// `n` must be null or a valid CF object.
pub unsafe fn number(n: CFTypeRef) -> Option<i64> {
    unsafe {
        if n.is_null() || CFGetTypeID(n) != CFNumberGetTypeID() {
            return None;
        }
        let mut v: i64 = 0;
        let ok = CFNumberGetValue(n as CFNumberRef, kCFNumberSInt64Type, &mut v as *mut i64 as *mut c_void);
        ok.then_some(v)
    }
}

/// Elements of a CFArray (borrowed); empty if `a` is null or not an array.
///
/// # Safety
/// `a` must be null or a valid CF object.
pub unsafe fn array(a: CFTypeRef) -> Vec<CFTypeRef> {
    unsafe {
        if a.is_null() || CFGetTypeID(a) != CFArrayGetTypeID() {
            return Vec::new();
        }
        let a = a as CFArrayRef;
        (0..CFArrayGetCount(a)).map(|i| CFArrayGetValueAtIndex(a, i)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_round_trip() {
        let s = cfstr("CPU Energy");
        assert_eq!(unsafe { string(s.as_ptr()) }.as_deref(), Some("CPU Energy"));
        assert_eq!(unsafe { number(s.as_ptr()) }, None);
        assert!(unsafe { array(s.as_ptr()) }.is_empty());
    }
}
