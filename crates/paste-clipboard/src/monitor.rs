use std::io::Cursor;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use arboard::Clipboard;
use chrono::Utc;
use image::{ImageBuffer, ImageFormat, Rgba};
use paste_core::{processor::compute_checksum, Database, NewClipboardPayload};
use sha2::{Digest, Sha256};

use crate::active_window::get_active_window_info;
use crate::sound::SoundPlayer;
use crate::transient::is_current_clipboard_transient_or_concealed;

/// Background clipboard monitor matching `PasteCore.PasteboardHistoryManager`.
#[derive(Clone)]
pub struct ClipboardMonitor {
    db: Database,
    sound: SoundPlayer,
    last_observed_checksum: Arc<Mutex<Option<String>>>,
    running: Arc<AtomicBool>,
    on_new_item: Arc<Mutex<Option<Box<dyn Fn() + Send + Sync>>>>,
}

impl ClipboardMonitor {
    pub fn new(db: Database, sound: SoundPlayer) -> Self {
        Self {
            db,
            sound,
            last_observed_checksum: Arc::new(Mutex::new(None)),
            running: Arc::new(AtomicBool::new(false)),
            on_new_item: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_on_change<F>(&self, callback: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        if let Ok(mut guard) = self.on_new_item.lock() {
            *guard = Some(Box::new(callback));
        }
    }

    /// Suppresses ingestion for an item we just wrote to the OS clipboard during Direct Paste.
    pub fn mark_internal_write(&self, checksum: &str) {
        if let Ok(mut guard) = self.last_observed_checksum.lock() {
            *guard = Some(checksum.to_string());
        }
    }

    pub fn start(&self, poll_interval: Duration) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let monitor = self.clone();
        std::thread::Builder::new()
            .name("clipboard-monitor".to_string())
            .spawn(move || {
                monitor.poll_loop(poll_interval);
            })
            .ok();
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    fn poll_loop(&self, interval: Duration) {
        let mut clipboard = match Clipboard::new() {
            Ok(cb) => cb,
            Err(err) => {
                log::warn!("ClipboardMonitor failed to initialize clipboard: {err}");
                return;
            }
        };

        // Prime initial checksum so existing clipboard content at startup isn't re-notified noisily
        if let Some(payload) = read_clipboard_payload(&mut clipboard) {
            let sum = compute_checksum(&payload);
            self.mark_internal_write(&sum);
        }

        while self.running.load(Ordering::SeqCst) {
            std::thread::sleep(interval);

            if let Ok(prefs) = self.db.load_preferences() {
                if prefs.is_paused {
                    if let Some(until) = prefs.pause_until_ms {
                        if Utc::now().timestamp_millis() >= until {
                            let mut updated = prefs.clone();
                            updated.is_paused = false;
                            updated.pause_until_ms = None;
                            let _ = self.db.save_preferences(&updated);
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
            }

            if is_current_clipboard_transient_or_concealed() {
                continue;
            }

            let Some(mut payload) = read_clipboard_payload(&mut clipboard) else {
                continue;
            };

            let checksum = compute_checksum(&payload);
            {
                let mut last = self.last_observed_checksum.lock().unwrap();
                if last.as_deref() == Some(checksum.as_str()) {
                    continue;
                }
                *last = Some(checksum);
            }

            if let Some(win) = get_active_window_info() {
                // Avoid tagging items copied from inside New-Paste itself as external app
                if !win.app_id.eq_ignore_ascii_case("new-paste.exe") {
                    payload.source_app_id = Some(win.app_id);
                    payload.source_app_name = Some(win.app_name);
                    payload.source_app_color = Some(win.accent_color_hex);
                }
            }

            match self.db.ingest_clipboard_payload(payload) {
                Ok((_, _is_new)) => {
                    if let Ok(prefs) = self.db.load_preferences() {
                        if prefs.enable_sound_effects {
                            self.sound.play_copy();
                        }
                    }
                    if let Ok(guard) = self.on_new_item.lock() {
                        if let Some(ref cb) = *guard {
                            cb();
                        }
                    }
                }
                Err(err) => {
                    log::debug!("Clipboard ingestion skipped/error: {err}");
                }
            }
        }
    }
}

fn read_clipboard_payload(cb: &mut Clipboard) -> Option<NewClipboardPayload> {
    if let Ok(text) = cb.get_text() {
        if !text.is_empty() {
            return Some(NewClipboardPayload {
                plain_text: Some(text),
                ..Default::default()
            });
        }
    }

    if let Ok(img) = cb.get_image() {
        let w = img.width as u32;
        let h = img.height as u32;
        if w > 0 && h > 0 {
            if let Some(rgba_buf) =
                ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(w, h, img.bytes.into_owned())
            {
                let mut png_out = Cursor::new(Vec::new());
                if image::DynamicImage::ImageRgba8(rgba_buf)
                    .write_to(&mut png_out, ImageFormat::Png)
                    .is_ok()
                {
                    return Some(NewClipboardPayload {
                        image_png: Some(png_out.into_inner()),
                        ..Default::default()
                    });
                }
            }
        }
    }

    None
}

#[allow(dead_code)]
fn quick_hash(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}
