use chrono::Utc;
use rusqlite::Connection;
use uuid::Uuid;

use crate::error::Result;
use crate::models::DeviceType;

pub fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;

        CREATE TABLE IF NOT EXISTS applications (
            bundle_identifier TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            raw_color TEXT NOT NULL,
            raw_icon BLOB
        );

        CREATE TABLE IF NOT EXISTS devices (
            identifier TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            app_version TEXT NOT NULL,
            raw_type INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS pinboards (
            identifier TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            raw_type INTEGER NOT NULL DEFAULT 0,
            color_hex TEXT NOT NULL,
            icon_name TEXT NOT NULL DEFAULT 'folder',
            tab_index INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS items (
            identifier TEXT PRIMARY KEY,
            title TEXT NOT NULL,
            checksum TEXT NOT NULL UNIQUE,
            raw_type INTEGER NOT NULL,
            raw_preview TEXT NOT NULL,
            timestamp INTEGER NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            display_order_in_pinboard INTEGER NOT NULL DEFAULT 0,
            source_app_id TEXT REFERENCES applications(bundle_identifier) ON DELETE SET NULL,
            device_id TEXT REFERENCES devices(identifier) ON DELETE SET NULL,
            pinboard_id TEXT REFERENCES pinboards(identifier) ON DELETE SET NULL,
            is_confidential INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_items_timestamp ON items(timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_items_pinboard ON items(pinboard_id, display_order_in_pinboard, timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_items_type ON items(raw_type);
        CREATE INDEX IF NOT EXISTS idx_items_app ON items(source_app_id);

        CREATE TABLE IF NOT EXISTS item_data (
            item_id TEXT PRIMARY KEY REFERENCES items(identifier) ON DELETE CASCADE,
            plain_text TEXT,
            rtf_data BLOB,
            html_data TEXT,
            image_png BLOB,
            file_paths_json TEXT,
            raw_representations_json TEXT
        );

        CREATE TABLE IF NOT EXISTS object_metadata (
            object_uri TEXT PRIMARY KEY,
            record_name TEXT,
            zone_name TEXT,
            is_pending_deletion INTEGER NOT NULL DEFAULT 0,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS preferences (
            key TEXT PRIMARY KEY,
            value_json TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS items_fts USING fts5(
            identifier UNINDEXED,
            title,
            plain_text,
            source_app_name,
            link_domain
        );
        "#,
    )?;

    seed_defaults(conn)?;
    Ok(())
}

fn seed_defaults(conn: &Connection) -> Result<()> {
    let now = Utc::now().timestamp_millis();

    let pinboard_count: i64 = conn.query_row("SELECT COUNT(*) FROM pinboards", [], |r| r.get(0))?;
    if pinboard_count == 0 {
        let defaults = [
            ("Code Snippets", "#4A90E2", "code", 0i64),
            ("Links & Docs", "#50E3C2", "link", 1i64),
            ("Design Colors", "#F5A623", "palette", 2i64),
        ];
        for (name, color, icon, idx) in defaults {
            let id = Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO pinboards (identifier, name, raw_type, color_hex, icon_name, tab_index, created_at, updated_at)
                 VALUES (?1, ?2, 0, ?3, ?4, ?5, ?6, ?6)",
                rusqlite::params![id, name, color, icon, idx, now],
            )?;
        }
    }

    let device_count: i64 = conn.query_row("SELECT COUNT(*) FROM devices", [], |r| r.get(0))?;
    if device_count == 0 {
        let host = std::env::var("COMPUTERNAME")
            .or_else(|_| std::env::var("HOSTNAME"))
            .unwrap_or_else(|_| "Local Workstation".to_string());
        let dev_type = DeviceType::current_os() as i64;
        conn.execute(
            "INSERT INTO devices (identifier, name, app_version, raw_type) VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params!["local-device", host, env!("CARGO_PKG_VERSION"), dev_type],
        )?;
    }

    Ok(())
}
