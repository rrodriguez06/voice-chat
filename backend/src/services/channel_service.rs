use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    config::LimitsConfig,
    models::{
        Channel, CreateChannelRequest, JoinChannelRequest, 
        ChannelResponse, DetailedChannelResponse
    },
    Error, Result,
};

// ID du channel par défaut qui ne peut pas être supprimé
const DEFAULT_CHANNEL_ID: &str = "00000000-0000-0000-0000-000000000001";
const SYSTEM_USER_ID: &str = "00000000-0000-0000-0000-000000000000";

#[derive(Debug)]
pub struct ChannelService {
    channels: Arc<DashMap<Uuid, Channel>>,
    limits: LimitsConfig,
}

impl ChannelService {
    pub fn new(limits: LimitsConfig) -> Self {
        let service = Self {
            channels: Arc::new(DashMap::new()),
            limits,
        };
        
        // Créer le channel par défaut
        service.create_default_channel();
        
        service
    }

    /// Crée un channel par défaut qui ne peut pas être supprimé
    fn create_default_channel(&self) {
        use crate::models::Channel;
        
        // ID fixe pour le channel par défaut (pour éviter qu'il soit supprimé)
        let default_channel_id = Uuid::parse_str(DEFAULT_CHANNEL_ID)
            .expect("Failed to parse default channel UUID");
        
        // Créer le channel par défaut avec un owner system (ID spécial)
        let system_user_id = Uuid::parse_str(SYSTEM_USER_ID)
            .expect("Failed to parse system user UUID");
        
        let default_channel = Channel {
            id: default_channel_id,
            name: "General".to_string(),
            description: Some("Default channel for all users".to_string()),
            owner_id: system_user_id,
            max_users: self.limits.max_users_per_channel,
            current_users: Vec::new(),
            is_private: false,
            password: None,
            created_at: std::time::SystemTime::now(),
        };
        
        self.channels.insert(default_channel_id, default_channel);
        tracing::info!("Created default channel 'General' ({})", default_channel_id);
    }

    pub fn create_channel(
        &self,
        request: CreateChannelRequest,
        owner_id: Uuid,
    ) -> Result<ChannelResponse> {
        // Check limits
        if self.channels.len() >= self.limits.max_channels {
            return Err(Error::Channel("Maximum number of channels reached".to_string()));
        }

        // Validate channel name
        if request.name.trim().is_empty() {
            return Err(Error::Channel("Channel name cannot be empty".to_string()));
        }

        if request.name.len() > 100 {
            return Err(Error::Channel("Channel name too long (max 100 characters)".to_string()));
        }

        // Check if channel name already exists
        if self.channels.iter().any(|entry| entry.value().name == request.name) {
            return Err(Error::Channel(format!(
                "Channel name '{}' is already taken",
                request.name
            )));
        }

        let max_users = request.max_users
            .unwrap_or(self.limits.max_users_per_channel)
            .min(self.limits.max_users_per_channel);

        let channel = Channel::new(
            request.name,
            request.description,
            owner_id,
            max_users,
            request.is_private.unwrap_or(false),
            request.password,
        );

        let channel_id = channel.id;
        let response = ChannelResponse::from(channel.clone());

        self.channels.insert(channel_id, channel);

        tracing::info!("Created new channel: {} ({})", response.name, channel_id);
        Ok(response)
    }

    pub fn get_channel(&self, channel_id: &Uuid) -> Result<DetailedChannelResponse> {
        match self.channels.get(channel_id) {
            Some(channel) => Ok(DetailedChannelResponse::from(channel.clone())),
            None => Err(Error::Channel(format!("Channel {} not found", channel_id))),
        }
    }

    pub fn list_channels(&self) -> Vec<ChannelResponse> {
        self.channels
            .iter()
            .filter(|entry| !entry.value().is_private) // Only show public channels
            .map(|entry| ChannelResponse::from(entry.value().clone()))
            .collect()
    }

    pub fn join_channel(
        &self,
        channel_id: &Uuid,
        user_id: Uuid,
        request: Option<JoinChannelRequest>,
    ) -> Result<()> {
        let mut channel = self.channels
            .get_mut(channel_id)
            .ok_or_else(|| Error::Channel(format!("Channel {} not found", channel_id)))?;

        let password = request.and_then(|r| r.password);

        if !channel.can_join(&user_id, password.as_deref()) {
            return Err(Error::Channel("Cannot join channel".to_string()));
        }

        if channel.add_user(user_id) {
            tracing::info!("User {} joined channel {}", user_id, channel_id);
            Ok(())
        } else {
            Err(Error::Channel("Failed to join channel".to_string()))
        }
    }

    pub fn leave_channel(&self, channel_id: &Uuid, user_id: &Uuid) -> Result<()> {
        let mut channel = self.channels
            .get_mut(channel_id)
            .ok_or_else(|| Error::Channel(format!("Channel {} not found", channel_id)))?;

        if channel.remove_user(user_id) {
            tracing::info!("User {} left channel {}", user_id, channel_id);
            
            // Remove empty channels (except if owner is still around)
            if channel.is_empty() {
                // For now, keep empty channels. We could implement auto-cleanup later
                tracing::debug!("Channel {} is now empty", channel_id);
            }
            
            Ok(())
        } else {
            Err(Error::Channel(format!("User {} not in channel {}", user_id, channel_id)))
        }
    }

    pub fn delete_channel(&self, channel_id: &Uuid, requester_id: &Uuid) -> Result<()> {
        // Vérifier si c'est le channel par défaut
        let default_channel_id = Uuid::parse_str(DEFAULT_CHANNEL_ID)
            .expect("Failed to parse default channel UUID");
        
        if *channel_id == default_channel_id {
            return Err(Error::Channel("Cannot delete the default channel".to_string()));
        }
        
        let channel = self.channels
            .get(channel_id)
            .ok_or_else(|| Error::Channel(format!("Channel {} not found", channel_id)))?;

        if !channel.is_owner(requester_id) {
            return Err(Error::Channel("Only channel owner can delete the channel".to_string()));
        }

        drop(channel); // Release the read lock

        if let Some((_, channel)) = self.channels.remove(channel_id) {
            tracing::info!("Deleted channel: {} ({})", channel.name, channel_id);
            Ok(())
        } else {
            Err(Error::Channel(format!("Channel {} not found", channel_id)))
        }
    }

    pub fn get_user_channels(&self, user_id: &Uuid) -> Vec<ChannelResponse> {
        self.channels
            .iter()
            .filter(|entry| entry.value().current_users.contains(user_id))
            .map(|entry| ChannelResponse::from(entry.value().clone()))
            .collect()
    }

    /// Retourne l'ID du channel par défaut
    pub fn get_default_channel_id() -> Uuid {
        Uuid::parse_str(DEFAULT_CHANNEL_ID)
            .expect("Failed to parse default channel UUID")
    }
    
    /// Vérifie si un channel est le channel par défaut
    pub fn is_default_channel(channel_id: &Uuid) -> bool {
        *channel_id == Self::get_default_channel_id()
    }

    pub fn get_users_in_channel(&self, channel_id: &Uuid) -> Result<Vec<Uuid>> {
        match self.channels.get(channel_id) {
            Some(channel) => Ok(channel.current_users.clone()),
            None => Err(Error::Channel(format!("Channel {} not found", channel_id))),
        }
    }

    // Force remove user from all channels (useful when user disconnects)
    pub fn remove_user_from_all_channels(&self, user_id: &Uuid) {
        for mut channel in self.channels.iter_mut() {
            if channel.remove_user(user_id) {
                tracing::info!("Removed user {} from channel {} due to disconnect", 
                    user_id, channel.id);
            }
        }
    }
}