use actix::{Actor};
use rusqlite::{params, Connection, OptionalExtension};

pub mod entities;
pub mod queries;

pub struct LanternDb {
    pub connection: Connection
}

impl LanternDb {
    fn create_session(&self, query: &queries::CreateSession) -> rusqlite::Result<()>
    {
        let mut stmt = self.connection.prepare("INSERT INTO lantern_sessions (session_token, started_at, expires_at) VALUES (?, ?, ?)")?;
        stmt.insert(params![query.session_token.clone(), query.started_at, query.expires_at])?;

        Ok(())
    }

    fn lookup_active_session(&self, query: &queries::LookupActiveSession) -> rusqlite::Result<Option<entities::Session>> {
        let mut stmt = self.connection.prepare("SELECT id, session_token, started_at, expires_at FROM lantern_sessions WHERE session_token=? AND expires_at > ? LIMIT 1")?;
        stmt.query_row(
            params![query.session_token.clone(), query.now],
            |row| Ok(entities::Session { id: row.get(0)?, session_token: row.get(1)?, started_at: row.get(2)?, expires_at: row.get(3)? })
        ).optional()
    }
}

impl actix::Message for queries::CreateSession {
    type Result = rusqlite::Result<()>;
}

impl actix::Message for queries::LookupActiveSession {
    type Result = rusqlite::Result<Option<entities::Session>>;
}

impl Actor for LanternDb {
    type Context = actix::prelude::Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.connection.execute(
            "CREATE TABLE IF NOT EXISTS lantern_sessions (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                session_token   VARCHAR(255) NOT NULL,
                started_at      DATETIME NOT NULL,
                expires_at      DATETIME NOT NULL
            )",
            params![],
        ).unwrap();

        println!("Connected to the database!")
    }
}

impl actix::Handler<queries::CreateSession> for LanternDb {
    type Result = rusqlite::Result<()>;

    fn handle(&mut self, msg: queries::CreateSession, _ctx: &mut actix::prelude::Context<Self>) -> Self::Result {
        self.create_session(&msg)
    }
}

impl actix::Handler<queries::LookupActiveSession> for LanternDb {
    type Result = rusqlite::Result<Option<entities::Session>>;

    fn handle(&mut self, msg: queries::LookupActiveSession, _ctx: &mut actix::prelude::Context<Self>) -> Self::Result {
        self.lookup_active_session(&msg)
    }
}
