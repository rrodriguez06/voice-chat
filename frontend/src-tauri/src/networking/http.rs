use crate::state::{AppState, UserState, ChannelInfo, ConnectionState};
use anyhow::{Result, Context};
use reqwest::Client;
use serde_json::Value;
use uuid::Uuid;
use super::udp::AudioUdpClient;

/// Client HTTP pour communiquer avec le backend
pub struct BackendClient {
    client: Client,
    base_url: String,
}

impl BackendClient {
    pub fn new(backend_url: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: backend_url.to_string(),
        }
    }

    /// Teste la connexion au backend
    pub async fn test_connection(&self) -> Result<bool> {
        let url = format!("{}/health", self.base_url);
        
        match self.client.get(&url).send().await {
            Ok(response) => Ok(response.status().is_success()),
            Err(_) => Ok(false),
        }
    }

    /// Connecte un utilisateur avec son username
    pub async fn connect_user(&self, username: &str) -> Result<UserState> {
        let url = format!("{}/api/users", self.base_url);
        
        let payload = serde_json::json!({
            "username": username
        });
        
        let response = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Failed to send user creation request")?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to create user: {}", response.status());
        }
        
        let user_data: Value = response.json().await
            .context("Failed to parse user creation response")?;
        
        Ok(UserState {
            id: Uuid::parse_str(user_data["id"].as_str().unwrap_or(""))
                .context("Invalid user ID format")?,
            username: user_data["username"].as_str()
                .unwrap_or("")
                .to_string(),
            connected_at: user_data["connected_at"].as_u64()
                .unwrap_or(0),
        })
    }

    /// Déconnecte un utilisateur du serveur
    pub async fn disconnect_user(&self, user_id: Uuid) -> Result<()> {
        let url = format!("{}/api/users/{}/disconnect", self.base_url, user_id);
        
        println!("🔌 Sending disconnect request to: {}", url);
        
        let response = self.client
            .post(&url)
            .send()
            .await
            .context("Failed to send disconnect request")?;
        
        println!("📡 Disconnect response status: {}", response.status());
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to disconnect user: {}", response.status());
        }
        
        println!("✅ User disconnected successfully");
        Ok(())
    }

    /// Récupère la liste des channels disponibles
    pub async fn get_channels(&self) -> Result<Vec<ChannelInfo>> {
        let url = format!("{}/api/channels", self.base_url);
        
        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to fetch channels")?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to get channels: {}", response.status());
        }
        
        let channels_data: Value = response.json().await
            .context("Failed to parse channels response")?;
        
        let mut channels = Vec::new();
        
        if let Some(channels_array) = channels_data.as_array() {
            for channel_data in channels_array {
                if let Ok(channel) = self.parse_channel_info(channel_data) {
                    channels.push(channel);
                }
            }
        }
        
        Ok(channels)
    }

    /// Rejoint un channel
    pub async fn join_channel(&self, user_id: Uuid, channel_id: Uuid) -> Result<()> {
        let url = format!("{}/api/channels/{}/join", self.base_url, channel_id);
        
        println!("🔗 Joining channel: {} for user: {}", channel_id, user_id);
        println!("📡 POST {}", url);
        
        let payload = serde_json::json!({
            "user_id": user_id.to_string()
        });
        
        println!("📦 Request payload: {}", payload);
        
        let response = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Failed to join channel")?;
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to join channel: {}", response.status());
        }
        
        Ok(())
    }

    /// Quitte un channel
    pub async fn leave_channel(&self, user_id: Uuid, channel_id: Uuid) -> Result<()> {
        let url = format!("{}/api/channels/{}/leave", self.base_url, channel_id);
        
        println!("🚪 Leaving channel: {} for user: {}", channel_id, user_id);
        println!("📡 POST {}", url);
        
        let payload = serde_json::json!({
            "user_id": user_id.to_string()
        });
        
        println!("📦 Request payload: {}", payload);
        
        let response = self.client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Failed to leave channel")?;
        
        println!("📡 Leave channel response status: {}", response.status());
        
        if !response.status().is_success() {
            anyhow::bail!("Failed to leave channel: {}", response.status());
        }
        
        Ok(())
    }

    /// Parse les données d'un channel depuis JSON
    fn parse_channel_info(&self, data: &Value) -> Result<ChannelInfo> {
        // Parser les utilisateurs du channel
        let mut users = Vec::new();
        if let Some(users_array) = data["users"].as_array() {
            for user_data in users_array {
                if let Ok(user) = self.parse_user_info(user_data) {
                    users.push(user);
                }
            }
        }

        Ok(ChannelInfo {
            id: Uuid::parse_str(data["id"].as_str().unwrap_or(""))
                .context("Invalid channel ID")?,
            name: data["name"].as_str()
                .unwrap_or("Unknown")
                .to_string(),
            description: data["description"].as_str()
                .map(|s| s.to_string()),
            // Le backend utilise "userCount" (camelCase)
            user_count: data["userCount"].as_u64()
                .unwrap_or(0) as usize,
            users,
        })
    }

    /// Parse les données d'un utilisateur depuis JSON
    fn parse_user_info(&self, data: &Value) -> Result<crate::state::UserInfo> {
        Ok(crate::state::UserInfo {
            id: Uuid::parse_str(data["id"].as_str().unwrap_or(""))
                .context("Invalid user ID")?,
            username: data["username"].as_str()
                .unwrap_or("Unknown")
                .to_string(),
            is_speaking: data["isSpeaking"].as_bool()
                .unwrap_or(false),
            mic_enabled: data["micEnabled"].as_bool()
                .unwrap_or(true),
            speaker_enabled: data["speakerEnabled"].as_bool()
                .unwrap_or(true),
        })
    }
}

use std::sync::Arc;
use parking_lot::RwLock;

/// Gestionnaire de la communication avec le backend
pub struct BackendManager {
    client: BackendClient,
    app_state: AppState,
    udp_client: Arc<RwLock<Option<AudioUdpClient>>>,
}

impl BackendManager {
    pub fn new(backend_url: &str, app_state: AppState) -> Self {
        Self {
            client: BackendClient::new(backend_url),
            app_state,
            udp_client: Arc::new(RwLock::new(None)),
        }
    }

    /// Initialise la connexion avec le backend
    pub async fn initialize(&self) -> Result<()> {
        // Tester la connexion
        self.app_state.set_connection_state(ConnectionState::Connecting);
        
        let is_connected = self.client.test_connection().await
            .context("Failed to test backend connection")?;
        
        if is_connected {
            self.app_state.set_connection_state(ConnectionState::Connected);
            
            // Charger les channels disponibles
            let channels = self.client.get_channels().await
                .context("Failed to load channels")?;
            self.app_state.update_channels(channels);
            
            Ok(())
        } else {
            self.app_state.set_connection_state(ConnectionState::Error);
            anyhow::bail!("Cannot connect to backend")
        }
    }

    /// Configure le client UDP pour l'audio
    pub async fn setup_udp_client(&self, backend_host: &str, udp_port: u16) -> Result<()> {
        let server_addr = format!("{}:{}", backend_host, udp_port).parse()
            .context("Invalid UDP server address")?;
            
        let udp_client = AudioUdpClient::new(server_addr).await
            .context("Failed to create UDP client")?;
            
        *self.udp_client.write() = Some(udp_client);
        
        println!("UDP client configured for {}:{}", backend_host, udp_port);
        Ok(())
    }

    /// Obtient le client UDP pour l'audio
    pub fn get_udp_client(&self) -> Option<AudioUdpClient> {
        self.udp_client.read().clone()
    }

    /// Connecte un utilisateur
    pub async fn connect_user(&self, username: &str) -> Result<()> {
        let user = self.client.connect_user(username).await
            .context("Failed to connect user")?;
        
        self.app_state.set_user(user);
        Ok(())
    }

    /// Déconnecte l'utilisateur actuel
    pub async fn disconnect_user(&self) -> Result<()> {
        if let Some(user) = self.app_state.get_user() {
            println!("🔄 Starting disconnect process for user: {} ({})", user.username, user.id);
            
            // Quitter le channel actuel si présent
            if self.app_state.get_current_channel().is_some() {
                println!("📤 Leaving current channel before disconnect");
                let _ = self.leave_current_channel().await;
            }
            
            // Déconnecter du serveur
            println!("🌐 Calling backend disconnect for user: {}", user.id);
            self.client.disconnect_user(user.id).await
                .context("Failed to disconnect user")?;
            
            // Nettoyer l'état local
            println!("🧹 Cleaning up local state");
            self.app_state.clear_user();
            self.app_state.set_current_channel(None);
            self.app_state.set_connection_state(ConnectionState::Disconnected);
            
            println!("✅ Disconnect process completed");
            Ok(())
        } else {
            anyhow::bail!("No user to disconnect")
        }
    }

    /// Rejoint un channel
    pub async fn join_channel(&self, channel_id: Uuid) -> Result<()> {
        if let Some(user) = self.app_state.get_user() {
            println!("🏠 BackendManager: Joining channel {} for user {}", channel_id, user.id);
            
            self.client.join_channel(user.id, channel_id).await
                .context("Failed to join channel")?;
            
            println!("✅ Successfully joined channel: {}", channel_id);
            self.app_state.set_current_channel(Some(channel_id));
            
            // Commencer l'audio si on a un client UDP configuré
            if let Some(udp_client) = self.get_udp_client() {
                println!("🎤 Audio UDP client available, ready for streaming");
            } else {
                println!("⚠️ No UDP client configured, audio won't work");
            }
            
            Ok(())
        } else {
            anyhow::bail!("No user connected")
        }
    }

    /// Quitte le channel actuel
    pub async fn leave_current_channel(&self) -> Result<()> {
        if let (Some(user), Some(channel_id)) = (
            self.app_state.get_user(),
            self.app_state.get_current_channel()
        ) {
            self.client.leave_channel(user.id, channel_id).await
                .context("Failed to leave channel")?;
            
            self.app_state.set_current_channel(None);
            Ok(())
        } else {
            anyhow::bail!("No user or channel to leave")
        }
    }

    /// Rafraîchit la liste des channels
    pub async fn refresh_channels(&self) -> Result<()> {
        let channels = self.client.get_channels().await
            .context("Failed to refresh channels")?;
        
        self.app_state.update_channels(channels);
        Ok(())
    }

    /// Récupère la liste des channels depuis le backend
    pub async fn get_channels(&self) -> Result<Vec<ChannelInfo>> {
        let channels = self.client.get_channels().await
            .context("Failed to get channels")?;
        
        // Mettre à jour l'état local aussi
        self.app_state.update_channels(channels.clone());
        Ok(channels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backend_client_creation() {
        let client = BackendClient::new("http://localhost:3000");
        assert_eq!(client.base_url, "http://localhost:3000");
    }

    #[tokio::test]
    async fn test_backend_manager_creation() {
        let state = AppState::new();
        let manager = BackendManager::new("http://localhost:3000", state);
        // Test basique - ne nécessite pas de vraie connexion
    }
}