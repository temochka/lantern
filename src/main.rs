use actix::{Actor, StreamHandler};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use rusqlite::{params, Connection};
use rusqlite::types::{FromSql, ValueRef, FromSqlResult};
use serde::{Serialize, Deserialize};
use serde_json;
use futures::future::Future;

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

struct LanternConnection {
    db_addr: actix::prelude::Addr<LanternDb>,
}

#[derive(Debug)]
struct Migration {
    timestamp: i64,
    app_version: i64,
    batch_order: i32,
    statement: String,
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum Request {
    Echo { id: String, text: String },
    Query { id: String, query: String },
    Migration { id: String, ddl: String }
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum Response {
    Echo { id: String, text: String },
    Query { id: String, results: serde_json::Value },
    Migration { id: String },
    Error { id: String, text: String },
    ChannelError { message: String },
}

impl Actor for LanternConnection {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<ws::Message, ws::ProtocolError> for LanternConnection {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Ping(msg) => ctx.pong(&msg),
            ws::Message::Text(text) => {
                let res = self.handle_request(&text);
                ctx.text(serde_json::to_string(&res).unwrap())
            },
            ws::Message::Binary(bin) => ctx.binary(bin),
            _ => (),
        }
    }
}

impl LanternConnection {
    fn handle_request(&mut self, text: &String) -> Response {
        let message: serde_json::Result<Request> = serde_json::from_str(text);

        match message {
            Ok(request) => {
                match request {
                    Request::Echo { id, text } => Response::Echo { id: id, text: text },
                    Request::Migration { id, ddl } => {
                        let result = self.db_addr.send(DbQuery { query: ddl }).wait().unwrap();

                        match result {
                            Ok(_) => Response::Migration { id: id },
                            Err(error) => Response::Error { id: id, text: format!("{}", error) }
                        }
                    },
                    Request::Query { id, query } => {
                        let result = self.db_addr.send(DbQuery { query }).wait().unwrap();

                        match result {
                            Ok(result) => Response::Query { id: id, results: result },
                            Err(error) => Response::Error { id: id, text: format!("{}", error) }
                        }
                    }
                }
            }
            Err(_) => Response::ChannelError { message: "Failed to parse request.".to_string() }
        }
    }
}

struct LanternDb {
    connection: Connection
}

struct DbQuery {
    query: String
}

impl actix::Message for DbQuery {
    type Result = Result<serde_json::Value, rusqlite::Error>;
}

impl Actor for LanternDb {
    type Context = actix::prelude::Context<Self>;

    fn started(&mut self, _ctx: &mut Self::Context) {
        self.connection.execute(
            "CREATE TABLE lantern_migrations (
                id              BIGINT PRIMARY KEY,
                app_version     BIGINT NOT NULL DEFAULT 0,
                batch_order     INT NOT NULL DEFAULT 0,
                statement       TEXT NOT NULL
            )",
            params![],
        ).unwrap();

        println!("Connected to the database!")
    }
}

fn parse_row(row: & rusqlite::Row) -> rusqlite::Result<serde_json::Value> {
    row
        .columns()
        .iter()
        .enumerate()
        .map(|(i, col)| Ok((col.name().to_string(), row.get::<_, JsonValue>(i).map(|JsonValue(v)| v)?)))
        .collect::<Result<serde_json::Map<String, serde_json::Value>, _>>()
        .map(|r| serde_json::Value::Object(r))
}

impl actix::Handler<DbQuery> for LanternDb {
    type Result = Result<serde_json::Value, rusqlite::Error>;

    fn handle(&mut self, msg: DbQuery, _ctx: &mut actix::prelude::Context<Self>) -> Self::Result {
        println!("Query received: {}", msg.query);

        let mut stmt = self.connection.prepare(&msg.query)?;
        let results = stmt.query_map(params![], |row| parse_row(row)).and_then(|r| r.collect())?;

        Ok(serde_json::Value::Array(results))
    }
}

#[derive(Clone)]
struct LanternServer {
    db_addr: actix::prelude::Addr<LanternDb>,
}

fn index() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

fn async_api(req: HttpRequest, stream: web::Payload, data: web::Data<LanternServer>) -> Result<HttpResponse, Error> {
    let resp = ws::start(LanternConnection { db_addr: data.db_addr.clone() }, &req, stream);
    println!("{:?}", resp);
    resp
}

fn main() {
    let sys = actix::System::new("Lantern");
    let addr = LanternDb::create(|_| {
        let conn = Connection::open_in_memory().unwrap();
        LanternDb { connection : conn }
    });
    let server = LanternServer { db_addr: addr };

    HttpServer::new(move || {
        App::new()
            .data(server.clone())
            .route("/", web::get().to(index))
            .route("/_api/async", web::get().to(async_api))
    })
        .bind("127.0.0.1:9000")
        .unwrap()
        .start();


    sys.run().unwrap();
}
