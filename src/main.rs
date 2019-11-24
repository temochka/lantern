use actix::*;
use actix::prelude::AsyncContext;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use rusqlite::{Connection};
use serde::{Serialize, Deserialize};
use serde_json;
use std::collections::{HashMap};
use futures::future::Future;

mod db;

#[derive(actix::prelude::Message)]
struct LiveQueryRefresh;

struct LanternConnection {
    db_addr: actix::prelude::Addr<db::LanternDb>,
    live_queries: db::LiveQueries,
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
    ReaderQuery { id: String, query: db::ReaderQuery },
    WriterQuery { id: String, query: db::WriterQuery },
    LiveQuery { id: String, queries: db::LiveQueries },
    Migration { id: String, ddl: String }
}

#[derive(Message)]
#[derive(Serialize)]
#[serde(tag = "type")]
enum Response {
    Echo { id: String, text: String },
    ReaderQuery { id: String, results: serde_json::Value },
    WriterQuery { id: String, results: db::WriterQueryResult },
    LiveQuery { id: String, results: db::LiveResults },
    Migration { id: String },
    Error { id: String, text: String },
    ChannelError { message: String },
}


impl actix::prelude::Handler<Response> for LanternConnection {
    type Result = ();

    fn handle(&mut self, msg: Response, ctx: &mut Self::Context) {
        ctx.text(serde_json::to_string(&msg).unwrap())
    }
}

impl actix::prelude::Handler<LiveQueryRefresh> for LanternConnection {
    type Result = ();

    fn handle(&mut self, _msg: LiveQueryRefresh, ctx: &mut Self::Context) {
        let result = self.db_addr.send(self.live_queries.clone()).wait().unwrap();
        let response = match result {
            Ok(results) => Response::LiveQuery { id: "LiveQuery".to_string(), results: results },
            Err(error) => Response::Error { id: "LiveQuery".to_string(), text: format!("{}", error) }
        };

        ctx.address().do_send(response);
    }
}

impl Actor for LanternConnection {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<ws::Message, ws::ProtocolError> for LanternConnection {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Ping(msg) => ctx.pong(&msg),
            ws::Message::Text(text) => {
                let message: serde_json::Result<Request> = serde_json::from_str(&text[..]);
                let response = match message {
                    Ok(request) => {
                        match request {
                            Request::Echo { id, text } => Response::Echo { id: id, text: text },
                            Request::Migration { id, ddl } => {
                                let result = self.db_addr.send(db::DbMigration { query: ddl }).wait().unwrap();
        
                                match result {
                                    Ok(_) => {
                                        ctx.address().do_send(LiveQueryRefresh {});
                                        Response::Migration { id: id }
                                    },
                                    Err(error) => Response::Error { id: id, text: format!("{}", error) }
                                }
                            },
                            Request::ReaderQuery { id, query } => {
                                let result = self.db_addr.send(query).wait().unwrap();

                                match result {
                                    Ok(result) => Response::ReaderQuery { id: id, results: result },
                                    Err(error) => Response::Error { id: id, text: format!("{}", error) }
                                }
                            },
                            Request::WriterQuery { id, query } => {
                                let result = self.db_addr.send(query).wait().unwrap();

                                match result {
                                    Ok(result) => {
                                        ctx.address().do_send(LiveQueryRefresh {});
                                        Response::WriterQuery { id: id, results: result }
                                    },
                                    Err(error) => Response::Error { id: id, text: format!("{}", error) }
                                }
                            },
                            Request::LiveQuery { id, queries } => {
                                self.live_queries = queries.clone();
        
                                let result = self.db_addr.send(queries).wait().unwrap();
        
                                match result {
                                    Ok(results) => Response::LiveQuery { id: id, results: results },
                                    Err(error) => Response::Error { id: id, text: format!("{}", error) }
                                }
                            }
                        }
                    }
                    Err(_) => Response::ChannelError { message: "Failed to parse request.".to_string() }
                };

                ctx.address().do_send(response);
            },
            _ => (),
        }
    }
}

fn index() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

fn async_api(req: HttpRequest, stream: web::Payload, data: web::Data<GlobalState>) -> Result<HttpResponse, Error> {
    let resp = ws::start(LanternConnection { db_addr: data.db_addr.clone(), live_queries : db::LiveQueries(HashMap::new()) }, &req, stream);
    println!("{:?}", resp);
    resp
}

struct GlobalState {
    db_addr: actix::prelude::Addr<db::LanternDb>,
}

fn main() {
    let sys = actix::System::new("Lantern");
    let db_addr = db::LanternDb::create(|_| {
        let conn = Connection::open_in_memory().unwrap();
        db::LanternDb { connection : conn }
    });
    let global_state = web::Data::new(GlobalState {
        db_addr: db_addr
    });

    HttpServer::new(move || {
        App::new()
            .register_data(global_state.clone())
            .route("/", web::get().to(index))
            .route("/_api/async", web::get().to(async_api))
    })
        .bind("127.0.0.1:9000")
        .unwrap()
        .start();


    sys.run().unwrap();
}
