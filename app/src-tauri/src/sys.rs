//! Small Windows queries: accent colour, display language, rounded window corners.
//! Hand-written FFI (as in the bridge) rather than another crate; every call is read-only except
//! the corner preference, which only affects our own windows.

#[cfg(windows)]
mod ffi {
    use std::ffi::c_void;

    pub const HKEY_CURRENT_USER: isize = 0x8000_0001_u32 as i32 as isize;
    pub const RRF_RT_REG_DWORD: u32 = 0x0000_0010;
    pub const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    pub const DWMWCP_ROUND: u32 = 2;

    #[link(name = "advapi32")]
    extern "system" {
        pub fn RegGetValueW(
            hkey: isize,
            sub_key: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut c_void,
            len: *mut u32,
        ) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        pub fn GetUserDefaultUILanguage() -> u16;
    }

    #[link(name = "dwmapi")]
    extern "system" {
        pub fn DwmSetWindowAttribute(
            hwnd: *mut c_void,
            attr: u32,
            value: *const c_void,
            size: u32,
        ) -> i32;
    }
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// The Windows accent colour as `#rrggbb` (DWM stores it as 0xAABBGGRR).
#[cfg(windows)]
pub fn accent_colour() -> Option<String> {
    let key = wide(r"Software\Microsoft\Windows\DWM");
    let name = wide("AccentColor");
    let mut v: u32 = 0;
    let mut len: u32 = 4;
    // SAFETY: valid NUL-terminated strings, a 4-byte buffer and its length.
    let rc = unsafe {
        ffi::RegGetValueW(
            ffi::HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            ffi::RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&mut v as *mut u32).cast(),
            &mut len,
        )
    };
    (rc == 0).then(|| abgr_to_hex(v))
}

#[cfg(not(windows))]
pub fn accent_colour() -> Option<String> {
    None
}

pub fn abgr_to_hex(v: u32) -> String {
    let r = v & 0xFF;
    let g = (v >> 8) & 0xFF;
    let b = (v >> 16) & 0xFF;
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// True when the Windows display language is French (any region).
#[cfg(windows)]
pub fn ui_language_is_french() -> bool {
    // SAFETY: no arguments, returns a LANGID.
    let langid = unsafe { ffi::GetUserDefaultUILanguage() };
    langid & 0x3FF == 0x0C
}

#[cfg(not(windows))]
pub fn ui_language_is_french() -> bool {
    false
}

/// Ask DWM for Windows 11 rounded corners on an undecorated window (ignored on Windows 10).
#[cfg(windows)]
pub fn round_corners(hwnd: *mut std::ffi::c_void) {
    let pref = ffi::DWMWCP_ROUND;
    // SAFETY: `hwnd` is one of our live windows; the value is a 4-byte enum.
    unsafe {
        ffi::DwmSetWindowAttribute(
            hwnd,
            ffi::DWMWA_WINDOW_CORNER_PREFERENCE,
            (&pref as *const u32).cast(),
            4,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accent_byte_order() {
        // DWM stores 0xAABBGGRR: default Windows 11 blue #0078d4 is 0xffd47800.
        assert_eq!(abgr_to_hex(0xffd4_7800), "#0078d4");
    }
}
