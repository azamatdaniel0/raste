use std::collections::HashMap;

use eframe::egui::{self, RichText, TextureHandle, Vec2};
use paste_core::{
    processor::remove_solid_background, Database, HistoryCapacity, ItemDataEntity, ItemEntity,
    ItemType, NewClipboardPayload, PasteStack, Preferences,
};

use crate::theme::{format_relative_time, ACCENT_BLUE, ACCENT_RED, BG_SURFACE, TEXT_MUTED, TEXT_PRIMARY};

pub struct PreviewState {
    pub item: ItemEntity,
    pub data: Option<ItemDataEntity>,
    pub edit_title: String,
    pub edit_text: String,
    pub full_image_tex: Option<TextureHandle>,
    pub status_msg: Option<String>,
}

impl PreviewState {
    pub fn open(ctx: &egui::Context, db: &Database, item: ItemEntity) -> Self {
        let data = db.get_item_data(&item.identifier).ok().flatten();
        let edit_title = item.title.clone();
        let edit_text = data
            .as_ref()
            .and_then(|d| d.plain_text.clone())
            .unwrap_or_default();

        let full_image_tex = data.as_ref().and_then(|d| {
            let png = d.image_png.as_ref()?;
            let img = image::load_from_memory(png).ok()?.to_rgba8();
            let size = [img.width() as usize, img.height() as usize];
            let color_img = egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw());
            Some(ctx.load_texture(
                format!("full_{}", item.identifier),
                color_img,
                egui::TextureOptions::LINEAR,
            ))
        });

        Self {
            item,
            data,
            edit_title,
            edit_text,
            full_image_tex,
            status_msg: None,
        }
    }
}

pub enum PreviewModalOutcome {
    KeepOpen,
    Close,
    Updated,
    Paste { item_id: String, plain_text: bool },
}

pub fn render_preview_modal(
    ctx: &egui::Context,
    db: &Database,
    state: &mut PreviewState,
    thumb_cache: &mut HashMap<String, TextureHandle>,
) -> PreviewModalOutcome {
    let mut outcome = PreviewModalOutcome::KeepOpen;
    let mut open = true;

    egui::Window::new(format!("QuickLook — {}", state.item.title))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(640.0, 480.0))
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Title:").strong().color(TEXT_MUTED));
                ui.add(
                    egui::TextEdit::singleline(&mut state.edit_title)
                        .desired_width(360.0),
                );
                if ui.button("📋 Paste").clicked() {
                    outcome = PreviewModalOutcome::Paste {
                        item_id: state.item.identifier.clone(),
                        plain_text: false,
                    };
                }
            });

            ui.separator();

            if state.item.item_type == ItemType::Image {
                if let Some(ref tex) = state.full_image_tex {
                    let avail = Vec2::new(ui.available_width(), 270.0);
                    let tsize = tex.size_vec2();
                    let scale = (avail.x / tsize.x).min(avail.y / tsize.y).min(1.0);
                    ui.centered_and_justified(|ui| {
                        ui.image(egui::load::SizedTexture::new(tex.id(), tsize * scale));
                    });
                }
                ui.horizontal(|ui| {
                    if ui.button("✨ Remove Solid Background").clicked() {
                        if let Some(ref png) = state.data.as_ref().and_then(|d| d.image_png.as_ref()) {
                            match remove_solid_background(png, 26)
                                .and_then(|cleaned| db.update_item_image(&state.item.identifier, cleaned))
                            {
                                Ok(updated) => {
                                    thumb_cache.remove(&updated.identifier);
                                    *state = PreviewState::open(ctx, db, updated.clone());
                                    state.status_msg = Some("Background removed!".to_string());
                                    outcome = PreviewModalOutcome::Updated;
                                }
                                Err(e) => {
                                    state.status_msg = Some(format!("Error: {e}"));
                                }
                            }
                        }
                    }
                    if ui.button("💾 Save Title").clicked() {
                        if let Ok(updated) = db.update_item(&state.item.identifier, Some(&state.edit_title), None) {
                            state.item = updated;
                            state.status_msg = Some("Saved!".to_string());
                            outcome = PreviewModalOutcome::Updated;
                        }
                    }
                });
            } else {
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut state.edit_text)
                                .desired_width(f32::INFINITY)
                                .desired_rows(12)
                                .code_editor(),
                        );
                    });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("💾 Save Changes").clicked() {
                        match db.update_item(
                            &state.item.identifier,
                            Some(&state.edit_title),
                            Some(&state.edit_text),
                        ) {
                            Ok(updated) => {
                                state.item = updated;
                                state.status_msg = Some("Saved changes to database.".to_string());
                                outcome = PreviewModalOutcome::Updated;
                            }
                            Err(e) => {
                                state.status_msg = Some(format!("Save failed: {e}"));
                            }
                        }
                    }
                    if let Some(ref msg) = state.status_msg {
                        ui.label(RichText::new(msg).color(ACCENT_BLUE));
                    }
                });
            }

            ui.separator();
            egui::Frame::new()
                .fill(BG_SURFACE)
                .inner_margin(egui::Margin::same(8))
                .corner_radius(egui::CornerRadius::same(6))
                .show(ui, |ui| {
                    ui.label(
                        RichText::new(format!(
                            "ID: {}  |  SHA-256: {}...  |  Source: {}  |  Copied: {}",
                            state.item.identifier,
                            &state.item.checksum[..16.min(state.item.checksum.len())],
                            state.item.source_app_name.as_deref().unwrap_or("Unknown"),
                            format_relative_time(state.item.timestamp),
                        ))
                        .size(11.0)
                        .color(TEXT_MUTED),
                    );
                });
        });

    if !open {
        return PreviewModalOutcome::Close;
    }
    outcome
}

pub enum StackHudAction {
    None,
    PasteNext,
    Close,
}

pub fn render_paste_stack_hud(
    ctx: &egui::Context,
    stack: &mut PasteStack,
) -> StackHudAction {
    let mut hud_action = StackHudAction::None;
    let mut open = stack.is_active;

    egui::Window::new("📚 Paste Stack")
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(290.0, 340.0))
        .anchor(egui::Align2::RIGHT_TOP, Vec2::new(-16.0, 56.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .button(format!("Order: {}", stack.direction.short_label()))
                    .on_hover_text(stack.direction.label())
                    .clicked()
                {
                    stack.toggle_direction();
                }
                if ui
                    .add_enabled(!stack.is_empty(), egui::Button::new("▶ Paste Next"))
                    .clicked()
                {
                    hud_action = StackHudAction::PasteNext;
                }
                if ui
                    .add_enabled(!stack.is_empty(), egui::Button::new("Clear"))
                    .clicked()
                {
                    stack.clear();
                }
            });

            ui.label(
                RichText::new(stack.direction.label())
                    .size(11.0)
                    .color(TEXT_MUTED),
            );
            ui.separator();

            if stack.is_empty() {
                ui.label(
                    RichText::new("Stack is empty.\nRight-click any card -> 'Add to Paste Stack'.")
                        .color(TEXT_MUTED),
                );
            } else {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let ordered: Vec<ItemEntity> =
                        stack.ordered_items().into_iter().cloned().collect();
                    for (idx, item) in ordered.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(format!("{}.", idx + 1))
                                    .strong()
                                    .color(ACCENT_BLUE),
                            );
                            ui.label(
                                RichText::new(&item.title)
                                    .color(TEXT_PRIMARY),
                            );
                        });
                    }
                });
            }
        });

    if !open {
        stack.is_active = false;
        return StackHudAction::Close;
    }
    hud_action
}

pub fn render_settings_modal(
    ctx: &egui::Context,
    db: &Database,
    open: &mut bool,
    prefs: &mut Preferences,
    new_blacklist_entry: &mut String,
    confirm_clear_history: &mut bool,
) -> bool {
    let mut changed_data = false;
    let mut is_open = *open;

    egui::Window::new("⚙ New-Paste Settings")
        .open(&mut is_open)
        .collapsible(false)
        .resizable(true)
        .default_size(Vec2::new(500.0, 440.0))
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            let mut dirty = false;

            ui.heading("General & Behavior");
            dirty |= ui
                .checkbox(&mut prefs.enable_direct_paste, "Enable Direct Paste (simulate Ctrl+V / Cmd+V)")
                .changed();
            dirty |= ui
                .checkbox(&mut prefs.paste_as_plain_text, "Always Paste as Plain Text")
                .changed();
            dirty |= ui
                .checkbox(&mut prefs.enable_sound_effects, "Play Copy & Paste Sound Effects")
                .changed();
            dirty |= ui
                .checkbox(&mut prefs.detect_secrets, "Detect Passwords & API Secrets (Lock Badge)")
                .changed();
            dirty |= ui
                .checkbox(&mut prefs.is_picks_enabled, "Enable Picks Context Recommendations")
                .changed();

            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label("History Retention Capacity:");
                egui::ComboBox::from_id_salt("history_capacity_combo")
                    .selected_text(prefs.history_capacity.label())
                    .show_ui(ui, |ui| {
                        for cap in HistoryCapacity::all() {
                            if ui
                                .selectable_value(&mut prefs.history_capacity, *cap, cap.label())
                                .changed()
                            {
                                dirty = true;
                            }
                        }
                    });
            });

            ui.separator();
            ui.heading("Ignored Applications (Blacklist)");
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(new_blacklist_entry)
                        .hint_text("e.g. KeePassXC.exe or com.bitwarden.desktop")
                        .desired_width(280.0),
                );
                if ui.button("Add").clicked() && !new_blacklist_entry.trim().is_empty() {
                    prefs.blacklisted_apps.push(new_blacklist_entry.trim().to_string());
                    new_blacklist_entry.clear();
                    dirty = true;
                }
            });

            let mut remove_idx = None;
            egui::ScrollArea::vertical().max_height(90.0).show(ui, |ui| {
                for (idx, app) in prefs.blacklisted_apps.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(app);
                        if ui.small_button("✕").clicked() {
                            remove_idx = Some(idx);
                        }
                    });
                }
            });
            if let Some(idx) = remove_idx {
                prefs.blacklisted_apps.remove(idx);
                dirty = true;
            }

            ui.separator();
            ui.heading("Model Context Protocol (MCP)");
            dirty |= ui
                .checkbox(&mut prefs.is_mcp_enabled, "Expose database via paste-mcp-stdio")
                .changed();
            ui.label(
                RichText::new(format!("Shared DB: {}", Database::default_db_path().display()))
                    .size(11.0)
                    .color(TEXT_MUTED),
            );

            ui.separator();
            ui.horizontal(|ui| {
                if !*confirm_clear_history {
                    if ui
                        .button(RichText::new("🗑 Erase Unpinned History").color(ACCENT_RED))
                        .clicked()
                    {
                        *confirm_clear_history = true;
                    }
                } else {
                    ui.label(RichText::new("Confirm erase?").color(ACCENT_RED));
                    if ui.button("Yes, Erase").clicked() {
                        let _ = db.clear_history(true);
                        *confirm_clear_history = false;
                        changed_data = true;
                    }
                    if ui.button("Cancel").clicked() {
                        *confirm_clear_history = false;
                    }
                }
            });

            if dirty {
                let _ = db.save_preferences(prefs);
            }
        });

    *open = is_open;
    changed_data
}

pub fn render_new_clip_modal(
    ctx: &egui::Context,
    db: &Database,
    open: &mut bool,
    draft_title: &mut String,
    draft_text: &mut String,
    active_pinboard_id: Option<&str>,
) -> bool {
    let mut created = false;
    let mut is_open = *open;
    let mut close_after = false;

    egui::Window::new("+ New Clip")
        .open(&mut is_open)
        .collapsible(false)
        .default_size(Vec2::new(420.0, 260.0))
        .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Title (optional):");
                ui.text_edit_singleline(draft_title);
            });
            ui.add(
                egui::TextEdit::multiline(draft_text)
                    .desired_width(f32::INFINITY)
                    .desired_rows(7)
                    .hint_text("Paste or type text, URL, #HEX color, or code..."),
            );
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!draft_text.trim().is_empty(), egui::Button::new("Save Clip"))
                    .clicked()
                {
                    let title = if draft_title.trim().is_empty() {
                        None
                    } else {
                        Some(draft_title.trim().to_string())
                    };
                    if db
                        .ingest_clipboard_payload(NewClipboardPayload {
                            plain_text: Some(draft_text.clone()),
                            custom_title: title,
                            pinboard_id: active_pinboard_id.map(ToString::to_string),
                            source_app_id: Some("new-paste".to_string()),
                            source_app_name: Some("New-Paste".to_string()),
                            source_app_color: Some("#4A90E2".to_string()),
                            ..Default::default()
                        })
                        .is_ok()
                    {
                        draft_title.clear();
                        draft_text.clear();
                        created = true;
                        close_after = true;
                    }
                }
            });
        });

    *open = is_open && !close_after;
    created
}
