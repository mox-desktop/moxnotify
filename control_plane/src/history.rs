use rusqlite::params;
use serde::Serialize;
use std::path::Path;
use prost::Message;
use base64::{Engine as _, engine::general_purpose};

use crate::collector::{NewNotification, Action, NotificationHints};

#[derive(Default, PartialEq, Clone, Copy, Serialize)]
pub enum HistoryState {
    #[default]
    Hidden,
    Shown,
}

pub struct History {
    db: rusqlite::Connection,
    state: HistoryState,
}

impl History {
    pub fn try_new(path: &Path) -> anyhow::Result<Self> {
        let db = rusqlite::Connection::open(path)?;
        db.execute(
            "CREATE TABLE IF NOT EXISTS notifications (
            rowid INTEGER PRIMARY KEY AUTOINCREMENT,
            id INTEGER,
            app_name TEXT,
            app_icon TEXT,
            summary TEXT,
            body TEXT,
            timeout INTEGER,
            actions TEXT,
            hints JSON
        );",
            (),
        )?;

        Ok(Self {
            db,
            state: HistoryState::default(),
        })
    }

    pub fn state(&self) -> HistoryState {
        self.state
    }

    pub fn is_shown(&self) -> bool {
        self.state() == HistoryState::Shown
    }

    pub fn is_hidden(&self) -> bool {
        self.state() == HistoryState::Hidden
    }

    pub fn set_state(&mut self, state: HistoryState) {
        self.state = state;
    }

    pub fn hide(&mut self) {
        self.state = HistoryState::Hidden;
    }

    pub fn show(&mut self) {
        self.state = HistoryState::Shown;
    }

    pub fn insert(&self, data: &NewNotification) -> anyhow::Result<()> {
        let mut actions_bytes = Vec::new();
        for action in &data.actions {
            let action_len = action.encoded_len();
            prost::encoding::encode_varint(action_len as u64, &mut actions_bytes);
            action.encode(&mut actions_bytes)?;
        }
        let actions_encoded = general_purpose::STANDARD.encode(&actions_bytes);
        
        let hints_encoded = if let Some(ref hints) = data.hints {
            let mut hints_bytes = Vec::new();
            hints.encode(&mut hints_bytes)?;
            general_purpose::STANDARD.encode(&hints_bytes)
        } else {
            String::new()
        };

        self.db.execute(
            "INSERT INTO notifications (id, app_name, app_icon, timeout, summary, body, actions, hints)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                data.id,
                data.app_name,
                data.app_icon.as_ref().map(|s| s.as_str()),
                data.timeout,
                data.summary,
                data.body,
                actions_encoded,
                hints_encoded,
            ],
        )?;

        Ok(())
    }

    pub fn last_insert_rowid(&self) -> u32 {
        self.db.last_insert_rowid() as u32
    }

    pub fn trim(&self, keep: i64) -> anyhow::Result<()> {
        self.db.execute(
            "DELETE FROM notifications WHERE rowid IN (
                SELECT rowid FROM notifications 
                ORDER BY rowid ASC 
                LIMIT MAX(0, (SELECT COUNT(*) FROM notifications) - ?)
            )",
            params![keep],
        )?;

        Ok(())
    }

    pub fn load_all(&self) -> anyhow::Result<Vec<NewNotification>> {
        let mut stmt = self.db.prepare(
            "SELECT id, app_name, app_icon, summary, body, timeout, actions, hints
             FROM notifications
             ORDER BY rowid DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            let actions_encoded: String = row.get(6)?;
            let actions_bytes = general_purpose::STANDARD.decode(&actions_encoded)
                .map_err(|_e| rusqlite::Error::InvalidColumnType(6, "actions".to_string(), rusqlite::types::Type::Text))?;
            
            // Decode actions - they're stored with length prefixes
            let mut actions = Vec::new();
            let mut cursor = std::io::Cursor::new(&actions_bytes);
            while cursor.position() < actions_bytes.len() as u64 {
                let len = prost::encoding::decode_varint(&mut cursor)
                    .map_err(|_e| rusqlite::Error::InvalidColumnType(6, "actions".to_string(), rusqlite::types::Type::Text))?;
                let pos = cursor.position() as usize;
                let action = Action::decode(&actions_bytes[pos..pos + len as usize])
                    .map_err(|_e| rusqlite::Error::InvalidColumnType(6, "actions".to_string(), rusqlite::types::Type::Text))?;
                cursor.set_position(pos as u64 + len);
                actions.push(action);
            }

            let hints_encoded: String = row.get(7)?;
            let hints = if hints_encoded.is_empty() {
                None
            } else {
                let hints_bytes = general_purpose::STANDARD.decode(&hints_encoded)
                    .map_err(|_e| rusqlite::Error::InvalidColumnType(7, "hints".to_string(), rusqlite::types::Type::Text))?;
                Some(NotificationHints::decode(&hints_bytes[..])
                    .map_err(|_e| rusqlite::Error::InvalidColumnType(7, "hints".to_string(), rusqlite::types::Type::Text))?)
            };

            Ok(NewNotification {
                id: row.get(0)?,
                app_name: row.get(1)?,
                app_icon: row.get::<_, Option<String>>(2)?,
                summary: row.get(3)?,
                body: row.get(4)?,
                timeout: row.get(5)?,
                actions,
                hints,
            })
        })?;

        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn delete(&self, id: u32) -> anyhow::Result<()> {
        self.db
            .execute("DELETE FROM notifications WHERE rowid = ?1", params![id])?;
        Ok(())
    }
}
