use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use crate::Result;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub server: ServerConfig,
    pub audio: AudioConfig,
    pub limits: LimitsConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub host: String,
    pub http_port: u16,
    pub websocket_port: u16,
    pub udp_port: u16,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u16,
    pub buffer_size: usize,
    pub max_packet_size: usize,
    /// Mode de test: renvoie l'audio à l'expéditeur pour tester la boucle complète
    pub loopback_mode: bool,
    /// Mode de test: simule un second utilisateur virtuel
    pub virtual_user_mode: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LimitsConfig {
    pub max_users_per_channel: usize,
    pub max_channels: usize,
    pub max_concurrent_connections: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                http_port: 8080,
                websocket_port: 8081,
                udp_port: 8082,
            },
            audio: AudioConfig {
                sample_rate: 48000,
                channels: 1,
                buffer_size: 1024,
                max_packet_size: 1400, // Safe for most networks
                loopback_mode: false,
                virtual_user_mode: false,
            },
            limits: LimitsConfig {
                max_users_per_channel: 10,
                max_channels: 50,
                max_concurrent_connections: 100,
            },
        }
    }
}

impl Config {
    pub fn load() -> Result<Self> {
        Self::load_from("config")
    }

    pub fn load_from(config_name: &str) -> Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(config_name).required(false))
            .add_source(config::Environment::with_prefix("VOICE_CHAT"))
            .build()?;

        match settings.try_deserialize() {
            Ok(config) => Ok(config),
            Err(_) => {
                tracing::warn!("Could not load config file {}, using defaults", config_name);
                Ok(Self::default())
            }
        }
    }

    pub fn http_addr(&self) -> SocketAddr {
        format!("{}:{}", self.server.host, self.server.http_port)
            .parse()
            .expect("Invalid HTTP address")
    }

    pub fn websocket_addr(&self) -> SocketAddr {
        format!("{}:{}", self.server.host, self.server.websocket_port)
            .parse()
            .expect("Invalid WebSocket address")
    }

    pub fn udp_addr(&self) -> SocketAddr {
        format!("{}:{}", self.server.host, self.server.udp_port)
            .parse()
            .expect("Invalid UDP address")
    }
}