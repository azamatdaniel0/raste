//! Bottom "deck" panel behaviour.
//!
//! On macOS the main window is turned into a borderless panel docked to the
//! bottom edge of the screen. It joins every Space (including other apps'
//! full-screen Spaces) so summoning it never switches away from the app the
//! user is working in, and it is toggled by a global shortcut.
//!
//! All functions must be called on the main thread. On other platforms they
//! are no-ops and the app keeps its regular window.

/// Height of the docked panel in points.
pub const PANEL_HEIGHT: f64 = 400.0;

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::c_void;
    use std::ptr::null_mut;
    use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::{NSPoint, NSRect, NSSize};

    use super::PANEL_HEIGHT;

    static WINDOW: AtomicPtr<AnyObject> = AtomicPtr::new(null_mut());
    static VISIBLE: AtomicBool = AtomicBool::new(true);
    /// Set once the panel has received keyboard focus since it was last shown.
    static HAD_FOCUS: AtomicBool = AtomicBool::new(false);

    // NSApplicationActivationPolicyAccessory: no Dock icon, may overlay full-screen apps.
    const ACTIVATION_POLICY_ACCESSORY: isize = 1;
    // NSWindowCollectionBehavior: canJoinAllSpaces | ignoresCycle | fullScreenAuxiliary
    const COLLECTION_BEHAVIOR: usize = (1 << 0) | (1 << 6) | (1 << 8);
    // NSStatusWindowLevel — above normal windows and the Dock.
    const WINDOW_LEVEL: isize = 25;

    pub fn install(ns_view: *mut c_void) {
        if ns_view.is_null() {
            return;
        }
        unsafe {
            let view = ns_view as *mut AnyObject;
            let window: *mut AnyObject = msg_send![view, window];
            if window.is_null() {
                return;
            }
            WINDOW.store(window, Ordering::SeqCst);

            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            let _: bool = msg_send![app, setActivationPolicy: ACTIVATION_POLICY_ACCESSORY];

            let _: () = msg_send![window, setCollectionBehavior: COLLECTION_BEHAVIOR];
            let _: () = msg_send![window, setLevel: WINDOW_LEVEL];
            let _: () = msg_send![window, setHidesOnDeactivate: false];
            let _: () = msg_send![window, setMovable: false];
            place(window);
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
        }
    }

    /// Docks the window to the bottom edge of the screen that has keyboard focus.
    unsafe fn place(window: *mut AnyObject) {
        let screen: *mut AnyObject = msg_send![class!(NSScreen), mainScreen];
        if screen.is_null() {
            return;
        }
        let frame: NSRect = msg_send![screen, frame];
        let height = PANEL_HEIGHT.min(frame.size.height);
        let rect = NSRect::new(
            NSPoint::new(frame.origin.x, frame.origin.y),
            NSSize::new(frame.size.width, height),
        );
        let _: () = msg_send![window, setFrame: rect, display: true];
    }

    pub fn is_visible() -> bool {
        VISIBLE.load(Ordering::SeqCst)
    }

    pub fn show() {
        let window = WINDOW.load(Ordering::SeqCst);
        if window.is_null() {
            return;
        }
        unsafe {
            place(window);
            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, activateIgnoringOtherApps: true];
            let nil: *mut AnyObject = null_mut();
            let _: () = msg_send![window, makeKeyAndOrderFront: nil];
        }
        HAD_FOCUS.store(false, Ordering::SeqCst);
        VISIBLE.store(true, Ordering::SeqCst);
        log::debug!("panel: shown");
    }

    /// Hides the panel. With `return_focus` the previously active app is
    /// re-activated (used when the user dismisses the panel or picks a clip).
    pub fn hide(return_focus: bool) {
        let window = WINDOW.load(Ordering::SeqCst);
        if window.is_null() {
            return;
        }
        unsafe {
            let nil: *mut AnyObject = null_mut();
            let _: () = msg_send![window, orderOut: nil];
            if return_focus {
                let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
                let _: () = msg_send![app, hide: nil];
            }
        }
        HAD_FOCUS.store(false, Ordering::SeqCst);
        VISIBLE.store(false, Ordering::SeqCst);
        log::debug!("panel: hidden (return_focus={return_focus})");
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> u8;
        fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
        static kAXTrustedCheckOptionPrompt: *const c_void;
        fn CGEventCreateKeyboardEvent(source: *const c_void, keycode: u16, key_down: bool) -> *mut c_void;
        fn CGEventSetFlags(event: *mut c_void, flags: u64);
        fn CGEventPost(tap: u32, event: *mut c_void);
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        static kCFBooleanTrue: *const c_void;
        fn CFRelease(cf: *const c_void);
    }

    static PROMPTED: AtomicBool = AtomicBool::new(false);

    /// Whether macOS lets this app post keystrokes (Privacy -> Accessibility).
    /// Shows the system permission prompt once per run when it does not.
    fn accessibility_trusted() -> bool {
        unsafe {
            if AXIsProcessTrusted() != 0 {
                return true;
            }
            if !PROMPTED.swap(true, Ordering::SeqCst) {
                let options: *mut AnyObject = msg_send![
                    class!(NSDictionary),
                    dictionaryWithObject: kCFBooleanTrue as *mut AnyObject,
                    forKey: kAXTrustedCheckOptionPrompt as *mut AnyObject
                ];
                AXIsProcessTrustedWithOptions(options as *const c_void);
            }
            false
        }
    }

    /// Hides the panel, hands focus back to the previous app and presses Cmd+V
    /// there. Returns `false` (leaving the panel alone) when macOS has not
    /// granted the Accessibility permission needed to post keystrokes.
    pub fn hide_and_paste() -> bool {
        if !accessibility_trusted() {
            return false;
        }
        hide(true);
        std::thread::spawn(|| {
            // Give the previous app time to become active again.
            std::thread::sleep(std::time::Duration::from_millis(160));
            const KEY_V: u16 = 9;
            const FLAG_COMMAND: u64 = 1 << 20;
            const HID_EVENT_TAP: u32 = 0;
            unsafe {
                for key_down in [true, false] {
                    let event = CGEventCreateKeyboardEvent(std::ptr::null(), KEY_V, key_down);
                    if event.is_null() {
                        return;
                    }
                    CGEventSetFlags(event, FLAG_COMMAND);
                    CGEventPost(HID_EVENT_TAP, event);
                    CFRelease(event as *const c_void);
                }
            }
        });
        true
    }

    pub fn toggle() {
        if is_visible() {
            hide(true);
        } else {
            show();
        }
    }

    /// Feeds the window focus state; hides the panel when the user clicks away.
    pub fn track_focus(focused: bool) {
        if !is_visible() {
            return;
        }
        if focused {
            if !HAD_FOCUS.swap(true, Ordering::SeqCst) {
                log::debug!("panel: focused");
            }
        } else if HAD_FOCUS.load(Ordering::SeqCst) {
            hide(false);
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn is_visible() -> bool {
        true
    }
    pub fn show() {}
    pub fn hide(_return_focus: bool) {}
    pub fn hide_and_paste() -> bool {
        false
    }
    pub fn track_focus(_focused: bool) {}
}

pub use imp::*;

/// Keeps the global shortcut registered for as long as it is alive.
pub struct PanelHotkey {
    #[cfg(target_os = "macos")]
    _manager: global_hotkey::GlobalHotKeyManager,
}

/// Converts the window into a docked panel and registers the global toggle
/// shortcut. Returns `None` when the shortcut could not be registered (the
/// panel then simply stays on screen).
#[cfg(target_os = "macos")]
pub fn setup(cc: &eframe::CreationContext<'_>, shortcut: &str) -> Option<PanelHotkey> {
    use std::str::FromStr;

    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    match cc.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::AppKit(handle)) => imp::install(handle.ns_view.as_ptr()),
        _ => {
            log::warn!("panel: no AppKit window handle, keeping a regular window");
            return None;
        }
    }

    let hotkey = HotKey::from_str(shortcut).unwrap_or_else(|err| {
        log::warn!("panel: cannot parse shortcut '{shortcut}' ({err}), using Shift+Cmd+V");
        HotKey::new(Some(Modifiers::SHIFT | Modifiers::META), Code::KeyV)
    });

    let manager = match GlobalHotKeyManager::new() {
        Ok(m) => m,
        Err(err) => {
            log::error!("panel: global hotkeys unavailable: {err}");
            return None;
        }
    };
    if let Err(err) = manager.register(hotkey) {
        log::error!("panel: failed to register '{shortcut}': {err}");
        return None;
    }

    let ctx = cc.egui_ctx.clone();
    let id = hotkey.id();
    GlobalHotKeyEvent::set_event_handler(Some(move |ev: GlobalHotKeyEvent| {
        if ev.id == id && ev.state == HotKeyState::Pressed {
            imp::toggle();
            ctx.request_repaint();
        }
    }));
    log::info!("panel: docked, toggle shortcut '{shortcut}' registered");

    Some(PanelHotkey { _manager: manager })
}

#[cfg(not(target_os = "macos"))]
pub fn setup(_cc: &eframe::CreationContext<'_>, _shortcut: &str) -> Option<PanelHotkey> {
    None
}
