use std::path::PathBuf;

/// Plays Copy and Paste audio cues asynchronously without blocking the UI thread.
#[derive(Debug, Clone)]
pub struct SoundPlayer {
    copy_wav: Option<PathBuf>,
    paste_wav: Option<PathBuf>,
}

impl Default for SoundPlayer {
    fn default() -> Self {
        Self::discover()
    }
}

impl SoundPlayer {
    pub fn discover() -> Self {
        let candidates = [
            PathBuf::from("assets/sounds"),
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("assets/sounds")))
                .unwrap_or_default(),
        ];

        let mut copy_wav = None;
        let mut paste_wav = None;

        for dir in candidates {
            let c = dir.join("copy.wav");
            if c.is_file() && copy_wav.is_none() {
                copy_wav = Some(c);
            }
            let p = dir.join("paste.wav");
            if p.is_file() && paste_wav.is_none() {
                paste_wav = Some(p);
            }
        }

        Self {
            copy_wav,
            paste_wav,
        }
    }

    pub fn play_copy(&self) {
        if let Some(ref path) = self.copy_wav {
            play_wav_async(path);
        }
    }

    pub fn play_paste(&self) {
        if let Some(ref path) = self.paste_wav {
            play_wav_async(path);
        }
    }
}

fn play_wav_async(path: &PathBuf) {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Media::Audio::{PlaySoundW, SND_ASYNC, SND_FILENAME, SND_NODEFAULT};

        let wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        unsafe {
            PlaySoundW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                SND_FILENAME | SND_ASYNC | SND_NODEFAULT,
            );
        }
    }

    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("afplay").arg(path).spawn();
    }

    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("aplay")
            .arg("-q")
            .arg(path)
            .spawn();
    }
}
