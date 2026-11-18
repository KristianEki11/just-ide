//! OS appearance detection (dark mode, high contrast).

/// System appearance settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OsAppearance {
    /// Whether the OS is in dark mode.
    pub dark_mode: bool,
    /// Whether high contrast mode is enabled.
    pub high_contrast: bool,
}

impl OsAppearance {
    /// Detects the current OS appearance.
    #[must_use]
    pub fn detect() -> Self {
        #[cfg(target_os = "windows")]
        {
            detect_windows_appearance()
        }

        #[cfg(not(target_os = "windows"))]
        {
            Self {
                dark_mode: true,
                high_contrast: false,
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn detect_windows_appearance() -> OsAppearance {
    OsAppearance {
        dark_mode: detect_dark_mode(),
        high_contrast: detect_high_contrast(),
    }
}

#[cfg(target_os = "windows")]
fn detect_dark_mode() -> bool {
    use windows_sys::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, REG_DWORD,
    };

    let subkey: Vec<u16> = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let value_name: Vec<u16> = "AppsUseLightTheme"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let mut hkey: HKEY = std::ptr::null_mut();
    let mut data: u32 = 0;
    #[allow(clippy::cast_possible_truncation)]
    let mut data_size: u32 = std::mem::size_of::<u32>() as u32;
    let mut data_type: u32 = 0;

    // SAFETY: `subkey` is a NUL-terminated UTF-16 slice valid for the call and `hkey` is stack-allocated.
    let open_result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            0,
            KEY_READ,
            &raw mut hkey,
        )
    };

    if open_result != 0 {
        return true;
    }

    // SAFETY: `hkey` is a valid open registry key, `value_name` is NUL-terminated UTF-16, and `data`
    // matches `data_size` (4 bytes for DWORD). `hkey` is closed unconditionally immediately afterward.
    let query_result = unsafe {
        RegQueryValueExW(
            hkey,
            value_name.as_ptr(),
            std::ptr::null_mut(),
            &raw mut data_type,
            (&raw mut data).cast::<u8>(),
            &raw mut data_size,
        )
    };

    // SAFETY: `hkey` was opened by `RegOpenKeyExW` and returned status 0.
    unsafe { RegCloseKey(hkey) };

    if query_result != 0 || data_type != REG_DWORD {
        return true;
    }

    data == 0
}

#[cfg(target_os = "windows")]
fn detect_high_contrast() -> bool {
    use windows_sys::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
    use windows_sys::Win32::UI::WindowsAndMessaging::{SystemParametersInfoW, SPI_GETHIGHCONTRAST};

    #[allow(clippy::cast_possible_truncation)]
    let cb_size = std::mem::size_of::<HIGHCONTRASTW>() as u32;
    let mut hc = HIGHCONTRASTW {
        cbSize: cb_size,
        dwFlags: 0,
        lpszDefaultScheme: std::ptr::null_mut(),
    };

    // SAFETY: `hc` is initialized with `cbSize = size_of::<HIGHCONTRASTW>()` and lives for the call;
    // `SPI_GETHIGHCONTRAST` only writes fixed-size scalar/pointer fields into `hc` without dereferencing
    // `lpszDefaultScheme` when initialized to null.
    let result = unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            hc.cbSize,
            (&raw mut hc).cast::<std::ffi::c_void>(),
            0,
        )
    };

    result != 0 && (hc.dwFlags & HCF_HIGHCONTRASTON) != 0
}
