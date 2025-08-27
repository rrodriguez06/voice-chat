use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    extract::State,
    response::Response,
};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    models::{ClientMessage, ServerMessage, CreateUserRequest},
    services::{UserService, ChannelService, AudioService},
    Error, Result,
};

pub struct WebSocketHandler {
    user_service: Arc<UserService>,
    channel_service: Arc<ChannelService>,
    audio_service: Arc<AudioService>,
    connections: Arc<DashMap<Uuid, broadcast::Sender<ServerMessage>>>,
    global_broadcast: broadcast::Sender<ServerMessage>,
}

impl WebSocketHandler {
    pub fn new(
        user_service: Arc<UserService>,
        channel_service: Arc<ChannelService>,
        audio_service: Arc<AudioService>,
    ) -> Self {
        let (global_broadcast, _) = broadcast::channel(1000);
        
        Self {
            user_service,
            channel_service,
            audio_service,
            connections: Arc::new(DashMap::new()),
            global_broadcast,
        }
    }

    pub async fn handle_upgrade(
        State(handler): State<Arc<Self>>,
        ws: WebSocketUpgrade,
    ) -> Response {
        ws.on_upgrade(move |socket| handler.handle_socket(socket))
    }

    async fn handle_socket(self: Arc<Self>, socket: WebSocket) {
        let (mut sender, mut receiver) = socket.split();
        let mut user_id: Option<Uuid> = None;

        // Handle incoming messages
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(axum::extract::ws::Message::Text(text)) => {
                    match serde_json::from_str::<ClientMessage>(&text) {
                        Ok(client_msg) => {
                            match self.handle_client_message(client_msg, &mut user_id).await {
                                Ok(response_msg) => {
                                    if let Some(msg) = response_msg {
                                        if let Ok(msg_text) = serde_json::to_string(&msg) {
                                            let _ = sender.send(axum::extract::ws::Message::Text(msg_text)).await;
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Error handling client message: {}", e);
                                    let error_msg = ServerMessage::Error {
                                        message: e.to_string(),
                                    };
                                    if let Ok(error_text) = serde_json::to_string(&error_msg) {
                                        let _ = sender.send(axum::extract::ws::Message::Text(error_text)).await;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Invalid message format: {}", e);
                            let error_msg = ServerMessage::Error {
                                message: "Invalid message format".to_string(),
                            };
                            if let Ok(error_text) = serde_json::to_string(&error_msg) {
                                let _ = sender.send(axum::extract::ws::Message::Text(error_text)).await;
                            }
                        }
                    }
                }
                Ok(axum::extract::ws::Message::Close(_)) => break,
                Err(e) => {
                    tracing::error!("WebSocket error: {}", e);
                    break;
                }
                _ => {}
            }
        }

        // Cleanup on disconnect
        if let Some(uid) = user_id {
            self.handle_user_disconnect(uid).await;
        }
    }

    async fn handle_client_message(
        &self,
        message: ClientMessage,
        user_id: &mut Option<Uuid>,
    ) -> Result<Option<ServerMessage>> {
        match message {
            ClientMessage::Authenticate { username } => {
                let user_response = self.user_service.create_user(CreateUserRequest { username })?;
                let uid = user_response.id;
                *user_id = Some(uid);

                // Create user-specific broadcast channel
                let (user_sender, _) = broadcast::channel(100);
                self.connections.insert(uid, user_sender);

                tracing::info!("User {} authenticated", uid);
                Ok(Some(ServerMessage::Authenticated { user_id: uid }))
            }

            ClientMessage::JoinChannel { channel_id, password } => {
                let uid = user_id.ok_or_else(|| Error::User("Not authenticated".to_string()))?;

                let join_request = crate::models::channel::JoinChannelRequest { password };
                
                // Join channel in channel service
                self.channel_service.join_channel(&channel_id, uid, Some(join_request))?;
                
                // Update user's current channel
                self.user_service.user_join_channel(&uid, channel_id)?;

                // Notify other users in channel
                self.broadcast_to_channel(
                    channel_id,
                    ServerMessage::UserJoined { channel_id, user_id: uid },
                    Some(uid)
                ).await?;

                // Send current user list to the new user
                let users = self.channel_service.get_users_in_channel(&channel_id)?;
                self.send_to_user(uid, ServerMessage::ChannelUsers { channel_id, users }).await?;

                tracing::info!("User {} joined channel {}", uid, channel_id);
                Ok(Some(ServerMessage::JoinedChannel { channel_id }))
            }

            ClientMessage::LeaveChannel { channel_id } => {
                let uid = user_id.ok_or_else(|| Error::User("Not authenticated".to_string()))?;

                // Leave channel
                self.channel_service.leave_channel(&channel_id, &uid)?;
                self.user_service.user_leave_channel(&uid)?;

                // Notify other users
                self.broadcast_to_channel(
                    channel_id,
                    ServerMessage::UserLeft { channel_id, user_id: uid },
                    Some(uid)
                ).await?;

                tracing::info!("User {} left channel {}", uid, channel_id);
                Ok(Some(ServerMessage::LeftChannel { channel_id }))
            }

            ClientMessage::SetStatus { status } => {
                let uid = user_id.ok_or_else(|| Error::User("Not authenticated".to_string()))?;
                
                self.user_service.update_user_status(&uid, status.clone())?;
                
                // Broadcast status change
                let _ = self.global_broadcast.send(ServerMessage::UserStatusChanged {
                    user_id: uid,
                    status,
                });
                Ok(None)
            }

            ClientMessage::StartAudio { channel_id } => {
                let uid = user_id.ok_or_else(|| Error::User("Not authenticated".to_string()))?;
                
                // Notify channel users that audio started
                self.broadcast_to_channel(
                    channel_id,
                    ServerMessage::AudioStarted { channel_id, user_id: uid },
                    None
                ).await?;
                Ok(None)
            }

            ClientMessage::StopAudio { channel_id } => {
                let uid = user_id.ok_or_else(|| Error::User("Not authenticated".to_string()))?;
                
                // Notify channel users that audio stopped
                self.broadcast_to_channel(
                    channel_id,
                    ServerMessage::AudioStopped { channel_id, user_id: uid },
                    None
                ).await?;
                Ok(None)
            }

            ClientMessage::Ping => {
                Ok(Some(ServerMessage::Pong))
            }
        }
    }

    async fn send_to_user(&self, user_id: Uuid, message: ServerMessage) -> Result<()> {
        if let Some(sender) = self.connections.get(&user_id) {
            sender.send(message).map_err(|_| Error::Network("Failed to send message".to_string()))?;
        }
        Ok(())
    }

    async fn broadcast_to_channel(
        &self,
        channel_id: Uuid,
        message: ServerMessage,
        exclude_user: Option<Uuid>,
    ) -> Result<()> {
        let users = self.channel_service.get_users_in_channel(&channel_id)?;
        
        for user_id in users {
            if Some(user_id) != exclude_user {
                if let Some(sender) = self.connections.get(&user_id) {
                    let _ = sender.send(message.clone());
                }
            }
        }
        
        Ok(())
    }

    async fn handle_user_disconnect(&self, user_id: Uuid) {
        // Remove from all channels
        self.channel_service.remove_user_from_all_channels(&user_id);
        
        // Remove user
        let _ = self.user_service.remove_user(&user_id);
        
        // Remove connection
        self.connections.remove(&user_id);

        // Broadcast disconnect
        let _ = self.global_broadcast.send(ServerMessage::UserStatusChanged {
            user_id,
            status: crate::models::user::UserStatus::Offline,
        });

        tracing::info!("User {} disconnected", user_id);
    }
}