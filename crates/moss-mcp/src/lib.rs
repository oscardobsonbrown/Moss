use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const MAX_MESSAGE: usize = 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Request {
    pub token: String,
    pub session_id: String,
    pub actor_name: String,
    pub workspace_id: String,
    pub method: String,
    pub args: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    pub result: Value,
}
impl Response {
    pub fn ok(result: Value) -> Self {
        Self { ok: true, result }
    }
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            result: serde_json::json!({"error":message.into()}),
        }
    }
}
