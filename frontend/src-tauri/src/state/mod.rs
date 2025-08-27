use serde::{Deserialize, Serialize};
use std::sync::Arc;
use parking_lot::RwLock;
use uuid::Uuid;

/// État global de l'application frontend
#[derive(Debug, Clone)]
pub struct AppState {
    pub user: Arc<RwLock<Option<UserState>>>,
    pub channels: Arc<RwLock<Vec<ChannelInfo>>>,
    pub current_channel: Arc<RwLock<Option<Uuid>>>,
    pub audio_devices: Arc<RwLock<AudioDevices>>,
    pub connection_state: Arc<RwLock<ConnectionState>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            user: Arc::new(RwLock::new(None)),
            channels: Arc::new(RwLock::new(Vec::new())),
            current_channel: Arc::new(RwLock::new(None)),
            audio_devices: Arc::new(RwLock::new(AudioDevices::default())),
            connection_state: Arc::new(RwLock::new(ConnectionState::Disconnected)),
        }
    }

    /// Définit l'utilisateur connecté
    pub fn set_user(&self, user: UserState) {
        *self.user.write() = Some(user);
    }

    /// Supprime l'utilisateur connecté (déconnexion)
    pub fn clear_user(&self) {
        *self.user.write() = None;
    }

    /// Obtient l'utilisateur connecté
    pub fn get_user(&self) -> Option<UserState> {
        self.user.read().clone()
    }

    /// Met à jour la liste des channels
    pub fn update_channels(&self, channels: Vec<ChannelInfo>) {
        *self.channels.write() = channels;
    }

    /// Obtient la liste des channels
    pub fn get_channels(&self) -> Vec<ChannelInfo> {
        self.channels.read().clone()
    }

    /// Définit le channel actuel
    pub fn set_current_channel(&self, channel_id: Option<Uuid>) {
        *self.current_channel.write() = channel_id;
    }

    /// Obtient le channel actuel
    pub fn get_current_channel(&self) -> Option<Uuid> {
        *self.current_channel.read()
    }

    /// Met à jour l'état de connexion
    pub fn set_connection_state(&self, state: ConnectionState) {
        *self.connection_state.write() = state;
    }

    /// Obtient l'état de connexion
    pub fn get_connection_state(&self) -> ConnectionState {
        *self.connection_state.read()
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

/// Informations de l'utilisateur connecté
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserState {
    pub id: Uuid,
    pub username: String,
    pub connected_at: u64,
}

/// Informations d'un channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelInfo {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "userCount")]
    pub user_count: usize,
    pub users: Vec<UserInfo>,
}

/// Informations d'un utilisateur dans un channel
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// Périphériques audio disponibles
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevices {
    pub input_devices: Vec<AudioDevice>,
    pub output_devices: Vec<AudioDevice>,
    pub selected_input: Option<String>,
    pub selected_output: Option<String>,
}

impl Default for AudioDevices {
    fn default() -> Self {
        Self {
            input_devices: Vec::new(),
            output_devices: Vec::new(),
            selected_input: None,
            selected_output: None,
        }
    }
}

/// Informations d'un périphérique audio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub name: String,
    pub id: String,
    pub is_default: bool,
}

/// État de la connexion au backend
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Error,
}

impl ConnectionState {
    pub fn is_connected(&self) -> bool {
        matches!(self, ConnectionState::Connected)
    }

    pub fn is_connecting(&self) -> bool {
        matches!(self, ConnectionState::Connecting)
    }

    pub fn is_disconnected(&self) -> bool {
        matches!(self, ConnectionState::Disconnected)
    }

    pub fn is_error(&self) -> bool {
        matches!(self, ConnectionState::Error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_creation() {
        let state = AppState::new();
        assert!(state.get_user().is_none());
        assert!(state.get_channels().is_empty());
        assert!(state.get_current_channel().is_none());
        assert!(state.get_connection_state().is_disconnected());
    }

    #[test]
    fn test_user_state_management() {
        let state = AppState::new();
        let user = UserState {
            id: Uuid::new_v4(),
            username: "test_user".to_string(),
            connected_at: 1234567890,
        };

        state.set_user(user.clone());
        let retrieved_user = state.get_user();
        assert!(retrieved_user.is_some());
        assert_eq!(retrieved_user.unwrap().username, user.username);
    }

    #[test]
    fn test_connection_state() {
        let state = ConnectionState::Disconnected;
        assert!(state.is_disconnected());
        assert!(!state.is_connected());

        let state = ConnectionState::Connected;
        assert!(state.is_connected());
        assert!(!state.is_disconnected());
    }
}