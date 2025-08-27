use crate::state::{AppState, UserInfo};
use anyhow::{Result, Context};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use uuid::Uuid;

/// Messages WebSocket envoyés au backend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WebSocketMessage {
    JoinChannel { 
        channel_id: Uuid, 
        user_id: Uuid 
    },
    LeaveChannel { 
        channel_id: Uuid 
    },
    UserSpeaking { 
        is_speaking: bool 
    },
    UserMuted { 
        is_muted: bool 
    },
}

/// Messages WebSocket reçus du backend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMessage {
    UserJoined { 
        user: UserInfo,
        channel_id: Uuid 
    },
    UserLeft { 
        user_id: Uuid,
        channel_id: Uuid 
    },
    UserSpeaking { 
        user_id: Uuid,
        is_speaking: bool 
    },
    UserMuted { 
        user_id: Uuid,
        is_muted: bool 
    },
    ChannelUpdate { 
        channel_id: Uuid,
        users: Vec<UserInfo> 
    },
    Error { 
        message: String 
    },
}

/// Gestionnaire de connexion WebSocket
pub struct WebSocketManager {
    app_state: AppState,
    ws_url: String,
}

impl WebSocketManager {
    pub fn new(ws_url: &str, app_state: AppState) -> Self {
        Self {
            app_state,
            ws_url: ws_url.to_string(),
        }
    }

    /// Démarre la connexion WebSocket
    pub async fn start(&self) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.ws_url).await
            .context("Failed to connect to WebSocket")?;

        println!("WebSocket connected to {}", self.ws_url);

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Task pour recevoir les messages
        let app_state_clone = self.app_state.clone();
        let receive_task = tokio::spawn(async move {
            while let Some(message) = ws_receiver.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        if let Ok(server_message) = serde_json::from_str::<ServerMessage>(&text) {
                            Self::handle_server_message(server_message, &app_state_clone).await;
                        } else {
                            eprintln!("Failed to parse server message: {}", text);
                        }
                    }
                    Ok(Message::Binary(_)) => {
                        // Gérer les messages binaires si nécessaire
                    }
                    Ok(Message::Close(_)) => {
                        println!("WebSocket connection closed by server");
                        break;
                    }
                    Err(e) => {
                        eprintln!("WebSocket receive error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Pour l'instant, on attend juste que la task se termine
        // Dans une vraie app, on aurait un système de channels pour envoyer des messages
        let _ = receive_task.await;

        Ok(())
    }

    /// Envoie un message via WebSocket
    pub async fn send_message(&self, message: WebSocketMessage) -> Result<()> {
        // Pour l'instant, cette fonction est un stub
        // Dans une vraie implémentation, on aurait une référence au sender WebSocket
        let json = serde_json::to_string(&message)
            .context("Failed to serialize WebSocket message")?;
        
        println!("Would send WebSocket message: {}", json);
        Ok(())
    }

    /// Gère les messages reçus du serveur
    async fn handle_server_message(message: ServerMessage, app_state: &AppState) {
        match message {
            ServerMessage::UserJoined { user, channel_id } => {
                println!("User {} joined channel {}", user.username, channel_id);
                
                // Mettre à jour la liste des utilisateurs du channel
                if let Some(current_channel) = app_state.get_current_channel() {
                    if current_channel == channel_id {
                        // Mettre à jour l'état du channel avec le nouvel utilisateur
                        Self::update_channel_users(app_state, channel_id, |users| {
                            users.push(user);
                        });
                    }
                }
            }
            
            ServerMessage::UserLeft { user_id, channel_id } => {
                println!("User {} left channel {}", user_id, channel_id);
                
                // Retirer l'utilisateur de la liste
                if let Some(current_channel) = app_state.get_current_channel() {
                    if current_channel == channel_id {
                        Self::update_channel_users(app_state, channel_id, |users| {
                            users.retain(|u| u.id != user_id);
                        });
                    }
                }
            }
            
            ServerMessage::UserSpeaking { user_id, is_speaking } => {
                // Mettre à jour l'état de parole de l'utilisateur
                if let Some(current_channel) = app_state.get_current_channel() {
                    Self::update_channel_users(app_state, current_channel, |users| {
                        if let Some(user) = users.iter_mut().find(|u| u.id == user_id) {
                            user.is_speaking = is_speaking;
                        }
                    });
                }
            }
            
            ServerMessage::UserMuted { user_id, is_muted } => {
                // Mettre à jour l'état de mute de l'utilisateur
                if let Some(current_channel) = app_state.get_current_channel() {
                    Self::update_channel_users(app_state, current_channel, |users| {
                        if let Some(user) = users.iter_mut().find(|u| u.id == user_id) {
                            user.mic_enabled = !is_muted;
                        }
                    });
                }
            }
            
            ServerMessage::ChannelUpdate { channel_id, users } => {
                // Mise à jour complète des utilisateurs du channel
                Self::update_channel_users(app_state, channel_id, |channel_users| {
                    *channel_users = users;
                });
            }
            
            ServerMessage::Error { message } => {
                eprintln!("Server error: {}", message);
            }
        }
    }

    /// Helper pour mettre à jour les utilisateurs d'un channel
    fn update_channel_users<F>(app_state: &AppState, channel_id: Uuid, update_fn: F)
    where
        F: FnOnce(&mut Vec<UserInfo>),
    {
        let mut channels = app_state.channels.write();
        if let Some(channel) = channels.iter_mut().find(|c| c.id == channel_id) {
            update_fn(&mut channel.users);
            channel.user_count = channel.users.len();
        }
    }

    /// Rejoint un channel via WebSocket
    pub async fn join_channel(&self, channel_id: Uuid) -> Result<()> {
        if let Some(user) = self.app_state.get_user() {
            let message = WebSocketMessage::JoinChannel {
                channel_id,
                user_id: user.id,
            };
            self.send_message(message).await
        } else {
            anyhow::bail!("No user connected")
        }
    }

    /// Quitte un channel via WebSocket
    pub async fn leave_channel(&self, channel_id: Uuid) -> Result<()> {
        let message = WebSocketMessage::LeaveChannel { channel_id };
        self.send_message(message).await
    }

    /// Signale que l'utilisateur parle
    pub async fn set_speaking(&self, is_speaking: bool) -> Result<()> {
        let message = WebSocketMessage::UserSpeaking { is_speaking };
        self.send_message(message).await
    }

    /// Signale que l'utilisateur est muté
    pub async fn set_muted(&self, is_muted: bool) -> Result<()> {
        let message = WebSocketMessage::UserMuted { is_muted };
        self.send_message(message).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::AppState;

    #[test]
    fn test_websocket_manager_creation() {
        let state = AppState::new();
        let manager = WebSocketManager::new("ws://localhost:3000/ws", state);
        assert_eq!(manager.ws_url, "ws://localhost:3000/ws");
    }

    #[test]
    fn test_websocket_message_serialization() {
        let message = WebSocketMessage::JoinChannel {
            channel_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
        };
        
        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("JoinChannel"));
    }

    #[test]
    fn test_server_message_deserialization() {
        let json = r#"{"type":"Error","message":"Test error"}"#;
        let message: ServerMessage = serde_json::from_str(json).unwrap();
        
        match message {
            ServerMessage::Error { message } => {
                assert_eq!(message, "Test error");
            }
            _ => panic!("Wrong message type"),
        }
    }
}