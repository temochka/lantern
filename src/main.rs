use actix::*;
use actix::prelude::AsyncContext;
use actix_files as fs;
use actix_web::cookie::Cookie;
use actix_web::{web, error, App, Error, HttpRequest, HttpResponse, HttpMessage, HttpServer};
use actix_web_actors::ws;
use chrono;
use rand::{Rng};
use rand::distributions::Alphanumeric;
use regex::Regex;
use rusqlite::{Connection};
use scrypt::{ScryptParams};
use serde::{Serialize, Deserialize};
use serde_json;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::prelude::*;
use std::iter;
use std::env;
use futures::future::{Future};

mod lantern_db;
mod user_db;

#[derive(actix::prelude::Message)]
struct LiveQueryRefresh;

struct LanternConnection {
    db_addr: actix::prelude::Addr<user_db::UserDb>,
    live_query_response_id: String,
    live_queries: user_db::LiveQueries,
    authenticated: bool,
    root_path: String
}

#[derive(Debug)]
struct Migration {
    timestamp: i64,
    app_version: i64,
    batch_order: i32,
    statement: String,
}

#[derive(Deserialize)]
struct AuthRequest {
    password: String,
}

#[derive(Serialize)]
struct AuthResponse {
    expires_at: String,
}

struct PathPrefixGuard {
    prefix: String,
}

impl actix_web::guard::Guard for PathPrefixGuard {
    fn check(&self, request: &actix_web::dev::RequestHead) -> bool {
        request.uri.path().starts_with(&self.prefix)
    }
}

#[derive(Deserialize)]
#[serde(tag = "type")]
enum WsRequest {
    Nop { id: String },
    Echo { id: String, text: String },
    ReaderQuery { id: String, query: user_db::ReaderQuery },
    WriterQuery { id: String, query: user_db::WriterQuery },
    LiveQuery { id: String, queries: user_db::LiveQueries },
    Migration { id: String, ddl: String }
}

#[derive(Message)]
#[derive(Serialize)]
#[serde(tag = "type")]
enum WsResponse {
    Nop { id: String },
    Hello { id: String },
    FatalError { id: String, error: String, message: String },
    Echo { id: String, text: String },
    ReaderQuery { id: String, results: serde_json::Value },
    WriterQuery { id: String, results: user_db::WriterQueryResult },
    LiveQuery { id: String, results: user_db::LiveResults },
    Migration { id: String },
    Error { id: String, text: String },
    ChannelError { message: String },
}


impl actix::prelude::Handler<WsResponse> for LanternConnection {
    type Result = ();

    fn handle(&mut self, msg: WsResponse, ctx: &mut Self::Context) {
        ctx.text(serde_json::to_string(&msg).unwrap());
        if let WsResponse::FatalError { id: _, error: _, message: _ } = msg { ctx.stop(); }
    }
}

impl actix::prelude::Handler<LiveQueryRefresh> for LanternConnection {
    type Result = ();

    fn handle(&mut self, _msg: LiveQueryRefresh, ctx: &mut Self::Context) {
        let result = self.db_addr.send(self.live_queries.clone()).wait().unwrap();
        let response = match result {
            Ok(results) => WsResponse::LiveQuery { id: self.live_query_response_id.clone(), results: results },
            Err(error) => WsResponse::Error { id: self.live_query_response_id.clone(), text: format!("{}", error) }
        };

        ctx.address().do_send(response);
    }
}

impl Actor for LanternConnection {
    type Context = ws::WebsocketContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        if !self.authenticated {
            ctx.address().do_send(WsResponse::FatalError {
                id: "server_authentication_required".to_string(),
                error: "authentication_required".to_string(),
                message: "Authentication required".to_string(),
            });
        } else {
            ctx.address().do_send(WsResponse::Hello { id: "server_hello".to_string() });
        }
    }
}

impl StreamHandler<ws::Message, ws::ProtocolError> for LanternConnection {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        if !self.authenticated { return; }

        match msg {
            ws::Message::Ping(msg) => ctx.pong(&msg),
            ws::Message::Text(text) => {
                let message: serde_json::Result<WsRequest> = serde_json::from_str(&text[..]);
                let response = match message {
                    Ok(request) => {
                        match request {
                            WsRequest::Nop { id } => WsResponse::Nop { id: id },
                            WsRequest::Echo { id, text } => WsResponse::Echo { id: id, text: text },
                            WsRequest::Migration { id, ddl } => {
                                let migration = user_db::DbMigration::new(ddl);
                                let result =
                                    self
                                        .db_addr
                                        .send(migration.clone())
                                        .wait()
                                        .unwrap()
                                        .map_err(|err| std::io::Error::new(std::io::ErrorKind::Other, format!("{}", err)))
                                        .and_then(|_| {
                                            ctx.address().do_send(LiveQueryRefresh {});
                                            write_migration(std::path::Path::new(&self.root_path), migration)
                                        })
                                        .and_then(|_| {
                                            let schema = self.db_addr.send(user_db::SchemaDump {}).wait().unwrap().unwrap();
                                            write_schema(std::path::Path::new(&self.root_path), schema)
                                        });
        
                                match result {
                                    Ok(_) => WsResponse::Migration { id: id },
                                    Err(error) => WsResponse::Error { id: id, text: format!("{}", error) }
                                }
                            },
                            WsRequest::ReaderQuery { id, query } => {
                                let result = self.db_addr.send(query).wait().unwrap();

                                match result {
                                    Ok(result) => WsResponse::ReaderQuery { id: id, results: result },
                                    Err(error) => WsResponse::Error { id: id, text: format!("{}", error) }
                                }
                            },
                            WsRequest::WriterQuery { id, query } => {
                                let result = self.db_addr.send(query).wait().unwrap();

                                match result {
                                    Ok(result) => {
                                        ctx.address().do_send(LiveQueryRefresh {});
                                        WsResponse::WriterQuery { id: id, results: result }
                                    },
                                    Err(error) => WsResponse::Error { id: id, text: format!("{}", error) }
                                }
                            },
                            WsRequest::LiveQuery { id, queries } => {
                                self.live_queries = queries.clone();
                                self.live_query_response_id = id.clone();
        
                                let result = self.db_addr.send(queries).wait().unwrap();
        
                                match result {
                                    Ok(results) => WsResponse::LiveQuery { id: id, results: results },
                                    Err(error) => WsResponse::Error { id: id, text: format!("{}", error) }
                                }
                            }
                        }
                    }
                    Err(_) => WsResponse::ChannelError { message: "Failed to parse request.".to_string() }
                };

                ctx.address().do_send(response);
            },
            _ => (),
        }
    }
}

fn auth(req: web::Json<AuthRequest>, data: web::Data<GlobalState>) -> actix_web::Result<web::HttpResponse> {
    if is_valid_password(&req.password, &data.password_salt, &data.password_hash) {
        let started_at = chrono::prelude::Utc::now();
        let expires_at = started_at.checked_add_signed(chrono::Duration::days(1)).unwrap();
        let token = random_token(128);
        let cookie = Cookie::build("lantern_session", token.clone())
            .path("/")
            .http_only(true)
            .finish();
        data.lantern_db_addr
            .send(lantern_db::queries::CreateSession { session_token: token, started_at, expires_at })
            .wait()
            .unwrap()
            .map_err(|e| {
                println!("{:?}", e);
                error::ErrorInternalServerError("Failed to start a new session.")
            })?;

        Ok(HttpResponse::Ok()
            .cookie(cookie)
            .json(AuthResponse {
                expires_at: expires_at.to_string()
            })
        )
    } else {
        Err(error::ErrorUnprocessableEntity("Invalid password."))
    }
}

fn ws_api(req: HttpRequest, stream: web::Payload, data: web::Data<GlobalState>) -> Result<HttpResponse, Error> {
    let session_token = req.cookie("lantern_session").map(|cookie| cookie.value().to_string()).unwrap_or("".to_string());
    let session = data.lantern_db_addr
        .send(lantern_db::queries::LookupActiveSession { session_token: session_token, now: chrono::Utc::now() })
        .wait()
        .unwrap()
        .map_err(|original_error| {
            println!("{:?}", original_error);
            error::ErrorInternalServerError("Failed to read session data.")
        })?;

    let resp = ws::start(
        LanternConnection {
            db_addr: data.user_db_addr.clone(),
            live_query_response_id : format!(""),
            live_queries : user_db::LiveQueries(HashMap::new()),
            authenticated: session.is_some(),
            root_path: data.root_path.clone(),
        },
        &req,
        stream
    );
    println!("{:?}", resp);
    resp
}

struct GlobalState {
    lantern_db_addr: actix::prelude::Addr<lantern_db::LanternDb>,
    user_db_addr: actix::prelude::Addr<user_db::UserDb>,
    password_hash: String,
    password_salt: String,
    root_path: String,
}

fn hash_password(password: &str, salt: &str) -> String {
    scrypt::scrypt_simple(&[salt, password].concat(), &ScryptParams::new(10, 8, 1).unwrap()).unwrap()
}

fn is_valid_password(password: &str, salt: &str, password_hash: &str) -> bool {
    scrypt::scrypt_check(&[salt, password].concat(), password_hash).is_ok()
}

fn random_token(length: usize) -> String {
    let mut rng = rand::thread_rng();
    iter::repeat(())
        .map(|_| rng.sample(Alphanumeric))
        .take(length)
        .collect()
}

fn init_lantern(root_path: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(root_path.join(".schema/migrations"))?;
    std::fs::create_dir_all(root_path.join(".lantern"))?;
    Ok(())
}

fn list_migrations(root_path: &std::path::Path) -> std::io::Result<Vec<i64>> {
    let regex = Regex::new(r"^(\d+)\.sql$").unwrap();

    Ok(std::fs::read_dir(root_path.join(".schema/migrations"))?
        .filter_map(|result|
            result
                .ok()
                .and_then(|entry| {
                    let filename = entry.file_name();
                    let filename_string = filename.to_str().unwrap();
                    regex
                        .captures(filename_string)
                        .map(|captures| captures.get(1).unwrap().as_str().parse::<i64>().unwrap())
                })
        )
        .collect()
    )
}

fn read_migration(root_path: &std::path::Path, version: i64) -> std::io::Result<user_db::DbMigration> {
    let sql = std::fs::read_to_string(root_path.join(format!(".schema/migrations/{}.sql", version)))?;

    Ok(user_db::DbMigration { id: version.to_string(), query: sql })
}

fn write_migration(root_path: &std::path::Path, migration: user_db::DbMigration) -> std::io::Result<()> {
    let mut file = File::create(root_path.join(format!(".schema/migrations/{}.sql", migration.id)))?;
    file.write_all(migration.query.as_bytes())?;
    Ok(())
}

fn rusqlite_error_to_io(error: rusqlite::Error) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::Other, format!("{}", error))
}

fn update_db(root_path: &std::path::Path) -> std::io::Result<()> {
    let conn = Connection::open(".lantern/user.sqlite3").map_err(rusqlite_error_to_io)?;
    let mut user_db = user_db::UserDb { connection : conn };
    let is_new_db = user_db.is_new_db().map_err(rusqlite_error_to_io)?;

    if is_new_db {
        let schema = read_schema(root_path.clone())?;
        user_db.load_schema(&schema).map_err(rusqlite_error_to_io)?;
    }

    let applied_migrations = user_db.applied_migrations().map_err(rusqlite_error_to_io)?;
    let max_migration = applied_migrations.iter().cloned().max().unwrap_or(0);
    let applied_migrations_set: HashSet<i64> = applied_migrations.iter().cloned().collect();
    let all_migrations = list_migrations(root_path)?;

    for version in all_migrations {
        let applied = applied_migrations_set.contains(&version);

        if applied {
            continue;
        } else if !applied && is_new_db && version < max_migration {
            user_db.track_migration(version).map_err(rusqlite_error_to_io)?;
        } else {
            user_db.run_migration(&read_migration(root_path, version)?).map_err(rusqlite_error_to_io)?;
        }
    }

    write_schema(root_path, user_db.dump_schema().map_err(rusqlite_error_to_io)?)?;

    Ok(())
}

fn read_schema(root_path: &std::path::Path) -> std::io::Result<String> {
    let full_path = root_path.join(".schema/schema.sql");

    if std::fs::metadata(full_path.clone()).is_ok() {
        std::fs::read_to_string(full_path)
    } else {
        Ok("".to_string())
    }
}

fn write_schema(root_path: &std::path::Path, schema: String) -> std::io::Result<()> {
    let mut file = File::create(root_path.join(".schema/schema.sql"))?;
    file.write_all(schema.as_bytes())?;
    Ok(())
}

fn main() {
    let cli_args: Vec<String> = env::args().collect();
    let path_arg = cli_args.get(1);
    if path_arg.is_none() {
        println!("Lantern is a lightweight web backend for personal productivity apps.\n");
        println!("Docs: https://github.com/temochka/lantern");
        println!("Usage:");
        println!("\tlantern <root>\t- Starts a lantern server in the given directory");
        println!("\tlantern\t\t- Display this message");
        println!("\nEnvironment variables:");
        println!("\tLANTERN_PASSWORD\t- Master authentication password");
        return ();
    }
    let lantern_root_path = std::path::Path::new(&path_arg.unwrap()).canonicalize().unwrap();
    let lantern_root = lantern_root_path.to_str().unwrap().to_string();
    let userdb_path = lantern_root_path.join(".lantern/user.sqlite3");
    let lanterndb_path = lantern_root_path.join(".lantern/lantern.sqlite3");

    init_lantern(lantern_root_path.as_path()).unwrap();
    update_db(lantern_root_path.as_path()).unwrap();
    let sys = actix::System::new("Lantern");
    let user_db_addr = user_db::UserDb::create(|_| {
        let conn = Connection::open(userdb_path).unwrap();
        user_db::UserDb { connection : conn }
    });
    let lantern_db_addr = lantern_db::LanternDb::create(|_| {
        let conn = Connection::open(lanterndb_path).unwrap();
        lantern_db::LanternDb { connection : conn }
    });
    let env_password = env::var("LANTERN_PASSWORD").ok();
    if !env_password.is_some() {
        println!("LANTERN_PASSWORD not set, starting Lantern with a random password.");
    }
    let password = env_password.unwrap_or(random_token(128));
    let salt = random_token(32);
    let global_state = web::Data::new(GlobalState {
        user_db_addr: user_db_addr,
        lantern_db_addr: lantern_db_addr,
        password_hash: hash_password(&password, &salt),
        password_salt: salt,
        root_path: lantern_root.clone(),
    });

    HttpServer::new(move || {
        App::new()
            .register_data(global_state.clone())
            .route("/_api/auth", web::post().to(auth))
            .route("/_api/ws", web::get().to(ws_api))
            .service(
                fs::Files::new("/", lantern_root.clone())
                    .index_file("index.html")
                    .use_guards(PathPrefixGuard { prefix: "/.".to_string() })
            )
    })
        .bind("127.0.0.1:4666")
        .unwrap()
        .start();

    sys.run().unwrap();
}
