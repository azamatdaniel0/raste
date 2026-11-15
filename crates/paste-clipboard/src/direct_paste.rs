use std::borrow::Cow;

use arboard::{Clipboard, ImageData};
use paste_core::{ItemDataEntity, ItemEntity, ItemType, PasteError, Result};

use crate::active_window::restore_window_focus;

/// Writes the selected clip back onto the system clipboard.
pub fn stage_item_to_clipboard(
    item: &ItemEntity,
    data: &ItemDataEntity,
    plain_text_only: bool,
) -> Result<()> {
    let mut cb = Clipboard::new()
        .map_err(|e| PasteError::InvalidInput(format!("failed to open clipboard: {e}")))?;

    if !plain_text_only && item.item_type == ItemType::Image {
        if let Some(ref png_bytes) = data.image_png {
            let decoded = image::load_from_memory(png_bytes)
                .map_err(|e| PasteError::Image(format!("failed to decode png: {e}")))?
                .to_rgba8();
            let (w, h) = decoded.dimensions();
            let img_data = ImageData {
                width: w as usize,
                height: h as usize,
                bytes: Cow::Owned(decoded.into_raw()),
            };
            cb.set_image(img_data)
                .map_err(|e| PasteError::InvalidInput(format!("clipboard set_image failed: {e}")))?;
            return Ok(());
        }
    }

    let text = data
        .plain_text
        .as_deref()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            if !data.file_paths.is_empty() {
                None
            } else {
                Some(item.title.as_str())
            }
        })
        .map(ToString::to_string)
        .unwrap_or_else(|| data.file_paths.join("\n"));

    cb.set_text(text)
        .map_err(|e| PasteError::InvalidInput(format!("clipboard set_text failed: {e}")))?;
    Ok(())
}

/// Restores focus to the target external window and synthesizes a paste keystroke (`Ctrl+V` / `Cmd+V`).
pub fn simulate_paste_keystroke(previous_hwnd: Option<isize>) -> Result<()> {
    if let Some(hwnd) = previous_hwnd {
        restore_window_focus(hwnd);
        std::thread::sleep(std::time::Duration::from_millis(55));
    }

    #[cfg(target_os = "windows")]
    {
        windows_send_ctrl_v();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("osascript")
            .args(["-e", "tell application \"System Events\" to keystroke \"v\" using command down"])
            .spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdotool")
            .args(["key", "ctrl+v"])
            .spawn();
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn windows_send_ctrl_v() {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL,
    };

    const VK_V: u16 = 0x56;

    let make_key = |vk: u16, flags: u32| INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };

    let inputs = [
        make_key(VK_CONTROL, 0),
        make_key(VK_V, 0),
        make_key(VK_V, KEYEVENTF_KEYUP),
        make_key(VK_CONTROL, KEYEVENTF_KEYUP),
    ];

    unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}
