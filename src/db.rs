use actix::{Actor};
use rusqlite::{params, Connection, OptionalExtension};
use rusqlite::types::{FromSql, ValueRef, FromSqlResult, ToSql};
use serde::{Serialize, Deserialize};
use std::collections::{HashMap};

pub mod entities;
pub mod queries;

#[derive(Serialize)]
struct JsonValue(serde_json::Value);

impl FromSql for JsonValue {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> FromSqlResult<Self> {
        match value {
            ValueRef::Null => Ok(JsonValue(serde_json::Value::Null)),
            ValueRef::Text(s) | ValueRef::Blob(s) => Ok(JsonValue(serde_json::Value::String(std::str::from_utf8(s).unwrap().to_string()))),
            ValueRef::Integer(n) => Ok(JsonValue(serde_json::Value::Number(serde_json::Number::from(n)))),
            ValueRef::Real(n) => Ok(JsonValue(serde_json::Value::Number(serde_json::Number::from_f64(n).unwrap()))),
        }
    }
}

pub struct LanternDb {
    pub connection: Connection
}

impl LanternDb {
    fn run_reader_query(&self, query: &ReaderQuery) -> rusqlite::Result<serde_json::Value> {
        let mut stmt = self.connection.prepare(&query.query)?;
        let results = stmt
            .query_map_named(&self.arguments_to_named_params(&query.arguments)[..], |row| self.parse_row(row))
            .and_then(|r| r.collect())?;

        Ok(serde_json::Value::Array(results))
    }

    fn run_writer_query(&self, query: &WriterQuery) -> rusqlite::Result<WriterQueryResult> {
        let changed_rows = self.connection.execute_named(&query.query, &self.arguments_to_named_params(&query.arguments)[..])?;

        Ok(WriterQueryResult { changed_rows: changed_rows, last_insert_rowid: self.connection.last_insert_rowid() })
    }

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

    fn run_live_queries(&self, LiveQueries(live_queries): &LiveQueries) -> rusqlite::Result<LiveResults> {
        let results: rusqlite::Result<HashMap<_, _>> = live_queries
            .iter()
            .map(|(name,query)| self.run_reader_query(query).map(|r| (name.clone(), r)))
            .collect();
        results.map(|results| LiveResults(results))
    }

    fn parse_row(&self, row: &rusqlite::Row) -> rusqlite::Result<serde_json::Value> {
        row
            .columns()
            .iter()
            .enumerate()
            .map(|(i, col)| Ok((col.name().to_string(), row.get::<_, JsonValue>(i).map(|JsonValue(v)| v)?)))
            .collect::<Result<serde_json::Map<String, serde_json::Value>, _>>()
            .map(|r| serde_json::Value::Object(r))
    }

    fn arguments_to_named_params<'a>(&self, arguments: &'a QueryArguments) -> Vec<(&'a str, &'a dyn ToSql)> {
        arguments.iter().map(|(name, val)| (&name[..], val as &dyn ToSql)).collect()
    }
}

#[derive(Deserialize)]
#[derive(Clone)]
pub struct ReaderQuery {
    pub query: String,
    pub arguments: QueryArguments
}

#[derive(Deserialize)]
#[derive(Clone)]
pub struct WriterQuery {
    pub query: String,
    pub arguments: QueryArguments
}

pub type QueryArguments = HashMap<String, String>;

pub struct DbMigration {
    pub query: String
}

#[derive(Serialize)]
pub struct WriterQueryResult {
    pub changed_rows: usize,
    pub last_insert_rowid: i64,
}

#[derive(Deserialize)]
#[derive(Clone)]
pub struct LiveQueries(pub HashMap<String, ReaderQuery>);

#[derive(Serialize)]
pub struct LiveResults(HashMap<String, serde_json::Value>);

impl actix::Message for ReaderQuery {
    type Result = Result<serde_json::Value, rusqlite::Error>;
}

impl actix::Message for WriterQuery {
    type Result = Result<WriterQueryResult, rusqlite::Error>;
}

impl actix::Message for DbMigration {
    type Result = Result<bool, rusqlite::Error>;
}

impl actix::Message for LiveQueries {
    type Result = Result<LiveResults, rusqlite::Error>;
}

impl actix::Message for LiveResults {
    type Result = Result<(), serde_json::Error>;
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
            "CREATE TABLE lantern_migrations (
                id              INTEGER PRIMARY KEY AUTOINCREMENT,
                app_version     INTEGER NOT NULL DEFAULT 0,
                batch_order     INTEGER NOT NULL DEFAULT 0,
                statement       TEXT NOT NULL
            )",
            params![],
        ).unwrap();

        self.connection.execute(
            "CREATE TABLE lantern_sessions (
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

impl actix::Handler<ReaderQuery> for LanternDb {
    type Result = Result<serde_json::Value, rusqlite::Error>;

    fn handle(&mut self, msg: ReaderQuery, _ctx: &mut actix::prelude::Context<Self>) -> Self::Result {
        println!("Query received: {}", msg.query);

        self.run_reader_query(&msg)
    }
}

impl actix::Handler<WriterQuery> for LanternDb {
    type Result = Result<WriterQueryResult, rusqlite::Error>;

    fn handle(&mut self, msg: WriterQuery, _ctx: &mut actix::prelude::Context<Self>) -> Self::Result {
        println!("Query received: {}", msg.query);

        self.run_writer_query(&msg)
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

impl actix::Handler<DbMigration> for LanternDb {
    type Result = Result<bool, rusqlite::Error>;

    fn handle(&mut self, msg: DbMigration, _ctx: &mut actix::prelude::Context<Self>) -> Self::Result {
        println!("Migration received: {}", msg.query);

        let tx = self.connection.transaction()?;
        tx.execute("INSERT INTO lantern_migrations (statement) VALUES (?)", params![msg.query])?;
        tx.execute(&msg.query[..], params![])?;
        tx.commit()?;

        Ok(true)
    }
}

impl actix::Handler<LiveQueries> for LanternDb {
    type Result = Result<LiveResults, rusqlite::Error>;

    fn handle(&mut self, msg: LiveQueries, _ctx: &mut actix::prelude::Context<Self>) -> Self::Result {
        self.run_live_queries(&msg)
    }
}
