//! SQLite storage: Bots, messages, routines. One connection behind a mutex;
//! every call is short.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::Serialize;
use serde_json::Value;
use std::sync::Mutex;

#[derive(Clone, Serialize)]
pub struct Bot {
    pub id: String,
    pub name: String,
    pub color: String,
    pub shape: String,
    pub harness: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    #[serde(skip)]
    pub session_id: Option<String>,
    pub primary: bool,
    pub hidden: bool,
    pub unread: bool,
    pub created_at: i64,
}

#[derive(Clone, Serialize)]
pub struct Message {
    pub id: i64,
    #[serde(rename = "botId")]
    pub bot_id: String,
    /// "user", "bot", or "routine"
    pub role: String,
    pub text: String,
    /// { title, options, picked? }
    pub question: Option<Value>,
    pub error: bool,
    /// "queued" | "running" | "done" for user and routine messages
    #[serde(skip)]
    pub status: Option<String>,
    /// For "peer" and "reply" messages: the other Bot's id.
    #[serde(rename = "fromBot")]
    pub from_bot: Option<String>,
    #[serde(rename = "at")]
    pub created_at: i64,
}

#[derive(Clone, Serialize)]
pub struct Routine {
    pub id: String,
    pub bot_id: String,
    pub name: String,
    pub cron: String,
    pub instruction: String,
    pub enabled: bool,
    pub last_run_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Default)]
pub struct BotPatch {
    pub name: Option<String>,
    pub color: Option<String>,
    pub shape: Option<String>,
    pub harness: Option<String>,
    pub model: Option<Option<String>>,
    pub effort: Option<Option<String>>,
    pub session_id: Option<Option<String>>,
    pub hidden: Option<bool>,
    pub unread: Option<bool>,
}

pub struct Db(Mutex<Connection>);

pub fn now() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn bot_row(r: &Row) -> rusqlite::Result<Bot> {
    Ok(Bot {
        id: r.get("id")?,
        name: r.get("name")?,
        color: r.get("color")?,
        shape: r.get("shape")?,
        harness: r.get("harness")?,
        model: r.get("model")?,
        effort: r.get("effort")?,
        session_id: r.get("session_id")?,
        primary: r.get("primary_bot")?,
        hidden: r.get("hidden")?,
        unread: r.get("unread")?,
        created_at: r.get("created_at")?,
    })
}

fn message_row(r: &Row) -> rusqlite::Result<Message> {
    let q: Option<String> = r.get("question")?;
    Ok(Message {
        id: r.get("id")?,
        bot_id: r.get("bot_id")?,
        role: r.get("role")?,
        text: r.get("text")?,
        question: q.and_then(|s| serde_json::from_str(&s).ok()),
        error: r.get("error")?,
        status: r.get("status")?,
        from_bot: r.get("from_bot")?,
        created_at: r.get("created_at")?,
    })
}

fn routine_row(r: &Row) -> rusqlite::Result<Routine> {
    Ok(Routine {
        id: r.get("id")?,
        bot_id: r.get("bot_id")?,
        name: r.get("name")?,
        cron: r.get("cron")?,
        instruction: r.get("instruction")?,
        enabled: r.get("enabled")?,
        last_run_at: r.get("last_run_at")?,
        created_at: r.get("created_at")?,
    })
}

impl Db {
    pub fn open(path: &std::path::Path) -> rusqlite::Result<Db> {
        let c = Connection::open(path)?;
        c.execute_batch(
            "PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            CREATE TABLE IF NOT EXISTS bots (
              id TEXT PRIMARY KEY,
              name TEXT NOT NULL,
              color TEXT NOT NULL,
              shape TEXT NOT NULL,
              harness TEXT NOT NULL,
              model TEXT,
              session_id TEXT,
              primary_bot INTEGER NOT NULL DEFAULT 0,
              hidden INTEGER NOT NULL DEFAULT 0,
              unread INTEGER NOT NULL DEFAULT 0,
              created_at INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS messages (
              id INTEGER PRIMARY KEY AUTOINCREMENT,
              bot_id TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
              role TEXT NOT NULL,
              text TEXT NOT NULL,
              question TEXT,
              error INTEGER NOT NULL DEFAULT 0,
              status TEXT,
              created_at INTEGER NOT NULL
            );
            CREATE INDEX IF NOT EXISTS messages_bot ON messages(bot_id, id);
            CREATE TABLE IF NOT EXISTS adapter_profiles (
              id TEXT PRIMARY KEY,
              spec TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings (
              key TEXT PRIMARY KEY,
              value TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS routines (
              id TEXT PRIMARY KEY,
              bot_id TEXT NOT NULL REFERENCES bots(id) ON DELETE CASCADE,
              name TEXT NOT NULL,
              cron TEXT NOT NULL,
              instruction TEXT NOT NULL,
              enabled INTEGER NOT NULL DEFAULT 1,
              last_run_at INTEGER,
              created_at INTEGER NOT NULL
            );",
        )?;
        // Added after the first release; fails harmlessly if it already exists.
        let _ = c.execute("ALTER TABLE messages ADD COLUMN from_bot TEXT", []);
        let _ = c.execute("ALTER TABLE bots ADD COLUMN effort TEXT", []);
        // Repair older cross-harness settings: the old UI retained Claude model IDs
        // when the primary bot was switched to Codex.
        let _ = c.execute(
            "UPDATE bots SET model = NULL, effort = NULL WHERE harness = 'codex' AND (model LIKE 'claude-%' OR model IN ('opus','sonnet','haiku','fable'))", []
        );
        Ok(Db(Mutex::new(c)))
    }

    fn c(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    // Bots

    pub fn bots(&self) -> Vec<Bot> {
        let c = self.c();
        let mut s = c.prepare("SELECT * FROM bots ORDER BY primary_bot DESC, created_at").unwrap();
        s.query_map([], bot_row).unwrap().filter_map(Result::ok).collect()
    }

    pub fn bot(&self, id: &str) -> Option<Bot> {
        self.c().query_row("SELECT * FROM bots WHERE id = ?", [id], bot_row).optional().ok().flatten()
    }

    pub fn create_bot(&self, name: &str, color: &str, shape: &str, harness: &str, model: Option<&str>, primary: bool) -> Bot {
        let id = crate::short_id();
        self.c()
            .execute(
                "INSERT INTO bots (id, name, color, shape, harness, model, primary_bot, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
                params![id, name, color, shape, harness, model, primary, now()],
            )
            .unwrap();
        self.bot(&id).unwrap()
    }

    pub fn update_bot(&self, id: &str, p: BotPatch) {
        let c = self.c();
        let set = |col: &str, v: &dyn rusqlite::ToSql| {
            c.execute(&format!("UPDATE bots SET {col} = ? WHERE id = ?"), params![v, id]).unwrap();
        };
        if let Some(v) = p.name { set("name", &v) }
        if let Some(v) = p.color { set("color", &v) }
        if let Some(v) = p.shape { set("shape", &v) }
        if let Some(v) = p.harness { set("harness", &v) }
        if let Some(v) = p.model { set("model", &v) }
        if let Some(v) = p.effort { set("effort", &v) }
        if let Some(v) = p.session_id { set("session_id", &v) }
        if let Some(v) = p.hidden { set("hidden", &v) }
        if let Some(v) = p.unread { set("unread", &v) }
    }

    pub fn setting(&self, key: &str) -> Option<String> {
        self.c().query_row("SELECT value FROM settings WHERE key = ?", [key], |r| r.get(0)).optional().ok().flatten()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> rusqlite::Result<()> {
        self.c().execute("INSERT INTO settings(key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value=excluded.value",
            params![key, value])?;
        Ok(())
    }

    pub fn set_primary(&self, bot_id: &str) -> rusqlite::Result<()> {
        let mut conn = self.c();
        let tx = conn.transaction()?;
        let exists: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM bots WHERE id = ?1)", [bot_id],
            |r| r.get(0))?;
        if !exists { return Err(rusqlite::Error::QueryReturnedNoRows); }
        tx.execute("UPDATE bots SET primary_bot = CASE WHEN id = ?1 THEN 1 ELSE 0 END", [bot_id])?;
        tx.commit()
    }

    pub fn bots_using(&self, harness: &str) -> usize {
        self.c().query_row("SELECT COUNT(*) FROM bots WHERE harness = ?1", [harness], |r| r.get::<_, i64>(0))
            .unwrap_or(0) as usize
    }

    pub fn adapter_specs(&self) -> Vec<crate::adapters::Spec> {
        let c = self.c();
        let mut q = c.prepare("SELECT spec FROM adapter_profiles ORDER BY id").unwrap();
        q.query_map([], |r| r.get::<_, String>(0)).unwrap()
            .filter_map(Result::ok)
            .filter_map(|data| serde_json::from_str(&data).ok())
            .collect()
    }

    pub fn save_adapter(&self, spec: &crate::adapters::Spec) -> rusqlite::Result<()> {
        let data = serde_json::to_string(spec).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.c().execute("INSERT INTO adapter_profiles(id, spec) VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET spec=excluded.spec",
            params![spec.id, data])?;
        Ok(())
    }

    pub fn delete_adapter(&self, id: &str) -> rusqlite::Result<()> {
        self.c().execute("DELETE FROM adapter_profiles WHERE id = ?", [id])?;
        Ok(())
    }

    pub fn delete_bot(&self, id: &str) {
        self.c().execute("DELETE FROM bots WHERE id = ?", [id]).unwrap();
    }

    // Messages

    pub fn messages(&self, bot_id: &str, limit: i64) -> Vec<Message> {
        let c = self.c();
        let mut s = c
            .prepare("SELECT * FROM (SELECT * FROM messages WHERE bot_id = ? ORDER BY id DESC LIMIT ?) ORDER BY id")
            .unwrap();
        s.query_map(params![bot_id, limit], message_row).unwrap().filter_map(Result::ok).collect()
    }

    pub fn last_message(&self, bot_id: &str) -> Option<Message> {
        self.c()
            .query_row(
                "SELECT * FROM messages WHERE bot_id = ? AND role != 'routine' ORDER BY id DESC LIMIT 1",
                [bot_id],
                message_row,
            )
            .optional()
            .ok()
            .flatten()
    }

    pub fn message(&self, id: i64) -> Option<Message> {
        self.c().query_row("SELECT * FROM messages WHERE id = ?", [id], message_row).optional().ok().flatten()
    }

    pub fn add_message(&self, bot_id: &str, role: &str, text: &str, question: Option<&Value>, error: bool, status: Option<&str>) -> Message {
        self.add_message_from(bot_id, role, text, question, error, status, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn add_message_from(&self, bot_id: &str, role: &str, text: &str, question: Option<&Value>, error: bool, status: Option<&str>, from_bot: Option<&str>) -> Message {
        let c = self.c();
        c.execute(
            "INSERT INTO messages (bot_id, role, text, question, error, status, from_bot, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            params![bot_id, role, text, question.map(|q| q.to_string()), error, status, from_bot, now()],
        )
        .unwrap();
        let id = c.last_insert_rowid();
        drop(c);
        self.message(id).unwrap()
    }

    pub fn last_id(&self) -> i64 {
        self.c().query_row("SELECT COALESCE(MAX(id), 0) FROM messages", [], |r| r.get(0)).unwrap_or(0)
    }

    /// The Bot's chat bubbles written after message `after`.
    pub fn replies_after(&self, bot_id: &str, after: i64) -> Vec<Message> {
        let c = self.c();
        let mut s = c.prepare("SELECT * FROM messages WHERE bot_id = ? AND id > ? AND role = 'bot' ORDER BY id").unwrap();
        s.query_map(params![bot_id, after], message_row).unwrap().filter_map(Result::ok).collect()
    }

    pub fn set_status(&self, id: i64, status: &str) {
        self.c().execute("UPDATE messages SET status = ? WHERE id = ?", params![status, id]).unwrap();
    }

    pub fn set_question(&self, id: i64, q: &Value) {
        self.c().execute("UPDATE messages SET question = ? WHERE id = ?", params![q.to_string(), id]).unwrap();
    }

    pub fn pending(&self) -> Vec<Message> {
        let c = self.c();
        let mut s = c.prepare("SELECT * FROM messages WHERE status IN ('queued', 'running') ORDER BY id").unwrap();
        s.query_map([], message_row).unwrap().filter_map(Result::ok).collect()
    }

    // Routines

    pub fn routines(&self, bot_id: Option<&str>) -> Vec<Routine> {
        let c = self.c();
        let mut s = c
            .prepare("SELECT * FROM routines WHERE ?1 IS NULL OR bot_id = ?1 ORDER BY created_at")
            .unwrap();
        s.query_map([bot_id], routine_row).unwrap().filter_map(Result::ok).collect()
    }

    pub fn routine(&self, id: &str) -> Option<Routine> {
        self.c().query_row("SELECT * FROM routines WHERE id = ?", [id], routine_row).optional().ok().flatten()
    }

    pub fn add_routine(&self, bot_id: &str, name: &str, cron: &str, instruction: &str) -> Routine {
        let id = crate::short_id();
        self.c()
            .execute(
                "INSERT INTO routines (id, bot_id, name, cron, instruction, created_at) VALUES (?, ?, ?, ?, ?, ?)",
                params![id, bot_id, name, cron, instruction, now()],
            )
            .unwrap();
        self.routine(&id).unwrap()
    }

    pub fn update_routine(&self, id: &str, name: Option<&str>, cron: Option<&str>, instruction: Option<&str>, enabled: Option<bool>) {
        let c = self.c();
        let set = |col: &str, v: &dyn rusqlite::ToSql| {
            c.execute(&format!("UPDATE routines SET {col} = ? WHERE id = ?"), params![v, id]).unwrap();
        };
        if let Some(v) = name { set("name", &v) }
        if let Some(v) = cron { set("cron", &v) }
        if let Some(v) = instruction { set("instruction", &v) }
        if let Some(v) = enabled { set("enabled", &v) }
    }

    pub fn routine_ran(&self, id: &str, at: i64) {
        self.c().execute("UPDATE routines SET last_run_at = ? WHERE id = ?", params![at, id]).unwrap();
    }

    pub fn delete_routine(&self, id: &str) {
        self.c().execute("DELETE FROM routines WHERE id = ?", [id]).unwrap();
    }
}


#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn adapters_defaults_and_legacy_codex_models_survive_restart() {
        let path = std::env::temp_dir().join(format!("superbot-{}-{}.sqlite",
            std::process::id(), crate::short_id()));
        {
            let db = Db::open(&path).unwrap();
            db.create_bot("Admin", "#ffffff", "waddle", "codex", Some("claude-sonnet-5-5"), true);
            db.set_setting("default_harness", "custom-cli").unwrap();
            let spec: crate::adapters::Spec = serde_json::from_value(serde_json::json!({
                "id": "custom-cli", "name": "Custom CLI", "program": "/bin/cat",
                "stdin": true, "output": "text", "session": "none",
                "model_list": [{"id":"example", "label":"Example"}]
            })).unwrap();
            assert!(spec.validate().is_ok());
            db.save_adapter(&spec).unwrap();
        }
        {
            let db = Db::open(&path).unwrap();
            assert_eq!(db.setting("default_harness").as_deref(), Some("custom-cli"));
            assert_eq!(db.adapter_specs().len(), 1);
            assert_eq!(db.adapter_specs()[0].id, "custom-cli");
            assert_eq!(db.bots()[0].model, None); // migration clears obsolete Claude selections
            let another = db.create_bot("Other", "#ffffff", "waddle", "pi", None, false);
            db.set_primary(&another.id).unwrap();
            assert!(db.bot(&another.id).unwrap().primary);
            assert_eq!(db.bots().iter().filter(|b| b.primary).count(), 1);
        }
        let _ = std::fs::remove_file(&path);
    }
}
