use std::io;
use winapi::shared::windef::{HBITMAP, HDC};
use windows::Win32::UI::HiDpi::{
    SetProcessDpiAwareness, SetProcessDpiAwarenessContext,
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, PROCESS_DPI_AWARENESS,
};

#[derive(Clone, Debug, Default)]
pub struct CursorData {
    pub id: u64,
    pub colors: Vec<u8>,
    pub hotx: i32,
    pub hoty: i32,
    pub width: i32,
    pub height: i32,
}

pub fn initialize_windows_features() -> io::Result<()> {
    set_dpi_awareness()?;
    set_process_priority()?;
    set_timer_resolution()?;
    disable_dwm_composition_on_old_windows();
    set_multimedia_thread_priority()?;

    log::info!("Windows features initialized successfully");
    Ok(())
}

fn set_timer_resolution() -> io::Result<()> {
    use winapi::um::timeapi::timeBeginPeriod;

    unsafe {
        let result = timeBeginPeriod(1);
        if result == 0 {
            log::info!("Set timer resolution to 1ms");
            Ok(())
        } else {
            log::warn!("Failed to set timer resolution");
            Ok(())
        }
    }
}

fn set_multimedia_thread_priority() -> io::Result<()> {
    use windows::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_TIME_CRITICAL,
    };

    unsafe {
        let handle = GetCurrentThread();
        if SetThreadPriority(handle, THREAD_PRIORITY_TIME_CRITICAL).is_ok() {
            log::info!("Set multimedia thread priority");
            Ok(())
        } else {
            log::warn!("Failed to set thread priority");
            Ok(())
        }
    }
}

fn disable_dwm_composition_on_old_windows() {
    let (major, _, _) = get_windows_version();
    if major < 10 {
        log::info!(
            "Windows < 10 detected, disabling DWM composition for better capture performance"
        );
    }
}

fn set_dpi_awareness() -> io::Result<()> {
    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok() {
            log::info!("Set DPI awareness to Per-Monitor V2");
            return Ok(());
        }

        if SetProcessDpiAwareness(PROCESS_DPI_AWARENESS(2)).is_ok() {
            log::info!("Set DPI awareness to Per-Monitor");
            return Ok(());
        }

        log::warn!("Failed to set DPI awareness, using default");
    }

    Ok(())
}

fn set_process_priority() -> io::Result<()> {
    use windows::Win32::System::Threading::{
        GetCurrentProcess, SetPriorityClass, ABOVE_NORMAL_PRIORITY_CLASS,
    };

    unsafe {
        let handle = GetCurrentProcess();
        if SetPriorityClass(handle, ABOVE_NORMAL_PRIORITY_CLASS).is_ok() {
            log::info!("Set process priority to above normal");
            Ok(())
        } else {
            log::warn!("Failed to set process priority");
            Ok(())
        }
    }
}

pub fn get_windows_version() -> (u32, u32, u32) {
    use windows::Win32::System::SystemInformation::{GetVersionExW, OSVERSIONINFOW};

    unsafe {
        // Try using RtlGetVersion from ntdll.dll via dynamic loading
        // This is more reliable than GetVersionExW which can be shimmed
        #[repr(C)]
        struct RTL_OSVERSIONINFOW {
            dwOSVersionInfoSize: u32,
            dwMajorVersion: u32,
            dwMinorVersion: u32,
            dwBuildNumber: u32,
            dwPlatformId: u32,
            szCSDVersion: [u16; 128],
        }

        type RtlGetVersionFn = unsafe extern "system" fn(*mut RTL_OSVERSIONINFOW) -> i32;

        // Try to load RtlGetVersion from ntdll.dll
        let ntdll =
            windows::Win32::System::LibraryLoader::LoadLibraryW(windows::core::w!("ntdll.dll"));

        if let Ok(ntdll) = ntdll {
            let proc = windows::Win32::System::LibraryLoader::GetProcAddress(
                ntdll,
                windows::core::s!("RtlGetVersion"),
            );

            if let Some(proc) = proc {
                let rtl_get_version: RtlGetVersionFn = std::mem::transmute(proc);
                let mut rtl_info: RTL_OSVERSIONINFOW = std::mem::zeroed();
                rtl_info.dwOSVersionInfoSize = std::mem::size_of::<RTL_OSVERSIONINFOW>() as u32;

                if rtl_get_version(&mut rtl_info) == 0 {
                    return (
                        rtl_info.dwMajorVersion,
                        rtl_info.dwMinorVersion,
                        rtl_info.dwBuildNumber,
                    );
                }
            }
        }

        // Fallback to GetVersionExW
        let mut legacy_info: OSVERSIONINFOW = std::mem::zeroed();
        legacy_info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;

        if GetVersionExW(&mut legacy_info).is_ok() {
            return (
                legacy_info.dwMajorVersion,
                legacy_info.dwMinorVersion,
                legacy_info.dwBuildNumber,
            );
        }

        (0, 0, 0)
    }
}

pub fn is_dxgi_available() -> bool {
    let (major, _, _) = get_windows_version();
    major >= 6
}

pub fn get_optimal_capture_method() -> CaptureMethod {
    if !is_dxgi_available() {
        return CaptureMethod::GDI;
    }

    let (major, minor, _) = get_windows_version();
    if major > 6 || (major == 6 && minor >= 2) {
        CaptureMethod::DXGI
    } else {
        CaptureMethod::GDI
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CaptureMethod {
    DXGI,
    GDI,
}

extern "C" {
    fn selectInputDesktop() -> i32;
    fn inputDesktopSelected() -> i32;
    fn handleMask(
        out: *mut u8,
        mask: *const u8,
        width: i32,
        height: i32,
        bm_width_bytes: i32,
        bm_height: i32,
    ) -> i32;
    fn drawOutline(outline: *mut u8, colors: *const u8, width: i32, height: i32, outline_len: i32);
    fn get_di_bits(out: *mut u8, dc: HDC, hbm: HBITMAP, width: i32, height: i32) -> i32;
}

pub fn set_error_mode() {
    use windows::Win32::System::Diagnostics::Debug::{
        SetErrorMode, SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX,
    };

    unsafe {
        SetErrorMode(SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX);
    }
}

pub fn is_elevated() -> bool {
    use windows::Win32::Security::{
        GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY,
    };
    use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    unsafe {
        let mut token = std::mem::zeroed();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).is_err() {
            return false;
        }

        let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
        let mut size = 0;

        if GetTokenInformation(
            token,
            TokenElevation,
            Some(&mut elevation as *mut _ as *mut _),
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        )
        .is_err()
        {
            return false;
        }

        elevation.TokenIsElevated != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_windows_version() {
        let (major, minor, build) = get_windows_version();
        assert!(major > 0);
        println!("Windows version: {}.{}.{}", major, minor, build);
    }

    #[test]
    fn test_dxgi_availability() {
        assert!(is_dxgi_available());
    }

    #[test]
    fn test_capture_method() {
        let method = get_optimal_capture_method();
        println!("Optimal capture method: {:?}", method);
    }
}
