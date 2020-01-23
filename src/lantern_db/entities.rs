use chrono;

pub struct Session {
    pub id: i64,
    pub session_token: String,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}
