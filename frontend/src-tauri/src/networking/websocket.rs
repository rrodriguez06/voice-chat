use crate::state::AppState;
use anyhow::{Result, Context};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
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
#[serde(tag = "event", content = "data")]
pub enum ServerMessage {
    Authenticated { user_id: Uuid },
    JoinedChannel { channel_id: Uuid },
    LeftChannel { channel_id: Uuid },
    UserJoined { channel_id: Uuid, user_id: Uuid },
    UserLeft { channel_id: Uuid, user_id: Uuid },
    ChannelUsers { channel_id: Uuid, users: Vec<Uuid> },
    UserStatusChanged { user_id: Uuid, status: String },
    AudioStarted { channel_id: Uuid, user_id: Uuid },
    AudioStopped { channel_id: Uuid, user_id: Uuid },
    Error { message: String },
    Pong,
}

/// Gestionnaire de connexion WebSocket
pub struct WebSocketManager {
    app_state: AppState,
    ws_url: String,
    app_handle: Option<AppHandle>,
}

impl WebSocketManager {
    pub fn new(ws_url: &str, app_state: AppState) -> Self {
        Self {
            app_state,
            ws_url: ws_url.to_string(),
            app_handle: None,
        }
    }

    /// Configure l'AppHandle pour l'émission d'événements
    pub fn set_app_handle(&mut self, app_handle: AppHandle) {
        self.app_handle = Some(app_handle);
    }

    /// Démarre la connexion WebSocket
    pub async fn start(&self) -> Result<()> {
        let (ws_stream, _) = connect_async(&self.ws_url).await
            .context("Failed to connect to WebSocket")?;

        println!("WebSocket connected to {}", self.ws_url);

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();

        // Task pour recevoir les messages
        let app_state_clone = self.app_state.clone();
        let app_handle_clone = self.app_handle.clone();
        let receive_task = tokio::spawn(async move {
            while let Some(message) = ws_receiver.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        if let Ok(server_message) = serde_json::from_str::<ServerMessage>(&text) {
                            Self::handle_server_message(server_message, &app_state_clone, &app_handle_clone).await;
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
    async fn handle_server_message(message: ServerMessage, app_state: &AppState, app_handle: &Option<AppHandle>) {
        match message {
            ServerMessage::UserJoined { channel_id, user_id } => {
                println!("✅ WebSocket: User {} joined channel {}", user_id, channel_id);
                
                // Envoyer l'événement au frontend JS
                if let Some(handle) = app_handle {
                    let _ = handle.emit("user-joined", serde_json::json!({
                        "userId": user_id,
                        "channelId": channel_id
                    }));
                }
            }
            
            ServerMessage::UserLeft { channel_id, user_id } => {
                println!("✅ WebSocket: User {} left channel {}", user_id, channel_id);
                
                // Envoyer l'événement au frontend JS
                if let Some(handle) = app_handle {
                    let _ = handle.emit("user-left", serde_json::json!({
                        "userId": user_id,
                        "channelId": channel_id
                    }));
                }
            }

            ServerMessage::ChannelUsers { channel_id, users } => {
                println!("✅ Channel {} users: {:?}", channel_id, users);
                
                // Envoyer la liste complète des utilisateurs au frontend JS
                if let Some(handle) = app_handle {
                    let _ = handle.emit("channel_users", serde_json::json!({
                        "channelId": channel_id,
                        "users": users
                    }));
                }
            }

            ServerMessage::JoinedChannel { channel_id } => {
                println!("✅ Successfully joined channel {}", channel_id);
                app_state.set_current_channel(Some(channel_id));
            }

            ServerMessage::LeftChannel { channel_id } => {
                println!("✅ Successfully left channel {}", channel_id);
                app_state.set_current_channel(None);
            }

            ServerMessage::Error { message } => {
                println!("❌ Server error: {}", message);
                if let Some(handle) = app_handle {
                    let _ = handle.emit("server_error", serde_json::json!({
                        "message": message
                    }));
                }
            }

            _ => {
                println!("📨 Unhandled message: {:?}", message);
            }
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