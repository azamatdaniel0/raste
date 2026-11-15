use rusqlite::{params, OptionalExtension};

use crate::error::Result;
use crate::models::Preferences;

use super::Database;

const PREFS_KEY: &str = "app_preferences";

impl Database {
    pub fn load_preferences(&self) -> Result<Preferences> {
        let conn = self.lock();
        let json_opt: Option<String> = conn
            .query_row(
                "SELECT value_json FROM preferences WHERE key = ?1",
                params![PREFS_KEY],
                |r| r.get(0),
            )
            .optional()?;

        match json_opt {
            Some(json) => Ok(serde_json::from_str(&json).unwrap_or_default()),
            None => Ok(Preferences::default()),
        }
    }

    pub fn save_preferences(&self, prefs: &Preferences) -> Result<()> {
        let json = serde_json::to_string(prefs)?;
        let conn = self.lock();
        conn.execute(
            "INSERT INTO preferences (key, value_json)
             VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json",
            params![PREFS_KEY, json],
        )?;
        Ok(())
    }
}
