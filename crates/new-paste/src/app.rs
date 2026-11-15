use std::collections::HashMap;
use std::time::{Duration, Instant};

use chrono::Utc;
use eframe::egui::{self, Color32, CornerRadius, Key, RichText, TextureHandle, Vec2};
use paste_clipboard::{
    get_active_window_info, simulate_paste_keystroke, stage_item_to_clipboard, ActiveWindowInfo,
    ClipboardMonitor, SoundPlayer,
};
use paste_core::{
    ApplicationEntity, Database, ItemEntity, ItemType, PasteStack, PicksEngine, Pinboard,
    Preferences, SearchFilter,
};

use crate::theme::{
    apply_paste_theme, color_from_hex, draw_intelligence_glow, ACCENT_AMBER, ACCENT_BLUE,
    ACCENT_PURPLE, ACCENT_RED, ACCENT_TEAL, BG_DRAWER, BG_SURFACE, TEXT_MUTED, TEXT_PRIMARY,
};
use crate::ui::cards::{render_card, CardAction};
use crate::ui::modals::{
    render_new_clip_modal, render_paste_stack_hud, render_preview_modal, render_settings_modal,
    PreviewModalOutcome, PreviewState, StackHudAction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DateRangeFilter {
    All,
    Today,
    Last7Days,
}

impl DateRangeFilter {
    fn label(&self) -> &'static str {
        match self {
            Self::All => "Any Time",
            Self::Today => "Today",
            Self::Last7Days => "Last 7d",
        }
    }

    fn from_ms(&self) -> Option<i64> {
        let now = Utc::now().timestamp_millis();
        match self {
            Self::All => None,
            Self::Today => Some(now - 86_400_000),
            Self::Last7Days => Some(now - 7 * 86_400_000),
        }
    }
}

pub struct NewPasteApp {
    db: Database,
    monitor: ClipboardMonitor,
    sound: SoundPlayer,
    prefs: Preferences,

    pinboards: Vec<Pinboard>,
    items: Vec<ItemEntity>,
    apps: Vec<ApplicationEntity>,
    active_pinboard_id: Option<String>,
    selected_item_id: Option<String>,

    // Search & filter pills
    search_query: String,
    focus_search_requested: bool,
    type_filter: Option<ItemType>,
    date_filter: DateRangeFilter,
    app_filter: Option<String>,

    // Picks (Intelligence Context)
    picks_mode: bool,
    last_external_window: Option<ActiveWindowInfo>,

    // Drawer height & textures
    card_height: f32,
    thumb_cache: HashMap<String, TextureHandle>,

    // Modals & HUDs
    preview_state: Option<PreviewState>,
    paste_stack: PasteStack,
    settings_open: bool,
    new_blacklist_entry: String,
    confirm_clear_history: bool,
    new_clip_open: bool,
    draft_clip_title: String,
    draft_clip_text: String,

    // Pinboard create / rename dialog
    new_pinboard_open: bool,
    new_pinboard_name: String,
    new_pinboard_color: String,
    renaming_pinboard_id: Option<String>,

    // Feedback toast
    toast: Option<(String, Instant)>,
    last_refresh_check: Instant,

    // Keeps the global show/hide shortcut registered (macOS docked panel)
    _panel_hotkey: Option<crate::panel::PanelHotkey>,
}

impl NewPasteApp {
    pub fn new(cc: &eframe::CreationContext<'_>, db: Database) -> Self {
        apply_paste_theme(&cc.egui_ctx);

        let _ = db.seed_demo_items_if_empty();
        let prefs = db.load_preferences().unwrap_or_default();
        let sound = SoundPlayer::discover();
        let monitor = ClipboardMonitor::new(db.clone(), sound.clone());

        let ctx_clone = cc.egui_ctx.clone();
        monitor.set_on_change(move || {
            ctx_clone.request_repaint();
        });
        monitor.start(Duration::from_millis(350));

        let panel_hotkey = crate::panel::setup(cc, &prefs.activation_shortcut);

        let mut app = Self {
            db,
            monitor,
            sound,
            prefs,
            pinboards: Vec::new(),
            items: Vec::new(),
            apps: Vec::new(),
            active_pinboard_id: None,
            selected_item_id: None,
            search_query: String::new(),
            focus_search_requested: false,
            type_filter: None,
            date_filter: DateRangeFilter::All,
            app_filter: None,
            picks_mode: false,
            last_external_window: get_active_window_info(),
            card_height: 255.0,
            thumb_cache: HashMap::new(),
            preview_state: None,
            paste_stack: PasteStack::default(),
            settings_open: false,
            new_blacklist_entry: String::new(),
            confirm_clear_history: false,
            new_clip_open: false,
            draft_clip_title: String::new(),
            draft_clip_text: String::new(),
            new_pinboard_open: false,
            new_pinboard_name: String::new(),
            new_pinboard_color: "#4A90E2".to_string(),
            renaming_pinboard_id: None,
            toast: None,
            last_refresh_check: Instant::now(),
            _panel_hotkey: panel_hotkey,
        };
        app.reload_data();
        app
    }

    fn show_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), Instant::now()));
    }

    fn reload_data(&mut self) {
        self.prefs = self.db.load_preferences().unwrap_or_default();
        self.pinboards = self.db.list_pinboards().unwrap_or_default();
        self.apps = self.db.list_distinct_apps().unwrap_or_default();

        let filter = SearchFilter {
            query: if self.search_query.trim().is_empty() {
                None
            } else {
                Some(self.search_query.clone())
            },
            item_type: self.type_filter,
            source_app: self.app_filter.clone(),
            pinboard_id: self.active_pinboard_id.clone(),
            date_from_ms: self.date_filter.from_ms(),
            limit: 150,
            ..Default::default()
        };
        self.items = self.db.search_items(&filter).unwrap_or_default();

        if self.selected_item_id.is_none()
            || !self
                .items
                .iter()
                .any(|it| Some(&it.identifier) == self.selected_item_id.as_ref())
        {
            self.selected_item_id = self.items.first().map(|it| it.identifier.clone());
        }
    }

    fn trigger_paste(&mut self, item_id: &str, force_plain: bool) {
        let Ok(Some(item)) = self.db.get_item(item_id) else {
            return;
        };
        let Ok(Some(data)) = self.db.get_item_data(item_id) else {
            return;
        };

        let plain = force_plain || self.prefs.paste_as_plain_text;
        self.monitor.mark_internal_write(&item.checksum);

        if let Err(err) = stage_item_to_clipboard(&item, &data, plain) {
            self.show_toast(format!("Clipboard error: {err}"));
            return;
        }

        if self.prefs.enable_sound_effects {
            self.sound.play_paste();
        }

        if self.prefs.enable_direct_paste {
            if let Some(ref ext) = self.last_external_window {
                if !ext.app_id.eq_ignore_ascii_case("new-paste.exe") {
                    let _ = simulate_paste_keystroke(Some(ext.hwnd));
                    self.show_toast(format!("Pasted '{}' -> {}", item.title, ext.app_name));
                    return;
                }
            }
        }

        if cfg!(target_os = "macos") {
            // Docked panel: get out of the way, hand focus back and paste right away.
            if self.prefs.enable_direct_paste {
                if crate::panel::hide_and_paste() {
                    return;
                }
                self.show_toast(
                    "Copied. To paste automatically, allow New-Paste in System Settings → Privacy & Security → Accessibility",
                );
                return;
            }
            crate::panel::hide(true);
        }

        self.show_toast(format!("Copied '{}' to clipboard", item.title));
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        // Don't hijack typing shortcuts while a modal is open
        if self.preview_state.is_some() || self.new_clip_open || self.new_pinboard_open {
            if ctx.input(|i| i.key_pressed(Key::Escape)) {
                self.preview_state = None;
                self.new_clip_open = false;
                self.new_pinboard_open = false;
            }
            return;
        }

        let search_has_focus = ctx.memory(|m| m.focused().is_some());

        // Esc dismisses the docked panel; Cmd+Q quits (there is no Dock icon or menu).
        if !self.settings_open && ctx.input(|i| i.key_pressed(Key::Escape)) {
            crate::panel::hide(true);
            return;
        }
        if ctx.input(|i| i.modifiers.command && i.key_pressed(Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        ctx.input(|i| {
            // Ctrl/Cmd + F -> focus search
            if i.modifiers.command && i.key_pressed(Key::F) {
                self.focus_search_requested = true;
            }

            // Ctrl/Cmd + [ or ] -> cycle pinboards
            if i.modifiers.command && i.key_pressed(Key::OpenBracket) {
                self.cycle_pinboard(-1);
            }
            if i.modifiers.command && i.key_pressed(Key::CloseBracket) {
                self.cycle_pinboard(1);
            }

            // Ctrl/Cmd + 1..9 -> quick paste
            if i.modifiers.command {
                let num_keys = [
                    Key::Num1,
                    Key::Num2,
                    Key::Num3,
                    Key::Num4,
                    Key::Num5,
                    Key::Num6,
                    Key::Num7,
                    Key::Num8,
                    Key::Num9,
                ];
                for (idx, k) in num_keys.iter().enumerate() {
                    if i.key_pressed(*k) {
                        if let Some(item) = self.items.get(idx) {
                            let id = item.identifier.clone();
                            self.selected_item_id = Some(id.clone());
                            // Trigger paste after closure
                            self.toast = Some((format!("__PASTE__:{id}"), Instant::now()));
                        }
                    }
                }
            }

            if !search_has_focus {
                if i.key_pressed(Key::ArrowRight) {
                    self.move_selection(1);
                } else if i.key_pressed(Key::ArrowLeft) {
                    self.move_selection(-1);
                } else if i.key_pressed(Key::Enter) {
                    if let Some(id) = self.selected_item_id.clone() {
                        self.toast = Some((format!("__PASTE__:{id}"), Instant::now()));
                    }
                } else if i.key_pressed(Key::Space) {
                    if let Some(id) = self.selected_item_id.clone() {
                        self.toast = Some((format!("__PREVIEW__:{id}"), Instant::now()));
                    }
                } else if i.key_pressed(Key::Delete) {
                    if let Some(id) = self.selected_item_id.clone() {
                        self.toast = Some((format!("__DELETE__:{id}"), Instant::now()));
                    }
                }
            }
        });

        // Execute deferred keyboard actions outside `ctx.input` borrow
        if let Some((msg, _)) = self.toast.clone() {
            if let Some(id) = msg.strip_prefix("__PASTE__:") {
                self.toast = None;
                self.trigger_paste(id, false);
            } else if let Some(id) = msg.strip_prefix("__PREVIEW__:") {
                self.toast = None;
                if let Some(item) = self.items.iter().find(|it| it.identifier == id).cloned() {
                    self.preview_state = Some(PreviewState::open(ctx, &self.db, item));
                }
            } else if let Some(id) = msg.strip_prefix("__DELETE__:") {
                self.toast = None;
                if self.db.delete_item(id).unwrap_or(false) {
                    self.reload_data();
                    self.show_toast("Item deleted");
                }
            }
        }
    }

    fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            return;
        }
        let current_idx = self
            .selected_item_id
            .as_ref()
            .and_then(|id| self.items.iter().position(|it| &it.identifier == id))
            .unwrap_or(0);
        let next_idx = (current_idx as isize + delta)
            .clamp(0, (self.items.len() - 1) as isize) as usize;
        self.selected_item_id = Some(self.items[next_idx].identifier.clone());
    }

    fn cycle_pinboard(&mut self, delta: isize) {
        // Tabs: [None (History), pb0, pb1, ...]
        let current_pos: isize = match &self.active_pinboard_id {
            None => 0,
            Some(id) => self
                .pinboards
                .iter()
                .position(|p| &p.identifier == id)
                .map(|i| (i + 1) as isize)
                .unwrap_or(0),
        };
        let total = (self.pinboards.len() + 1) as isize;
        let next_pos = (current_pos + delta).rem_euclid(total);
        self.active_pinboard_id = if next_pos == 0 {
            None
        } else {
            self.pinboards
                .get((next_pos - 1) as usize)
                .map(|p| p.identifier.clone())
        };
        self.reload_data();
    }
}

impl eframe::App for NewPasteApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Periodically refresh to reflect background clipboard captures or MCP changes
        if self.last_refresh_check.elapsed() >= Duration::from_millis(650) {
            self.last_refresh_check = Instant::now();
            if let Some(win) = get_active_window_info() {
                if !win.app_id.eq_ignore_ascii_case("new-paste.exe") {
                    self.last_external_window = Some(win);
                }
            }
            self.reload_data();
        }

        // Hide the docked panel when the user clicks into another app.
        crate::panel::track_focus(ctx.input(|i| i.focused));

        self.handle_keyboard_shortcuts(ctx);

        let mut needs_reload = false;
        let mut pending_card_action: Option<CardAction> = None;

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(BG_DRAWER)
                    .inner_margin(egui::Margin::symmetric(14, 8)),
            )
            .show(ctx, |ui| {
                // 1. Top Resize Handle (mirrors Paste.MainWindow.ResizeHandle)
                ui.vertical_centered(|ui| {
                    let (handle_rect, handle_resp) = ui.allocate_exact_size(
                        Vec2::new(72.0, 8.0),
                        egui::Sense::drag(),
                    );
                    ui.painter().rect_filled(
                        handle_rect.shrink2(Vec2::new(0.0, 2.0)),
                        CornerRadius::same(3),
                        Color32::from_rgb(75, 82, 102),
                    );
                    if handle_resp.dragged() {
                        self.card_height = (self.card_height - handle_resp.drag_delta().y)
                            .clamp(175.0, 420.0);
                    }
                    handle_resp.on_hover_text("Drag to resize card height");
                });

                ui.add_space(4.0);

                // 2. Pinboard Tab Bar + Top Actions
                ui.horizontal(|ui| {
                    egui::ScrollArea::horizontal()
                        .id_salt("pinboard_tab_scroll")
                        .max_width((ui.available_width() - 340.0).max(240.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let is_history = self.active_pinboard_id.is_none();
                                let hist_btn = egui::Button::new(
                                    RichText::new("🕒 Clipboard History").strong(),
                                )
                                .fill(if is_history {
                                    ACCENT_BLUE.gamma_multiply(0.35)
                                } else {
                                    BG_SURFACE
                                });
                                if ui.add(hist_btn).clicked() {
                                    self.active_pinboard_id = None;
                                    needs_reload = true;
                                }

                                for pb in self.pinboards.clone() {
                                    let is_active =
                                        self.active_pinboard_id.as_deref() == Some(&pb.identifier);
                                    let dot_color = color_from_hex(&pb.color_hex);
                                    let label = format!("● {} ({})", pb.name, pb.item_count);
                                    let btn = egui::Button::new(
                                        RichText::new(label).color(if is_active {
                                            Color32::WHITE
                                        } else {
                                            TEXT_PRIMARY
                                        }),
                                    )
                                    .fill(if is_active {
                                        dot_color.gamma_multiply(0.35)
                                    } else {
                                        BG_SURFACE
                                    });
                                    let resp = ui.add(btn);
                                    if resp.clicked() {
                                        self.active_pinboard_id = Some(pb.identifier.clone());
                                        needs_reload = true;
                                    }
                                    resp.context_menu(|ui| {
                                        if ui.button("✏ Rename Pinboard").clicked() {
                                            self.renaming_pinboard_id = Some(pb.identifier.clone());
                                            self.new_pinboard_name = pb.name.clone();
                                            self.new_pinboard_color = pb.color_hex.clone();
                                            self.new_pinboard_open = true;
                                            ui.close_menu();
                                        }
                                        if ui
                                            .button(RichText::new("🗑 Delete Pinboard").color(ACCENT_RED))
                                            .clicked()
                                        {
                                            let _ = self.db.delete_pinboard(&pb.identifier);
                                            if self.active_pinboard_id.as_deref() == Some(&pb.identifier) {
                                                self.active_pinboard_id = None;
                                            }
                                            needs_reload = true;
                                            ui.close_menu();
                                        }
                                    });
                                }

                                if ui
                                    .button("+")
                                    .on_hover_text("Create New Pinboard")
                                    .clicked()
                                {
                                    self.renaming_pinboard_id = None;
                                    self.new_pinboard_name.clear();
                                    self.new_pinboard_color = "#4A90E2".to_string();
                                    self.new_pinboard_open = true;
                                }
                            });
                        });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("⚙").on_hover_text("Settings & MCP").clicked() {
                            self.settings_open = !self.settings_open;
                        }

                        // Plain text mode toggle
                        let plain_btn = egui::Button::new(
                            RichText::new("Aa").color(if self.prefs.paste_as_plain_text {
                                ACCENT_TEAL
                            } else {
                                TEXT_MUTED
                            }),
                        );
                        if ui
                            .add(plain_btn)
                            .on_hover_text("Toggle Always Paste as Plain Text")
                            .clicked()
                        {
                            self.prefs.paste_as_plain_text = !self.prefs.paste_as_plain_text;
                            let _ = self.db.save_preferences(&self.prefs);
                        }

                        // Pause recording menu
                        let pause_label = if self.prefs.is_paused {
                            RichText::new("⏸ Paused").color(ACCENT_AMBER)
                        } else {
                            RichText::new("● Live").color(ACCENT_TEAL)
                        };
                        ui.menu_button(pause_label, |ui| {
                            if ui.button("▶ Resume Recording").clicked() {
                                self.prefs.is_paused = false;
                                self.prefs.pause_until_ms = None;
                                let _ = self.db.save_preferences(&self.prefs);
                                ui.close_menu();
                            }
                            if ui.button("⏸ Pause 15 Minutes").clicked() {
                                self.prefs.is_paused = true;
                                self.prefs.pause_until_ms =
                                    Some(Utc::now().timestamp_millis() + 15 * 60_000);
                                let _ = self.db.save_preferences(&self.prefs);
                                ui.close_menu();
                            }
                            if ui.button("⏸ Pause 1 Hour").clicked() {
                                self.prefs.is_paused = true;
                                self.prefs.pause_until_ms =
                                    Some(Utc::now().timestamp_millis() + 3_600_000);
                                let _ = self.db.save_preferences(&self.prefs);
                                ui.close_menu();
                            }
                            if ui.button("⏸ Pause Indefinitely").clicked() {
                                self.prefs.is_paused = true;
                                self.prefs.pause_until_ms = None;
                                let _ = self.db.save_preferences(&self.prefs);
                                ui.close_menu();
                            }
                        });

                        // Paste Stack HUD toggle
                        let stack_label = format!("📚 Stack ({})", self.paste_stack.len());
                        if ui
                            .selectable_label(self.paste_stack.is_active, stack_label)
                            .on_hover_text("Toggle Paste Stack (FIFO / LIFO)")
                            .clicked()
                        {
                            self.paste_stack.is_active = !self.paste_stack.is_active;
                        }

                        if ui.button("+ Clip").clicked() {
                            self.new_clip_open = true;
                        }
                    });
                });

                ui.add_space(6.0);

                // 3. Search Bar + Interactive Filter Pills + Picks Toggle
                ui.horizontal(|ui| {
                    let search_edit = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("🔍 Search clips, apps, URLs... (Ctrl+F)")
                            .desired_width(240.0),
                    );
                    if self.focus_search_requested {
                        search_edit.request_focus();
                        self.focus_search_requested = false;
                    }
                    if search_edit.changed() {
                        needs_reload = true;
                    }
                    if !self.search_query.is_empty() && ui.small_button("✕").clicked() {
                        self.search_query.clear();
                        needs_reload = true;
                    }

                    ui.separator();

                    // Type Filter Pills
                    let type_options: &[(Option<ItemType>, &str)] = &[
                        (None, "All"),
                        (Some(ItemType::Text), "Text"),
                        (Some(ItemType::Link), "Links"),
                        (Some(ItemType::Color), "Colors"),
                        (Some(ItemType::Image), "Images"),
                        (Some(ItemType::Files), "Files"),
                    ];
                    for (t_opt, label) in type_options {
                        let selected = self.type_filter == *t_opt;
                        if ui.selectable_label(selected, *label).clicked() {
                            self.type_filter = *t_opt;
                            needs_reload = true;
                        }
                    }

                    ui.separator();

                    // Date pill cycle
                    if ui
                        .button(format!("📅 {}", self.date_filter.label()))
                        .clicked()
                    {
                        self.date_filter = match self.date_filter {
                            DateRangeFilter::All => DateRangeFilter::Today,
                            DateRangeFilter::Today => DateRangeFilter::Last7Days,
                            DateRangeFilter::Last7Days => DateRangeFilter::All,
                        };
                        needs_reload = true;
                    }

                    // App filter dropdown
                    let app_label = self
                        .app_filter
                        .as_deref()
                        .unwrap_or("All Apps")
                        .to_string();
                    egui::ComboBox::from_id_salt("app_filter_combo")
                        .selected_text(format!("🖥 {app_label}"))
                        .width(120.0)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(self.app_filter.is_none(), "All Apps")
                                .clicked()
                            {
                                self.app_filter = None;
                                needs_reload = true;
                            }
                            for app in &self.apps {
                                let selected =
                                    self.app_filter.as_deref() == Some(&app.name);
                                if ui.selectable_label(selected, &app.name).clicked() {
                                    self.app_filter = Some(app.name.clone());
                                    needs_reload = true;
                                }
                            }
                        });

                    // Picks (Contextual Intelligence) toggle
                    if self.prefs.is_picks_enabled {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let picks_btn = egui::Button::new(
                                RichText::new("✨ Picks").strong().color(if self.picks_mode {
                                    Color32::WHITE
                                } else {
                                    ACCENT_PURPLE
                                }),
                            )
                            .fill(if self.picks_mode {
                                ACCENT_PURPLE.gamma_multiply(0.55)
                            } else {
                                BG_SURFACE
                            });
                            if ui
                                .add(picks_btn)
                                .on_hover_text("Rank clips matching your active window context")
                                .clicked()
                            {
                                self.picks_mode = !self.picks_mode;
                            }
                        });
                    }
                });

                ui.add_space(8.0);

                // 4. Horizontal Card Carousel (ItemCollectionView)
                let display_list: Vec<(ItemEntity, Option<String>)> = if self.picks_mode {
                    let ext_app = self
                        .last_external_window
                        .as_ref()
                        .map(|w| w.app_name.as_str());
                    let ext_title = self
                        .last_external_window
                        .as_ref()
                        .map(|w| w.window_title.as_str());
                    PicksEngine::rank_picks(&self.items, ext_app, ext_title, 25)
                        .into_iter()
                        .map(|p| (p.item, Some(p.reason)))
                        .collect()
                } else {
                    self.items.iter().cloned().map(|it| (it, None)).collect()
                };

                if display_list.is_empty() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            RichText::new(
                                "No clipboard items match the current view.\nCopy anything or click '+ Clip' to add an item.",
                            )
                            .size(14.0)
                            .color(TEXT_MUTED),
                        );
                    });
                } else {
                    egui::ScrollArea::horizontal()
                        .id_salt("cards_carousel_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 12.0;
                                let card_size = Vec2::new(244.0, self.card_height);
                                for (idx, (item, reason)) in display_list.iter().enumerate() {
                                    let is_selected =
                                        self.selected_item_id.as_deref() == Some(&item.identifier);
                                    if let Some(act) = render_card(
                                        ui,
                                        item,
                                        idx,
                                        is_selected,
                                        card_size,
                                        &self.pinboards,
                                        &mut self.thumb_cache,
                                        reason.as_deref(),
                                    ) {
                                        pending_card_action = Some(act);
                                    }
                                }
                            });
                        });
                }

                // Bottom Status / Toast Bar
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(format!(
                            "{} item(s)  ·  Enter: Paste  ·  Space: QuickLook & Edit  ·  Ctrl+1..9: Quick Paste",
                            display_list.len()
                        ))
                        .size(11.0)
                        .color(TEXT_MUTED),
                    );
                    if let Some((ref msg, created_at)) = self.toast {
                        if created_at.elapsed() < Duration::from_secs(4) {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(msg).size(11.5).strong().color(ACCENT_TEAL));
                            });
                        }
                    }
                });
            });

        // Iridescent Picks border glow
        if self.picks_mode {
            draw_intelligence_glow(ctx, ctx.screen_rect());
        }

        // Process card actions
        if let Some(act) = pending_card_action {
            match act {
                CardAction::Select(id) => {
                    self.selected_item_id = Some(id);
                }
                CardAction::Paste {
                    item_id,
                    plain_text,
                } => {
                    self.selected_item_id = Some(item_id.clone());
                    self.trigger_paste(&item_id, plain_text);
                }
                CardAction::Preview(id) => {
                    if let Some(item) = self.items.iter().find(|it| it.identifier == id).cloned() {
                        self.preview_state = Some(PreviewState::open(ctx, &self.db, item));
                    }
                }
                CardAction::AddToStack(item) => {
                    let title = item.title.clone();
                    self.paste_stack.push(item);
                    self.paste_stack.is_active = true;
                    self.show_toast(format!("Added '{title}' to Paste Stack"));
                }
                CardAction::PinTo {
                    item_id,
                    pinboard_id,
                } => {
                    if self.db.add_item_to_pinboard(&item_id, &pinboard_id).is_ok() {
                        needs_reload = true;
                        self.show_toast("Pinned item to Pinboard");
                    }
                }
                CardAction::Unpin(item_id) => {
                    if self.db.remove_item_from_pinboard(&item_id).is_ok() {
                        needs_reload = true;
                        self.show_toast("Unpinned item");
                    }
                }
                CardAction::Delete(item_id) => {
                    if self.db.delete_item(&item_id).unwrap_or(false) {
                        needs_reload = true;
                        self.show_toast("Deleted item");
                    }
                }
            }
        }

        // Render QuickLook Preview Modal
        if let Some(mut preview) = self.preview_state.take() {
            match render_preview_modal(ctx, &self.db, &mut preview, &mut self.thumb_cache) {
                PreviewModalOutcome::KeepOpen => {
                    self.preview_state = Some(preview);
                }
                PreviewModalOutcome::Close => {}
                PreviewModalOutcome::Updated => {
                    needs_reload = true;
                    self.preview_state = Some(preview);
                }
                PreviewModalOutcome::Paste {
                    item_id,
                    plain_text,
                } => {
                    self.trigger_paste(&item_id, plain_text);
                }
            }
        }

        // Render Paste Stack HUD
        if self.paste_stack.is_active {
            if let StackHudAction::PasteNext = render_paste_stack_hud(ctx, &mut self.paste_stack) {
                if let Some(next_item) = self.paste_stack.pop_next() {
                    self.trigger_paste(&next_item.identifier, false);
                }
            }
        }

        // Render Settings Modal
        if self.settings_open {
            if render_settings_modal(
                ctx,
                &self.db,
                &mut self.settings_open,
                &mut self.prefs,
                &mut self.new_blacklist_entry,
                &mut self.confirm_clear_history,
            ) {
                needs_reload = true;
            }
        }

        // Render New Clip Modal
        if self.new_clip_open {
            if render_new_clip_modal(
                ctx,
                &self.db,
                &mut self.new_clip_open,
                &mut self.draft_clip_title,
                &mut self.draft_clip_text,
                self.active_pinboard_id.as_deref(),
            ) {
                needs_reload = true;
                self.show_toast("Created new clip");
            }
        }

        // Render Create / Rename Pinboard Modal
        if self.new_pinboard_open {
            let title = if self.renaming_pinboard_id.is_some() {
                "Rename Pinboard"
            } else {
                "New Pinboard"
            };
            let mut open = self.new_pinboard_open;
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.text_edit_singleline(&mut self.new_pinboard_name);
                    });
                    if self.renaming_pinboard_id.is_none() {
                        ui.horizontal(|ui| {
                            ui.label("Color:");
                            for c in ["#4A90E2", "#50E3C2", "#F5A623", "#BD10E0", "#FF5E57", "#6C5CE7"] {
                                let col = color_from_hex(c);
                                let btn = egui::Button::new("  ").fill(col);
                                if ui.add(btn).clicked() {
                                    self.new_pinboard_color = c.to_string();
                                }
                            }
                        });
                    }
                    ui.horizontal(|ui| {
                        if ui
                            .add_enabled(!self.new_pinboard_name.trim().is_empty(), egui::Button::new("Save"))
                            .clicked()
                        {
                            if let Some(ref pb_id) = self.renaming_pinboard_id {
                                let _ = self.db.rename_pinboard(pb_id, &self.new_pinboard_name);
                            } else if let Ok(pb) = self
                                .db
                                .create_pinboard(&self.new_pinboard_name, Some(&self.new_pinboard_color))
                            {
                                self.active_pinboard_id = Some(pb.identifier);
                            }
                            self.new_pinboard_open = false;
                            needs_reload = true;
                        }
                    });
                });
            if !open {
                self.new_pinboard_open = false;
            }
        }

        if needs_reload {
            self.reload_data();
        }
    }
}
