use chrono::Utc;
use rusqlite::{params, OptionalExtension};
use uuid::Uuid;

use crate::error::{PasteError, Result};
use crate::models::{ItemEntity, Pinboard};

use super::Database;

impl Database {
    pub fn list_pinboards(&self) -> Result<Vec<Pinboard>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT
                p.identifier, p.name, p.raw_type, p.color_hex, p.icon_name,
                p.tab_index, p.created_at, p.updated_at,
                (SELECT COUNT(*) FROM items i WHERE i.pinboard_id = p.identifier) AS item_count
             FROM pinboards p
             ORDER BY p.tab_index ASC, p.created_at ASC",
        )?;

        let rows = stmt
            .query_map([], |r| {
                let count: i64 = r.get(8)?;
                Ok(Pinboard {
                    identifier: r.get(0)?,
                    name: r.get(1)?,
                    raw_type: r.get(2)?,
                    color_hex: r.get(3)?,
                    icon_name: r.get(4)?,
                    tab_index: r.get(5)?,
                    created_at: r.get(6)?,
                    updated_at: r.get(7)?,
                    item_count: count.max(0) as usize,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn get_pinboard(&self, id: &str) -> Result<Option<Pinboard>> {
        let conn = self.lock();
        let mut stmt = conn.prepare(
            "SELECT
                p.identifier, p.name, p.raw_type, p.color_hex, p.icon_name,
                p.tab_index, p.created_at, p.updated_at,
                (SELECT COUNT(*) FROM items i WHERE i.pinboard_id = p.identifier) AS item_count
             FROM pinboards p
             WHERE p.identifier = ?1",
        )?;
        let row = stmt
            .query_row(params![id], |r| {
                let count: i64 = r.get(8)?;
                Ok(Pinboard {
                    identifier: r.get(0)?,
                    name: r.get(1)?,
                    raw_type: r.get(2)?,
                    color_hex: r.get(3)?,
                    icon_name: r.get(4)?,
                    tab_index: r.get(5)?,
                    created_at: r.get(6)?,
                    updated_at: r.get(7)?,
                    item_count: count.max(0) as usize,
                })
            })
            .optional()?;
        Ok(row)
    }

    pub fn create_pinboard(&self, name: &str, color_hex: Option<&str>) -> Result<Pinboard> {
        let clean_name = name.trim();
        if clean_name.is_empty() {
            return Err(PasteError::InvalidInput("pinboard name cannot be empty".to_string()));
        }

        let palette = ["#4A90E2", "#50E3C2", "#F5A623", "#BD10E0", "#FF5E57", "#7ED321"];
        let now = Utc::now().timestamp_millis();
        let id = Uuid::new_v4().to_string();

        let conn = self.lock();
        let next_idx: i64 = conn.query_row(
            "SELECT COALESCE(MAX(tab_index), -1) + 1 FROM pinboards",
            [],
            |r| r.get(0),
        )?;
        let color = color_hex
            .unwrap_or_else(|| palette[(next_idx.max(0) as usize) % palette.len()]);

        conn.execute(
            "INSERT INTO pinboards (identifier, name, raw_type, color_hex, icon_name, tab_index, created_at, updated_at)
             VALUES (?1, ?2, 0, ?3, 'pin', ?4, ?5, ?5)",
            params![id, clean_name, color, next_idx, now],
        )?;
        drop(conn);

        self.get_pinboard(&id)?
            .ok_or_else(|| PasteError::NotFound(id))
    }

    pub fn rename_pinboard(&self, id: &str, new_name: &str) -> Result<Pinboard> {
        let clean_name = new_name.trim();
        if clean_name.is_empty() {
            return Err(PasteError::InvalidInput("pinboard name cannot be empty".to_string()));
        }
        let now = Utc::now().timestamp_millis();
        let conn = self.lock();
        let affected = conn.execute(
            "UPDATE pinboards SET name = ?2, updated_at = ?3 WHERE identifier = ?1",
            params![id, clean_name, now],
        )?;
        if affected == 0 {
            return Err(PasteError::NotFound(format!("pinboard '{id}' not found")));
        }
        drop(conn);
        self.get_pinboard(id)?
            .ok_or_else(|| PasteError::NotFound(id.to_string()))
    }

    pub fn recolor_pinboard(&self, id: &str, color_hex: &str) -> Result<Pinboard> {
        let now = Utc::now().timestamp_millis();
        let conn = self.lock();
        let affected = conn.execute(
            "UPDATE pinboards SET color_hex = ?2, updated_at = ?3 WHERE identifier = ?1",
            params![id, color_hex, now],
        )?;
        if affected == 0 {
            return Err(PasteError::NotFound(format!("pinboard '{id}' not found")));
        }
        drop(conn);
        self.get_pinboard(id)?
            .ok_or_else(|| PasteError::NotFound(id.to_string()))
    }

    pub fn delete_pinboard(&self, id: &str) -> Result<bool> {
        let conn = self.lock();
        // Items stay in Clipboard History (`pinboard_id` becomes NULL via ON DELETE SET NULL)
        let affected = conn.execute("DELETE FROM pinboards WHERE identifier = ?1", params![id])?;
        Ok(affected > 0)
    }

    pub fn add_item_to_pinboard(&self, item_id: &str, pinboard_id: &str) -> Result<ItemEntity> {
        if self.get_pinboard(pinboard_id)?.is_none() {
            return Err(PasteError::NotFound(format!(
                "pinboard '{pinboard_id}' not found"
            )));
        }
        let now = Utc::now().timestamp_millis();
        let conn = self.lock();
        let max_order: i64 = conn.query_row(
            "SELECT COALESCE(MAX(display_order_in_pinboard), 0) + 1 FROM items WHERE pinboard_id = ?1",
            params![pinboard_id],
            |r| r.get(0),
        )?;
        let affected = conn.execute(
            "UPDATE items
             SET pinboard_id = ?2,
                 display_order_in_pinboard = CASE WHEN pinboard_id = ?2 THEN display_order_in_pinboard ELSE ?3 END,
                 updated_at = ?4
             WHERE identifier = ?1",
            params![item_id, pinboard_id, max_order, now],
        )?;
        if affected == 0 {
            return Err(PasteError::NotFound(format!("item '{item_id}' not found")));
        }
        drop(conn);
        self.get_item(item_id)?
            .ok_or_else(|| PasteError::NotFound(item_id.to_string()))
    }

    pub fn remove_item_from_pinboard(&self, item_id: &str) -> Result<ItemEntity> {
        let now = Utc::now().timestamp_millis();
        let conn = self.lock();
        let affected = conn.execute(
            "UPDATE items
             SET pinboard_id = NULL, display_order_in_pinboard = 0, updated_at = ?2
             WHERE identifier = ?1",
            params![item_id, now],
        )?;
        if affected == 0 {
            return Err(PasteError::NotFound(format!("item '{item_id}' not found")));
        }
        drop(conn);
        self.get_item(item_id)?
            .ok_or_else(|| PasteError::NotFound(item_id.to_string()))
    }
}
