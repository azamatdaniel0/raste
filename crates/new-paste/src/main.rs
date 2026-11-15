mod app;
mod panel;
mod theme;
mod ui;

use app::NewPasteApp;
use eframe::egui;
use paste_core::Database;

fn load_app_icon() -> Option<egui::IconData> {
    // Embedded so the icon is found no matter where the binary is launched from.
    let icon_bytes: &[u8] = include_bytes!("../../../assets/icons/paste-icon.png");
    let img = image::load_from_memory(icon_bytes).ok()?.to_rgba8();
    let (width, height) = img.dimensions();
    Some(egui::IconData {
        rgba: img.into_raw(),
        width,
        height,
    })
}

fn main() -> eframe::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let db_path = Database::default_db_path();
    let db = Database::open(&db_path).expect("failed to open new-paste SQLite database");

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("New-Paste — Cross-Platform Clipboard Manager")
        .with_inner_size([1160.0, 400.0])
        .with_min_inner_size([720.0, 310.0]);

    // On macOS the window is a borderless panel docked to the bottom of the screen.
    if cfg!(target_os = "macos") {
        viewport = viewport
            .with_inner_size([1160.0, panel::PANEL_HEIGHT as f32])
            .with_decorations(false)
            .with_resizable(false);
    }

    if let Some(icon) = load_app_icon() {
        viewport = viewport.with_icon(std::sync::Arc::new(icon));
    }

    let options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    eframe::run_native(
        "New-Paste",
        options,
        Box::new(move |cc| Ok(Box::new(NewPasteApp::new(cc, db)))),
    )
}
