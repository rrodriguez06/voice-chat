use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::{
    models::{User, CreateUserRequest, UserResponse},
    Error, Result,
};

#[derive(Debug)]
pub struct UserService {
    users: Arc<DashMap<Uuid, User>>,
    username_to_id: Arc<DashMap<String, Uuid>>,
}

impl UserService {
    pub fn new() -> Self {
        Self {
            users: Arc::new(DashMap::new()),
            username_to_id: Arc::new(DashMap::new()),
        }
    }

    pub fn create_user(&self, request: CreateUserRequest) -> Result<UserResponse> {
        // Check if username already exists
        if self.username_to_id.contains_key(&request.username) {
            return Err(Error::User(format!(
                "Username '{}' is already taken",
                request.username
            )));
        }

        // Validate username
        if request.username.trim().is_empty() {
            return Err(Error::User("Username cannot be empty".to_string()));
        }

        if request.username.len() > 50 {
            return Err(Error::User("Username too long (max 50 characters)".to_string()));
        }

        // Create new user
        let user = User::new(request.username.clone());
        let user_id = user.id;
        let response = UserResponse::from(user.clone());

        // Store user
        self.users.insert(user_id, user);
        self.username_to_id.insert(request.username, user_id);

        tracing::info!("Created new user: {} ({})", response.username, user_id);
        Ok(response)
    }

    pub fn get_user(&self, user_id: &Uuid) -> Result<UserResponse> {
        match self.users.get(user_id) {
            Some(user) => Ok(UserResponse::from(user.clone())),
            None => Err(Error::User(format!("User {} not found", user_id))),
        }
    }

    pub fn get_user_by_username(&self, username: &str) -> Result<UserResponse> {
        let user_id = self.username_to_id
            .get(username)
            .ok_or_else(|| Error::User(format!("User '{}' not found", username)))?;
        
        self.get_user(&user_id)
    }

    pub fn update_user_status(&self, user_id: &Uuid, status: crate::models::user::UserStatus) -> Result<()> {
        match self.users.get_mut(user_id) {
            Some(mut user) => {
                user.set_status(status);
                Ok(())
            }
            None => Err(Error::User(format!("User {} not found", user_id))),
        }
    }

    pub fn user_join_channel(&self, user_id: &Uuid, channel_id: Uuid) -> Result<()> {
        match self.users.get_mut(user_id) {
            Some(mut user) => {
                user.join_channel(channel_id);
                tracing::info!("User {} joined channel {}", user_id, channel_id);
                Ok(())
            }
            None => Err(Error::User(format!("User {} not found", user_id))),
        }
    }

    pub fn user_leave_channel(&self, user_id: &Uuid) -> Result<()> {
        match self.users.get_mut(user_id) {
            Some(mut user) => {
                let channel_id = user.current_channel;
                user.leave_channel();
                if let Some(channel_id) = channel_id {
                    tracing::info!("User {} left channel {}", user_id, channel_id);
                }
                Ok(())
            }
            None => Err(Error::User(format!("User {} not found", user_id))),
        }
    }

    pub fn remove_user(&self, user_id: &Uuid) -> Result<()> {
        if let Some((_, user)) = self.users.remove(user_id) {
            self.username_to_id.remove(&user.username);
            tracing::info!("Removed user: {} ({})", user.username, user_id);
            Ok(())
        } else {
            Err(Error::User(format!("User {} not found", user_id)))
        }
    }

    pub fn list_users(&self) -> Vec<UserResponse> {
        self.users
            .iter()
            .map(|entry| UserResponse::from(entry.value().clone()))
            .collect()
    }

    pub fn get_users_in_channel(&self, channel_id: &Uuid) -> Vec<UserResponse> {
        self.users
            .iter()
            .filter(|entry| entry.value().current_channel == Some(*channel_id))
            .map(|entry| UserResponse::from(entry.value().clone()))
            .collect()
    }
}