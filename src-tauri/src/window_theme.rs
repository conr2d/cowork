use cowork_errors::Envelope;

#[tauri::command]
pub fn set_window_theme(window: tauri::WebviewWindow, theme: String) -> Result<(), Envelope> {
    let (paper, ink) = colors_for_theme(&theme);
    apply_window_theme(window, paper, ink);
    Ok(())
}

const fn colorref(r: u8, g: u8, b: u8) -> u32 {
    (b as u32) << 16 | (g as u32) << 8 | r as u32
}

fn colors_for_theme(theme: &str) -> (u32, u32) {
    match theme {
        "light" => (colorref(0xfa, 0xf8, 0xf5), colorref(0x1c, 0x1a, 0x17)),
        _ => (colorref(0x1a, 0x19, 0x16), colorref(0xec, 0xe6, 0xda)),
    }
}

#[cfg(windows)]
fn apply_window_theme(window: tauri::WebviewWindow, paper: u32, ink: u32) {
    use std::mem::size_of;

    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DwmSetWindowAttribute,
    };

    let hwnd = match window.hwnd() {
        Ok(hwnd) => hwnd.0,
        Err(_) => return,
    };

    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_COLOR as u32,
            (&paper as *const u32).cast(),
            size_of::<u32>() as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TEXT_COLOR as u32,
            (&ink as *const u32).cast(),
            size_of::<u32>() as u32,
        );
    }
}

#[cfg(not(windows))]
fn apply_window_theme(_window: tauri::WebviewWindow, _paper: u32, _ink: u32) {}
