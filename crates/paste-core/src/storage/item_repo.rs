use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::error::{PasteError, Result};
use crate::models::{
    ApplicationEntity, ItemDataEntity, ItemEntity, ItemPreview, ItemType, NewClipboardPayload,
};
use crate::processor::{classify_and_build_preview, compute_checksum};

use super::Database;

impl Database {
    /// Ingests a captured clipboard payload or manual clip.
    ///
    /// Returns `(item_entity, is_new_record)`:
    /// - If an identical SHA-256 `checksum` already exists, updates its `timestamp` & `updated_at`
    ///   and moves it to the top of the carousel (`is_new_record = false`).
    /// - If the source app is blacklisted in Preferences, returns `Err(PasteError::InvalidInput)`.
    pub fn ingest_clipboard_payload(
        &self,
        payload: NewClipboardPayload,
    ) -> Result<(ItemEntity, bool)> {
        let prefs = self.load_preferences()?;

        if let Some(ref app_id) = payload.source_app_id {
            if prefs
                .blacklisted_apps
                .iter()
                .any(|b| b.eq_ignore_ascii_case(app_id))
            {
                return Err(PasteError::InvalidInput(format!(
                    "application '{app_id}' is blacklisted"
                )));
            }
        }

        let checksum = compute_checksum(&payload);
        let now = Utc::now().timestamp_millis();

        let conn = self.lock();

        if let Some(ref app_id) = payload.source_app_id {
            let app_name = payload
                .source_app_name
                .as_deref()
                .unwrap_or(app_id.as_str());
            let app_color = payload
                .source_app_color
                .as_deref()
                .unwrap_or("#4A90E2");
            conn.execute(
                "INSERT INTO applications (bundle_identifier, name, raw_color, raw_icon)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(bundle_identifier) DO UPDATE SET
                   name = excluded.name,
                   raw_color = excluded.raw_color,
                   raw_icon = COALESCE(excluded.raw_icon, applications.raw_icon)",
                params![app_id, app_name, app_color, payload.source_app_icon],
            )?;
        }

        // Check for SHA-256 duplicate
        let existing_id: Option<String> = conn
            .query_row(
                "SELECT identifier FROM items WHERE checksum = ?1",
                params![checksum],
                |r| r.get(0),
            )
            .optional()?;

        if let Some(id) = existing_id {
            conn.execute(
                "UPDATE items
                 SET timestamp = ?2,
                     updated_at = ?2,
                     source_app_id = COALESCE(?3, source_app_id),
                     pinboard_id = COALESCE(?4, pinboard_id)
                 WHERE identifier = ?1",
                params![id, now, payload.source_app_id, payload.pinboard_id],
            )?;
            drop(conn);
            let updated = self
                .get_item(&id)?
                .ok_or_else(|| PasteError::NotFound(id))?;
            return Ok((updated, false));
        }

        let (item_type, title, preview, is_confidential) =
            classify_and_build_preview(&payload, prefs.detect_secrets);

        let id = Uuid::new_v4().to_string();
        let preview_json = serde_json::to_string(&preview)?;
        let device_id = payload
            .device_id
            .clone()
            .unwrap_or_else(|| "local-device".to_string());

        conn.execute(
            "INSERT INTO items (
                identifier, title, checksum, raw_type, raw_preview,
                timestamp, created_at, updated_at, display_order_in_pinboard,
                source_app_id, device_id, pinboard_id, is_confidential
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?6, 0, ?7, ?8, ?9, ?10)",
            params![
                id,
                title,
                checksum,
                item_type as i64,
                preview_json,
                now,
                payload.source_app_id,
                device_id,
                payload.pinboard_id,
                if is_confidential { 1i64 } else { 0i64 }
            ],
        )?;

        let file_paths_json = if payload.file_paths.is_empty() {
            None
        } else {
            Some(serde_json::to_string(&payload.file_paths)?)
        };

        conn.execute(
            "INSERT INTO item_data (
                item_id, plain_text, rtf_data, html_data, image_png, file_paths_json, raw_representations_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id,
                payload.plain_text,
                payload.rtf_data,
                payload.html_data,
                payload.image_png,
                file_paths_json,
                Option::<String>::None
            ],
        )?;

        let app_name_for_fts = payload.source_app_name.as_deref().unwrap_or("");
        let plain_for_fts = payload.plain_text.as_deref().unwrap_or(&preview.snippet);
        let domain_for_fts = preview.link_domain.as_deref().unwrap_or("");
        conn.execute(
            "INSERT INTO items_fts (identifier, title, plain_text, source_app_name, link_domain)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, title, plain_for_fts, app_name_for_fts, domain_for_fts],
        )?;

        // Enforce retention capacity on unpinned items
        if let Some(retention_ms) = prefs.history_capacity.retention_millis() {
            let cutoff = now - retention_ms;
            conn.execute(
                "DELETE FROM items_fts WHERE identifier IN (
                    SELECT identifier FROM items WHERE pinboard_id IS NULL AND timestamp < ?1
                 )",
                params![cutoff],
            )?;
            conn.execute(
                "DELETE FROM items WHERE pinboard_id IS NULL AND timestamp < ?1",
                params![cutoff],
            )?;
        }

        drop(conn);
        let created = self
            .get_item(&id)?
            .ok_or_else(|| PasteError::NotFound(id))?;
        Ok((created, true))
    }

    pub fn get_item(&self, id: &str) -> Result<Option<ItemEntity>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT
                i.identifier, i.title, i.checksum, i.raw_type, i.raw_preview,
                i.timestamp, i.created_at, i.updated_at, i.display_order_in_pinboard,
                i.source_app_id, a.name, a.raw_color,
                i.device_id, d.name, i.pinboard_id, i.is_confidential
             FROM items i
             LEFT JOIN applications a ON i.source_app_id = a.bundle_identifier
             LEFT JOIN devices d ON i.device_id = d.identifier
             WHERE i.identifier = ?1",
        )?;

        let row = stmt.query_row(params![id], map_item_row).optional()?;
        Ok(row)
    }

    pub fn get_item_data(&self, id: &str) -> Result<Option<ItemDataEntity>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT item_id, plain_text, rtf_data, html_data, image_png, file_paths_json, raw_representations_json
             FROM item_data WHERE item_id = ?1",
        )?;

        let row = stmt
            .query_row(params![id], |r| {
                let files_json: Option<String> = r.get(5)?;
                let file_paths = files_json
                    .as_deref()
                    .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
                    .unwrap_or_default();
                Ok(ItemDataEntity {
                    item_id: r.get(0)?,
                    plain_text: r.get(1)?,
                    rtf_data: r.get(2)?,
                    html_data: r.get(3)?,
                    image_png: r.get(4)?,
                    file_paths,
                    raw_representations_json: r.get(6)?,
                })
            })
            .optional()?;

        Ok(row)
    }

    pub fn update_item(
        &self,
        id: &str,
        new_title: Option<&str>,
        new_text: Option<&str>,
    ) -> Result<ItemEntity> {
        let existing = self
            .get_item(id)?
            .ok_or_else(|| PasteError::NotFound(format!("item '{id}' not found")))?;
        let existing_data = self.get_item_data(id)?;
        let now = Utc::now().timestamp_millis();

        let prefs = self.load_preferences()?;

        let updated_payload = NewClipboardPayload {
            plain_text: new_text
                .map(ToString::to_string)
                .or_else(|| existing_data.as_ref().and_then(|d| d.plain_text.clone())),
            rtf_data: existing_data.as_ref().and_then(|d| d.rtf_data.clone()),
            html_data: existing_data.as_ref().and_then(|d| d.html_data.clone()),
            image_png: existing_data.as_ref().and_then(|d| d.image_png.clone()),
            file_paths: existing_data
                .as_ref()
                .map(|d| d.file_paths.clone())
                .unwrap_or_default(),
            source_app_id: existing.source_app_id.clone(),
            source_app_name: existing.source_app_name.clone(),
            source_app_color: existing.source_app_color.clone(),
            source_app_icon: None,
            device_id: existing.device_id.clone(),
            pinboard_id: existing.pinboard_id.clone(),
            custom_title: new_title.map(ToString::to_string),
        };

        let (item_type, auto_title, preview, is_confidential) = if new_text.is_some() {
            classify_and_build_preview(&updated_payload, prefs.detect_secrets)
        } else {
            (
                existing.item_type,
                existing.title.clone(),
                existing.preview.clone(),
                existing.is_confidential,
            )
        };

        let final_title = new_title.map(ToString::to_string).unwrap_or(auto_title);
        let preview_json = serde_json::to_string(&preview)?;
        let new_checksum = compute_checksum(&updated_payload);

        let conn = self.lock();
        // Avoid UNIQUE constraint collision if edited text matches another item
        let final_checksum = {
            let collision: Option<String> = conn
                .query_row(
                    "SELECT identifier FROM items WHERE checksum = ?1 AND identifier != ?2",
                    params![new_checksum, id],
                    |r| r.get(0),
                )
                .optional()?;
            if collision.is_some() {
                format!("{new_checksum}-{id}")
            } else {
                new_checksum
            }
        };

        conn.execute(
            "UPDATE items
             SET title = ?2,
                 checksum = ?3,
                 raw_type = ?4,
                 raw_preview = ?5,
                 updated_at = ?6,
                 is_confidential = ?7
             WHERE identifier = ?1",
            params![
                id,
                final_title,
                final_checksum,
                item_type as i64,
                preview_json,
                now,
                if is_confidential { 1i64 } else { 0i64 }
            ],
        )?;

        if let Some(txt) = new_text {
            conn.execute(
                "UPDATE item_data SET plain_text = ?2 WHERE item_id = ?1",
                params![id, txt],
            )?;
        }

        conn.execute(
            "UPDATE items_fts SET title = ?2, plain_text = ?3 WHERE identifier = ?1",
            params![
                id,
                final_title,
                new_text.unwrap_or(&preview.snippet)
            ],
        )?;

        drop(conn);
        self.get_item(id)?
            .ok_or_else(|| PasteError::NotFound(id.to_string()))
    }

    pub fn update_item_image(&self, id: &str, new_png: Vec<u8>) -> Result<ItemEntity> {
        let existing = self
            .get_item(id)?
            .ok_or_else(|| PasteError::NotFound(format!("item '{id}' not found")))?;
        let now = Utc::now().timestamp_millis();

        let payload = NewClipboardPayload {
            image_png: Some(new_png.clone()),
            custom_title: Some(existing.title.clone()),
            ..Default::default()
        };
        let (_, _, preview, _) = classify_and_build_preview(&payload, false);
        let preview_json = serde_json::to_string(&preview)?;

        let conn = self.lock();
        conn.execute(
            "UPDATE items SET raw_preview = ?2, updated_at = ?3 WHERE identifier = ?1",
            params![id, preview_json, now],
        )?;
        conn.execute(
            "UPDATE item_data SET image_png = ?2 WHERE item_id = ?1",
            params![id, new_png],
        )?;
        drop(conn);

        self.get_item(id)?
            .ok_or_else(|| PasteError::NotFound(id.to_string()))
    }

    pub fn delete_item(&self, id: &str) -> Result<bool> {
        let conn = self.lock();
        conn.execute("DELETE FROM items_fts WHERE identifier = ?1", params![id])?;
        let affected = conn.execute("DELETE FROM items WHERE identifier = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn clear_history(&self, keep_pinned: bool) -> Result<usize> {
        let conn = self.lock();
        if keep_pinned {
            conn.execute(
                "DELETE FROM items_fts WHERE identifier IN (SELECT identifier FROM items WHERE pinboard_id IS NULL)",
                [],
            )?;
            let count = conn.execute("DELETE FROM items WHERE pinboard_id IS NULL", [])?;
            Ok(count)
        } else {
            conn.execute("DELETE FROM items_fts", [])?;
            let count = conn.execute("DELETE FROM items", [])?;
            Ok(count)
        }
    }

    pub fn list_distinct_apps(&self) -> Result<Vec<ApplicationEntity>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT DISTINCT a.bundle_identifier, a.name, a.raw_color, a.raw_icon
             FROM applications a
             INNER JOIN items i ON i.source_app_id = a.bundle_identifier
             ORDER BY a.name ASC",
        )?;
        let rows = stmt
            .query_map([], |r| {
                Ok(ApplicationEntity {
                    bundle_identifier: r.get(0)?,
                    name: r.get(1)?,
                    raw_color: r.get(2)?,
                    raw_icon: r.get(3)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Populates initial showcase items on a brand-new empty database so the user
    /// immediately sees all card types (Code, Color, Link, JSON, Shell).
    pub fn seed_demo_items_if_empty(&self) -> Result<()> {
        let count: i64 = {
            let conn = self.lock();
            conn.query_row("SELECT COUNT(*) FROM items", [], |r| r.get(0))?
        };
        if count > 0 {
            return Ok(());
        }

        let pinboards = self.list_pinboards()?;
        let code_pb = pinboards.first().map(|p| p.identifier.clone());
        let link_pb = pinboards.get(1).map(|p| p.identifier.clone());
        let color_pb = pinboards.get(2).map(|p| p.identifier.clone());

        let samples = vec![
            NewClipboardPayload {
                plain_text: Some("#6C5CE7".to_string()),
                source_app_id: Some("figma.exe".to_string()),
                source_app_name: Some("Figma".to_string()),
                source_app_color: Some("#A259FF".to_string()),
                pinboard_id: color_pb.clone(),
                ..Default::default()
            },
            NewClipboardPayload {
                plain_text: Some("#00CEC9".to_string()),
                source_app_id: Some("vectorcraft.exe".to_string()),
                source_app_name: Some("VectorCraft".to_string()),
                source_app_color: Some("#00B894".to_string()),
                pinboard_id: color_pb,
                ..Default::default()
            },
            NewClipboardPayload {
                plain_text: Some("https://doc.rust-lang.org/cargo/reference/workspaces.html".to_string()),
                source_app_id: Some("zen.exe".to_string()),
                source_app_name: Some("Zen Browser".to_string()),
                source_app_color: Some("#50E3C2".to_string()),
                pinboard_id: link_pb,
                ..Default::default()
            },
            NewClipboardPayload {
                plain_text: Some(
                    "fn deduplicate_by_sha256(bytes: &[u8]) -> String {\n    let mut hasher = Sha256::new();\n    hasher.update(bytes);\n    hex::encode(hasher.finalize())\n}".to_string()
                ),
                source_app_id: Some("Code.exe".to_string()),
                source_app_name: Some("VS Code".to_string()),
                source_app_color: Some("#007ACC".to_string()),
                pinboard_id: code_pb,
                ..Default::default()
            },
            NewClipboardPayload {
                plain_text: Some("cargo test --workspace -- --nocapture".to_string()),
                source_app_id: Some("WindowsTerminal.exe".to_string()),
                source_app_name: Some("Terminal".to_string()),
                source_app_color: Some("#4A90E2".to_string()),
                ..Default::default()
            },
            NewClipboardPayload {
                plain_text: Some("{\n  \"mcpServers\": {\n    \"new-paste\": {\n      \"command\": \"paste-mcp-stdio.exe\"\n    }\n  }\n}".to_string()),
                source_app_id: Some("zed.exe".to_string()),
                source_app_name: Some("Zed".to_string()),
                source_app_color: Some("#F5A623".to_string()),
                ..Default::default()
            },
        ];

        for sample in samples {
            let _ = self.ingest_clipboard_payload(sample)?;
            std::thread::sleep(std::time::Duration::from_millis(2));
        }

        Ok(())
    }
}

pub(crate) fn map_item_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<ItemEntity> {
    let raw_type: i64 = r.get(3)?;
    let preview_str: String = r.get(4)?;
    let preview: ItemPreview = serde_json::from_str(&preview_str).unwrap_or_default();
    let is_conf: i64 = r.get(15)?;

    Ok(ItemEntity {
        identifier: r.get(0)?,
        title: r.get(1)?,
        checksum: r.get(2)?,
        item_type: ItemType::from_raw(raw_type),
        preview,
        timestamp: r.get(5)?,
        created_at: r.get(6)?,
        updated_at: r.get(7)?,
        display_order_in_pinboard: r.get(8)?,
        source_app_id: r.get(9)?,
        source_app_name: r.get(10)?,
        source_app_color: r.get(11)?,
        device_id: r.get(12)?,
        device_name: r.get(13)?,
        pinboard_id: r.get(14)?,
        is_confidential: is_conf != 0,
    })
}
