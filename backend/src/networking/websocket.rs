use axum::{
    extract::ws::{WebSocket, WebSocketUpgrade},
    extract::State,
    response::Response,
};
use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};
use uuid::Uuid;

use crate::{
    models::{ClientMessage, ServerMessage},
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
        let (sender, mut receiver) = socket.split();
        let sender = Arc::new(Mutex::new(sender));
        let mut user_id: Option<Uuid> = None;
        let mut broadcast_receiver: Option<broadcast::Receiver<ServerMessage>> = None;

        // Clone sender for broadcast task
        let sender_for_broadcast = sender.clone();

        // Handle incoming messages
        while let Some(msg) = receiver.next().await {
            match msg {
                Ok(axum::extract::ws::Message::Text(text)) => {
                    match serde_json::from_str::<ClientMessage>(&text) {
                        Ok(client_msg) => {
                            match self.handle_client_message(client_msg, &mut user_id, &mut broadcast_receiver).await {
                                Ok(response_msg) => {
                                    if let Some(msg) = response_msg {
                                        if let Ok(msg_text) = serde_json::to_string(&msg) {
                                            let mut sender_guard = sender.lock().await;
                                            let _ = sender_guard.send(axum::extract::ws::Message::Text(msg_text)).await;
                                        }
                                    }
                                    
                                    // If we just authenticated and have a new receiver, start broadcast listener
                                    if let Some(mut receiver) = broadcast_receiver.take() {
                                        let sender_clone = sender_for_broadcast.clone();
                                        tokio::spawn(async move {
                                            while let Ok(broadcast_msg) = receiver.recv().await {
                                                tracing::debug!("Forwarding broadcast message to WebSocket: {:?}", broadcast_msg);
                                                if let Ok(msg_text) = serde_json::to_string(&broadcast_msg) {
                                                    let mut sender_guard = sender_clone.lock().await;
                                                    let _ = sender_guard.send(axum::extract::ws::Message::Text(msg_text)).await;
                                                }
                                            }
                                        });
                                    }
                                }
                                Err(e) => {
                                    tracing::error!("Error handling client message: {}", e);
                                    let error_msg = ServerMessage::Error {
                                        message: e.to_string(),
                                    };
                                    if let Ok(error_text) = serde_json::to_string(&error_msg) {
                                        let mut sender_guard = sender.lock().await;
                                        let _ = sender_guard.send(axum::extract::ws::Message::Text(error_text)).await;
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
                                let mut sender_guard = sender.lock().await;
                                let _ = sender_guard.send(axum::extract::ws::Message::Text(error_text)).await;
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
        broadcast_receiver: &mut Option<broadcast::Receiver<ServerMessage>>,
    ) -> Result<Option<ServerMessage>> {
        match message {
            ClientMessage::Authenticate { username } => {
                // Find existing user by username instead of creating a new one
                let existing_user = self.user_service.get_user_by_username(&username)
                    .map_err(|_| Error::User(format!("User '{}' not found. Please connect via HTTP first.", username)))?;
                
                let uid = existing_user.id;
                *user_id = Some(uid);

                // Create user-specific broadcast channel
                let (user_sender, user_receiver) = broadcast::channel(100);
                self.connections.insert(uid, user_sender);
                *broadcast_receiver = Some(user_receiver);

                tracing::info!("User {} ({}) authenticated via WebSocket", username, uid);
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
            tracing::debug!("Sending message to user {}: {:?}", user_id, message);
            sender.send(message).map_err(|_| Error::Network("Failed to send message".to_string()))?;
        } else {
            tracing::warn!("No connection found for user {}", user_id);
        }
        Ok(())
    }

    pub async fn broadcast_to_channel(
        &self,
        channel_id: Uuid,
        message: ServerMessage,
        exclude_user: Option<Uuid>,
    ) -> Result<()> {
        let users = self.channel_service.get_users_in_channel(&channel_id)?;
        
        tracing::debug!("Broadcasting to channel {}: {:?} to {} users", channel_id, message, users.len());
        
        for user_id in users {
            if Some(user_id) != exclude_user {
                if let Some(sender) = self.connections.get(&user_id) {
                    tracing::debug!("Sending broadcast message to user {}", user_id);
                    let _ = sender.send(message.clone());
                } else {
                    tracing::warn!("No connection found for user {} in channel {}", user_id, channel_id);
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