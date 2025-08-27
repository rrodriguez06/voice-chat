use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub max_users: usize,
    pub current_users: Vec<Uuid>,
    pub is_private: bool,
    pub password: Option<String>,
    pub created_at: SystemTime,
}

impl Channel {
    pub fn new(
        name: String,
        description: Option<String>,
        owner_id: Uuid,
        max_users: usize,
        is_private: bool,
        password: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            description,
            owner_id,
            max_users,
            current_users: Vec::new(),
            is_private,
            password,
            created_at: SystemTime::now(),
        }
    }

    pub fn can_join(&self, user_id: &Uuid, password: Option<&str>) -> bool {
        // Check if user is already in the channel
        if self.current_users.contains(user_id) {
            return true;
        }

        // Check if channel is full
        if self.current_users.len() >= self.max_users {
            return false;
        }

        // Check password if required
        if let Some(required_password) = &self.password {
            match password {
                Some(provided_password) => required_password == provided_password,
                None => false,
            }
        } else {
            true
        }
    }

    pub fn add_user(&mut self, user_id: Uuid) -> bool {
        if !self.current_users.contains(&user_id) && self.current_users.len() < self.max_users {
            self.current_users.push(user_id);
            true
        } else {
            false
        }
    }

    pub fn remove_user(&mut self, user_id: &Uuid) -> bool {
        if let Some(pos) = self.current_users.iter().position(|id| id == user_id) {
            self.current_users.remove(pos);
            true
        } else {
            false
        }
    }

    pub fn is_owner(&self, user_id: &Uuid) -> bool {
        self.owner_id == *user_id
    }

    pub fn is_empty(&self) -> bool {
        self.current_users.is_empty()
    }

    pub fn user_count(&self) -> usize {
        self.current_users.len()
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateChannelRequest {
    pub name: String,
    pub description: Option<String>,
    pub max_users: Option<usize>,
    pub is_private: Option<bool>,
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct JoinChannelRequest {
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct HttpJoinChannelRequest {
    pub user_id: String,
    pub udp_port: Option<u16>, // Port UDP sur lequel le client écoute
}

#[derive(Debug, Serialize)]
pub struct ChannelResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub max_users: usize,
    pub current_user_count: usize,
    pub is_private: bool,
    pub has_password: bool,
    pub created_at: SystemTime,
}

impl From<Channel> for ChannelResponse {
    fn from(channel: Channel) -> Self {
        Self {
            id: channel.id,
            name: channel.name,
            description: channel.description,
            owner_id: channel.owner_id,
            max_users: channel.max_users,
            current_user_count: channel.current_users.len(),
            is_private: channel.is_private,
            has_password: channel.password.is_some(),
            created_at: channel.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct DetailedChannelResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub max_users: usize,
    pub current_users: Vec<Uuid>,
    pub is_private: bool,
    pub has_password: bool,
    pub created_at: SystemTime,
}

impl From<Channel> for DetailedChannelResponse {
    fn from(channel: Channel) -> Self {
        Self {
            id: channel.id,
            name: channel.name,
            description: channel.description,
            owner_id: channel.owner_id,
            max_users: channel.max_users,
            current_users: channel.current_users,
            is_private: channel.is_private,
            has_password: channel.password.is_some(),
            created_at: channel.created_at,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct EnrichedChannelResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub owner_id: Uuid,
    pub max_users: usize,
    #[serde(rename = "userCount")]
    pub user_count: usize, // Compat frontend
    pub current_users: Vec<Uuid>,
    pub users: Vec<UserInfo>, // Informations enrichies des utilisateurs
    pub is_private: bool,
    pub has_password: bool,
    pub created_at: SystemTime,
}

#[derive(Debug, Serialize)]
pub struct UserInfo {
    pub id: Uuid,
    pub username: String,
    #[serde(rename = "isSpeaking")]
    pub is_speaking: bool,
    #[serde(rename = "micEnabled")]
    pub mic_enabled: bool,
    #[serde(rename = "speakerEnabled")]
    pub speaker_enabled: bool,
}