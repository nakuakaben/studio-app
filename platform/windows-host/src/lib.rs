//! Narrow safe interface to the Windows host-architecture query.
#![cfg(windows)]
#![deny(unsafe_op_in_unsafe_fn)]

use windows_sys::Win32::System::{
    SystemInformation::{IMAGE_FILE_MACHINE_AMD64, IMAGE_FILE_MACHINE_ARM64},
    Threading::{GetCurrentProcess, IsWow64Process2},
};

pub enum Architecture {
    X86_64,
    Aarch64,
}

pub fn architecture() -> Option<Architecture> {
    let (mut process, mut host) = (0, 0);
    // SAFETY: GetCurrentProcess gives a valid pseudohandle, and both output
    // pointers refer to live, writable USHORTs for the duration of the call.
    if unsafe { IsWow64Process2(GetCurrentProcess(), &mut process, &mut host) } == 0 {
        return None;
    }
    match host {
        IMAGE_FILE_MACHINE_AMD64 => Some(Architecture::X86_64),
        IMAGE_FILE_MACHINE_ARM64 => Some(Architecture::Aarch64),
        _ => None,
    }
}
