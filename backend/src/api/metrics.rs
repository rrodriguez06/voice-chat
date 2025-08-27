use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::audio::{MetricsCollector, RealTimeMetrics, HealthReport};

/// Configuration pour les endpoints de métriques
#[derive(Debug, Clone)]
pub struct MetricsApiConfig {
    pub enable_detailed_metrics: bool,
    pub enable_alerts: bool,
    pub max_history_points: usize,
}

impl Default for MetricsApiConfig {
    fn default() -> Self {
        Self {
            enable_detailed_metrics: true,
            enable_alerts: true,
            max_history_points: 1000,
        }
    }
}

/// État partagé pour les endpoints de métriques
#[derive(Debug, Clone)]
pub struct MetricsApiState {
    pub metrics_collector: Arc<RwLock<MetricsCollector>>,
    pub config: MetricsApiConfig,
}

/// Paramètres de requête pour l'historique des métriques
#[derive(Debug, Deserialize)]
pub struct HistoryQuery {
    /// Nombre de points à retourner (par défaut: 100)
    limit: Option<usize>,
    /// Filtrer par composant spécifique
    component: Option<String>,
    /// Période en secondes (par défaut: 3600 = 1 heure)
    period: Option<u64>,
}

/// Paramètres de requête pour les alertes
#[derive(Debug, Deserialize)]
pub struct AlertsQuery {
    /// Niveau de sévérité minimum
    severity: Option<String>,
    /// Composant spécifique
    component: Option<String>,
    /// Nombre maximum d'alertes
    limit: Option<usize>,
}

/// Réponse pour les métriques actuelles
#[derive(Debug, Serialize)]
pub struct CurrentMetricsResponse {
    pub success: bool,
    pub data: RealTimeMetrics,
    pub timestamp: u64,
}

/// Réponse pour l'historique des métriques
#[derive(Debug, Serialize)]
pub struct HistoryResponse {
    pub success: bool,
    pub data: Vec<RealTimeMetrics>,
    pub total_points: usize,
    pub period_seconds: u64,
}

/// Réponse pour les alertes
#[derive(Debug, Serialize)]
pub struct AlertsResponse {
    pub success: bool,
    pub data: Vec<crate::audio::metrics::AlertInfo>,
    pub total_alerts: usize,
}

/// Réponse d'erreur
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
    pub code: String,
}

/// Crée le routeur pour les endpoints de métriques
pub fn create_metrics_router(state: MetricsApiState) -> Router {
    Router::new()
        .route("/current", get(get_current_metrics))
        .route("/history", get(get_metrics_history))
        .route("/metrics/health", get(get_health_report))
        .route("/alerts", get(get_alerts))
        .route("/summary", get(get_metrics_summary))
        .route("/component/:component", get(get_component_metrics))
        .with_state(state)
}

/// GET /metrics/current - Récupère les métriques actuelles
pub async fn get_current_metrics(
    State(state): State<MetricsApiState>,
) -> Result<Json<CurrentMetricsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let collector = state.metrics_collector.read().await;
    
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        collector.get_current_metrics()
    ).await {
        Ok(metrics) => {
            let response = CurrentMetricsResponse {
                success: true,
                data: metrics,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            };
            Ok(Json(response))
        }
        Err(_) => {
            let error = ErrorResponse {
                success: false,
                error: "Timeout retrieving current metrics".to_string(),
                code: "TIMEOUT".to_string(),
            };
            Err((StatusCode::REQUEST_TIMEOUT, Json(error)))
        }
    }
}

/// GET /metrics/history - Récupère l'historique des métriques
pub async fn get_metrics_history(
    Query(params): Query<HistoryQuery>,
    State(state): State<MetricsApiState>,
) -> Result<Json<HistoryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let collector = state.metrics_collector.read().await;
    
    let limit = params.limit.unwrap_or(100).min(state.config.max_history_points);
    
    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        collector.get_metrics_history(Some(limit))
    ).await {
        Ok(history) => {
            let response = HistoryResponse {
                success: true,
                data: history.clone(),
                total_points: history.len(),
                period_seconds: params.period.unwrap_or(3600),
            };
            Ok(Json(response))
        }
        Err(_) => {
            let error = ErrorResponse {
                success: false,
                error: "Timeout retrieving metrics history".to_string(),
                code: "TIMEOUT".to_string(),
            };
            Err((StatusCode::REQUEST_TIMEOUT, Json(error)))
        }
    }
}

/// GET /metrics/health - Rapport de santé complet
pub async fn get_health_report(
    State(state): State<MetricsApiState>,
) -> Result<Json<HealthReport>, (StatusCode, Json<ErrorResponse>)> {
    let collector = state.metrics_collector.read().await;
    
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        collector.generate_health_report()
    ).await {
        Ok(report) => Ok(Json(report)),
        Err(_) => {
            let error = ErrorResponse {
                success: false,
                error: "Timeout generating health report".to_string(),
                code: "TIMEOUT".to_string(),
            };
            Err((StatusCode::REQUEST_TIMEOUT, Json(error)))
        }
    }
}

/// GET /metrics/alerts - Récupère les alertes actives
pub async fn get_alerts(
    Query(params): Query<AlertsQuery>,
    State(state): State<MetricsApiState>,
) -> Result<Json<AlertsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let collector = state.metrics_collector.read().await;
    
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        collector.get_active_alerts()
    ).await {
        Ok(mut alerts) => {
            // Filtrer par sévérité si spécifiée
            if let Some(severity) = &params.severity {
                alerts.retain(|alert| {
                    match severity.to_lowercase().as_str() {
                        "critical" => matches!(alert.severity, crate::audio::metrics::AlertSeverity::Critical),
                        "error" => matches!(alert.severity, crate::audio::metrics::AlertSeverity::Error | crate::audio::metrics::AlertSeverity::Critical),
                        "warning" => !matches!(alert.severity, crate::audio::metrics::AlertSeverity::Info),
                        "info" => true,
                        _ => true,
                    }
                });
            }
            
            // Filtrer par composant si spécifié
            if let Some(component) = &params.component {
                alerts.retain(|alert| alert.component == *component);
            }
            
            // Limiter le nombre d'alertes
            if let Some(limit) = params.limit {
                alerts.truncate(limit);
            }
            
            let total_alerts = alerts.len();
            
            let response = AlertsResponse {
                success: true,
                data: alerts,
                total_alerts,
            };
            Ok(Json(response))
        }
        Err(_) => {
            let error = ErrorResponse {
                success: false,
                error: "Timeout retrieving alerts".to_string(),
                code: "TIMEOUT".to_string(),
            };
            Err((StatusCode::REQUEST_TIMEOUT, Json(error)))
        }
    }
}

/// GET /metrics/summary - Résumé des métriques principales
pub async fn get_metrics_summary(
    State(state): State<MetricsApiState>,
) -> Result<Json<MetricsSummary>, (StatusCode, Json<ErrorResponse>)> {
    let collector = state.metrics_collector.read().await;
    
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        collector.get_current_metrics()
    ).await {
        Ok(metrics) => {
            let summary = MetricsSummary {
                timestamp: metrics.timestamp,
                health_score: metrics.system_health.overall_health_score,
                active_channels: metrics.audio_metrics.active_channels,
                active_users: metrics.audio_metrics.active_users,
                average_latency_ms: metrics.audio_metrics.average_latency_ms,
                packet_loss_percentage: metrics.audio_metrics.packet_loss_percentage,
                cpu_usage_percent: metrics.performance_metrics.cpu_usage_percent,
                memory_usage_mb: metrics.performance_metrics.memory_usage_mb,
                network_throughput_mbps: (metrics.network_metrics.bytes_received_per_second + metrics.network_metrics.bytes_sent_per_second) as f32 / 125000.0, // Conversion en Mbps
                active_connections: metrics.network_metrics.connection_count,
                uptime_seconds: metrics.system_health.uptime_seconds,
                error_rate_percent: metrics.system_health.error_rate_percent,
            };
            Ok(Json(summary))
        }
        Err(_) => {
            let error = ErrorResponse {
                success: false,
                error: "Timeout retrieving metrics summary".to_string(),
                code: "TIMEOUT".to_string(),
            };
            Err((StatusCode::REQUEST_TIMEOUT, Json(error)))
        }
    }
}

/// GET /metrics/component/:component - Métriques pour un composant spécifique
pub async fn get_component_metrics(
    Path(component): Path<String>,
    State(state): State<MetricsApiState>,
) -> Result<Json<ComponentMetrics>, (StatusCode, Json<ErrorResponse>)> {
    let collector = state.metrics_collector.read().await;
    
    match tokio::time::timeout(
        std::time::Duration::from_secs(5),
        collector.get_current_metrics()
    ).await {
        Ok(metrics) => {
            let component_metrics = match component.to_lowercase().as_str() {
                "audio" => ComponentMetrics::Audio {
                    active_channels: metrics.audio_metrics.active_channels,
                    active_users: metrics.audio_metrics.active_users,
                    average_latency_ms: metrics.audio_metrics.average_latency_ms,
                    packet_loss_percentage: metrics.audio_metrics.packet_loss_percentage,
                    audio_quality_score: metrics.audio_metrics.audio_quality_score,
                    mixer_stats: metrics.audio_metrics.mixer_stats,
                    buffer_stats: metrics.audio_metrics.buffer_stats,
                    routing_stats: metrics.audio_metrics.routing_stats,
                },
                "performance" => ComponentMetrics::Performance {
                    cpu_usage_percent: metrics.performance_metrics.cpu_usage_percent,
                    memory_usage_mb: metrics.performance_metrics.memory_usage_mb,
                    thread_pool_utilization: metrics.performance_metrics.thread_pool_utilization,
                    processing_queue_size: metrics.performance_metrics.processing_queue_size,
                    average_processing_time_us: metrics.performance_metrics.average_processing_time_us,
                    concurrent_operations: metrics.performance_metrics.concurrent_operations,
                },
                "network" => ComponentMetrics::Network {
                    packets_received_per_second: metrics.network_metrics.packets_received_per_second,
                    packets_sent_per_second: metrics.network_metrics.packets_sent_per_second,
                    bytes_received_per_second: metrics.network_metrics.bytes_received_per_second,
                    bytes_sent_per_second: metrics.network_metrics.bytes_sent_per_second,
                    connection_count: metrics.network_metrics.connection_count,
                    failed_connections: metrics.network_metrics.failed_connections,
                    network_jitter_ms: metrics.network_metrics.network_jitter_ms,
                },
                _ => {
                    let error = ErrorResponse {
                        success: false,
                        error: format!("Unknown component: {}", component),
                        code: "INVALID_COMPONENT".to_string(),
                    };
                    return Err((StatusCode::BAD_REQUEST, Json(error)));
                }
            };
            
            Ok(Json(component_metrics))
        }
        Err(_) => {
            let error = ErrorResponse {
                success: false,
                error: "Timeout retrieving component metrics".to_string(),
                code: "TIMEOUT".to_string(),
            };
            Err((StatusCode::REQUEST_TIMEOUT, Json(error)))
        }
    }
}

/// Résumé simplifié des métriques
#[derive(Debug, Serialize)]
pub struct MetricsSummary {
    pub timestamp: u64,
    pub health_score: f32,
    pub active_channels: usize,
    pub active_users: usize,
    pub average_latency_ms: f32,
    pub packet_loss_percentage: f32,
    pub cpu_usage_percent: f32,
    pub memory_usage_mb: u64,
    pub network_throughput_mbps: f32,
    pub active_connections: usize,
    pub uptime_seconds: u64,
    pub error_rate_percent: f32,
}

/// Métriques par composant
#[derive(Debug, Serialize)]
#[serde(tag = "component_type")]
pub enum ComponentMetrics {
    Audio {
        active_channels: usize,
        active_users: usize,
        average_latency_ms: f32,
        packet_loss_percentage: f32,
        audio_quality_score: f32,
        mixer_stats: crate::audio::metrics::MixerStats,
        buffer_stats: crate::audio::metrics::BufferStats,
        routing_stats: crate::audio::metrics::RoutingMetrics,
    },
    Performance {
        cpu_usage_percent: f32,
        memory_usage_mb: u64,
        thread_pool_utilization: f32,
        processing_queue_size: usize,
        average_processing_time_us: u64,
        concurrent_operations: usize,
    },
    Network {
        packets_received_per_second: u64,
        packets_sent_per_second: u64,
        bytes_received_per_second: u64,
        bytes_sent_per_second: u64,
        connection_count: usize,
        failed_connections: u64,
        network_jitter_ms: f32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum_test::TestServer;
    use crate::audio::{MetricsCollector, MetricsConfig};

    async fn create_test_state() -> MetricsApiState {
        let config = MetricsConfig::default();
        let collector = MetricsCollector::new(config);
        
        MetricsApiState {
            metrics_collector: Arc::new(RwLock::new(collector)),
            config: MetricsApiConfig::default(),
        }
    }

    #[tokio::test]
    async fn test_current_metrics_endpoint() {
        let state = create_test_state().await;
        let app = create_metrics_router(state);
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/current").await;
        assert_eq!(response.status_code(), 200);
    }

    #[tokio::test]
    async fn test_health_endpoint() {
        let state = create_test_state().await;
        let app = create_metrics_router(state);
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/health").await;
        assert_eq!(response.status_code(), 200);
    }

    #[tokio::test]
    async fn test_component_metrics_endpoint() {
        let state = create_test_state().await;
        let app = create_metrics_router(state);
        let server = TestServer::new(app).unwrap();
        
        let response = server.get("/component/audio").await;
        assert_eq!(response.status_code(), 200);
        
        let response = server.get("/component/invalid").await;
        assert_eq!(response.status_code(), 400);
    }
}