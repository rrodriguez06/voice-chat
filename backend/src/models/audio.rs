use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioStatsResponse {
    pub channel_id: Uuid,
    pub connected_users: usize,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
    pub average_latency_ms: f32,
    pub packet_loss_rate: f32,
    pub jitter_ms: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserAudioStatus {
    pub user_id: Uuid,
    pub is_speaking: bool,
    pub volume_level: f32,
    pub last_packet_time: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfigResponse {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: usize,
    pub max_packet_size: usize,
    pub codec: String,
}