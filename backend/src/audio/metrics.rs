use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use serde::{Serialize, Deserialize};

use crate::audio::{
    AudioThreadPool, AudioMixer,
    AudioUdpServer, AudioRouter,
    performance::ThreadPoolStats,
    mixer::MixerGlobalStats,
    server::ServerStats,
};

/// Configuration du système de métriques
#[derive(Debug, Clone)]
pub struct MetricsConfig {
    pub collection_interval: Duration,
    pub history_retention: Duration,
    pub enable_detailed_metrics: bool,
    pub alert_thresholds: AlertThresholds,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            collection_interval: Duration::from_secs(5),
            history_retention: Duration::from_secs(24 * 3600), // 24 heures en secondes
            enable_detailed_metrics: true,
            alert_thresholds: AlertThresholds::default(),
        }
    }
}

/// Seuils d'alerte pour les métriques
#[derive(Debug, Clone)]
pub struct AlertThresholds {
    pub high_latency_ms: f32,
    pub high_packet_loss_percent: f32,
    pub high_cpu_usage_percent: f32,
    pub high_memory_usage_mb: u64,
    pub low_audio_quality_score: f32,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            high_latency_ms: 150.0,
            high_packet_loss_percent: 5.0,
            high_cpu_usage_percent: 80.0,
            high_memory_usage_mb: 1024,
            low_audio_quality_score: 0.7,
        }
    }
}

/// Métriques en temps réel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealTimeMetrics {
    pub timestamp: u64,
    pub audio_metrics: AudioSystemMetrics,
    pub performance_metrics: PerformanceMetrics,
    pub network_metrics: NetworkMetrics,
    pub system_health: SystemHealthMetrics,
}

/// Métriques du système audio
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSystemMetrics {
    pub active_channels: usize,
    pub active_users: usize,
    pub total_audio_streams: usize,
    pub average_latency_ms: f32,
    pub packet_loss_percentage: f32,
    pub audio_quality_score: f32,
    pub mixer_stats: MixerStats,
    pub buffer_stats: BufferStats,
    pub routing_stats: RoutingMetrics,
}

/// Métriques de performance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceMetrics {
    pub cpu_usage_percent: f32,
    pub memory_usage_mb: u64,
    pub thread_pool_utilization: f32,
    pub processing_queue_size: usize,
    pub average_processing_time_us: u64,
    pub concurrent_operations: usize,
}

/// Métriques réseau
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkMetrics {
    pub packets_received_per_second: u64,
    pub packets_sent_per_second: u64,
    pub bytes_received_per_second: u64,
    pub bytes_sent_per_second: u64,
    pub connection_count: usize,
    pub failed_connections: u64,
    pub network_jitter_ms: f32,
}

/// Métriques de santé système
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemHealthMetrics {
    pub overall_health_score: f32,
    pub active_alerts: Vec<AlertInfo>,
    pub uptime_seconds: u64,
    pub error_rate_percent: f32,
    pub service_availability_percent: f32,
}

/// Information d'alerte
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertInfo {
    pub id: String,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: u64,
    pub component: String,
}

/// Niveau de sévérité d'alerte
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}

/// Statistiques simplifiées du mixer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MixerStats {
    pub active_mixers: usize,
    pub total_samples_processed: u64,
    pub compression_ratio: f32,
    pub peak_level: f32,
}

/// Statistiques simplifiées du buffer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferStats {
    pub total_buffers: usize,
    pub average_fill_level: f32,
    pub underruns: u64,
    pub overruns: u64,
}

/// Métriques de routage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingMetrics {
    pub active_routes: usize,
    pub routing_decisions_per_second: u64,
    pub adaptive_adjustments: u64,
    pub route_quality_average: f32,
}

/// Collecteur de métriques principal
#[derive(Debug)]
pub struct MetricsCollector {
    config: MetricsConfig,
    current_metrics: Arc<RwLock<RealTimeMetrics>>,
    metrics_history: Arc<RwLock<Vec<RealTimeMetrics>>>,
    alerts: Arc<RwLock<Vec<AlertInfo>>>,
    start_time: Instant,
    
    // Références aux composants surveillés
    thread_pool: Option<Arc<AudioThreadPool>>,
    udp_server: Option<Arc<RwLock<AudioUdpServer>>>,
    mixer: Option<Arc<RwLock<AudioMixer>>>,
    router: Option<Arc<RwLock<AudioRouter>>>,
    
    // Compteurs pour les dérivées
    last_collection: Instant,
    last_packets_received: u64,
    last_packets_sent: u64,
    last_bytes_received: u64,
    last_bytes_sent: u64,
}

impl MetricsCollector {
    /// Crée un nouveau collecteur de métriques
    pub fn new(config: MetricsConfig) -> Self {
        let now = Instant::now();
        let initial_metrics = Self::create_empty_metrics();
        
        Self {
            config,
            current_metrics: Arc::new(RwLock::new(initial_metrics)),
            metrics_history: Arc::new(RwLock::new(Vec::new())),
            alerts: Arc::new(RwLock::new(Vec::new())),
            start_time: now,
            thread_pool: None,
            udp_server: None,
            mixer: None,
            router: None,
            last_collection: now,
            last_packets_received: 0,
            last_packets_sent: 0,
            last_bytes_received: 0,
            last_bytes_sent: 0,
        }
    }

    /// Enregistre les composants à surveiller
    pub fn register_components(
        &mut self,
        thread_pool: Option<Arc<AudioThreadPool>>,
        udp_server: Option<Arc<RwLock<AudioUdpServer>>>,
        mixer: Option<Arc<RwLock<AudioMixer>>>,
        router: Option<Arc<RwLock<AudioRouter>>>,
    ) {
        self.thread_pool = thread_pool;
        self.udp_server = udp_server;
        self.mixer = mixer;
        self.router = router;
    }

    /// Lance la collection automatique de métriques
    pub async fn start_collection(&mut self) {
        let current_metrics = self.current_metrics.clone();
        let metrics_history = self.metrics_history.clone();
        let alerts = self.alerts.clone();
        let config = self.config.clone();
        
        let thread_pool = self.thread_pool.clone();
        let udp_server = self.udp_server.clone();
        let mixer = self.mixer.clone();
        let router = self.router.clone();
        
        let start_time = self.start_time;
        let mut last_collection = self.last_collection;
        let mut last_packets_received = self.last_packets_received;
        let mut last_packets_sent = self.last_packets_sent;
        let mut last_bytes_received = self.last_bytes_received;
        let mut last_bytes_sent = self.last_bytes_sent;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(config.collection_interval);
            
            loop {
                interval.tick().await;
                
                let now = Instant::now();
                let time_delta = now.duration_since(last_collection).as_secs_f32();
                
                // Collecter les métriques
                let metrics = Self::collect_all_metrics(
                    &thread_pool,
                    &udp_server,
                    &mixer,
                    &router,
                    start_time,
                    time_delta,
                    &mut last_packets_received,
                    &mut last_packets_sent,
                    &mut last_bytes_received,
                    &mut last_bytes_sent,
                ).await;

                // Détecter les alertes
                let new_alerts = Self::detect_alerts(&metrics, &config.alert_thresholds);
                
                // Mettre à jour les métriques courantes
                {
                    let mut current = current_metrics.write().await;
                    *current = metrics.clone();
                }

                // Ajouter à l'historique
                {
                    let mut history = metrics_history.write().await;
                    history.push(metrics);
                    
                    // Nettoyer l'historique ancien
                    let cutoff_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() - config.history_retention.as_secs();
                    
                    history.retain(|m| m.timestamp >= cutoff_time);
                }

                // Mettre à jour les alertes
                {
                    let mut alerts_guard = alerts.write().await;
                    alerts_guard.extend(new_alerts);
                    
                    // Garder seulement les alertes récentes
                    let cutoff_time = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() - 3600; // 1 heure
                    
                    alerts_guard.retain(|a| a.timestamp >= cutoff_time);
                }

                last_collection = now;
            }
        });
    }

    /// Collecte toutes les métriques des composants
    async fn collect_all_metrics(
        thread_pool: &Option<Arc<AudioThreadPool>>,
        udp_server: &Option<Arc<RwLock<AudioUdpServer>>>,
        mixer: &Option<Arc<RwLock<AudioMixer>>>,
        router: &Option<Arc<RwLock<AudioRouter>>>,
        start_time: Instant,
        time_delta: f32,
        last_packets_received: &mut u64,
        last_packets_sent: &mut u64,
        last_bytes_received: &mut u64,
        last_bytes_sent: &mut u64,
    ) -> RealTimeMetrics {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Métriques audio
        let audio_metrics = Self::collect_audio_metrics(mixer, router).await;
        
        // Métriques performance
        let performance_metrics = Self::collect_performance_metrics(thread_pool).await;
        
        // Métriques réseau
        let network_metrics = Self::collect_network_metrics(
            udp_server,
            time_delta,
            last_packets_received,
            last_packets_sent,
            last_bytes_received,
            last_bytes_sent,
        ).await;
        
        // Santé système
        let system_health = Self::calculate_system_health(
            &audio_metrics,
            &performance_metrics,
            &network_metrics,
            start_time,
        );

        RealTimeMetrics {
            timestamp,
            audio_metrics,
            performance_metrics,
            network_metrics,
            system_health,
        }
    }

    /// Collecte les métriques audio
    async fn collect_audio_metrics(
        mixer: &Option<Arc<RwLock<AudioMixer>>>,
        router: &Option<Arc<RwLock<AudioRouter>>>,
    ) -> AudioSystemMetrics {
        let mixer_stats = if let Some(mixer) = mixer {
            let mixer_guard = mixer.read().await;
            let global_stats = mixer_guard.global_stats();
            MixerStats {
                active_mixers: global_stats.active_channels,
                total_samples_processed: 0, // Approximation
                compression_ratio: 4.0, // Valeur par défaut
                peak_level: 0.0,
            }
        } else {
            MixerStats {
                active_mixers: 0,
                total_samples_processed: 0,
                compression_ratio: 1.0,
                peak_level: 0.0,
            }
        };

        let routing_stats = if let Some(router) = router {
            let router_guard = router.read().await;
            let (_total_clients, total_channels, _total_buffers) = router_guard.get_global_stats();
            RoutingMetrics {
                active_routes: total_channels,
                routing_decisions_per_second: 0, // Calculé ailleurs
                adaptive_adjustments: 0, // Pas encore disponible
                route_quality_average: 1.0, // Valeur par défaut
            }
        } else {
            RoutingMetrics {
                active_routes: 0,
                routing_decisions_per_second: 0,
                adaptive_adjustments: 0,
                route_quality_average: 1.0,
            }
        };

        AudioSystemMetrics {
            active_channels: mixer_stats.active_mixers,
            active_users: 0, // À calculer depuis les connexions
            total_audio_streams: mixer_stats.active_mixers,
            average_latency_ms: 50.0, // Approximation
            packet_loss_percentage: 0.1,
            audio_quality_score: 0.95,
            mixer_stats,
            buffer_stats: BufferStats {
                total_buffers: 10,
                average_fill_level: 0.5,
                underruns: 0,
                overruns: 0,
            },
            routing_stats,
        }
    }

    /// Collecte les métriques de performance
    async fn collect_performance_metrics(
        thread_pool: &Option<Arc<AudioThreadPool>>
    ) -> PerformanceMetrics {
        if let Some(pool) = thread_pool {
            let stats = pool.get_stats().await;
            PerformanceMetrics {
                cpu_usage_percent: Self::get_cpu_usage(),
                memory_usage_mb: Self::get_memory_usage(),
                thread_pool_utilization: (stats.active_workers as f32 / (stats.active_workers + 1) as f32) * 100.0,
                processing_queue_size: stats.queued_tasks,
                average_processing_time_us: stats.average_processing_time_us,
                concurrent_operations: stats.active_workers,
            }
        } else {
            PerformanceMetrics {
                cpu_usage_percent: 0.0,
                memory_usage_mb: 0,
                thread_pool_utilization: 0.0,
                processing_queue_size: 0,
                average_processing_time_us: 0,
                concurrent_operations: 0,
            }
        }
    }

    /// Collecte les métriques réseau
    async fn collect_network_metrics(
        udp_server: &Option<Arc<RwLock<AudioUdpServer>>>,
        time_delta: f32,
        last_packets_received: &mut u64,
        last_packets_sent: &mut u64,
        last_bytes_received: &mut u64,
        last_bytes_sent: &mut u64,
    ) -> NetworkMetrics {
        if let Some(server) = udp_server {
            let server_guard = server.read().await;
            let stats = server_guard.get_stats().await;
            
            // Calculer les taux par seconde
            let packets_received_delta = stats.total_packets_received - *last_packets_received;
            let packets_sent_delta = stats.total_packets_sent - *last_packets_sent;
            let bytes_received_delta = stats.bytes_received - *last_bytes_received;
            let bytes_sent_delta = stats.bytes_sent - *last_bytes_sent;
            
            *last_packets_received = stats.total_packets_received;
            *last_packets_sent = stats.total_packets_sent;
            *last_bytes_received = stats.bytes_received;
            *last_bytes_sent = stats.bytes_sent;
            
            NetworkMetrics {
                packets_received_per_second: (packets_received_delta as f32 / time_delta) as u64,
                packets_sent_per_second: (packets_sent_delta as f32 / time_delta) as u64,
                bytes_received_per_second: (bytes_received_delta as f32 / time_delta) as u64,
                bytes_sent_per_second: (bytes_sent_delta as f32 / time_delta) as u64,
                connection_count: stats.active_connections,
                failed_connections: stats.packet_errors,
                network_jitter_ms: 5.0, // Approximation
            }
        } else {
            NetworkMetrics {
                packets_received_per_second: 0,
                packets_sent_per_second: 0,
                bytes_received_per_second: 0,
                bytes_sent_per_second: 0,
                connection_count: 0,
                failed_connections: 0,
                network_jitter_ms: 0.0,
            }
        }
    }

    /// Calcule la santé globale du système
    fn calculate_system_health(
        audio: &AudioSystemMetrics,
        performance: &PerformanceMetrics,
        network: &NetworkMetrics,
        start_time: Instant,
    ) -> SystemHealthMetrics {
        // Score de santé basé sur plusieurs facteurs
        let latency_score = (200.0 - audio.average_latency_ms.min(200.0)) / 200.0;
        let cpu_score = (100.0 - performance.cpu_usage_percent.min(100.0)) / 100.0;
        let packet_loss_score = (10.0 - audio.packet_loss_percentage.min(10.0)) / 10.0;
        let quality_score = audio.audio_quality_score;
        
        let overall_health_score = (latency_score + cpu_score + packet_loss_score + quality_score) / 4.0;
        
        SystemHealthMetrics {
            overall_health_score,
            active_alerts: vec![], // Calculé par detect_alerts
            uptime_seconds: start_time.elapsed().as_secs(),
            error_rate_percent: (network.failed_connections as f32 / (network.connection_count.max(1) as f32)) * 100.0,
            service_availability_percent: if overall_health_score > 0.7 { 100.0 } else { overall_health_score * 100.0 },
        }
    }

    /// Détecte les alertes basées sur les seuils
    fn detect_alerts(metrics: &RealTimeMetrics, thresholds: &AlertThresholds) -> Vec<AlertInfo> {
        let mut alerts = Vec::new();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Alerte latence élevée
        if metrics.audio_metrics.average_latency_ms > thresholds.high_latency_ms {
            alerts.push(AlertInfo {
                id: format!("latency-{}", timestamp),
                severity: AlertSeverity::Warning,
                message: format!("High audio latency: {:.1}ms", metrics.audio_metrics.average_latency_ms),
                timestamp,
                component: "audio".to_string(),
            });
        }

        // Alerte perte de packets
        if metrics.audio_metrics.packet_loss_percentage > thresholds.high_packet_loss_percent {
            alerts.push(AlertInfo {
                id: format!("packet-loss-{}", timestamp),
                severity: AlertSeverity::Error,
                message: format!("High packet loss: {:.1}%", metrics.audio_metrics.packet_loss_percentage),
                timestamp,
                component: "network".to_string(),
            });
        }

        // Alerte utilisation CPU
        if metrics.performance_metrics.cpu_usage_percent > thresholds.high_cpu_usage_percent {
            alerts.push(AlertInfo {
                id: format!("cpu-{}", timestamp),
                severity: AlertSeverity::Warning,
                message: format!("High CPU usage: {:.1}%", metrics.performance_metrics.cpu_usage_percent),
                timestamp,
                component: "performance".to_string(),
            });
        }

        alerts
    }

    /// Obtient l'utilisation CPU (simulation)
    fn get_cpu_usage() -> f32 {
        // En production, utiliser une vraie bibliothèque de monitoring système
        25.0
    }

    /// Obtient l'utilisation mémoire (simulation)
    fn get_memory_usage() -> u64 {
        // En production, utiliser une vraie bibliothèque de monitoring système
        512
    }

    /// Crée des métriques vides
    fn create_empty_metrics() -> RealTimeMetrics {
        RealTimeMetrics {
            timestamp: 0,
            audio_metrics: AudioSystemMetrics {
                active_channels: 0,
                active_users: 0,
                total_audio_streams: 0,
                average_latency_ms: 0.0,
                packet_loss_percentage: 0.0,
                audio_quality_score: 1.0,
                mixer_stats: MixerStats {
                    active_mixers: 0,
                    total_samples_processed: 0,
                    compression_ratio: 1.0,
                    peak_level: 0.0,
                },
                buffer_stats: BufferStats {
                    total_buffers: 0,
                    average_fill_level: 0.0,
                    underruns: 0,
                    overruns: 0,
                },
                routing_stats: RoutingMetrics {
                    active_routes: 0,
                    routing_decisions_per_second: 0,
                    adaptive_adjustments: 0,
                    route_quality_average: 1.0,
                },
            },
            performance_metrics: PerformanceMetrics {
                cpu_usage_percent: 0.0,
                memory_usage_mb: 0,
                thread_pool_utilization: 0.0,
                processing_queue_size: 0,
                average_processing_time_us: 0,
                concurrent_operations: 0,
            },
            network_metrics: NetworkMetrics {
                packets_received_per_second: 0,
                packets_sent_per_second: 0,
                bytes_received_per_second: 0,
                bytes_sent_per_second: 0,
                connection_count: 0,
                failed_connections: 0,
                network_jitter_ms: 0.0,
            },
            system_health: SystemHealthMetrics {
                overall_health_score: 1.0,
                active_alerts: vec![],
                uptime_seconds: 0,
                error_rate_percent: 0.0,
                service_availability_percent: 100.0,
            },
        }
    }

    /// Récupère les métriques actuelles
    pub async fn get_current_metrics(&self) -> RealTimeMetrics {
        self.current_metrics.read().await.clone()
    }

    /// Récupère l'historique des métriques
    pub async fn get_metrics_history(&self, limit: Option<usize>) -> Vec<RealTimeMetrics> {
        let history = self.metrics_history.read().await;
        match limit {
            Some(limit) => history.iter().rev().take(limit).cloned().collect(),
            None => history.clone(),
        }
    }

    /// Récupère les alertes actives
    pub async fn get_active_alerts(&self) -> Vec<AlertInfo> {
        self.alerts.read().await.clone()
    }

    /// Génère un rapport de santé
    pub async fn generate_health_report(&self) -> HealthReport {
        let metrics = self.get_current_metrics().await;
        let alerts = self.get_active_alerts().await;
        
        HealthReport {
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            overall_status: if metrics.system_health.overall_health_score > 0.8 {
                "Healthy".to_string()
            } else if metrics.system_health.overall_health_score > 0.6 {
                "Warning".to_string()
            } else {
                "Critical".to_string()
            },
            health_score: metrics.system_health.overall_health_score,
            component_status: ComponentStatus {
                audio_system: if metrics.audio_metrics.audio_quality_score > 0.8 { "Healthy" } else { "Warning" }.to_string(),
                performance: if metrics.performance_metrics.cpu_usage_percent < 80.0 { "Healthy" } else { "Warning" }.to_string(),
                network: if metrics.network_metrics.connection_count > 0 { "Healthy" } else { "Info" }.to_string(),
            },
            metrics,
            recent_alerts: alerts,
        }
    }
}

/// Rapport de santé complet
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthReport {
    pub timestamp: u64,
    pub overall_status: String,
    pub health_score: f32,
    pub component_status: ComponentStatus,
    pub metrics: RealTimeMetrics,
    pub recent_alerts: Vec<AlertInfo>,
}

/// État des composants
#[derive(Debug, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub audio_system: String,
    pub performance: String,
    pub network: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_config_creation() {
        let config = MetricsConfig::default();
        assert_eq!(config.collection_interval, Duration::from_secs(5));
        assert_eq!(config.history_retention, Duration::from_hours(24));
        assert!(config.enable_detailed_metrics);
    }

    #[test]
    fn test_alert_detection() {
        let metrics = MetricsCollector::create_empty_metrics();
        let thresholds = AlertThresholds::default();
        
        let alerts = MetricsCollector::detect_alerts(&metrics, &thresholds);
        assert!(alerts.is_empty()); // Pas d'alertes avec des métriques vides
    }

    #[tokio::test]
    async fn test_metrics_collector() {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config);
        
        let metrics = collector.get_current_metrics().await;
        assert_eq!(metrics.audio_metrics.active_channels, 0);
    }
}