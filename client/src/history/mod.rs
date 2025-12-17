use crate::dbus::xdg::NotificationData;
use rusqlite::params;
use serde::Serialize;
use std::path::Path;
use zbus::zvariant::Type;

#[derive(Default, PartialEq, Clone, Copy, Type, Serialize)]
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

    pub fn insert(&self, data: &NotificationData) -> anyhow::Result<()> {
        self.db.execute(
            "INSERT INTO notifications (id, app_name, app_icon, timeout, summary, body, actions, hints)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                data.id,
                data.app_name,
                data.app_icon,
                data.timeout,
                data.summary,
                data.body,
                serde_json::to_string(&data.actions)?,
                serde_json::to_string(&data.hints)?
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

    pub fn load_all(&self) -> anyhow::Result<Vec<NotificationData>> {
        let mut stmt = self.db.prepare(
            "SELECT rowid, app_name, app_icon, summary, body, actions, hints
             FROM notifications
             ORDER BY rowid DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(NotificationData {
                id: row.get(0)?,
                app_name: row.get(1)?,
                app_icon: row.get::<_, Option<Box<str>>>(2)?,
                summary: row.get::<_, Box<str>>(3)?,
                body: row.get::<_, Box<str>>(4)?,
                timeout: 0,
                actions: {
                    let json: Box<str> = row.get(5)?;
                    serde_json::from_str(&json).unwrap()
                },
                hints: {
                    let json: Box<str> = row.get(6)?;
                    serde_json::from_str(&json).unwrap()
                },
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
