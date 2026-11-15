use eframe::egui::{self, Color32, CornerRadius, Stroke};
use paste_core::ItemType;

pub const BG_DRAWER: Color32 = Color32::from_rgb(20, 22, 28);
pub const BG_SURFACE: Color32 = Color32::from_rgb(30, 34, 45);
pub const BG_CARD: Color32 = Color32::from_rgb(34, 38, 50);
pub const BG_CARD_HOVER: Color32 = Color32::from_rgb(42, 47, 62);
pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(56, 62, 80);
pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(236, 240, 248);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(148, 158, 178);
pub const ACCENT_BLUE: Color32 = Color32::from_rgb(74, 144, 226);
pub const ACCENT_TEAL: Color32 = Color32::from_rgb(80, 227, 194);
pub const ACCENT_PURPLE: Color32 = Color32::from_rgb(162, 89, 255);
pub const ACCENT_AMBER: Color32 = Color32::from_rgb(245, 166, 35);
pub const ACCENT_RED: Color32 = Color32::from_rgb(255, 94, 87);

pub fn apply_paste_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = BG_DRAWER;
    visuals.panel_fill = BG_DRAWER;
    visuals.extreme_bg_color = Color32::from_rgb(14, 16, 20);
    visuals.faint_bg_color = BG_SURFACE;
    visuals.widgets.noninteractive.bg_fill = BG_SURFACE;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, BORDER_SUBTLE);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, TEXT_PRIMARY);
    visuals.widgets.inactive.bg_fill = Color32::from_rgb(40, 45, 58);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
    visuals.widgets.hovered.bg_fill = Color32::from_rgb(55, 62, 80);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(8);
    visuals.widgets.active.bg_fill = ACCENT_BLUE;
    visuals.widgets.active.corner_radius = CornerRadius::same(8);
    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(74, 144, 226, 90);
    visuals.selection.stroke = Stroke::new(1.5_f32, ACCENT_BLUE);
    ctx.set_visuals(visuals);
}

pub fn color_from_hex(hex: &str) -> Color32 {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() >= 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&clean[0..2], 16),
            u8::from_str_radix(&clean[2..4], 16),
            u8::from_str_radix(&clean[4..6], 16),
        ) {
            return Color32::from_rgb(r, g, b);
        }
    }
    ACCENT_BLUE
}

pub fn item_type_accent(item_type: ItemType, custom_color_hex: Option<&str>) -> Color32 {
    if item_type == ItemType::Color {
        if let Some(hex) = custom_color_hex {
            return color_from_hex(hex);
        }
    }
    match item_type {
        ItemType::Text => ACCENT_BLUE,
        ItemType::Link => ACCENT_TEAL,
        ItemType::Image => ACCENT_PURPLE,
        ItemType::Color => ACCENT_AMBER,
        ItemType::Files => ACCENT_AMBER,
        ItemType::Unknown => TEXT_MUTED,
    }
}

pub fn contrast_text_for_bg(bg: Color32) -> Color32 {
    let lum = 0.299 * (bg.r() as f32) + 0.587 * (bg.g() as f32) + 0.114 * (bg.b() as f32);
    if lum > 155.0 {
        Color32::from_rgb(18, 20, 26)
    } else {
        Color32::WHITE
    }
}

/// Draws Paste's animated iridescent glow border when Intelligence "Picks" mode is active.
pub fn draw_intelligence_glow(ctx: &egui::Context, rect: egui::Rect) {
    let time = ctx.input(|i| i.time) as f32;
    let phase = (time * 1.8).sin() * 0.5 + 0.5;
    let color_a = Color32::from_rgb(
        (90.0 + 140.0 * phase) as u8,
        (120.0 + 90.0 * (1.0 - phase)) as u8,
        245,
    );
    let color_b = Color32::from_rgb(
        240,
        (95.0 + 120.0 * phase) as u8,
        (180.0 - 70.0 * phase) as u8,
    );

    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("intelligence_glow"),
    ));
    let outer = rect.shrink(1.5);
    painter.rect_stroke(
        outer,
        CornerRadius::same(10),
        Stroke::new(3.0_f32, color_a.gamma_multiply(0.55)),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        outer.shrink(2.0),
        CornerRadius::same(8),
        Stroke::new(1.5_f32, color_b.gamma_multiply(0.7)),
        egui::StrokeKind::Inside,
    );
    ctx.request_repaint();
}

pub fn format_relative_time(timestamp_ms: i64) -> String {
    let now = chrono::Utc::now().timestamp_millis();
    let diff_secs = ((now - timestamp_ms) / 1000).max(0);
    if diff_secs < 45 {
        "Just now".to_string()
    } else if diff_secs < 3600 {
        format!("{}m ago", diff_secs / 60)
    } else if diff_secs < 86_400 {
        format!("{}h ago", diff_secs / 3600)
    } else if diff_secs < 172_800 {
        "Yesterday".to_string()
    } else {
        format!("{}d ago", diff_secs / 86_400)
    }
}
