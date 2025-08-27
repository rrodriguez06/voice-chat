use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub message_type: MessageType,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum MessageType {
    // Connection messages
    Connect { user_id: Uuid },
    Disconnect { user_id: Uuid },
    
    // Channel messages
    JoinChannel { channel_id: Uuid, user_id: Uuid },
    LeaveChannel { channel_id: Uuid, user_id: Uuid },
    ChannelUserList { channel_id: Uuid, users: Vec<Uuid> },
    
    // Audio messages
    AudioStart { user_id: Uuid, channel_id: Uuid },
    AudioStop { user_id: Uuid, channel_id: Uuid },
    
    // Status messages
    UserStatusUpdate { user_id: Uuid, status: crate::models::user::UserStatus },
    
    // System messages
    Error { message: String },
    Ping,
    Pong,
}

impl Message {
    pub fn new(message_type: MessageType) -> Self {
        Self {
            id: Uuid::new_v4(),
            message_type,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
        }
    }

    pub fn connect(user_id: Uuid) -> Self {
        Self::new(MessageType::Connect { user_id })
    }

    pub fn disconnect(user_id: Uuid) -> Self {
        Self::new(MessageType::Disconnect { user_id })
    }

    pub fn join_channel(channel_id: Uuid, user_id: Uuid) -> Self {
        Self::new(MessageType::JoinChannel { channel_id, user_id })
    }

    pub fn leave_channel(channel_id: Uuid, user_id: Uuid) -> Self {
        Self::new(MessageType::LeaveChannel { channel_id, user_id })
    }

    pub fn channel_user_list(channel_id: Uuid, users: Vec<Uuid>) -> Self {
        Self::new(MessageType::ChannelUserList { channel_id, users })
    }

    pub fn error(message: String) -> Self {
        Self::new(MessageType::Error { message })
    }

    pub fn ping() -> Self {
        Self::new(MessageType::Ping)
    }

    pub fn pong() -> Self {
        Self::new(MessageType::Pong)
    }
}

// Client to server messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", content = "payload")]
pub enum ClientMessage {
    Authenticate { username: String },
    JoinChannel { channel_id: Uuid, password: Option<String> },
    LeaveChannel { channel_id: Uuid },
    SetStatus { status: crate::models::user::UserStatus },
    StartAudio { channel_id: Uuid },
    StopAudio { channel_id: Uuid },
    Ping,
}

// Server to client messages
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "data")]
pub enum ServerMessage {
    Authenticated { user_id: Uuid },
    JoinedChannel { channel_id: Uuid },
    LeftChannel { channel_id: Uuid },
    UserJoined { channel_id: Uuid, user_id: Uuid },
    UserLeft { channel_id: Uuid, user_id: Uuid },
    ChannelUsers { channel_id: Uuid, users: Vec<Uuid> },
    UserStatusChanged { user_id: Uuid, status: crate::models::user::UserStatus },
    AudioStarted { channel_id: Uuid, user_id: Uuid },
    AudioStopped { channel_id: Uuid, user_id: Uuid },
    Error { message: String },
    Pong,
}