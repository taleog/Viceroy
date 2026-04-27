#![cfg(target_os = "windows")]

use anyhow::{anyhow, Result};
use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS,
};
use windows_sys::Win32::UI::Shell::{
    SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_SMALLICON,
    SHGFI_USEFILEATTRIBUTES,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

pub struct RgbaIcon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

fn to_wide(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(once(0)).collect()
}

pub fn load_file_icon_rgba(path: &str) -> Result<RgbaIcon> {
    if path.trim().is_empty() {
        return Err(anyhow!("empty path"));
    }

    // First try using the real path.
    let mut info: SHFILEINFOW = unsafe { std::mem::zeroed() };
    let wide = to_wide(path);
    let flags = SHGFI_ICON | SHGFI_LARGEICON;

    let ok = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            0,
            &mut info,
            std::mem::size_of::<SHFILEINFOW>() as u32,
            flags,
        )
    };

    let mut hicon = info.hIcon;

    // Fallback: use file attributes (works for missing targets, but still gives type icon).
    if ok == 0 || hicon.is_null() {
        let mut info2: SHFILEINFOW = unsafe { std::mem::zeroed() };
        let flags2 = SHGFI_ICON | SHGFI_SMALLICON | SHGFI_USEFILEATTRIBUTES;
        let ok2 = unsafe {
            SHGetFileInfoW(
                wide.as_ptr(),
                0,
                &mut info2,
                std::mem::size_of::<SHFILEINFOW>() as u32,
                flags2,
            )
        };
        if ok2 == 0 || info2.hIcon.is_null() {
            return Err(anyhow!("SHGetFileInfoW failed"));
        }
        hicon = info2.hIcon;
    }

    // Convert HICON -> RGBA bitmap.
    let mut icon_info: ICONINFO = unsafe { std::mem::zeroed() };
    let ok_icon = unsafe { GetIconInfo(hicon, &mut icon_info) };
    if ok_icon == 0 {
        unsafe {
            DestroyIcon(hicon);
        }
        return Err(anyhow!("GetIconInfo failed"));
    }

    let color_bmp = icon_info.hbmColor;
    let mask_bmp = icon_info.hbmMask;

    if color_bmp.is_null() {
        unsafe {
            if !mask_bmp.is_null() {
                DeleteObject(mask_bmp);
            }
            DestroyIcon(hicon);
        }
        return Err(anyhow!("icon has no color bitmap"));
    }

    let mut bmp: BITMAP = unsafe { std::mem::zeroed() };
    let got = unsafe {
        GetObjectW(
            color_bmp,
            std::mem::size_of::<BITMAP>() as i32,
            &mut bmp as *mut _ as *mut _,
        )
    };
    if got == 0 {
        unsafe {
            DeleteObject(color_bmp);
            if !mask_bmp.is_null() {
                DeleteObject(mask_bmp);
            }
            DestroyIcon(hicon);
        }
        return Err(anyhow!("GetObjectW failed"));
    }

    let width = bmp.bmWidth.max(1) as u32;
    let height = bmp.bmHeight.max(1) as u32;

    // Prepare DIB
    let mut header = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: width as i32,
        biHeight: -(height as i32), // top-down
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        biSizeImage: 0,
        biXPelsPerMeter: 0,
        biYPelsPerMeter: 0,
        biClrUsed: 0,
        biClrImportant: 0,
    };

    let mut bmi: BITMAPINFO = unsafe { std::mem::zeroed() };
    bmi.bmiHeader = header;

    let mut bgra = vec![0u8; (width * height * 4) as usize];

    unsafe {
        let hdc = GetDC(0 as HWND);
        let mem_dc = CreateCompatibleDC(hdc);
        let scanlines = GetDIBits(
            mem_dc,
            color_bmp,
            0,
            height,
            bgra.as_mut_ptr() as *mut _,
            &mut bmi,
            DIB_RGB_COLORS,
        );
        DeleteDC(mem_dc);
        ReleaseDC(0 as HWND, hdc);

        // Cleanup GDI objects
        DeleteObject(color_bmp);
        if !mask_bmp.is_null() {
            DeleteObject(mask_bmp);
        }
        DestroyIcon(hicon);

        if scanlines == 0 {
            return Err(anyhow!("GetDIBits failed"));
        }
    }

    // Convert BGRA -> RGBA (and un-premultiply if needed).
    let mut rgba = vec![0u8; bgra.len()];
    for i in (0..bgra.len()).step_by(4) {
        let b = bgra[i];
        let g = bgra[i + 1];
        let r = bgra[i + 2];
        let a = bgra[i + 3];

        if a != 0 && a != 255 {
            let r2 = (r as u32 * 255 / a as u32).min(255) as u8;
            let g2 = (g as u32 * 255 / a as u32).min(255) as u8;
            let b2 = (b as u32 * 255 / a as u32).min(255) as u8;
            rgba[i] = r2;
            rgba[i + 1] = g2;
            rgba[i + 2] = b2;
            rgba[i + 3] = a;
        } else {
            rgba[i] = r;
            rgba[i + 1] = g;
            rgba[i + 2] = b;
            rgba[i + 3] = a;
        }
    }

    Ok(RgbaIcon {
        width,
        height,
        rgba,
    })
}
