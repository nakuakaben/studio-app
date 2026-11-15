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

/// Open an already SHA-256-verified and Internet-marked NSIS installer through
/// Windows Shell. Do not bypass the Mark of the Web, SmartScreen, or UAC.
/// The caller must obtain explicit user consent before calling this function.
pub fn execute_attachment(path: &std::path::Path, _source: &str) -> Result<u32, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{CloseHandle, HANDLE, WAIT_OBJECT_0},
            System::{
                Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED},
                Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE},
            },
            UI::Shell::{
                ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW,
            },
        },
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            // SAFETY: this guard is created only after COM initialization.
            unsafe { CoUninitialize() };
        }
    }
    struct Process(HANDLE);
    impl Drop for Process {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                // SAFETY: ShellExecuteExW returned this process handle.
                let _ = unsafe { CloseHandle(self.0) };
            }
        }
    }

    // Only the reviewed NSIS executable is eligible. No shell verbs, parameters,
    // elevation request or alternate executable are accepted from the catalog.
    if path.extension().and_then(|ext| ext.to_str()) != Some("exe")
        || !path.is_file()
        || !path.is_absolute()
    {
        return Err("Chemin du package NSIS invalide.".into());
    }
    let name: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    if name[..name.len() - 1].contains(&0) {
        return Err("Chemin du package NSIS invalide.".into());
    }
    // SAFETY: COM apartment stays active until ShellExecuteExW returns and the
    // process has exited; its input pointers refer to live UTF-16 buffers.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .map_err(|error| format!("Initialisation Windows : {error}"))?;
    let _apartment = Apartment;
    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
        lpFile: PCWSTR(name.as_ptr()),
        // SW_SHOWNORMAL = 1. SHELLEXECUTEINFOW::default() otherwise gives
        // nShow=0 (SW_HIDE), silently hiding the interactive NSIS wizard.
        nShow: 1,
        ..Default::default()
    };
    // The standard Shell launch preserves the MOTW and invokes all native
    // policy prompts. Never use CreateProcess or 'runas' as a fallback.
    unsafe { ShellExecuteExW(&mut info) }
        .map_err(|error| format!("Ouverture Windows du NSIS : {error}"))?;
    let process = Process(info.hProcess);
    if process.0.is_invalid() {
        return Err("Windows n’a renvoyé aucun processus NSIS.".into());
    }
    // SAFETY: process handle remains valid during wait and exit-code read.
    if unsafe { WaitForSingleObject(process.0, INFINITE) } != WAIT_OBJECT_0 {
        return Err("Attente du processus NSIS interrompue.".into());
    }
    let mut status = 0;
    unsafe { GetExitCodeProcess(process.0, &mut status) }
        .map_err(|error| format!("Code de sortie NSIS indisponible : {error}"))?;
    Ok(status)
}
