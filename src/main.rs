use actix::{Actor, StreamHandler};
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer, Responder};
use actix_web_actors::ws;
use serde::{Serialize, Deserialize};
use serde_json;

struct MyWs;

#[derive(Deserialize)]
#[serde(tag = "type")]
enum Request {
    Echo { id: String, text: String },
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum Response {
    Echo { id: String, text: String },
    ChannelError { message: String },
}

impl Actor for MyWs {
    type Context = ws::WebsocketContext<Self>;
}

impl StreamHandler<ws::Message, ws::ProtocolError> for MyWs {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        match msg {
            ws::Message::Ping(msg) => ctx.pong(&msg),
            ws::Message::Text(text) => ctx.text(handle_request(&text)),
            ws::Message::Binary(bin) => ctx.binary(bin),
            _ => (),
        }
    }
}

fn handle_request(text: &String) -> String {
    let message: serde_json::Result<Request> = serde_json::from_str(text);
    let response: Response = match message {
        Ok(request) => {
            match request {
                Request::Echo { id, text } => Response::Echo { id: id, text: text },
            }
        }
        Err(_) => Response::ChannelError { message: "Failed to parse request.".to_string() }
    };

    serde_json::to_string(&response).unwrap()
}

fn index() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

fn async_api(req: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    let resp = ws::start(MyWs {}, &req, stream);
    println!("{:?}", resp);
    resp
}

fn main() {
    HttpServer::new(|| {
        App::new()
            .route("/", web::get().to(index))
            .route("/_api/async", web::get().to(async_api))
    })
    .bind("127.0.0.1:9000")
    .unwrap()
    .run()
    .unwrap();
}
