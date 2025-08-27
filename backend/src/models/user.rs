use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub username: String,
    pub status: UserStatus,
    pub current_channel: Option<Uuid>,
    pub created_at: SystemTime,
    pub last_seen: SystemTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum UserStatus {
    Online,
    Away,
    InChannel,
    Offline,
}

impl User {
    pub fn new(username: String) -> Self {
        let now = SystemTime::now();
        Self {
            id: Uuid::new_v4(),
            username,
            status: UserStatus::Online,
            current_channel: None,
            created_at: now,
            last_seen: now,
        }
    }

    pub fn join_channel(&mut self, channel_id: Uuid) {
        self.current_channel = Some(channel_id);
        self.status = UserStatus::InChannel;
        self.update_last_seen();
    }

    pub fn leave_channel(&mut self) {
        self.current_channel = None;
        self.status = UserStatus::Online;
        self.update_last_seen();
    }

    pub fn update_last_seen(&mut self) {
        self.last_seen = SystemTime::now();
    }

    pub fn set_status(&mut self, status: UserStatus) {
        self.status = status;
        self.update_last_seen();
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub status: UserStatus,
    pub current_channel: Option<Uuid>,
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            username: user.username,
            status: user.status,
            current_channel: user.current_channel,
        }
    }
}