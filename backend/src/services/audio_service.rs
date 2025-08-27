use std::sync::Arc;
use crate::{
    config::AudioConfig,
    audio::AudioRouter,
    networking::UdpServer,
    services::{UserService, ChannelService},
};

#[derive(Debug)]
pub struct AudioService {
    config: AudioConfig,
    router: Arc<AudioRouter>,
    udp_server: Option<Arc<UdpServer>>,
}

impl AudioService {
    pub fn new(config: AudioConfig) -> Self {
        Self {
            config,
            router: Arc::new(AudioRouter::new()),
            udp_server: None,
        }
    }

    pub fn with_services(
        config: AudioConfig,
        _user_service: Arc<UserService>,
        _channel_service: Arc<ChannelService>,
    ) -> Self {
        let router = Arc::new(AudioRouter::new());
        
        Self {
            config,
            router,
            udp_server: None,
        }
    }

    /// Récupère le routeur audio
    pub fn router(&self) -> Arc<AudioRouter> {
        self.router.clone()
    }

    /// Démarre le serveur UDP audio
    pub async fn start_udp_server(
        &mut self,
        config: crate::config::Config,
        user_service: Arc<UserService>,
        channel_service: Arc<ChannelService>,
    ) -> crate::Result<()> {
        let udp_server = Arc::new(UdpServer::new(
            config,
            self.router.clone(),
            user_service,
            channel_service,
        ));

        self.udp_server = Some(udp_server.clone());

        // Démarrer le serveur UDP en arrière-plan
        let server = udp_server.clone();
        tokio::spawn(async move {
            if let Err(e) = server.start().await {
                tracing::error!("UDP server error: {}", e);
            }
        });

        tracing::info!("Audio service UDP server started");
        Ok(())
    }

    /// Ajoute un utilisateur à un channel audio
    pub fn add_user_to_channel(&self, user_id: uuid::Uuid, channel_id: uuid::Uuid) {
        self.router.add_user_to_channel(user_id, channel_id);
        
        if let Some(ref udp_server) = self.udp_server {
            udp_server.add_user_to_channel(user_id, channel_id);
        }
    }

    /// Supprime un utilisateur d'un channel audio
    pub fn remove_user_from_channel(&self, user_id: &uuid::Uuid, channel_id: &uuid::Uuid) {
        self.router.remove_user_from_channel(user_id, channel_id);
        
        if let Some(ref udp_server) = self.udp_server {
            udp_server.remove_user_from_channel(user_id, channel_id);
        }
    }

    /// Supprime complètement un utilisateur
    pub fn remove_user(&self, user_id: &uuid::Uuid) {
        self.router.unregister_client(user_id);
        
        if let Some(ref udp_server) = self.udp_server {
            udp_server.remove_user(user_id);
        }
    }

    /// Ajuste la latence pour un channel
    pub fn adjust_channel_latency(&self, channel_id: &uuid::Uuid, latency_ms: u64) {
        self.router.adjust_channel_latency(channel_id, latency_ms);
    }

    /// Récupère les statistiques d'un channel
    pub fn get_channel_stats(&self, channel_id: &uuid::Uuid) -> Option<crate::audio::router::RoutingStats> {
        if let Some(ref udp_server) = self.udp_server {
            udp_server.get_stats(channel_id)
        } else {
            self.router.get_channel_stats(channel_id)
        }
    }

    // Méthodes de configuration existantes
    pub fn get_sample_rate(&self) -> u32 {
        self.config.sample_rate
    }

    pub fn get_channels(&self) -> u16 {
        self.config.channels
    }

    pub fn get_buffer_size(&self) -> usize {
        self.config.buffer_size
    }

    pub fn get_max_packet_size(&self) -> usize {
        self.config.max_packet_size
    }
}