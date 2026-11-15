use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use eframe::egui::{
    self, Color32, ColorImage, CornerRadius, RichText, Sense, Stroke, TextureHandle, Vec2,
};
use paste_core::{ItemEntity, ItemType, Pinboard};

use crate::theme::{
    color_from_hex, contrast_text_for_bg, format_relative_time, item_type_accent, ACCENT_BLUE,
    ACCENT_RED, BG_CARD, BG_CARD_HOVER, BORDER_SUBTLE, TEXT_MUTED, TEXT_PRIMARY,
};

/// Actions emitted by interacting with a card in the carousel.
#[derive(Debug, Clone)]
pub enum CardAction {
    Select(String),
    Paste { item_id: String, plain_text: bool },
    Preview(String),
    AddToStack(ItemEntity),
    PinTo { item_id: String, pinboard_id: String },
    Unpin(String),
    Delete(String),
}

pub fn get_or_decode_thumbnail(
    ctx: &egui::Context,
    cache: &mut HashMap<String, TextureHandle>,
    item: &ItemEntity,
) -> Option<TextureHandle> {
    if let Some(handle) = cache.get(&item.identifier) {
        return Some(handle.clone());
    }
    let b64 = item.preview.thumbnail_png_b64.as_deref()?;
    let png_bytes = BASE64.decode(b64).ok()?;
    let img = image::load_from_memory(&png_bytes).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    let color_img = ColorImage::from_rgba_unmultiplied(size, img.as_raw());
    let handle = ctx.load_texture(
        format!("thumb_{}", item.identifier),
        color_img,
        egui::TextureOptions::LINEAR,
    );
    cache.insert(item.identifier.clone(), handle.clone());
    Some(handle)
}

pub fn render_card(
    ui: &mut egui::Ui,
    item: &ItemEntity,
    index: usize,
    is_selected: bool,
    card_size: Vec2,
    pinboards: &[Pinboard],
    thumb_cache: &mut HashMap<String, TextureHandle>,
    pick_reason: Option<&str>,
) -> Option<CardAction> {
    let mut action: Option<CardAction> = None;

    let accent = item_type_accent(item.item_type, item.preview.color_hex.as_deref());
    let header_bg = item
        .source_app_color
        .as_deref()
        .map(color_from_hex)
        .unwrap_or(accent);

    let (rect, resp) = ui.allocate_exact_size(card_size, Sense::click());
    let hovered = resp.hovered();

    let bg = if hovered { BG_CARD_HOVER } else { BG_CARD };
    let border = if is_selected {
        Stroke::new(2.0_f32, ACCENT_BLUE)
    } else if hovered {
        Stroke::new(1.5_f32, accent.gamma_multiply(0.8))
    } else {
        Stroke::new(1.0_f32, BORDER_SUBTLE)
    };

    let painter = ui.painter_at(rect);
    painter.rect(
        rect,
        CornerRadius::same(12),
        bg,
        border,
        egui::StrokeKind::Inside,
    );

    // Header band (top 54px)
    let header_height = 54.0;
    let header_rect = egui::Rect::from_min_size(rect.min, Vec2::new(rect.width(), header_height));
    painter.rect_filled(
        header_rect,
        CornerRadius {
            nw: 12,
            ne: 12,
            sw: 0,
            se: 0,
        },
        header_bg.gamma_multiply(0.88),
    );

    let header_fg = contrast_text_for_bg(header_bg);
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink(10.0))
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );

    // Header content
    child_ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let app_label = item
                .source_app_name
                .as_deref()
                .unwrap_or_else(|| item.item_type.display_label());
            ui.label(
                RichText::new(app_label)
                    .size(13.0)
                    .strong()
                    .color(header_fg),
            );
            let time_str = format_relative_time(item.timestamp);
            let subtitle = if let Some(reason) = pick_reason {
                format!("{time_str} · {reason}")
            } else {
                format!("{time_str} · {}", item.item_type.display_label())
            };
            ui.label(
                RichText::new(subtitle)
                    .size(10.5)
                    .color(header_fg.gamma_multiply(0.85)),
            );
        });

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if index < 9 {
                let badge = if cfg!(target_os = "macos") {
                    format!("⌘{}", index + 1)
                } else {
                    format!("^{}", index + 1)
                };
                egui::Frame::new()
                    .fill(Color32::from_black_alpha(65))
                    .corner_radius(CornerRadius::same(5))
                    .inner_margin(egui::Margin::symmetric(5, 2))
                    .show(ui, |ui| {
                        ui.label(RichText::new(badge).size(10.5).strong().color(Color32::WHITE));
                    });
            }
            if item.is_confidential {
                ui.label(RichText::new("🔒").size(12.0))
                    .on_hover_text("Confidential / Secret detected");
            }
            if item.pinboard_id.is_some() {
                ui.label(RichText::new("📌").size(11.5))
                    .on_hover_text("Pinned to Pinboard");
            }
        });
    });

    child_ui.add_space(12.0);

    // Body area
    let body_height = (rect.height() - header_height - 34.0).max(40.0);
    child_ui.allocate_ui(Vec2::new(rect.width() - 20.0, body_height), |ui| {
        match item.item_type {
            ItemType::Color => {
                let swatch = item
                    .preview
                    .color_hex
                    .as_deref()
                    .map(color_from_hex)
                    .unwrap_or(accent);
                let swatch_rect = ui.allocate_space(Vec2::new(ui.available_width(), 72.0)).1;
                ui.painter().rect_filled(swatch_rect, CornerRadius::same(8), swatch);
                ui.add_space(8.0);
                ui.label(
                    RichText::new(item.preview.color_hex.as_deref().unwrap_or(&item.title))
                        .size(16.0)
                        .strong()
                        .monospace()
                        .color(TEXT_PRIMARY),
                );
                if let Some([r, g, b, a]) = item.preview.color_rgba {
                    ui.label(
                        RichText::new(format!("rgb({r}, {g}, {b})  α:{a}"))
                            .size(11.0)
                            .monospace()
                            .color(TEXT_MUTED),
                    );
                }
            }
            ItemType::Link => {
                if let Some(ref domain) = item.preview.link_domain {
                    egui::Frame::new()
                        .fill(accent.gamma_multiply(0.18))
                        .corner_radius(CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(7, 3))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(format!("🌐 {domain}"))
                                    .size(11.5)
                                    .strong()
                                    .color(accent),
                            );
                        });
                    ui.add_space(6.0);
                }
                ui.label(
                    RichText::new(&item.title)
                        .size(13.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.add_space(4.0);
                let url_preview: String = item.preview.snippet.chars().take(120).collect();
                ui.label(
                    RichText::new(url_preview)
                        .size(11.0)
                        .color(TEXT_MUTED),
                );
            }
            ItemType::Image => {
                if let Some(tex) = get_or_decode_thumbnail(ui.ctx(), thumb_cache, item) {
                    let avail = ui.available_size();
                    let tex_size = tex.size_vec2();
                    let scale = (avail.x / tex_size.x)
                        .min((avail.y - 24.0).max(40.0) / tex_size.y)
                        .min(1.0);
                    let draw_size = tex_size * scale;
                    ui.centered_and_justified(|ui| {
                        ui.image(egui::load::SizedTexture::new(tex.id(), draw_size));
                    });
                } else {
                    ui.label(RichText::new("🖼 Image").size(14.0).color(TEXT_PRIMARY));
                }
            }
            ItemType::Files => {
                ui.label(
                    RichText::new(format!("📁 {} file(s)", item.preview.file_count))
                        .size(13.0)
                        .strong()
                        .color(accent),
                );
                ui.add_space(6.0);
                for f in item.preview.file_names.iter().take(4) {
                    ui.label(RichText::new(format!("• {f}")).size(11.5).color(TEXT_PRIMARY));
                }
            }
            ItemType::Text | ItemType::Unknown => {
                if let Some(ref lang) = item.preview.language_hint {
                    egui::Frame::new()
                        .fill(ACCENT_BLUE.gamma_multiply(0.18))
                        .corner_radius(CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(5, 1))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(lang.to_ascii_uppercase())
                                    .size(9.5)
                                    .strong()
                                    .color(ACCENT_BLUE),
                            );
                        });
                    ui.add_space(4.0);
                }

                let preview_text = if item.is_confidential {
                    "•••••••••••• (Secret / Credential Hidden — press Space to view)".to_string()
                } else {
                    item.preview.snippet.chars().take(220).collect()
                };

                let mut text_rt = RichText::new(preview_text).size(12.0).color(TEXT_PRIMARY);
                if item.preview.language_hint.is_some() {
                    text_rt = text_rt.monospace();
                }
                ui.add(egui::Label::new(text_rt).truncate());
            }
        }
    });

    // Footer bar inside card
    let footer_pos = egui::Pos2::new(rect.min.x + 10.0, rect.max.y - 22.0);
    let footer_rect = egui::Rect::from_min_size(footer_pos, Vec2::new(rect.width() - 20.0, 18.0));
    let mut footer_ui = ui.new_child(egui::UiBuilder::new().max_rect(footer_rect));
    footer_ui.horizontal(|ui| {
        let footer_text = match item.item_type {
            ItemType::Image => format!(
                "{}×{} px",
                item.preview.image_width.unwrap_or(0),
                item.preview.image_height.unwrap_or(0)
            ),
            ItemType::Color => "Color Swatch".to_string(),
            ItemType::Files => format!("{} item(s)", item.preview.file_count),
            _ => format!(
                "{} chars · {} lines",
                item.preview.char_count, item.preview.line_count
            ),
        };
        ui.label(RichText::new(footer_text).size(10.0).color(TEXT_MUTED));
    });

    if resp.double_clicked() {
        action = Some(CardAction::Paste {
            item_id: item.identifier.clone(),
            plain_text: false,
        });
    } else if resp.clicked() {
        action = Some(CardAction::Select(item.identifier.clone()));
    }

    resp.context_menu(|ui| {
        if ui.button("📋 Paste to Active App (Enter)").clicked() {
            action = Some(CardAction::Paste {
                item_id: item.identifier.clone(),
                plain_text: false,
            });
            ui.close_menu();
        }
        if ui.button("📄 Paste as Plain Text").clicked() {
            action = Some(CardAction::Paste {
                item_id: item.identifier.clone(),
                plain_text: true,
            });
            ui.close_menu();
        }
        if ui.button("🔍 QuickLook & Edit (Space)").clicked() {
            action = Some(CardAction::Preview(item.identifier.clone()));
            ui.close_menu();
        }
        if ui.button("📚 Add to Paste Stack").clicked() {
            action = Some(CardAction::AddToStack(item.clone()));
            ui.close_menu();
        }
        ui.separator();
        ui.menu_button("📌 Pin to Pinboard", |ui| {
            for pb in pinboards {
                if ui.button(format!("• {}", pb.name)).clicked() {
                    action = Some(CardAction::PinTo {
                        item_id: item.identifier.clone(),
                        pinboard_id: pb.identifier.clone(),
                    });
                    ui.close_menu();
                }
            }
        });
        if item.pinboard_id.is_some() && ui.button("Unpin from Pinboard").clicked() {
            action = Some(CardAction::Unpin(item.identifier.clone()));
            ui.close_menu();
        }
        ui.separator();
        if ui
            .button(RichText::new("🗑 Delete Item").color(ACCENT_RED))
            .clicked()
        {
            action = Some(CardAction::Delete(item.identifier.clone()));
            ui.close_menu();
        }
    });

    action
}
