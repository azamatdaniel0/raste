use chrono::Utc;

use crate::models::{ItemEntity, ItemType};

/// A contextually ranked clipboard recommendation ("Picks").
#[derive(Debug, Clone)]
pub struct PickRecommendation {
    pub item: ItemEntity,
    pub score: f32,
    pub reason: String,
}

/// Cross-platform contextual ranker matching Paste's "Picks" feature.
pub struct PicksEngine;

impl PicksEngine {
    pub fn rank_picks(
        items: &[ItemEntity],
        active_app_name: Option<&str>,
        active_window_title: Option<&str>,
        limit: usize,
    ) -> Vec<PickRecommendation> {
        let now_ms = Utc::now().timestamp_millis();
        let app_lower = active_app_name.unwrap_or("").to_ascii_lowercase();
        let win_lower = active_window_title.unwrap_or("").to_ascii_lowercase();

        let is_ide_context = ["code", "zed", "rust", "idea", "terminal", "powershell", "cmd", "wt", "vim", "nvim", "helix"]
            .iter()
            .any(|k| app_lower.contains(k) || win_lower.contains(k));
        let is_browser_context = ["chrome", "firefox", "zen", "edge", "safari", "brave", "arc"]
            .iter()
            .any(|k| app_lower.contains(k));
        let is_design_context = ["figma", "vectorcraft", "photoshop", "gimp", "blender", "illustrator"]
            .iter()
            .any(|k| app_lower.contains(k));

        let mut scored: Vec<PickRecommendation> = items
            .iter()
            .filter(|i| !i.is_confidential)
            .map(|item| {
                let mut score = 0.0f32;
                let mut reason = "Recent clip".to_string();

                let age_hours = ((now_ms - item.timestamp).max(0) as f32) / 3_600_000.0;
                score += (10.0 / (1.0 + age_hours * 0.5)).max(0.5);

                if item.pinboard_id.is_some() {
                    score += 4.5;
                    reason = "Pinned snippet".to_string();
                }

                if is_ide_context {
                    match item.item_type {
                        ItemType::Text if item.preview.language_hint.is_some() => {
                            score += 9.0;
                            reason = format!(
                                "Code ({}) for editor",
                                item.preview.language_hint.as_deref().unwrap_or("snippet")
                            );
                        }
                        ItemType::Files => {
                            score += 6.0;
                            reason = "File path for dev context".to_string();
                        }
                        ItemType::Link => {
                            score += 4.0;
                            reason = "Reference link".to_string();
                        }
                        _ => {}
                    }
                } else if is_browser_context {
                    match item.item_type {
                        ItemType::Link => {
                            score += 8.5;
                            reason = "Link for browser".to_string();
                        }
                        ItemType::Text => {
                            score += 4.5;
                            reason = "Text snippet for form/search".to_string();
                        }
                        _ => {}
                    }
                } else if is_design_context {
                    match item.item_type {
                        ItemType::Color => {
                            score += 10.0;
                            reason = "Color swatch for design app".to_string();
                        }
                        ItemType::Image => {
                            score += 9.0;
                            reason = "Image asset for design app".to_string();
                        }
                        _ => {}
                    }
                }

                PickRecommendation {
                    item: item.clone(),
                    score,
                    reason,
                }
            })
            .collect();

        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(limit);
        scored
    }
}
