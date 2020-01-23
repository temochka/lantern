use actix::*;
use actix::prelude::AsyncContext;
use actix_files as fs;
use actix_web::cookie::Cookie;
use actix_web::{web, error, App, Error, HttpRequest, HttpResponse, HttpMessage, HttpServer};
use actix_web_actors::ws;
use chrono;
use rand::{Rng};
use rand::distributions::Alphanumeric;
use rusqlite::{Connection};
use scrypt::{ScryptParams};
use serde::{Serialize, Deserialize};
use serde_json;
use std::collections::{HashMap};
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
                                let result = self.db_addr.send(user_db::DbMigration { query: ddl }).wait().unwrap();
        
                                match result {
                                    Ok(_) => {
                                        ctx.address().do_send(LiveQueryRefresh {});
                                        WsResponse::Migration { id: id }
                                    },
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
                expires_at: "2020-04-01 00:00:00".to_string()
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

fn init_lantern() -> std::io::Result<()> {
    std::fs::create_dir_all(".lantern/migrations")?;
    Ok(())
}

fn main() {
    init_lantern().unwrap();
    let sys = actix::System::new("Lantern");
    let user_db_addr = user_db::UserDb::create(|_| {
        let conn = Connection::open(".lantern/user.sqlite3").unwrap();
        user_db::UserDb { connection : conn }
    });
    let lantern_db_addr = lantern_db::LanternDb::create(|_| {
        let conn = Connection::open(".lantern/lantern.sqlite3").unwrap();
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
    });

    HttpServer::new(move || {
        App::new()
            .register_data(global_state.clone())
            .route("/_api/auth", web::post().to(auth))
            .route("/_api/ws", web::get().to(ws_api))
            .service(fs::Files::new("/", ".").index_file("index.html"))
    })
        .bind("127.0.0.1:4666")
        .unwrap()
        .start();

    sys.run().unwrap();
}
