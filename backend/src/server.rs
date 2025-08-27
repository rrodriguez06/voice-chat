use axum::{
    routing::{get, post},
    Router,
};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

use crate::{
    config::Config,
    handlers::ApiHandlers,
    services::{UserService, ChannelService, AudioService},
    networking::WebSocketHandler,
    audio::{MetricsCollector, MetricsConfig},
    api::{create_metrics_router, MetricsApiState, MetricsApiConfig, create_advanced_router, AdvancedApiState, AdvancedApiConfig},
    Result,
};

pub struct Server {
    config: Config,
    user_service: Arc<UserService>,
    channel_service: Arc<ChannelService>,
    audio_service: Arc<AudioService>,
    metrics_collector: Arc<RwLock<MetricsCollector>>,
}

impl Server {
    pub async fn new(config: Config) -> Result<Self> {
        let user_service = Arc::new(UserService::new());
        let channel_service = Arc::new(ChannelService::new(config.limits.clone()));
        
        // Créer le service audio avec les services
        let mut audio_service = AudioService::with_services(
            config.audio.clone(),
            user_service.clone(),
            channel_service.clone(),
        );

        // Démarrer le serveur UDP audio
        audio_service.start_udp_server(
            config.clone(),
            user_service.clone(),
            channel_service.clone(),
        ).await?;

        // Créer le collecteur de métriques (simplifié pour l'instant)
        let metrics_config = MetricsConfig::default();
        let metrics_collector = MetricsCollector::new(metrics_config);

        Ok(Self {
            config,
            user_service,
            channel_service,
            audio_service: Arc::new(audio_service),
            metrics_collector: Arc::new(RwLock::new(metrics_collector)),
        })
    }

    pub async fn run(self) -> Result<()> {
        // Create WebSocket handler first
        let ws_handler = Arc::new(WebSocketHandler::new(
            self.user_service.clone(),
            self.channel_service.clone(),
            self.audio_service.clone(),
        ));

        // Create API handlers with WebSocket handler
        let api_handlers = Arc::new(ApiHandlers::new(
            self.user_service.clone(),
            self.channel_service.clone(),
            self.audio_service.clone(),
            ws_handler.clone(),
        ));

        // Create metrics API state
        let metrics_api_state = MetricsApiState {
            metrics_collector: self.metrics_collector.clone(),
            config: MetricsApiConfig::default(),
        };

        // Create advanced API state
        let advanced_api_state = AdvancedApiState {
            user_service: self.user_service.clone(),
            channel_service: self.channel_service.clone(),
            audio_service: self.audio_service.clone(),
            config: AdvancedApiConfig::default(),
        };

        // Create routers
        let metrics_router = create_metrics_router(metrics_api_state);
        let advanced_router = create_advanced_router(advanced_api_state);

        // Build HTTP router
        let app = Router::new()
            .route("/health", get(|| async { "OK" }))
            .route("/api/users", post({
                move |state, json| async move { ApiHandlers::create_user(state, json).await }
            }))
            .route("/api/users/:id", get({
                move |state, path| async move { ApiHandlers::get_user(state, path).await }
            }))
            .route("/api/users/:id/disconnect", post({
                move |state, path| async move { ApiHandlers::disconnect_user(state, path).await }
            }))
            .route("/api/channels", get({
                move |state| async move { ApiHandlers::list_channels(state).await }
            }))
            .route("/api/channels", post({
                move |state, json| async move { ApiHandlers::create_channel(state, json).await }
            }))
            .route("/api/channels/:id", get({
                move |state, path| async move { ApiHandlers::get_channel(state, path).await }
            }))
            .route("/api/channels/:id/join", post({
                move |state, path, json| async move { ApiHandlers::join_channel(state, path, json).await }
            }))
            .route("/api/channels/:id/leave", post({
                move |state, path, json| async move { ApiHandlers::leave_channel(state, path, json).await }
            }))
            .route("/api/channels/:id/audio/stats", get({
                move |state, path| async move { ApiHandlers::get_audio_stats(state, path).await }
            }))
            .route("/api/audio/config", get({
                move |state| async move { ApiHandlers::get_audio_config(state).await }
            }))
            .route("/ws", get({
                let handler = ws_handler.clone();
                move |ws| WebSocketHandler::handle_upgrade(axum::extract::State(handler), ws)
            }))
            .with_state(api_handlers)
            .merge(metrics_router)
            .nest("/api/advanced", advanced_router)
            .layer(CorsLayer::permissive())
            .layer(TraceLayer::new_for_http());

        // Start HTTP server
        let http_addr = self.config.http_addr();
        info!("Starting HTTP server on {}", http_addr);
        
        let listener = TcpListener::bind(http_addr).await?;
        axum::serve(listener, app).await?;

        Ok(())
    }
}