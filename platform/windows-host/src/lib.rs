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

/// Start a marked download through Windows Attachment Execution Services.
/// The OS handles policy, security UI and the default open verb.
pub fn execute_attachment(path: &std::path::Path, source: &str) -> Result<u32, String> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{CloseHandle, HANDLE, HWND, WAIT_OBJECT_0},
            System::{
                Com::{
                    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
                    COINIT_APARTMENTTHREADED,
                },
                Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE},
            },
            UI::Shell::{AttachmentServices, IAttachmentExecute},
        },
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            // SAFETY: this guard is created only after COM initialization on this thread.
            unsafe { CoUninitialize() };
        }
    }
    struct Process(HANDLE);
    impl Drop for Process {
        fn drop(&mut self) {
            if !self.0.is_invalid() {
                // SAFETY: Execute returned ownership of this process handle.
                let _ = unsafe { CloseHandle(self.0) };
            }
        }
    }

    let path: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let source: Vec<_> = source.encode_utf16().chain(Some(0)).collect();
    if path[..path.len() - 1].contains(&0) || source[..source.len() - 1].contains(&0) {
        return Err("Chemin ou provenance Windows invalide.".into());
    }
    let os_error = |error: windows::core::Error| error.to_string();
    // SAFETY: the call has a null reserved pointer. RAII balances both S_OK and S_FALSE.
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .map_err(os_error)?;
    let _apartment = Apartment;
    // SAFETY: correct CLSID/IID, no COM aggregation, live null-terminated UTF-16 buffers.
    let attachment: IAttachmentExecute = unsafe {
        let attachment: IAttachmentExecute =
            CoCreateInstance(&AttachmentServices, None, CLSCTX_INPROC_SERVER).map_err(os_error)?;
        attachment
            .SetLocalPath(PCWSTR(path.as_ptr()))
            .map_err(os_error)?;
        attachment
            .SetSource(PCWSTR(source.as_ptr()))
            .map_err(os_error)?;
        attachment
    };
    let mut process = Process(HANDLE::default());
    // SAFETY: synchronous call with a live process output slot and the default verb.
    unsafe { attachment.Execute(HWND::default(), PCWSTR::null(), Some(&mut process.0)) }
        .map_err(os_error)?;
    if process.0.is_invalid() {
        return Err("Windows n’a renvoyé aucun processus d’installation.".into());
    }
    // SAFETY: the process handle remains open through both calls.
    if unsafe { WaitForSingleObject(process.0, INFINITE) } != WAIT_OBJECT_0 {
        return Err("Attente du processus NSIS interrompue.".into());
    }
    let mut status = 0;
    unsafe { GetExitCodeProcess(process.0, &mut status) }.map_err(os_error)?;
    Ok(status)
}
