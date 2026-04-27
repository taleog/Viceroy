use anyhow::{anyhow, Result};
use std::sync::mpsc::{self, Receiver};
use std::thread;

use crate::windows_hotkey_log as hklog;
use crate::windows_hwnd;

#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    Pressed,
    Error(String),
}

pub fn start_hotkey_listener(hotkey: &str) -> Receiver<HotkeyEvent> {
    let (tx, rx) = mpsc::channel();
    let hotkey = hotkey.trim().to_string();

    thread::spawn(move || {
        hklog::append(&format!("hotkey: starting listener (configured='{hotkey}')"));
        if hotkey.is_empty() {
            hklog::append("hotkey: empty hotkey");
            let _ = tx.send(HotkeyEvent::Error(
                "Hotkey is empty. Set something like Ctrl+Alt+Space.".to_string(),
            ));
            return;
        }

        let (mods, vk, pretty) = match parse_hotkey(&hotkey) {
            Ok(v) => v,
            Err(err) => {
                let _ = tx.send(HotkeyEvent::Error(format!("Invalid hotkey '{hotkey}': {err:#}")));
                return;
            }
        };

        unsafe {
            use windows_sys::Win32::Foundation::{BOOL, GetLastError};
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey};
            use windows_sys::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};

            // Use a fixed hotkey ID for this process.
            let id: i32 = 1;

            hklog::append(&format!("hotkey: attempting RegisterHotKey('{pretty}')"));
            let mut ok = RegisterHotKey(std::ptr::null_mut(), id, mods, vk);

            if ok == 0 {
                let code = GetLastError();
                hklog::append(&format!(
                    "hotkey: RegisterHotKey failed for '{pretty}' (err={code})"
                ));

                // Ctrl+Space is commonly taken by IME/input method toggles.
                // If the user configured something taken, try a safe fallback.
                let fallback = "Ctrl+Alt+Space";
                if let Ok((fallback_mods, fallback_vk, fallback_pretty)) = parse_hotkey(fallback) {
                    hklog::append(&format!(
                        "hotkey: attempting fallback RegisterHotKey('{fallback_pretty}')"
                    ));
                    ok = RegisterHotKey(std::ptr::null_mut(), id, fallback_mods, fallback_vk);
                    if ok != 0 {
                        hklog::append(&format!(
                            "hotkey: fallback registered OK ('{fallback_pretty}')"
                        ));
                        let _ = tx.send(HotkeyEvent::Error(format!(
                            "Hotkey '{pretty}' could not be registered (error {code}). Using fallback '{fallback_pretty}'. Update Settings → Hotkey to keep it."
                        )));
                    } else {
                        let code2 = GetLastError();
                        hklog::append(&format!(
                            "hotkey: fallback failed too ('{fallback_pretty}', err={code2})"
                        ));
                        let _ = tx.send(HotkeyEvent::Error(format!(
                            "Failed to register global hotkey ({pretty}). Error code {code}. Tried fallback '{fallback}' but also failed (error {code2}). This usually means another app already uses it (PowerToys Run, IME, etc.)."
                        )));
                        return;
                    }
                } else {
                    let _ = tx.send(HotkeyEvent::Error(format!(
                        "Failed to register global hotkey ({pretty}). Error code {code}."
                    )));
                    return;
                }
            } else {
                hklog::append(&format!("hotkey: registered OK ('{pretty}')"));
            }

            // Let the UI know registration succeeded (nice-to-have).
            // (We reuse Error variant for status? No: keep quiet; UI can infer.)

            let mut msg: MSG = std::mem::zeroed();
            loop {
                let ret: BOOL = GetMessageW(&mut msg as *mut MSG, std::ptr::null_mut(), 0, 0);
                if ret == 0 {
                    // WM_QUIT
                    break;
                }
                if ret == -1 {
                    // error
                    break;
                }
                if msg.message == WM_HOTKEY && msg.wParam == id as usize {
                    hklog::append("hotkey: WM_HOTKEY received");

                    // Restore/focus the actual Win32 window immediately.
                    // This avoids relying on egui repaint/update loops, which may pause when minimized/hidden.
                    if let Some(hwnd) = windows_hwnd::get() {
                        use windows_sys::Win32::UI::WindowsAndMessaging::{
                            SetForegroundWindow, ShowWindow, SW_RESTORE,
                        };
                        unsafe {
                            ShowWindow(hwnd as _, SW_RESTORE);
                            SetForegroundWindow(hwnd as _);
                        }
                        hklog::append("hotkey: ShowWindow(SW_RESTORE) attempted");
                    } else {
                        hklog::append("hotkey: no HWND captured yet");
                    }

                    let _ = tx.send(HotkeyEvent::Pressed);
                }
            }

            let _ = UnregisterHotKey(std::ptr::null_mut(), id);
        }
    });

    rx
}

fn parse_hotkey(hotkey: &str) -> Result<(u32, u32, String)> {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;

    let parts = hotkey
        .split('+')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();

    if parts.is_empty() {
        return Err(anyhow!("hotkey has no parts"));
    }

    let mut mods: u32 = 0;
    let mut key_part: Option<&str> = None;

    for p in parts {
        let token = p.to_ascii_lowercase();
        match token.as_str() {
            "alt" => mods |= MOD_ALT,
            "ctrl" | "control" => mods |= MOD_CONTROL,
            "shift" => mods |= MOD_SHIFT,
            "win" | "super" | "meta" => mods |= MOD_WIN,
            _ => {
                if key_part.is_some() {
                    return Err(anyhow!("multiple key parts found (got '{p}' after '{:?}')", key_part));
                }
                key_part = Some(p);
            }
        }
    }

    let key = key_part.ok_or_else(|| anyhow!("missing key (example: Ctrl+Space)"))?;
    let vk: u32 = match key.to_ascii_lowercase().as_str() {
        "space" => VK_SPACE as u32,
        "tab" => VK_TAB as u32,
        "enter" | "return" => VK_RETURN as u32,
        "escape" | "esc" => VK_ESCAPE as u32,
        "backspace" => VK_BACK as u32,
        "delete" | "del" => VK_DELETE as u32,
        "up" => VK_UP as u32,
        "down" => VK_DOWN as u32,
        "left" => VK_LEFT as u32,
        "right" => VK_RIGHT as u32,
        // Letters
        k if k.len() == 1 && k.chars().next().unwrap().is_ascii_alphabetic() => {
            let c = k.chars().next().unwrap().to_ascii_uppercase() as u8;
            (VK_A as u32) + (c - b'A') as u32
        }
        // Digits
        k if k.len() == 1 && k.chars().next().unwrap().is_ascii_digit() => {
            let c = k.chars().next().unwrap() as u8;
            (VK_0 as u32) + (c - b'0') as u32
        }
        // F-keys
        k if k.starts_with('f') => {
            let n: u32 = k[1..]
                .parse()
                .map_err(|_| anyhow!("invalid function key '{key}'"))?;
            if !(1..=24).contains(&n) {
                return Err(anyhow!("function key out of range: {key}"));
            }
            (VK_F1 as u32) + (n - 1)
        }
        _ => {
            return Err(anyhow!(
                "unsupported key '{key}'. Supported examples: Ctrl+Space, Alt+Space, Ctrl+F1, Ctrl+K"
            ))
        }
    };

    if mods == 0 {
        // Force at least one modifier to avoid hijacking normal typing.
        return Err(anyhow!("hotkey needs a modifier (Ctrl/Alt/Shift/Win)"));
    }

    let pretty = format!("{hotkey}");
    Ok((mods, vk, pretty))
}
