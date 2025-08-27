// Modules du frontend
mod state;
mod networking;
mod audio;

use state::AppState;
use networking::{BackendManager, WebSocketManager};
use audio::{AudioDeviceManager, AudioCaptureManager, AudioPlaybackManager};

use tauri::{AppHandle, State, Manager, WindowEvent};
use anyhow::Result;
use uuid::Uuid;
use std::sync::Arc;
use tokio::sync::Mutex;

/// État global de l'application Tauri
pub struct TauriAppState {
    pub app_state: AppState,
    pub backend_manager: Arc<std::sync::RwLock<Arc<BackendManager>>>,
    pub websocket_manager: Arc<Mutex<Option<WebSocketManager>>>,
    pub audio_device_manager: Arc<AudioDeviceManager>,
    pub audio_capture_manager: Arc<AudioCaptureManager>,
    pub audio_playback_manager: Arc<AudioPlaybackManager>,
}

impl TauriAppState {
    pub fn new() -> Result<Self> {
        let app_state = AppState::new();
        
        // Ne pas créer de BackendManager avec une URL par défaut
        // Il sera créé à la demande lors de la connexion
        let backend_manager = Arc::new(std::sync::RwLock::new(Arc::new(BackendManager::new(
            "", // URL vide - sera utilisée lors de la connexion
            app_state.clone()
        ))));
        
        let websocket_manager = Arc::new(Mutex::new(None));
        
        let audio_device_manager = Arc::new(AudioDeviceManager::new()?);
        let audio_capture_manager = Arc::new(AudioCaptureManager::new());
        let audio_playback_manager = Arc::new(AudioPlaybackManager::new());
        
        Ok(Self {
            app_state,
            backend_manager,
            websocket_manager,
            audio_device_manager,
            audio_capture_manager,
            audio_playback_manager,
        })
    }

    /// Configure l'AppHandle pour les événements WebSocket
    pub fn configure_app_handle(&self, app_handle: tauri::AppHandle) {
        // Nous n'avons plus besoin de cette logique car nous gérons le WebSocketManager différemment
        // Le WebSocketManager sera créé et configuré dans start_websocket_connection
    }
    
    /// Démarre la connexion WebSocket avec l'AppHandle configuré
    pub async fn start_websocket_connection(&self, ws_url: &str, app_handle: tauri::AppHandle) -> Result<(), String> {
        println!("🔗 Starting WebSocket connection to: {}", ws_url);
        
        // Arrêter l'ancienne connexion WebSocket si elle existe
        self.stop_websocket_connection().await;
        
        // Créer un nouveau WebSocketManager
        let mut ws_manager = WebSocketManager::new();
        
        // Obtenir le nom d'utilisateur depuis le state
        let username = if let Some(user) = self.app_state.get_user() {
            user.username.clone()
        } else {
            return Err("No user found in app state".to_string());
        };
        
        // Démarrer la connexion WebSocket
        match ws_manager.start(app_handle.clone(), ws_url.to_string(), username).await {
            Ok(()) => {
                println!("✅ WebSocket connection established successfully");
                
                // Stocker le manager dans le state
                let mut guard = self.websocket_manager.lock().await;
                *guard = Some(ws_manager);
                
                Ok(())
            }
            Err(e) => {
                eprintln!("❌ WebSocket connection failed: {}", e);
                Err(format!("WebSocket connection failed: {}", e))
            }
        }
    }

    /// Arrête la connexion WebSocket
    pub async fn stop_websocket_connection(&self) {
        println!("🛑 Stopping WebSocket connection...");
        
        let mut guard = self.websocket_manager.lock().await;
        if let Some(mut ws_manager) = guard.take() {
            let _ = ws_manager.stop().await;
            println!("✅ WebSocket connection stopped");
        } else {
            println!("ℹ️ No active WebSocket connection to stop");
        }
    }

    /// Met à jour le BackendManager avec une nouvelle URL
    pub fn update_backend_manager(&self, url: &str) {
        let new_manager = Arc::new(BackendManager::new(url, self.app_state.clone()));
        *self.backend_manager.write().unwrap() = new_manager;
    }

    /// Obtient une référence au BackendManager actuel
    pub fn get_backend_manager(&self) -> Arc<BackendManager> {
        self.backend_manager.read().unwrap().clone()
    }

    /// Configure l'audio UDP avec le backend
    pub async fn setup_audio_udp(&self, backend_host: &str) -> Result<(), String> {
        let backend_manager = self.get_backend_manager();
        
        // Configurer le client UDP (port par défaut 8082)
        backend_manager.setup_udp_client(backend_host, 8082).await
            .map_err(|e| format!("Failed to setup UDP client: {}", e))?;
            
        // Configurer l'AudioCaptureManager avec le client UDP
        if let Some(udp_client) = backend_manager.get_udp_client() {
            self.audio_capture_manager.set_udp_client(udp_client).await;
            // println!("Audio UDP configured successfully");
        }
        
        Ok(())
    }
}

// Commandes Tauri pour l'interface frontend

#[tauri::command]
async fn initialize_app(app: tauri::AppHandle, state: State<'_, TauriAppState>) -> Result<(), String> {
    println!("🚀 Initializing app with AppHandle...");
    
    // Configurer l'AppHandle pour les événements WebSocket
    state.configure_app_handle(app.clone());
    
    // Stocker l'AppHandle pour une utilisation ultérieure
    // Note: On peut utiliser app.clone() dans connect_to_server quand on en a besoin
    
    Ok(())
}

#[tauri::command]
async fn initialize_backend(state: State<'_, TauriAppState>) -> Result<(), String> {
    // Ne rien faire ici - l'initialisation se fera lors de la connexion au serveur
    println!("Backend initialized (no auto-connection)");
    Ok(())
}

#[tauri::command]
async fn start_websocket(app: tauri::AppHandle, ws_url: String, state: State<'_, TauriAppState>) -> Result<(), String> {
    println!("🔗 Starting WebSocket connection from command...");
    state.start_websocket_connection(&ws_url, app).await
}

#[tauri::command]
async fn stop_websocket(state: State<'_, TauriAppState>) -> Result<(), String> {
    println!("🛑 Stopping WebSocket connection from command...");
    state.stop_websocket_connection().await;
    Ok(())
}

#[tauri::command]
async fn connect_to_server(server_url: String, username: String, state: State<'_, TauriAppState>) -> Result<serde_json::Value, String> {
    // Mettre à jour le BackendManager avec la nouvelle URL
    state.update_backend_manager(&server_url);
    let backend_manager = state.get_backend_manager();
    
    // Tester la connexion
    match backend_manager.initialize().await {
        Ok(_) => {
            // Connecter l'utilisateur
            match backend_manager.connect_user(&username).await {
                Ok(_) => {
                    // Configurer l'audio UDP
                    let parsed_url = server_url.replace("http://", "").replace("https://", "");
                    let backend_host = parsed_url.split(':').next().unwrap_or("localhost");
                    
                    if let Err(e) = state.setup_audio_udp(backend_host).await {
                        eprintln!("Warning: Failed to setup audio UDP: {}", e);
                        // Continuer même si l'UDP échoue
                    }
                    
                    // La connexion WebSocket doit être démarrée séparément
                    // via la commande start_websocket après cette réponse
                    let backend_host = parsed_url.split(':').next().unwrap_or("localhost");
                    // WebSocket utilise le même port que HTTP avec route /ws
                    let websocket_url = format!("ws://{}:8080/ws", backend_host);
                    println!("✅ Server connection successful. WebSocket URL: {}", websocket_url);
                    
                    // Récupérer les channels
                    let channels = state.app_state.get_channels();
                    let user = state.app_state.get_user();
                    
                    Ok(serde_json::json!({
                        "success": true,
                        "user": user,
                        "channels": channels,
                        "websocketUrl": websocket_url
                    }))
                },
                Err(e) => {
                    eprintln!("Failed to connect user: {}", e);
                    Ok(serde_json::json!({
                        "success": false,
                        "error": format!("Failed to connect user: {}", e)
                    }))
                }
            }
        },
        Err(e) => {
            eprintln!("Failed to initialize backend: {}", e);
            Ok(serde_json::json!({
                "success": false,
                "error": format!("Cannot connect to server {}: {}", server_url, e)
            }))
        }
    }
}

#[tauri::command]
async fn connect_user(username: String, state: State<'_, TauriAppState>) -> Result<(), String> {
    state.get_backend_manager().connect_user(&username).await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn disconnect_user(state: State<'_, TauriAppState>) -> Result<(), String> {
    // Arrêter le WebSocket d'abord
    state.stop_websocket_connection().await;
    
    // Puis déconnecter l'utilisateur du backend
    state.get_backend_manager().disconnect_user().await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_channels(state: State<'_, TauriAppState>) -> Result<Vec<state::ChannelInfo>, String> {
    // Récupérer les channels depuis le backend (pas l'état local)
    let backend_manager = state.get_backend_manager();
    
    match backend_manager.get_channels().await {
        Ok(channels) => {
            // Mettre à jour l'état local avec les données fraîches
            state.app_state.update_channels(channels.clone());
            Ok(channels)
        },
        Err(e) => {
            eprintln!("Failed to get channels from backend: {}", e);
            // En cas d'erreur, retourner l'état local comme fallback
            Ok(state.app_state.get_channels())
        }
    }
}

#[tauri::command]
async fn join_channel(channel_id: String, state: State<'_, TauriAppState>) -> Result<(), String> {
    let uuid = Uuid::parse_str(&channel_id)
        .map_err(|e| format!("Invalid channel ID: {}", e))?;
    
    // Rejoindre le channel
    state.get_backend_manager().join_channel(uuid).await
        .map_err(|e| e.to_string())?;
    
    // println!("🎵 Successfully joined channel {}, starting audio playback and capture...", channel_id);
    
    // Démarrer automatiquement la lecture audio après avoir rejoint le channel
    if let Some(user) = state.app_state.get_user() {
        state.audio_playback_manager.set_user(user.id);
        
        // Configurer le device de sortie par défaut si pas encore fait
        if state.audio_playback_manager.get_device_name().is_none() {
            let _ = state.audio_playback_manager.set_device("default".to_string());
        }
        
        // Démarrer la lecture audio pour recevoir l'audio du channel
        // Utiliser le socket partagé du client UDP si disponible
        let udp_client_option = state.backend_manager.read().unwrap().get_udp_client();
        if let Some(udp_client) = udp_client_option {
            let shared_socket = udp_client.get_shared_socket();
            let server_addr = udp_client.get_server_addr(); // Utiliser la même adresse que le client UDP
            if let Err(e) = state.audio_playback_manager.start_playback_with_shared_socket(server_addr, shared_socket).await {
                // println!("⚠️ Warning: Failed to start audio playback with shared socket: {}", e);
                // Fallback vers la méthode normale
                if let Err(e2) = state.audio_playback_manager.start_playback(server_addr).await {
                    // println!("⚠️ Warning: Failed to start audio playback (fallback): {}", e2);
                }
            } else {
                // println!("✅ Audio playback started successfully with shared socket");
            }
        } else {
            // Pas de client UDP, utiliser l'adresse par défaut locale
            let server_addr: std::net::SocketAddr = "127.0.0.1:8082".parse()
                .map_err(|e| format!("Invalid server address: {}", e))?;
            if let Err(e) = state.audio_playback_manager.start_playback(server_addr).await {
                // println!("⚠️ Warning: Failed to start audio playback: {}", e);
            } else {
                // println!("✅ Audio playback started successfully");
            }
        }
        
        // Démarrer automatiquement la capture audio après avoir rejoint le channel
        if let Some(channel_id) = state.app_state.get_current_channel() {
            state.audio_capture_manager.set_user_and_channel(user.id, channel_id);
            
            // Configurer le device d'entrée par défaut si pas encore fait
            if let Err(e) = state.audio_capture_manager.set_device("default".to_string()) {
                // println!("⚠️ Warning: Failed to set audio input device: {}", e);
            }
            
            // Démarrer la capture audio pour envoyer notre voix
            if let Err(e) = state.audio_capture_manager.start_recording() {
                // println!("⚠️ Warning: Failed to start audio capture: {}", e);
                // Ne pas faire échouer le join pour autant
            } else {
                // println!("✅ Audio capture started successfully");
            }
        }
    }
    
    Ok(())
}

#[tauri::command]
async fn leave_current_channel(state: State<'_, TauriAppState>) -> Result<(), String> {
    // Quitter le channel
    state.get_backend_manager().leave_current_channel().await
        .map_err(|e| e.to_string())?;
    
    // println!("🎵 Left channel, stopping audio playback and capture...");
    
    // Arrêter la capture audio quand on quitte le channel
    if let Err(e) = state.audio_capture_manager.stop_recording() {
        // println!("⚠️ Warning: Failed to stop audio capture: {}", e);
    } else {
        // println!("✅ Audio capture stopped successfully");
    }
    
    // Arrêter la lecture audio quand on quitte le channel
    if let Err(e) = state.audio_playback_manager.stop_playback() {
        // println!("⚠️ Warning: Failed to stop audio playback: {}", e);
    } else {
        // println!("✅ Audio playback stopped successfully");
    }
    
    Ok(())
}

#[tauri::command]
async fn scan_audio_devices(state: State<'_, TauriAppState>) -> Result<state::AudioDevices, String> {
    state.audio_device_manager.scan_devices()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn select_input_device(device_id: String, state: State<'_, TauriAppState>) -> Result<(), String> {
    state.audio_device_manager.select_input_device(&device_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn select_output_device(device_id: String, state: State<'_, TauriAppState>) -> Result<(), String> {
    state.audio_playback_manager.set_device(device_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn start_audio_capture(state: State<'_, TauriAppState>) -> Result<(), String> {
    // Configurer l'utilisateur et le channel dans AudioCaptureManager
    if let (Some(user), Some(channel_id)) = (
        state.app_state.get_user(),
        state.app_state.get_current_channel()
    ) {
        state.audio_capture_manager.set_user_and_channel(user.id, channel_id);
        
        // Utiliser le périphérique par défaut ou celui configuré
        let device_name = "default".to_string();
        state.audio_capture_manager.set_device(device_name)
            .map_err(|e| format!("Failed to set audio device: {}", e))?;
            
        state.audio_capture_manager.start_recording()
            .map_err(|e| format!("Failed to start audio recording: {}", e))?;
            
        // println!("Audio capture started for user {} in channel {}", user.username, channel_id);
        Ok(())
    } else {
        Err("No user connected or no channel joined".to_string())
    }
}

#[tauri::command]
async fn stop_audio_capture(state: State<'_, TauriAppState>) -> Result<(), String> {
    state.audio_capture_manager.stop_recording()
        .map_err(|e| format!("Failed to stop audio recording: {}", e))?;
        
    // println!("Audio capture stopped");
    Ok(())
}

#[tauri::command]
async fn start_audio_playback(state: State<'_, TauriAppState>) -> Result<(), String> {
    // Configurer l'utilisateur actuel
    if let Some(user) = state.app_state.get_user() {
        state.audio_playback_manager.set_user(user.id);
        
        // Utiliser l'adresse du serveur UDP du client UDP si disponible
        let udp_client_option = state.backend_manager.read().unwrap().get_udp_client();
        let server_addr = if let Some(udp_client) = &udp_client_option {
            udp_client.get_server_addr()
        } else {
            // Fallback vers l'adresse par défaut locale
            "127.0.0.1:8082".parse()
                .map_err(|e| format!("Invalid server address: {}", e))?
        };
        
        // Utiliser le socket partagé du client UDP si disponible
        if let Some(udp_client) = udp_client_option {
            let shared_socket = udp_client.get_shared_socket();
            state.audio_playback_manager.start_playback_with_shared_socket(server_addr, shared_socket).await
                .map_err(|e| e.to_string())
        } else {
            // Pas de client UDP, utiliser la méthode normale
            state.audio_playback_manager.start_playback(server_addr).await
                .map_err(|e| e.to_string())
        }
    } else {
        Err("No user connected".to_string())
    }
}

#[tauri::command]
async fn stop_audio_playback(state: State<'_, TauriAppState>) -> Result<(), String> {
    state.audio_playback_manager.stop_playback()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn play_test_sound(state: State<'_, TauriAppState>) -> Result<(), String> {
    // Simple son de test (pas encore implémenté dans le nouveau AudioPlaybackManager)
    println!("🔊 Test sound requested");
    Ok(())
}

#[tauri::command]
async fn get_connection_state(state: State<'_, TauriAppState>) -> Result<state::ConnectionState, String> {
    Ok(state.app_state.get_connection_state())
}

#[tauri::command]
async fn get_current_user(state: State<'_, TauriAppState>) -> Result<Option<state::UserState>, String> {
    Ok(state.app_state.get_user())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialiser l'état de l'application
    let tauri_state = TauriAppState::new()
        .expect("Failed to initialize application state");
    
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(tauri_state)
        .invoke_handler(tauri::generate_handler![
            initialize_app,
            initialize_backend,
            start_websocket,
            stop_websocket,
            connect_to_server,
            connect_user,
            disconnect_user,
            get_channels,
            join_channel,
            leave_current_channel,
            scan_audio_devices,
            select_input_device,
            select_output_device,
            start_audio_capture,
            stop_audio_capture,
            start_audio_playback,
            stop_audio_playback,
            play_test_sound,
            get_connection_state,
            get_current_user
        ])
        .on_window_event(|window, event| {
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    println!("🚪 Application closing, checking for active user...");
                    
                    // Obtenir l'état de l'application
                    let app_handle = window.app_handle();
                    let state = app_handle.state::<TauriAppState>();
                    
                    // Vérifier s'il y a un utilisateur connecté
                    if let Some(user) = state.app_state.get_user() {
                        // Bloquer la fermeture seulement s'il y a un utilisateur connecté
                        api.prevent_close();
                        
                        println!("🔄 User {} is connected, disconnecting before close...", user.username);
                        
                        // Cloner les éléments nécessaires pour le bloc async
                        let backend_manager = state.get_backend_manager();
                        let app_handle_clone = app_handle.clone();
                        
                        // Déconnecter en arrière-plan
                        tauri::async_runtime::spawn(async move {
                            let state = app_handle_clone.state::<TauriAppState>();
                            
                            // Arrêter le WebSocket d'abord
                            state.stop_websocket_connection().await;
                            
                            // Puis déconnecter l'utilisateur
                            match backend_manager.disconnect_user().await {
                                Ok(_) => {
                                    println!("✅ User disconnected successfully, closing application");
                                },
                                Err(e) => {
                                    println!("⚠️ Failed to disconnect user: {}, but closing anyway", e);
                                }
                            }
                            
                            // Fermer l'application après la déconnexion
                            app_handle_clone.exit(0);
                        });
                    } else {
                        println!("👤 No user connected, allowing natural close");
                        // Pas d'utilisateur connecté, laisser la fermeture naturelle se faire
                        // Ne pas appeler prevent_close() ni window.close()
                    }
                },
                _ => {}
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
