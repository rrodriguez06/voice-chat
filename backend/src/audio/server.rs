use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::{RwLock, mpsc};
use uuid::Uuid;
use bytes::Bytes;
use tracing::{info, error, debug, warn};
use crate::audio::{AudioPacket, AudioThreadPool, AudioProcessingPriority};

/// Configuration du serveur UDP audio
#[derive(Debug, Clone)]
pub struct AudioServerConfig {
    pub bind_address: String,
    pub max_packet_size: usize,
    pub connection_timeout: std::time::Duration,
    pub enable_packet_validation: bool,
    pub max_concurrent_connections: usize,
}

impl Default for AudioServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:7878".to_string(),
            max_packet_size: 1472, // MTU Ethernet - headers
            connection_timeout: std::time::Duration::from_secs(30),
            enable_packet_validation: false, // Désactivé pour simplifier
            max_concurrent_connections: 1000,
        }
    }
}

/// Informations de connexion client
#[derive(Debug, Clone)]
pub struct ClientConnection {
    pub user_id: Uuid,
    pub channel_id: Uuid,
    pub addr: SocketAddr,
    pub last_packet_time: std::time::Instant,
    pub packets_received: u64,
    pub packets_sent: u64,
}

/// Statistiques du serveur UDP
#[derive(Debug, Clone)]
pub struct ServerStats {
    pub active_connections: usize,
    pub total_packets_received: u64,
    pub total_packets_sent: u64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub packet_errors: u64,
    pub uptime: std::time::Duration,
}

/// Serveur UDP pour le streaming audio temps réel
#[derive(Debug)]
pub struct AudioUdpServer {
    config: AudioServerConfig,
    socket: Arc<UdpSocket>,
    connections: Arc<RwLock<HashMap<SocketAddr, ClientConnection>>>,
    thread_pool: Arc<AudioThreadPool>,
    stats: Arc<RwLock<ServerStats>>,
    start_time: std::time::Instant,
    shutdown_sender: Option<mpsc::Sender<()>>,
}

impl AudioUdpServer {
    /// Crée un nouveau serveur UDP audio
    pub async fn new(
        config: AudioServerConfig,
        thread_pool: Arc<AudioThreadPool>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let socket = UdpSocket::bind(&config.bind_address).await?;
        info!("Audio UDP server bound to {}", config.bind_address);

        let stats = ServerStats {
            active_connections: 0,
            total_packets_received: 0,
            total_packets_sent: 0,
            bytes_received: 0,
            bytes_sent: 0,
            packet_errors: 0,
            uptime: std::time::Duration::new(0, 0),
        };

        Ok(Self {
            config,
            socket: Arc::new(socket),
            connections: Arc::new(RwLock::new(HashMap::new())),
            thread_pool,
            stats: Arc::new(RwLock::new(stats)),
            start_time: std::time::Instant::now(),
            shutdown_sender: None,
        })
    }

    /// Démarre le serveur et écoute les packets UDP
    pub async fn start(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (shutdown_sender, mut shutdown_receiver) = mpsc::channel(1);
        self.shutdown_sender = Some(shutdown_sender);

        let socket = self.socket.clone();
        let connections = self.connections.clone();
        let thread_pool = self.thread_pool.clone();
        let stats = self.stats.clone();
        let config = self.config.clone();

        info!("Starting UDP audio server on {}", config.bind_address);

        // Boucle principale du serveur
        loop {
            let mut buffer = vec![0u8; config.max_packet_size];
            
            tokio::select! {
                // Réception de packets UDP
                result = socket.recv_from(&mut buffer) => {
                    match result {
                        Ok((len, addr)) => {
                            let packet_data = buffer[..len].to_vec();
                            
                            // Traiter le packet en arrière-plan
                            let connections_clone = connections.clone();
                            let thread_pool_clone = thread_pool.clone();
                            let stats_clone = stats.clone();
                            let socket_clone = socket.clone();
                            
                            tokio::spawn(async move {
                                Self::handle_packet(
                                    packet_data,
                                    addr,
                                    connections_clone,
                                    thread_pool_clone,
                                    stats_clone,
                                    socket_clone,
                                ).await;
                            });
                        }
                        Err(e) => {
                            error!("Error receiving UDP packet: {}", e);
                            let mut stats_guard = stats.write().await;
                            stats_guard.packet_errors += 1;
                        }
                    }
                }
                
                // Signal d'arrêt
                _ = shutdown_receiver.recv() => {
                    info!("Shutdown signal received, stopping UDP server");
                    break;
                }
                
                // Nettoyage périodique des connexions expirées
                _ = tokio::time::sleep(std::time::Duration::from_secs(10)) => {
                    Self::cleanup_expired_connections(&connections, &config).await;
                }
            }
        }

        info!("UDP audio server stopped");
        Ok(())
    }

    /// Traite un packet UDP reçu
    async fn handle_packet(
        packet_data: Vec<u8>,
        addr: SocketAddr,
        connections: Arc<RwLock<HashMap<SocketAddr, ClientConnection>>>,
        thread_pool: Arc<AudioThreadPool>,
        stats: Arc<RwLock<ServerStats>>,
        socket: Arc<UdpSocket>,
    ) {
        // Mettre à jour les statistiques
        {
            let mut stats_guard = stats.write().await;
            stats_guard.total_packets_received += 1;
            stats_guard.bytes_received += packet_data.len() as u64;
        }

        // Désérialiser le packet audio
        let audio_packet = match AudioPacket::from_bytes(&packet_data) {
            Ok(packet) => packet,
            Err(_) => {
                debug!("Failed to parse audio packet from {}", addr);
                {
                    let mut stats_guard = stats.write().await;
                    stats_guard.packet_errors += 1;
                }
                return;
            }
        };

        // Mettre à jour les informations de connexion
        let channel_id = {
            let mut connections_guard = connections.write().await;
            let connection = connections_guard.entry(addr).or_insert_with(|| {
                ClientConnection {
                    user_id: audio_packet.header.user_id,
                    channel_id: audio_packet.header.channel_id,
                    addr,
                    last_packet_time: std::time::Instant::now(),
                    packets_received: 0,
                    packets_sent: 0,
                }
            });
            
            connection.last_packet_time = std::time::Instant::now();
            connection.packets_received += 1;
            audio_packet.header.channel_id
        };

        // Si c'est un packet audio, le traiter pour mixage
        if audio_packet.has_audio() {
            // Collecter tous les packets pour ce channel dans une fenêtre temporelle
            let packets = vec![audio_packet]; // En production, implémenter une fenêtre de collecte
            
            // Soumettre pour mixage asynchrone
            match thread_pool.submit_mix_task(
                channel_id,
                packets,
                AudioProcessingPriority::Normal,
            ).await {
                Ok(receiver) => {
                    // Attendre le résultat du mixage
                    if let Ok(Some(mixed_audio)) = receiver.await {
                        // Distribuer l'audio mixé aux autres clients du channel
                        Self::distribute_mixed_audio(
                            mixed_audio,
                            channel_id,
                            addr, // Exclure l'expéditeur original
                            connections,
                            socket,
                            stats,
                        ).await;
                    }
                }
                Err(e) => {
                    warn!("Failed to submit mix task: {}", e);
                }
            }
        }
    }

    /// Distribue l'audio mixé aux clients du channel
    async fn distribute_mixed_audio(
        mixed_audio: Bytes,
        channel_id: Uuid,
        sender_addr: SocketAddr,
        connections: Arc<RwLock<HashMap<SocketAddr, ClientConnection>>>,
        socket: Arc<UdpSocket>,
        stats: Arc<RwLock<ServerStats>>,
    ) {
        let connections_guard = connections.read().await;
        
        // Identifier tous les clients du même channel, sauf l'expéditeur
        let recipients: Vec<_> = connections_guard
            .iter()
            .filter(|(addr, conn)| {
                conn.channel_id == channel_id && **addr != sender_addr
            })
            .map(|(addr, _)| *addr)
            .collect();

        drop(connections_guard);

        // Envoyer à tous les destinataires
        for recipient_addr in recipients {
            match socket.send_to(&mixed_audio, recipient_addr).await {
                Ok(bytes_sent) => {
                    // Mettre à jour les statistiques
                    {
                        let mut stats_guard = stats.write().await;
                        stats_guard.total_packets_sent += 1;
                        stats_guard.bytes_sent += bytes_sent as u64;
                    }

                    // Mettre à jour les statistiques de connexion
                    {
                        let mut connections_guard = connections.write().await;
                        if let Some(connection) = connections_guard.get_mut(&recipient_addr) {
                            connection.packets_sent += 1;
                        }
                    }
                }
                Err(e) => {
                    debug!("Failed to send audio to {}: {}", recipient_addr, e);
                    let mut stats_guard = stats.write().await;
                    stats_guard.packet_errors += 1;
                }
            }
        }
    }

    /// Nettoie les connexions expirées
    async fn cleanup_expired_connections(
        connections: &Arc<RwLock<HashMap<SocketAddr, ClientConnection>>>,
        config: &AudioServerConfig,
    ) {
        let mut connections_guard = connections.write().await;
        let now = std::time::Instant::now();
        
        connections_guard.retain(|addr, connection| {
            let elapsed = now.duration_since(connection.last_packet_time);
            let should_keep = elapsed < config.connection_timeout;
            
            if !should_keep {
                debug!("Removing expired connection from {}", addr);
            }
            
            should_keep
        });
    }

    /// Récupère les statistiques du serveur
    pub async fn get_stats(&self) -> ServerStats {
        let mut stats = self.stats.read().await.clone();
        stats.uptime = self.start_time.elapsed();
        stats.active_connections = self.connections.read().await.len();
        stats
    }

    /// Récupère les connexions actives
    pub async fn get_connections(&self) -> Vec<ClientConnection> {
        self.connections.read().await.values().cloned().collect()
    }

    /// Arrête le serveur
    pub async fn shutdown(&mut self) -> Result<(), &'static str> {
        if let Some(sender) = self.shutdown_sender.take() {
            let _ = sender.send(()).await;
            Ok(())
        } else {
            Err("Server is not running")
        }
    }

    /// Force la déconnexion d'un client
    pub async fn disconnect_client(&self, addr: SocketAddr) {
        let mut connections_guard = self.connections.write().await;
        if let Some(connection) = connections_guard.remove(&addr) {
            info!("Forcibly disconnected client {} (user: {})", addr, connection.user_id);
        }
    }

    /// Envoie un packet audio à un client spécifique
    pub async fn send_audio_to_client(
        &self,
        audio_data: Bytes,
        target_addr: SocketAddr,
    ) -> Result<(), std::io::Error> {
        match self.socket.send_to(&audio_data, target_addr).await {
            Ok(bytes_sent) => {
                let mut stats_guard = self.stats.write().await;
                stats_guard.total_packets_sent += 1;
                stats_guard.bytes_sent += bytes_sent as u64;
                Ok(())
            }
            Err(e) => {
                let mut stats_guard = self.stats.write().await;
                stats_guard.packet_errors += 1;
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use crate::audio::{AudioMixer, AudioThreadPoolConfig};

    #[tokio::test]
    async fn test_server_creation() {
        let config = AudioServerConfig {
            bind_address: "127.0.0.1:0".to_string(),
            ..Default::default()
        };
        
        let mixer = Arc::new(RwLock::new(AudioMixer::default()));
        let thread_pool_config = AudioThreadPoolConfig::default();
        let thread_pool = Arc::new(
            crate::audio::AudioThreadPool::new(thread_pool_config, mixer).await
        );
        
        let server = AudioUdpServer::new(config, thread_pool).await;
        
        assert!(server.is_ok());
        
        let server = server.unwrap();
        let stats = server.get_stats().await;
        assert_eq!(stats.active_connections, 0);
        assert_eq!(stats.total_packets_received, 0);
    }

    #[tokio::test]
    async fn test_stats() {
        let config = AudioServerConfig {
            bind_address: "127.0.0.1:0".to_string(),
            ..Default::default()
        };
        
        let mixer = Arc::new(RwLock::new(AudioMixer::default()));
        let thread_pool_config = AudioThreadPoolConfig::default();
        let thread_pool = Arc::new(
            crate::audio::AudioThreadPool::new(thread_pool_config, mixer).await
        );
        
        let server = AudioUdpServer::new(config, thread_pool).await.unwrap();
        
        let stats = server.get_stats().await;
        assert!(stats.uptime.as_secs() >= 0);
        assert_eq!(stats.packet_errors, 0);
    }
}