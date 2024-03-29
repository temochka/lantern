use actix_web::dev::Payload;
use actix_web::{Error, FromRequest};
use futures::future::{BoxFuture, TryFutureExt};

use crate::lantern_db;
use crate::lantern;

pub enum SessionError {
    InternalError(String),
    AuthenticationError(String),
}

impl Into<Error> for SessionError {
    fn into(self) -> Error {
        match self {
            SessionError::InternalError(msg) => actix_web::error::ErrorInternalServerError(msg),
            SessionError::AuthenticationError(msg) => actix_web::error::ErrorUnauthorized(msg)
        }
    }
}

impl FromRequest for lantern_db::entities::Session {
    type Error = SessionError;
    type Future = BoxFuture<'static, Result<Self, Self::Error>>;

    fn from_request(req: &actix_web::HttpRequest, _payload: &mut Payload) -> Self::Future {
        let data = req.app_data::<actix_web::web::Data<lantern::GlobalState>>().unwrap();
        let session_token = req.cookie("lantern_session").map(|cookie| cookie.value().to_string()).unwrap_or("".to_string());

        if data.skip_auth {
            let started_at = chrono::prelude::Utc::now();
            let expires_at = started_at.checked_add_signed(chrono::Duration::days(1)).unwrap();
            Box::pin(futures::future::ready(Ok(lantern_db::entities::Session { id: 42, session_token: String::from("magic"), started_at: started_at, expires_at: expires_at })))
        } else {
            Box::pin(
                data
                    .lantern_db_addr
                    .send(lantern_db::queries::LookupActiveSession { session_token: session_token, now: chrono::Utc::now() })
                    .map_err(|e| SessionError::InternalError(format!("Internal Server Error: {}", e)))
                    .and_then(|query_result| {
                        futures::future::ready(
                            query_result
                                .map_err(|e| SessionError::InternalError(format!("Query execution error: {}", e)))
                                .and_then(|maybe_session| maybe_session.ok_or(SessionError::AuthenticationError("Authentication required".to_string())))
                        )
                    })
            )
        }
    }
}
