use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    models::{CreateUserRequest, UserResponse, CreateChannelRequest, ChannelResponse, DetailedChannelResponse, EnrichedChannelResponse, AudioStatsResponse, AudioConfigResponse, JoinChannelRequest, HttpJoinChannelRequest},
    services::{UserService, ChannelService, AudioService},
};

pub struct ApiHandlers {
    user_service: Arc<UserService>,
    channel_service: Arc<ChannelService>,
    audio_service: Arc<AudioService>,
}

impl ApiHandlers {
    pub fn new(
        user_service: Arc<UserService>,
        channel_service: Arc<ChannelService>,
        audio_service: Arc<AudioService>,
    ) -> Self {
        Self {
            user_service,
            channel_service,
            audio_service,
        }
    }

    pub async fn create_user(
        State(handlers): State<Arc<Self>>,
        Json(request): Json<CreateUserRequest>,
    ) -> Result<Json<UserResponse>, (StatusCode, String)> {
        match handlers.user_service.create_user(request) {
            Ok(user) => Ok(Json(user)),
            Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
        }
    }

    pub async fn get_user(
        State(handlers): State<Arc<Self>>,
        Path(user_id): Path<Uuid>,
    ) -> Result<Json<UserResponse>, (StatusCode, String)> {
        match handlers.user_service.get_user(&user_id) {
            Ok(user) => Ok(Json(user)),
            Err(e) => Err((StatusCode::NOT_FOUND, e.to_string())),
        }
    }

    pub async fn disconnect_user(
        State(handlers): State<Arc<Self>>,
        Path(user_id): Path<Uuid>,
    ) -> Result<Json<String>, (StatusCode, String)> {
        tracing::info!("🔌 Received disconnect request for user: {}", user_id);
        
        // First leave any channel the user is in
        let _ = handlers.user_service.user_leave_channel(&user_id);
        
        // Then remove the user completely
        match handlers.user_service.remove_user(&user_id) {
            Ok(_) => {
                tracing::info!("✅ User {} disconnected successfully", user_id);
                Ok(Json("User disconnected successfully".to_string()))
            },
            Err(e) => {
                tracing::error!("❌ Failed to disconnect user {}: {}", user_id, e);
                Err((StatusCode::NOT_FOUND, e.to_string()))
            },
        }
    }

    pub async fn list_channels(
        State(handlers): State<Arc<Self>>,
    ) -> Result<Json<Vec<EnrichedChannelResponse>>, (StatusCode, String)> {
        let channels = handlers.channel_service.list_channels();
        let mut enriched_channels = Vec::new();
        
        for channel_response in channels {
            // Get detailed channel to have user list
            if let Ok(detailed_channel) = handlers.channel_service.get_channel(&channel_response.id) {
                let mut users = Vec::new();
                
                // Get user info for each user in channel
                for user_id in &detailed_channel.current_users {
                    if let Ok(user) = handlers.user_service.get_user(user_id) {
                        users.push(crate::models::UserInfo {
                            id: user.id,
                            username: user.username,
                            is_speaking: false, // TODO: Get from audio service
                            mic_enabled: true,  // TODO: Get from audio service
                            speaker_enabled: true, // TODO: Get from audio service
                        });
                    }
                }
                
                enriched_channels.push(crate::models::EnrichedChannelResponse {
                    id: detailed_channel.id,
                    name: detailed_channel.name,
                    description: detailed_channel.description,
                    owner_id: detailed_channel.owner_id,
                    max_users: detailed_channel.max_users,
                    user_count: detailed_channel.current_users.len(), // Frontend compat
                    current_users: detailed_channel.current_users,
                    users,
                    is_private: detailed_channel.is_private,
                    has_password: detailed_channel.has_password,
                    created_at: detailed_channel.created_at,
                });
            }
        }
        
        Ok(Json(enriched_channels))
    }

    pub async fn create_channel(
        State(handlers): State<Arc<Self>>,
        Json(request): Json<CreateChannelRequest>,
    ) -> Result<Json<ChannelResponse>, (StatusCode, String)> {
        // For now, use a dummy owner_id. In a real implementation,
        // this would come from authentication
        let owner_id = Uuid::new_v4();
        
        match handlers.channel_service.create_channel(request, owner_id) {
            Ok(channel) => Ok(Json(channel)),
            Err(e) => Err((StatusCode::BAD_REQUEST, e.to_string())),
        }
    }

    pub async fn get_channel(
        State(handlers): State<Arc<Self>>,
        Path(channel_id): Path<Uuid>,
    ) -> Result<Json<DetailedChannelResponse>, (StatusCode, String)> {
        match handlers.channel_service.get_channel(&channel_id) {
            Ok(channel) => Ok(Json(channel)),
            Err(e) => Err((StatusCode::NOT_FOUND, e.to_string())),
        }
    }

    pub async fn get_audio_stats(
        State(handlers): State<Arc<Self>>,
        Path(channel_id): Path<Uuid>,
    ) -> Result<Json<AudioStatsResponse>, (StatusCode, String)> {
        if let Some(stats) = handlers.audio_service.get_channel_stats(&channel_id) {
            let response = AudioStatsResponse {
                channel_id,
                connected_users: stats.connected_users,
                packets_sent: stats.packets_sent,
                packets_received: stats.packets_received,
                bytes_sent: stats.bytes_sent,
                bytes_received: stats.bytes_received,
                average_latency_ms: stats.average_latency_ms,
                packet_loss_rate: stats.packet_loss_rate,
                jitter_ms: stats.jitter_ms,
            };
            Ok(Json(response))
        } else {
            Err((StatusCode::NOT_FOUND, "Channel not found or no audio statistics available".to_string()))
        }
    }

    pub async fn join_channel(
        State(handlers): State<Arc<Self>>,
        Path(channel_id): Path<Uuid>,
        Json(request): Json<HttpJoinChannelRequest>,
    ) -> Result<Json<()>, (StatusCode, String)> {
        tracing::info!("🏠 Received join channel request: channel={}, user_id={}", channel_id, request.user_id);
        
        // Parse user_id from the request
        let user_id = Uuid::parse_str(&request.user_id)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user_id format".to_string()))?;

        tracing::info!("📝 Parsed user_id: {}", user_id);

        // Create a standard JoinChannelRequest (without password for now)
        let join_request = JoinChannelRequest {
            password: None,
        };

        // Call the channel service to join the channel
        tracing::info!("📞 Calling channel_service.join_channel...");
        match handlers.channel_service.join_channel(&channel_id, user_id, Some(join_request)) {
            Ok(_) => {
                tracing::info!("✅ Successfully joined channel: {} for user: {}", channel_id, user_id);
                
                // Get updated channel info to log
                if let Ok(channel) = handlers.channel_service.get_channel(&channel_id) {
                    tracing::info!("📊 Channel '{}' now has {} users", channel.name, channel.current_users.len());
                }
                
                Ok(Json(()))
            },
            Err(err) => {
                tracing::error!("❌ Failed to join channel: {}", err);
                Err((StatusCode::BAD_REQUEST, err.to_string()))
            },
        }
    }

    pub async fn leave_channel(
        State(handlers): State<Arc<Self>>,
        Path(channel_id): Path<Uuid>,
        Json(request): Json<HttpJoinChannelRequest>,
    ) -> Result<Json<()>, (StatusCode, String)> {
        tracing::info!("🚪 Received leave channel request: channel={}, user_id={}", channel_id, request.user_id);
        
        // Parse user_id from the request
        let user_id = Uuid::parse_str(&request.user_id)
            .map_err(|_| (StatusCode::BAD_REQUEST, "Invalid user_id format".to_string()))?;

        tracing::info!("📝 Parsed user_id: {}", user_id);

        // Call the channel service to leave the channel
        tracing::info!("📞 Calling channel_service.leave_channel...");
        match handlers.channel_service.leave_channel(&channel_id, &user_id) {
            Ok(_) => {
                tracing::info!("✅ Successfully left channel: {} for user: {}", channel_id, user_id);
                
                // Get updated channel info to log
                if let Ok(channel) = handlers.channel_service.get_channel(&channel_id) {
                    tracing::info!("📊 Channel '{}' now has {} users", channel.name, channel.current_users.len());
                }
                
                Ok(Json(()))
            },
            Err(err) => {
                tracing::error!("❌ Failed to leave channel: {}", err);
                Err((StatusCode::BAD_REQUEST, err.to_string()))
            },
        }
    }

    pub async fn get_audio_config(
        State(handlers): State<Arc<Self>>,
    ) -> Result<Json<AudioConfigResponse>, (StatusCode, String)> {
        let response = AudioConfigResponse {
            sample_rate: handlers.audio_service.get_sample_rate(),
            channels: handlers.audio_service.get_channels(),
            buffer_size: handlers.audio_service.get_buffer_size(),
            max_packet_size: handlers.audio_service.get_max_packet_size(),
            codec: "Opus".to_string(), // For now, hardcoded
        };
        Ok(Json(response))
    }
}