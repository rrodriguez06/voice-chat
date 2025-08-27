use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use uuid::Uuid;
use crate::audio::{AudioPacket, AudioBuffer};

/// Configuration de routage pour un channel
#[derive(Debug, Clone)]
pub struct ChannelRoutingConfig {
    pub max_users: usize,
    pub quality_mode: QualityMode,
    pub latency_target_ms: u64,
    pub enable_echo_cancellation: bool,
    pub enable_noise_suppression: bool,
    pub bitrate_kbps: u32,
}

/// Mode de qualité audio
#[derive(Debug, Clone, PartialEq)]
pub enum QualityMode {
    Low,      // Optimisé pour bande passante
    Medium,   // Équilibré
    High,     // Optimisé pour qualité
    Adaptive, // S'adapte automatiquement
}

/// Statistiques de routage pour un channel
#[derive(Debug, Clone)]
pub struct RoutingStats {
    pub packets_received: u64,
    pub packets_routed: u64,
    pub packets_sent: u64,
    pub bytes_received: u64,
    pub bytes_sent: u64,
    pub active_users: usize,
    pub connected_users: usize,
    pub average_latency_ms: f32,
    pub packet_loss_rate: f32,
    pub jitter_ms: f32,
    pub created_at: std::time::Instant,
}

/// Rapport de performance d'un channel
#[derive(Debug, Clone)]
pub struct ChannelPerformanceReport {
    pub channel_id: Uuid,
    pub quality_score: f32,
    pub packet_loss_rate: f32,
    pub average_latency_ms: f32,
    pub jitter_ms: f32,
    pub connected_users: usize,
    pub active_users: usize,
    pub recommendations: Vec<String>,
    pub timestamp: std::time::Instant,
}

/// Routeur audio pour diriger les packets entre utilisateurs
#[derive(Debug)]
pub struct AudioRouter {
    /// Buffers par utilisateur dans chaque channel
    user_buffers: DashMap<(Uuid, Uuid), AudioBuffer>, // (user_id, channel_id) -> buffer
    /// Adresses UDP des clients connectés
    client_addresses: DashMap<Uuid, SocketAddr>,
    /// Statistiques de routage
    stats: Arc<DashMap<Uuid, RoutingStats>>, // channel_id -> stats
    /// Configuration de routage par channel
    channel_configs: DashMap<Uuid, ChannelRoutingConfig>,
    /// Synchronisation inter-canaux
    channel_sync: DashMap<Uuid, u64>, // channel_id -> dernier timestamp traité
}

impl AudioRouter {
    pub fn new() -> Self {
        Self {
            user_buffers: DashMap::new(),
            client_addresses: DashMap::new(),
            stats: Arc::new(DashMap::new()),
            channel_configs: DashMap::new(),
            channel_sync: DashMap::new(),
        }
    }

    /// Enregistre l'adresse d'un client
    pub fn register_client(&self, user_id: Uuid, address: SocketAddr) {
        // println!("📍 AudioRouter: Registering client {} at address {}", user_id, address);
        self.client_addresses.insert(user_id, address);
        // println!("📍 AudioRouter: Client {} registered successfully. Total clients: {}", 
        //     user_id, self.client_addresses.len());
    }

    /// Supprime un client
    pub fn unregister_client(&self, user_id: &Uuid) {
        self.client_addresses.remove(user_id);
        // Supprimer les buffers de cet utilisateur
        self.user_buffers.retain(|(uid, _), _| uid != user_id);
    }

    /// Ajoute un utilisateur à un channel
    pub fn add_user_to_channel(&self, user_id: Uuid, channel_id: Uuid) {
        let key = (user_id, channel_id);
        if !self.user_buffers.contains_key(&key) {
            self.user_buffers.insert(key, AudioBuffer::new(1024, 48000)); // 1KB buffer à 48kHz
        }

        // Mettre à jour les statistiques
        self.stats.entry(channel_id).or_insert_with(|| RoutingStats {
            packets_received: 0,
            packets_routed: 0,
            packets_sent: 0,
            bytes_received: 0,
            bytes_sent: 0,
            active_users: 0,
            connected_users: 0,
            average_latency_ms: 0.0,
            packet_loss_rate: 0.0,
            jitter_ms: 0.0,
            created_at: std::time::Instant::now(),
        }).connected_users += 1;
    }

    /// Supprime un utilisateur d'un channel
    pub fn remove_user_from_channel(&self, user_id: &Uuid, channel_id: &Uuid) {
        let key = (*user_id, *channel_id);
        self.user_buffers.remove(&key);

        // Mettre à jour les statistiques
        if let Some(mut stats) = self.stats.get_mut(channel_id) {
            if stats.connected_users > 0 {
                stats.connected_users -= 1;
            }
        }
    }

    /// Route un packet audio vers les autres utilisateurs du channel
    pub fn route_packet(&self, packet: &AudioPacket, from_user: Uuid, channel_id: Uuid) -> Vec<SocketAddr> {
        // println!("🔀 AudioRouter: Routing packet from user {} in channel {}", from_user, channel_id);
        
        let mut destinations = Vec::new();
        let packets_received = 1_u64;
        let mut packets_routed = 0_u64;

        // Trouver tous les utilisateurs du channel (sauf l'expéditeur)
        // println!("🔀 AudioRouter: Searching for users in channel {}", channel_id);
        for entry in self.user_buffers.iter() {
            let (user_id, ch_id) = entry.key();
            if *ch_id == channel_id && *user_id != from_user {
                println!("🎯 AudioRouter: Found target user {} in channel {}", user_id, ch_id);
                packets_routed += 1;
                
                // Récupérer l'adresse de destination
                if let Some(addr) = self.client_addresses.get(user_id) {
                    println!("📤 AudioRouter: Routing to user {} at {}", user_id, addr.value());
                    destinations.push(*addr.value());
                } else {
                    println!("⚠️ AudioRouter: No address found for user {} (not registered)", user_id);
                }
            } // else if *ch_id == channel_id {
                // println!("📤 AudioRouter: Skipping sender {} (same as source)", user_id);
            // }
        }
        
        // println!("🔀 AudioRouter: Routing summary - Found {} destinations for {} users", 
        //     destinations.len(), packets_routed);

        // Mettre à jour les statistiques
        if let Some(mut stats) = self.stats.get_mut(&channel_id) {
            stats.packets_received += packets_received;
            stats.packets_routed += packets_routed;
            stats.bytes_received += packet.payload.len() as u64;
            
            // Calculer quelques métriques basiques
            if stats.packets_received > 0 {
                stats.packet_loss_rate = 1.0 - (stats.packets_routed as f32 / stats.packets_received as f32);
            }
        }

        destinations
    }

    /// Récupère les données audio pour un utilisateur spécifique
    pub fn get_audio_data(&self, user_id: &Uuid, channel_id: &Uuid) -> Option<Vec<AudioPacket>> {
        let key = (*user_id, *channel_id);
        if let Some(mut buffer_ref) = self.user_buffers.get_mut(&key) {
            let buffer = buffer_ref.value_mut();
            let packets = buffer.get_ready_packets();
            if !packets.is_empty() {
                Some(packets)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Ajuste la latence cible pour un channel
    pub fn adjust_channel_latency(&self, channel_id: &Uuid, latency_ms: u64) {
        if let Some(mut stats) = self.stats.get_mut(channel_id) {
            stats.average_latency_ms = latency_ms as f32;
        }
    }

    /// Récupère les statistiques d'un channel
    pub fn get_channel_stats(&self, channel_id: &Uuid) -> Option<RoutingStats> {
        self.stats.get(channel_id).map(|stats| stats.clone())
    }

    /// Récupère la liste des utilisateurs connectés à un channel
    pub fn get_channel_users(&self, channel_id: &Uuid) -> Vec<Uuid> {
        self.user_buffers.iter()
            .filter_map(|entry| {
                let (user_id, ch_id) = entry.key();
                if *ch_id == *channel_id {
                    Some(*user_id)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Met à jour le statut d'activité d'un utilisateur
    pub fn update_user_activity(&self, channel_id: &Uuid, is_active: bool) {
        if let Some(mut stats) = self.stats.get_mut(channel_id) {
            if is_active {
                stats.active_users = stats.active_users.saturating_add(1);
            } else {
                stats.active_users = stats.active_users.saturating_sub(1);
            }
        }
    }

    /// Nettoie les ressources pour un channel
    pub fn cleanup_channel(&self, channel_id: &Uuid) {
        // Supprimer tous les buffers du channel
        self.user_buffers.retain(|(_, ch_id), _| ch_id != channel_id);
        // Supprimer les statistiques
        self.stats.remove(channel_id);
    }

    /// Récupère les métriques globales
    pub fn get_global_stats(&self) -> (usize, usize, usize) {
        let total_clients = self.client_addresses.len();
        let total_channels = self.stats.len();
        let total_buffers = self.user_buffers.len();
        
        (total_clients, total_channels, total_buffers)
    }

    /// Nettoie périodiquement les buffers (pour UDP server)
    pub fn cleanup_buffers(&self) {
        // Nettoyer les packets expirés dans tous les buffers
        for mut entry in self.user_buffers.iter_mut() {
            entry.value_mut().cleanup();
        }
    }

    /// Recoit un packet et détermine s'il doit être traité
    pub fn receive_packet(&self, _packet: AudioPacket, _from_addr: SocketAddr) -> bool {
        // Pour l'instant, accepter tous les packets
        // TODO: Validation de packet, limitation de débit, etc.
        true
    }

    /// Route vers un channel (version simplifiée)
    pub fn route_to_channel(&self, packet: AudioPacket) -> Vec<SocketAddr> {
        let user_id = packet.header.user_id;
        let channel_id = packet.header.channel_id;
        
        self.route_packet(&packet, user_id, channel_id)
    }

    /// Récupère les packets prêts pour un utilisateur
    pub fn get_packets_for_user(&self, user_id: &Uuid, channel_id: &Uuid) -> Vec<AudioPacket> {
        self.get_audio_data(user_id, channel_id).unwrap_or_default()
    }

    /// Configure le routage pour un channel
    pub fn configure_channel(&self, channel_id: Uuid, config: ChannelRoutingConfig) {
        self.channel_configs.insert(channel_id, config);
    }

    /// Récupère la configuration d'un channel
    pub fn get_channel_config(&self, channel_id: &Uuid) -> Option<ChannelRoutingConfig> {
        self.channel_configs.get(channel_id).map(|config| config.clone())
    }

    /// Route avec intelligence adaptative
    pub fn intelligent_route(&self, packet: &AudioPacket, from_user: Uuid, channel_id: Uuid) -> Vec<SocketAddr> {
        // Récupérer la configuration du channel
        let config = self.get_channel_config(&channel_id).unwrap_or_else(|| {
            ChannelRoutingConfig {
                max_users: 10,
                quality_mode: QualityMode::Medium,
                latency_target_ms: 50,
                enable_echo_cancellation: false,
                enable_noise_suppression: false,
                bitrate_kbps: 64,
            }
        });

        // Mettre à jour la synchronisation
        self.channel_sync.insert(channel_id, packet.header.timestamp);

        // Router selon la configuration
        match config.quality_mode {
            QualityMode::Low => self.route_low_quality(packet, from_user, channel_id),
            QualityMode::High => self.route_high_quality(packet, from_user, channel_id),
            QualityMode::Adaptive => self.route_adaptive(packet, from_user, channel_id),
            QualityMode::Medium => self.route_packet(packet, from_user, channel_id),
        }
    }

    /// Routage optimisé pour faible bande passante
    fn route_low_quality(&self, packet: &AudioPacket, from_user: Uuid, channel_id: Uuid) -> Vec<SocketAddr> {
        // Ne router que vers les utilisateurs actifs récemment
        let mut destinations = Vec::new();

        for entry in self.user_buffers.iter() {
            let (user_id, ch_id) = entry.key();
            if *ch_id == channel_id && *user_id != from_user {
                // Vérifier l'activité récente (simplifiée pour l'instant)
                if let Some(addr) = self.client_addresses.get(user_id) {
                    destinations.push(*addr.value());
                }
            }
        }

        // Mettre à jour les stats
        self.update_routing_stats(&channel_id, 1, destinations.len(), packet.payload.len());
        destinations
    }

    /// Routage optimisé pour haute qualité
    fn route_high_quality(&self, packet: &AudioPacket, from_user: Uuid, channel_id: Uuid) -> Vec<SocketAddr> {
        // Router vers tous les utilisateurs avec vérifications supplémentaires
        let destinations = self.route_packet(packet, from_user, channel_id);
        
        // Pour la haute qualité, on pourrait ajouter:
        // - Détection de perte de packets
        // - Retransmission
        // - FEC (Forward Error Correction)
        
        destinations
    }

    /// Routage adaptatif selon les conditions réseau
    fn route_adaptive(&self, packet: &AudioPacket, from_user: Uuid, channel_id: Uuid) -> Vec<SocketAddr> {
        // Analyser les conditions actuelles
        if let Some(stats) = self.stats.get(&channel_id) {
            if stats.packet_loss_rate > 5.0 || stats.jitter_ms > 100.0 {
                // Conditions dégradées -> mode faible qualité
                self.route_low_quality(packet, from_user, channel_id)
            } else if stats.packet_loss_rate < 1.0 && stats.jitter_ms < 20.0 {
                // Bonnes conditions -> haute qualité
                self.route_high_quality(packet, from_user, channel_id)
            } else {
                // Conditions moyennes -> mode standard
                self.route_packet(packet, from_user, channel_id)
            }
        } else {
            // Pas de stats -> mode par défaut
            self.route_packet(packet, from_user, channel_id)
        }
    }

    /// Met à jour les statistiques de routage
    fn update_routing_stats(&self, channel_id: &Uuid, packets_received: usize, packets_routed: usize, bytes: usize) {
        if let Some(mut stats) = self.stats.get_mut(channel_id) {
            stats.packets_received += packets_received as u64;
            stats.packets_routed += packets_routed as u64;
            stats.bytes_received += bytes as u64;
            
            // Calculer le taux de perte
            if stats.packets_received > 0 {
                stats.packet_loss_rate = 1.0 - (stats.packets_routed as f32 / stats.packets_received as f32);
            }
        }
    }

    /// Analyse les performances du channel
    pub fn analyze_channel_performance(&self, channel_id: &Uuid) -> Option<ChannelPerformanceReport> {
        let stats = self.stats.get(channel_id)?;
        let config = self.get_channel_config(channel_id);

        let quality_score = self.calculate_channel_quality_score(&stats);
        let recommendations = self.generate_recommendations(&stats, &config);

        Some(ChannelPerformanceReport {
            channel_id: *channel_id,
            quality_score,
            packet_loss_rate: stats.packet_loss_rate,
            average_latency_ms: stats.average_latency_ms,
            jitter_ms: stats.jitter_ms,
            connected_users: stats.connected_users,
            active_users: stats.active_users,
            recommendations,
            timestamp: std::time::Instant::now(),
        })
    }

    /// Calcule un score de qualité pour le channel
    fn calculate_channel_quality_score(&self, stats: &RoutingStats) -> f32 {
        let mut score = 1.0;

        // Pénaliser la perte de packets
        score -= stats.packet_loss_rate * 0.4;

        // Pénaliser la latence élevée
        let latency_penalty = (stats.average_latency_ms / 200.0).min(1.0) * 0.3;
        score -= latency_penalty;

        // Pénaliser le jitter élevé
        let jitter_penalty = (stats.jitter_ms / 100.0).min(1.0) * 0.3;
        score -= jitter_penalty;

        score.max(0.0)
    }

    /// Génère des recommandations d'optimisation
    fn generate_recommendations(&self, stats: &RoutingStats, config: &Option<ChannelRoutingConfig>) -> Vec<String> {
        let mut recommendations = Vec::new();

        if stats.packet_loss_rate > 5.0 {
            recommendations.push("Taux de perte élevé - Considérer réduire la qualité audio".to_string());
        }

        if stats.jitter_ms > 50.0 {
            recommendations.push("Jitter élevé - Vérifier la stabilité du réseau".to_string());
        }

        if stats.average_latency_ms > 150.0 {
            recommendations.push("Latence élevée - Optimiser la configuration de buffer".to_string());
        }

        if let Some(cfg) = config {
            if cfg.quality_mode == QualityMode::High && stats.packet_loss_rate > 2.0 {
                recommendations.push("Mode haute qualité non optimal - Basculer en mode adaptatif".to_string());
            }
        }

        if stats.connected_users > 8 {
            recommendations.push("Nombre d'utilisateurs élevé - Considérer diviser le channel".to_string());
        }

        recommendations
    }
}