use chrono;

pub struct CreateSession {
    pub session_token: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

pub struct LookupActiveSession {
    pub session_token: String,
    pub now: chrono::DateTime<chrono::Utc>,
}
