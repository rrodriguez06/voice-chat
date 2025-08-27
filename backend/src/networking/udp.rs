use tokio::net::UdpSocket;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::{
    config::Config, 
    audio::{AudioPacket, AudioRouter},
    services::{UserService, ChannelService},
    Result
};

#[derive(Debug)]
pub struct UdpServer {
    config: Config,
    router: Arc<AudioRouter>,
    user_service: Arc<UserService>,
    channel_service: Arc<ChannelService>,
}

impl UdpServer {
    pub fn new(
        config: Config,
        router: Arc<AudioRouter>,
        user_service: Arc<UserService>,
        channel_service: Arc<ChannelService>,
    ) -> Self {
        Self {
            config,
            router,
            user_service,
            channel_service,
        }
    }

    pub async fn start(&self) -> Result<()> {
        let addr = self.config.udp_addr();
        let socket = Arc::new(UdpSocket::bind(addr).await?);
        
        tracing::info!("UDP audio server listening on {}", addr);

        // Canal pour les packets sortants
        let (tx, mut rx) = mpsc::channel::<(AudioPacket, SocketAddr)>(10000);

        // Task pour l'envoi des packets
        let send_socket = socket.clone();
        let send_task = tokio::spawn(async move {
            while let Some((packet, addr)) = rx.recv().await {
                let packet_bytes = packet.to_bytes();
                
                if let Err(e) = send_socket.send_to(&packet_bytes, addr).await {
                    tracing::error!("Failed to send UDP packet to {}: {}", addr, e);
                } else {
                    tracing::trace!("Sent audio packet to {} ({}bytes)", addr, packet_bytes.len());
                }
            }
        });

        // Task principal pour la réception
        let recv_socket = socket.clone();
        let router_clone = self.router.clone();
        let user_service = self.user_service.clone();
        let channel_service = self.channel_service.clone();
        let config_clone = self.config.clone();
        let sender = tx.clone();
        
        let recv_task = tokio::spawn(async move {
            let mut buf = vec![0u8; 4096]; // Buffer agrandi pour les packets audio (au lieu de 2048)
            
            loop {
                match recv_socket.recv_from(&mut buf).await {
                    Ok((size, from_addr)) => {
                        // Traiter le packet reçu
                        if let Err(e) = Self::handle_received_packet(
                            &buf[..size],
                            from_addr,
                            &router_clone,
                            &user_service,
                            &channel_service,
                            &config_clone,
                            &sender,
                        ).await {
                            tracing::warn!("Error processing packet from {}: {}", from_addr, e);
                        }
                    }
                    Err(e) => {
                        tracing::error!("UDP receive error: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                }
            }
        });

        // Task de nettoyage périodique
        let cleanup_router = self.router.clone();
        let cleanup_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(5));
            
            loop {
                interval.tick().await;
                cleanup_router.cleanup_buffers();
                tracing::trace!("Cleaned up audio buffers");
            }
        });

        // Attendre que toutes les tâches se terminent
        tokio::select! {
            _ = recv_task => {
                tracing::error!("UDP receive task terminated");
            }
            _ = send_task => {
                tracing::error!("UDP send task terminated");
            }
            _ = cleanup_task => {
                tracing::error!("UDP cleanup task terminated");
            }
        }

        Ok(())
    }

    async fn handle_received_packet(
        packet_data: &[u8],
        from_addr: SocketAddr,
        router: &Arc<AudioRouter>,
        user_service: &Arc<UserService>,
        channel_service: &Arc<ChannelService>,
        config: &Config,
        sender: &mpsc::Sender<(AudioPacket, SocketAddr)>,
    ) -> Result<()> {
        // Log de réception
        println!("🎵 UdpServer: Received {} bytes from {}", packet_data.len(), from_addr);
        
        // Désérialiser le packet
        let packet = match AudioPacket::from_bytes(packet_data) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("❌ Invalid audio packet from {}: {}", from_addr, e);
                return Ok(());
            }
        };

        println!("🎵 UdpServer: Audio packet - User: {}, Channel: {}, Type: {:?}, Seq: {}, Payload len: {}", 
            packet.header.user_id,
            packet.header.channel_id,
            packet.header.packet_type,
            packet.header.sequence,
            packet.payload.len()
        );

        // Vérifier que l'utilisateur existe et est dans le bon channel
        let user_id = packet.header.user_id;
        let channel_id = packet.header.channel_id;

        // Vérifier l'utilisateur
        if user_service.get_user(&user_id).is_err() {
            tracing::warn!("❌ Received packet from unknown user {}", user_id);
            return Ok(());
        }

        // Vérifier que l'utilisateur est dans le channel
        let users_in_channel = match channel_service.get_users_in_channel(&channel_id) {
            Ok(users) => users,
            Err(_) => {
                tracing::warn!("❌ Received packet for unknown channel {}", channel_id);
                return Ok(());
            }
        };

        if !users_in_channel.contains(&user_id) {
            tracing::warn!("User {} not in channel {}", user_id, channel_id);
            return Ok(());
        }

        // Enregistrer l'adresse du client s'il n'est pas déjà enregistré
        router.register_client(user_id, from_addr);

        // Traiter le packet selon son type
        match packet.header.packet_type {
            crate::audio::PacketType::Audio => {
                // Mode loopback pour test local
                if config.audio.loopback_mode {
                    println!("🔄 UdpServer: Loopback mode - returning audio to sender");
                    // En mode loopback, envoyer au port de playback fixe (8083)
                    let playback_addr = std::net::SocketAddr::new(from_addr.ip(), 8083);
                    if let Err(e) = sender.send((packet.clone(), playback_addr)).await {
                        tracing::error!("Failed to send loopback packet: {}", e);
                    }
                    return Ok(());
                }

                // Router vers les autres utilisateurs du channel
                if router.receive_packet(packet.clone(), from_addr) {
                    let targets = router.route_to_channel(packet.clone());
                    
                    // Envoyer aux clients cibles
                    for target_addr in targets {
                        // Envoyer le packet original à chaque destination
                        if let Err(e) = sender.send((packet.clone(), target_addr)).await {
                            tracing::error!("Failed to queue packet for {}: {}", target_addr, e);
                        }
                    }
                }
            }

            crate::audio::PacketType::AudioStart => {
                tracing::info!("User {} started audio in channel {}", user_id, channel_id);
                // Notifier les autres utilisateurs via WebSocket si nécessaire
                let targets = router.route_to_channel(packet);
                for target_addr in targets {
                    // Envoyer le packet de notification
                    if let Err(e) = sender.send((
                        crate::audio::AudioPacket::audio_start(user_id, channel_id, 0),
                        target_addr
                    )).await {
                        tracing::error!("Failed to send audio start notification: {}", e);
                    }
                }
            }

            crate::audio::PacketType::AudioStop => {
                tracing::info!("User {} stopped audio in channel {}", user_id, channel_id);
                let targets = router.route_to_channel(packet);
                for target_addr in targets {
                    if let Err(e) = sender.send((
                        crate::audio::AudioPacket::audio_stop(user_id, channel_id, 0),
                        target_addr
                    )).await {
                        tracing::error!("Failed to send audio stop notification: {}", e);
                    }
                }
            }

            crate::audio::PacketType::Silence => {
                // Les packets de silence ne sont généralement pas routés
                tracing::trace!("Received silence packet from {}", user_id);
            }

            crate::audio::PacketType::Sync => {
                // Répondre avec un packet de sync pour la latence
                if let Err(e) = sender.send((packet, from_addr)).await {
                    tracing::error!("Failed to send sync response: {}", e);
                }
            }
        }

        Ok(())
    }

    /// Ajoute un utilisateur au routage audio
    pub fn add_user_to_channel(&self, user_id: Uuid, channel_id: Uuid) {
        self.router.add_user_to_channel(user_id, channel_id);
    }

    /// Supprime un utilisateur du routage audio
    pub fn remove_user_from_channel(&self, user_id: &Uuid, channel_id: &Uuid) {
        self.router.remove_user_from_channel(user_id, channel_id);
    }

    /// Supprime complètement un utilisateur
    pub fn remove_user(&self, user_id: &Uuid) {
        self.router.unregister_client(user_id);
    }

    /// Récupère les statistiques du serveur
    pub fn get_stats(&self, channel_id: &Uuid) -> Option<crate::audio::router::RoutingStats> {
        self.router.get_channel_stats(channel_id)
    }
}