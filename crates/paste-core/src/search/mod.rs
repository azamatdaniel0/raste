use rusqlite::types::Value;

use crate::error::Result;
use crate::models::{ItemEntity, ItemType};
use crate::storage::item_repo::map_item_row;
use crate::storage::Database;

/// Structured search parameters matching Paste's SearchBar & MCP `search_saved_items`.
#[derive(Debug, Clone, Default)]
pub struct SearchFilter {
    pub query: Option<String>,
    pub item_type: Option<ItemType>,
    pub source_app: Option<String>,
    pub device: Option<String>,
    pub title: Option<String>,
    pub pinboard_id: Option<String>,
    pub date_from_ms: Option<i64>,
    pub date_to_ms: Option<i64>,
    pub limit: usize,
}

impl Database {
    pub fn search_items(&self, filter: &SearchFilter) -> Result<Vec<ItemEntity>> {
        let limit = filter.limit.clamp(1, 500) as i64;
        let mut sql = String::from(
            "SELECT
                i.identifier, i.title, i.checksum, i.raw_type, i.raw_preview,
                i.timestamp, i.created_at, i.updated_at, i.display_order_in_pinboard,
                i.source_app_id, a.name, a.raw_color,
                i.device_id, d.name, i.pinboard_id, i.is_confidential
             FROM items i
             LEFT JOIN applications a ON i.source_app_id = a.bundle_identifier
             LEFT JOIN devices d ON i.device_id = d.identifier
             LEFT JOIN item_data idata ON i.identifier = idata.item_id
             WHERE 1=1",
        );

        let mut params_vec: Vec<Value> = Vec::new();

        if let Some(ref q) = filter.query {
            let trimmed = q.trim();
            if !trimmed.is_empty() {
                let pattern = format!("%{}%", trimmed.to_lowercase());
                sql.push_str(
                    " AND (
                        LOWER(i.title) LIKE ?
                        OR LOWER(COALESCE(idata.plain_text, '')) LIKE ?
                        OR LOWER(COALESCE(a.name, '')) LIKE ?
                        OR LOWER(i.raw_preview) LIKE ?
                    )",
                );
                params_vec.push(Value::Text(pattern.clone()));
                params_vec.push(Value::Text(pattern.clone()));
                params_vec.push(Value::Text(pattern.clone()));
                params_vec.push(Value::Text(pattern));
            }
        }

        if let Some(item_type) = filter.item_type {
            sql.push_str(" AND i.raw_type = ?");
            params_vec.push(Value::Integer(item_type as i64));
        }

        if let Some(ref app) = filter.source_app {
            let trimmed = app.trim();
            if !trimmed.is_empty() {
                let pat = format!("%{}%", trimmed.to_lowercase());
                sql.push_str(" AND (LOWER(COALESCE(i.source_app_id, '')) LIKE ? OR LOWER(COALESCE(a.name, '')) LIKE ?)");
                params_vec.push(Value::Text(pat.clone()));
                params_vec.push(Value::Text(pat));
            }
        }

        if let Some(ref dev) = filter.device {
            let trimmed = dev.trim();
            if !trimmed.is_empty() {
                let pat = format!("%{}%", trimmed.to_lowercase());
                sql.push_str(" AND (LOWER(COALESCE(i.device_id, '')) LIKE ? OR LOWER(COALESCE(d.name, '')) LIKE ?)");
                params_vec.push(Value::Text(pat.clone()));
                params_vec.push(Value::Text(pat));
            }
        }

        if let Some(ref title) = filter.title {
            let trimmed = title.trim();
            if !trimmed.is_empty() {
                sql.push_str(" AND LOWER(i.title) LIKE ?");
                params_vec.push(Value::Text(format!("%{}%", trimmed.to_lowercase())));
            }
        }

        if let Some(ref pb_id) = filter.pinboard_id {
            sql.push_str(" AND i.pinboard_id = ?");
            params_vec.push(Value::Text(pb_id.clone()));
        }

        if let Some(from_ms) = filter.date_from_ms {
            sql.push_str(" AND i.timestamp >= ?");
            params_vec.push(Value::Integer(from_ms));
        }

        if let Some(to_ms) = filter.date_to_ms {
            sql.push_str(" AND i.timestamp <= ?");
            params_vec.push(Value::Integer(to_ms));
        }

        if filter.pinboard_id.is_some() {
            sql.push_str(" ORDER BY i.display_order_in_pinboard ASC, i.timestamp DESC LIMIT ?");
        } else {
            sql.push_str(" ORDER BY i.timestamp DESC LIMIT ?");
        }
        params_vec.push(Value::Integer(limit));

        let conn = self.lock();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt
            .query_map(rusqlite::params_from_iter(params_vec.iter()), map_item_row)?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }
}
