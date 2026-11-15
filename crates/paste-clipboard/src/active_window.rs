use std::path::Path;

/// Metadata about the active foreground window / source application.
#[derive(Debug, Clone)]
pub struct ActiveWindowInfo {
    pub app_id: String,
    pub app_name: String,
    pub window_title: String,
    pub accent_color_hex: String,
    pub hwnd: isize,
}

/// Captures the currently focused OS window and its owning application.
pub fn get_active_window_info() -> Option<ActiveWindowInfo> {
    #[cfg(target_os = "windows")]
    {
        windows_active_window()
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

/// Restores OS focus to a previously captured window handle (used by Direct Paste).
pub fn restore_window_focus(hwnd: isize) -> bool {
    if hwnd == 0 {
        return false;
    }
    #[cfg(target_os = "windows")]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
        };
        let handle = hwnd as windows_sys::Win32::Foundation::HWND;
        if IsIconic(handle) != 0 {
            ShowWindow(handle, SW_RESTORE);
        }
        SetForegroundWindow(handle) != 0
    }
    #[cfg(not(target_os = "windows"))]
    {
        false
    }
}

fn friendly_app_name(exe_name: &str) -> String {
    let stem = Path::new(exe_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(exe_name);
    match stem.to_ascii_lowercase().as_str() {
        "code" => "Visual Studio Code".to_string(),
        "zed" => "Zed".to_string(),
        "windowsterminal" | "wt" => "Windows Terminal".to_string(),
        "powershell" | "pwsh" => "PowerShell".to_string(),
        "chrome" => "Google Chrome".to_string(),
        "msedge" => "Microsoft Edge".to_string(),
        "firefox" => "Firefox".to_string(),
        "zen" => "Zen Browser".to_string(),
        "explorer" => "File Explorer".to_string(),
        "notepad" => "Notepad".to_string(),
        "figma" => "Figma".to_string(),
        "slack" => "Slack".to_string(),
        "discord" => "Discord".to_string(),
        "telegram" => "Telegram".to_string(),
        "new-paste" => "New-Paste".to_string(),
        _ => stem.to_string(),
    }
}

fn deterministic_accent_color(name: &str) -> String {
    const PALETTE: &[&str] = &[
        "#4A90E2", "#50E3C2", "#F5A623", "#BD10E0", "#FF5E57", "#00B894", "#6C5CE7", "#0984E3",
    ];
    let hash = name
        .bytes()
        .fold(5381u64, |acc, b| acc.wrapping_mul(33).wrapping_add(b as u64));
    PALETTE[(hash as usize) % PALETTE.len()].to_string()
}

#[cfg(target_os = "windows")]
fn windows_active_window() -> Option<ActiveWindowInfo> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId,
    };

    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_null() {
            return None;
        }

        let mut title_buf = [0u16; 512];
        let title_len = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
        let window_title = if title_len > 0 {
            String::from_utf16_lossy(&title_buf[..title_len as usize])
        } else {
            String::new()
        };

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == 0 {
            return None;
        }

        let proc_handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if proc_handle.is_null() {
            return None;
        }

        let mut path_buf = [0u16; 1024];
        let mut size = path_buf.len() as u32;
        let ok = QueryFullProcessImageNameW(proc_handle, 0, path_buf.as_mut_ptr(), &mut size);
        CloseHandle(proc_handle);

        if ok == 0 || size == 0 {
            return None;
        }

        let full_path = String::from_utf16_lossy(&path_buf[..size as usize]);
        let exe_name = Path::new(&full_path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown.exe")
            .to_string();

        let app_name = friendly_app_name(&exe_name);
        let accent_color_hex = deterministic_accent_color(&exe_name);

        Some(ActiveWindowInfo {
            app_id: exe_name,
            app_name,
            window_title,
            accent_color_hex,
            hwnd: hwnd as isize,
        })
    }
}
