use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, Host, SampleFormat, SampleRate, Stream, StreamConfig,
};
use crate::state::{AudioDevice, AudioDevices};
use anyhow::{Result, Context};
use std::sync::Arc;
use parking_lot::RwLock;

/// Gestionnaire des périphériques audio
pub struct AudioDeviceManager {
    host: Host,
    devices: Arc<RwLock<AudioDevices>>,
}

impl AudioDeviceManager {
    pub fn new() -> Result<Self> {
        let host = cpal::default_host();
        
        Ok(Self {
            host,
            devices: Arc::new(RwLock::new(AudioDevices::default())),
        })
    }

    /// Scanne et met à jour la liste des périphériques audio
    pub fn scan_devices(&self) -> Result<AudioDevices> {
        println!("Scanning audio devices...");
        
        let mut input_devices = Vec::new();
        let mut output_devices = Vec::new();

        // Périphériques d'entrée (microphones)
        match self.host.input_devices() {
            Ok(input_device_iter) => {
                for device in input_device_iter {
                    if let Ok(name) = device.name() {
                        let is_default = self.host.default_input_device()
                            .map(|d| d.name().unwrap_or_default() == name)
                            .unwrap_or(false);

                        println!("Found input device: {} (default: {})", name, is_default);
                        
                        input_devices.push(AudioDevice {
                            name: name.clone(),
                            id: name, // Pour CPAL, on utilise le nom comme ID
                            is_default,
                        });
                    }
                }
            }
            Err(e) => {
                println!("Warning: Failed to enumerate input devices: {}", e);
            }
        }

        // Périphériques de sortie (haut-parleurs/casques)
        match self.host.output_devices() {
            Ok(output_device_iter) => {
                for device in output_device_iter {
                    if let Ok(name) = device.name() {
                        let is_default = self.host.default_output_device()
                            .map(|d| d.name().unwrap_or_default() == name)
                            .unwrap_or(false);

                        println!("Found output device: {} (default: {})", name, is_default);
                        
                        output_devices.push(AudioDevice {
                            name: name.clone(),
                            id: name,
                            is_default,
                        });
                    }
                }
            }
            Err(e) => {
                println!("Warning: Failed to enumerate output devices: {}", e);
            }
        }

        // Sélectionner les périphériques par défaut
        let selected_input = input_devices.iter()
            .find(|d| d.is_default)
            .map(|d| d.id.clone());

        let selected_output = output_devices.iter()
            .find(|d| d.is_default)
            .map(|d| d.id.clone());

        let audio_devices = AudioDevices {
            input_devices,
            output_devices,
            selected_input,
            selected_output,
        };

        println!("Audio scan complete: {} input, {} output devices", 
                 audio_devices.input_devices.len(), 
                 audio_devices.output_devices.len());

        // Mettre à jour l'état interne
        *self.devices.write() = audio_devices.clone();

        Ok(audio_devices)
    }

    /// Obtient les périphériques audio actuellement scannés
    pub fn get_devices(&self) -> AudioDevices {
        self.devices.read().clone()
    }

    /// Sélectionne un périphérique d'entrée
    pub fn select_input_device(&self, device_id: &str) -> Result<()> {
        let mut devices = self.devices.write();
        
        // Vérifier que le périphérique existe
        if devices.input_devices.iter().any(|d| d.id == device_id) {
            devices.selected_input = Some(device_id.to_string());
            Ok(())
        } else {
            anyhow::bail!("Input device not found: {}", device_id)
        }
    }

    /// Sélectionne un périphérique de sortie
    pub fn select_output_device(&self, device_id: &str) -> Result<()> {
        let mut devices = self.devices.write();
        
        // Vérifier que le périphérique existe
        if devices.output_devices.iter().any(|d| d.id == device_id) {
            devices.selected_output = Some(device_id.to_string());
            Ok(())
        } else {
            anyhow::bail!("Output device not found: {}", device_id)
        }
    }

    /// Obtient le périphérique d'entrée sélectionné
    pub fn get_input_device(&self) -> Result<Option<Device>> {
        let devices = self.devices.read();
        
        if let Some(selected_id) = &devices.selected_input {
            // Chercher le périphérique par nom
            let input_devices = self.host.input_devices()
                .context("Failed to get input devices")?;
            
            for device in input_devices {
                if let Ok(name) = device.name() {
                    if name == *selected_id {
                        return Ok(Some(device));
                    }
                }
            }
        }
        
        Ok(None)
    }

    /// Obtient le périphérique de sortie sélectionné
    pub fn get_output_device(&self) -> Result<Option<Device>> {
        let devices = self.devices.read();
        
        if let Some(selected_id) = &devices.selected_output {
            // Chercher le périphérique par nom
            let output_devices = self.host.output_devices()
                .context("Failed to get output devices")?;
            
            for device in output_devices {
                if let Ok(name) = device.name() {
                    if name == *selected_id {
                        return Ok(Some(device));
                    }
                }
            }
        }
        
        Ok(None)
    }

    /// Teste un périphérique d'entrée
    pub fn test_input_device(&self, device_id: &str) -> Result<bool> {
        // Trouver le périphérique
        let input_devices = self.host.input_devices()
            .context("Failed to get input devices")?;
        
        for device in input_devices {
            if let Ok(name) = device.name() {
                if name == device_id {
                    // Essayer de créer une configuration par défaut
                    let config = device.default_input_config()
                        .context("Failed to get default input config")?;
                    
                    // Vérifier que le périphérique supporte au moins un format audio
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }

    /// Teste un périphérique de sortie
    pub fn test_output_device(&self, device_id: &str) -> Result<bool> {
        // Trouver le périphérique
        let output_devices = self.host.output_devices()
            .context("Failed to get output devices")?;
        
        for device in output_devices {
            if let Ok(name) = device.name() {
                if name == device_id {
                    // Essayer de créer une configuration par défaut
                    let config = device.default_output_config()
                        .context("Failed to get default output config")?;
                    
                    // Vérifier que le périphérique supporte au moins un format audio
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }
}

impl Default for AudioDeviceManager {
    fn default() -> Self {
        Self::new().expect("Failed to create AudioDeviceManager")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_device_manager_creation() {
        let manager = AudioDeviceManager::new();
        assert!(manager.is_ok());
    }

    #[test]
    fn test_device_scanning() {
        let manager = AudioDeviceManager::new().unwrap();
        let devices = manager.scan_devices();
        
        // On ne peut pas garantir qu'il y aura des périphériques, 
        // mais la fonction ne doit pas planter
        assert!(devices.is_ok());
    }

    #[test]
    fn test_device_selection() {
        let manager = AudioDeviceManager::new().unwrap();
        
        // Tester la sélection d'un périphérique inexistant
        let result = manager.select_input_device("nonexistent_device");
        assert!(result.is_err());
    }
}