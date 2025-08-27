use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketMessage {
    #[serde(rename = "event")]
    pub message_type: String,
    pub data: serde_json::Value,
}

pub struct WebSocketManager {
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        Self {
            shutdown_tx: None,
        }
    }

    pub async fn start(&mut self, app_handle: AppHandle, server_url: String, username: String) -> Result<()> {
        println!("🚀 Starting WebSocket connection to {}", server_url);
        
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();
        self.shutdown_tx = Some(shutdown_tx);

        let app_handle_clone = app_handle.clone();
        tokio::spawn(async move {
            println!("🔌 Starting WebSocket task");
            
            let ws_url = server_url.clone();
            println!("🔗 Connecting to: {}", ws_url);
            
            let connection_result = connect_async(&ws_url).await;
            
            match connection_result {
                Ok((ws_stream, _)) => {
                    println!("✅ WebSocket connected successfully");
                    
                    let (mut write, mut read) = ws_stream.split();
                    
                    let auth_message = serde_json::json!({
                        "action": "Authenticate",
                        "payload": {
                            "username": username
                        }
                    });
                    
                    if let Err(e) = write.send(Message::Text(auth_message.to_string())).await {
                        println!("❌ Failed to send auth message: {}", e);
                        return;
                    }
                    
                    println!("🔐 Auth message sent");
                    
                    loop {
                        tokio::select! {
                            msg = read.next() => {
                                match msg {
                                    Some(Ok(Message::Text(text))) => {
                                        println!("📩 Received WebSocket message: {}", text);
                                        
                                        if let Ok(ws_message) = serde_json::from_str::<WebSocketMessage>(&text) {
                                            // Traiter le message et émettre l'événement approprié
                                            Self::handle_websocket_message(&app_handle_clone, ws_message).await;
                                        } else {
                                            println!("⚠️ Failed to parse WebSocket message as JSON");
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) => {
                                        println!("🔌 WebSocket connection closed by server");
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        println!("❌ WebSocket error: {}", e);
                                        break;
                                    }
                                    None => {
                                        println!("🔌 WebSocket stream ended");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                            _ = &mut shutdown_rx => {
                                println!("🛑 Received shutdown signal, closing WebSocket");
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    println!("❌ Failed to connect to WebSocket: {}", e);
                }
            }
            println!("🔌 WebSocket task ended");
        });

        println!("✅ WebSocket connection started in background");
        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        println!("🛑 Stopping WebSocket connection");
        
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            if let Err(_) = shutdown_tx.send(()) {
                println!("⚠️ Shutdown receiver already dropped");
            } else {
                println!("✅ Shutdown signal sent");
            }
        } else {
            println!("⚠️ No active WebSocket connection to stop");
        }
        
        Ok(())
    }

    async fn handle_websocket_message(app_handle: &AppHandle, message: WebSocketMessage) {
        println!("🔄 Processing WebSocket message: {}", message.message_type);
        
        match message.message_type.as_str() {
            "UserJoined" => {
                println!("👤 User joined channel - triggering UI refresh");
                if let Err(e) = app_handle.emit("user-joined", &message.data) {
                    println!("❌ Failed to emit user-joined event: {}", e);
                } else {
                    println!("✅ Emitted user-joined event to frontend");
                }
            },
            "UserLeft" => {
                println!("👤 User left channel - triggering UI refresh");
                if let Err(e) = app_handle.emit("user-left", &message.data) {
                    println!("❌ Failed to emit user-left event: {}", e);
                } else {
                    println!("✅ Emitted user-left event to frontend");
                }
            },
            "ChannelUsers" => {
                println!("👥 Channel users updated - triggering UI refresh");
                if let Err(e) = app_handle.emit("channel_users", &message.data) {
                    println!("❌ Failed to emit channel_users event: {}", e);
                } else {
                    println!("✅ Emitted channel_users event to frontend");
                }
            },
            "Authenticated" => {
                println!("🔐 WebSocket authenticated successfully");
                if let Err(e) = app_handle.emit("websocket-authenticated", &message.data) {
                    println!("❌ Failed to emit websocket-authenticated event: {}", e);
                } else {
                    println!("✅ Emitted websocket-authenticated event to frontend");
                }
            },
            "Error" => {
                println!("❌ WebSocket error received");
                if let Err(e) = app_handle.emit("websocket-error", &message.data) {
                    println!("❌ Failed to emit websocket-error event: {}", e);
                } else {
                    println!("✅ Emitted websocket-error event to frontend");
                }
            },
            _ => {
                println!("📨 Unhandled WebSocket message type: {}", message.message_type);
                // Émettre l'événement générique pour les types non gérés
                if let Err(e) = app_handle.emit("websocket-message", &message) {
                    println!("❌ Failed to emit generic websocket-message: {}", e);
                } else {
                    println!("✅ Emitted generic websocket-message to frontend");
                }
            }
        }
    }
}
