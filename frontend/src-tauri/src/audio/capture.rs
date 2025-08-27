use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, SampleFormat, StreamConfig,
};
use anyhow::{Result, Context};
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::mpsc;
use uuid::Uuid;
use crate::networking::AudioUdpClient;

/// Gestionnaire de capture audio (microphone)
/// Ne stocke pas le Stream directement pour éviter les problèmes Send/Sync
#[derive(Debug)]
pub struct AudioCaptureManager {
    device_name: Arc<RwLock<Option<String>>>,
    is_recording: Arc<RwLock<bool>>,
    udp_client: Arc<RwLock<Option<AudioUdpClient>>>,
    user_id: Arc<RwLock<Option<Uuid>>>,
    channel_id: Arc<RwLock<Option<Uuid>>>,
    // Utiliser un channel pour contrôler l'enregistrement
    control_tx: Arc<RwLock<Option<mpsc::UnboundedSender<bool>>>>,
}

impl AudioCaptureManager {
    pub fn new() -> Self {
        Self {
            device_name: Arc::new(RwLock::new(None)),
            is_recording: Arc::new(RwLock::new(false)),
            udp_client: Arc::new(RwLock::new(None)),
            user_id: Arc::new(RwLock::new(None)),
            channel_id: Arc::new(RwLock::new(None)),
            control_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Configure le client UDP pour l'audio
    pub async fn set_udp_client(&self, client: AudioUdpClient) {
        *self.udp_client.write() = Some(client);
    }

    /// Configure l'utilisateur et le channel
    pub fn set_user_and_channel(&self, user_id: Uuid, channel_id: Uuid) {
        *self.user_id.write() = Some(user_id);
        *self.channel_id.write() = Some(channel_id);
    }

    /// Configure le périphérique de capture
    pub fn set_device(&self, device_name: String) -> Result<()> {
        // Arrêter le stream actuel s'il existe
        self.stop_recording()?;
        
        *self.device_name.write() = Some(device_name);
        Ok(())
    }

    /// Démarre l'enregistrement audio
    pub fn start_recording(&self) -> Result<()> {
        println!("🎤 AudioCaptureManager: Starting audio recording...");
        
        if *self.is_recording.read() {
            println!("⚠️ AudioCaptureManager: Already recording, ignoring start request");
            return Ok(()); // Déjà en cours d'enregistrement
        }

        let device_name = self.device_name.read()
            .as_ref()
            .context("No audio device configured")?
            .clone();
        println!("🎤 AudioCaptureManager: Using device: {}", device_name);

        let udp_client = self.udp_client.read().clone()
            .context("No UDP client configured")?;
        println!("🎤 AudioCaptureManager: UDP client configured");
        
        let user_id = self.user_id.read()
            .context("No user ID configured")?;
        println!("🎤 AudioCaptureManager: User ID: {}", user_id);
        
        let channel_id = self.channel_id.read()
            .context("No channel ID configured")?;
        println!("🎤 AudioCaptureManager: Channel ID: {}", channel_id);

        // Créer un channel de contrôle
        let (control_tx, mut control_rx) = mpsc::unbounded_channel::<bool>();
        *self.control_tx.write() = Some(control_tx);

        let is_recording = self.is_recording.clone();
        
        // Démarrer l'enregistrement dans une tâche séparée
        println!("🎤 AudioCaptureManager: Spawning capture task...");
        tokio::spawn(async move {
            if let Err(e) = Self::start_capture_task(
                device_name,
                udp_client,
                user_id,
                channel_id,
                is_recording,
                &mut control_rx,
            ).await {
                eprintln!("❌ Audio capture error: {}", e);
            }
        });

        *self.is_recording.write() = true;
        println!("✅ AudioCaptureManager: Audio recording started successfully");
        Ok(())
    }

    /// Tâche de capture audio (isolée du state Tauri)
    async fn start_capture_task(
        device_name: String,
        udp_client: AudioUdpClient,
        user_id: Uuid,
        channel_id: Uuid,
        is_recording: Arc<RwLock<bool>>,
        control_rx: &mut mpsc::UnboundedReceiver<bool>,
    ) -> Result<()> {
        println!("🎤 CaptureTask: Starting audio capture task for device: {}", device_name);
        
        // Channel pour les données audio ET métadonnées
        let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<(Vec<f32>, u32, u8)>();
        
        // Clone is_recording pour les threads
        let is_recording_capture = is_recording.clone();
        let is_recording_stream = is_recording.clone();
        
        // Créer et démarrer le stream dans un thread séparé
        println!("🎤 CaptureTask: Creating audio capture thread...");
        std::thread::spawn(move || {
            println!("🎤 CaptureThread: Getting audio host and device...");
            // Obtenir le device CPAL (dans le thread)
            let host = cpal::default_host();
            let device = if device_name == "default" {
                host.default_input_device()
            } else {
                host.input_devices()
                    .ok()
                    .and_then(|mut devices| devices.find(|d| d.name().unwrap_or_default() == device_name))
            };

            if let Some(device) = device {
                // println!("🎤 CaptureThread: Found audio device: {:?}", device.name());
                
                // Configuration du stream
                if let Ok(config) = device.default_input_config() {
                    let sample_rate = config.sample_rate().0;
                    let channels = config.channels() as u8;
                    // println!("🎤 CaptureThread: Audio config - Sample rate: {}, Channels: {}", sample_rate, channels);

                    // println!("🎤 CaptureThread: Creating audio stream with format: {:?}", config.sample_format());
                    
                    // Créer le stream selon le format
                    let stream_result = match config.sample_format() {
                        SampleFormat::F32 => Self::create_stream::<f32>(&device, &config.into(), audio_tx.clone(), sample_rate, channels),
                        SampleFormat::I16 => Self::create_stream::<i16>(&device, &config.into(), audio_tx.clone(), sample_rate, channels),
                        SampleFormat::U16 => Self::create_stream::<u16>(&device, &config.into(), audio_tx.clone(), sample_rate, channels),
                        _ => {
                            eprintln!("❌ CaptureThread: Unsupported sample format: {:?}", config.sample_format());
                            return;
                        }
                    };

                    if let Ok(stream) = stream_result {
                        println!("🎤 CaptureThread: Stream created successfully, starting playback...");
                        if let Err(e) = stream.play() {
                            eprintln!("❌ CaptureThread: Failed to start audio stream: {}", e);
                            return;
                        }
                        println!("✅ CaptureThread: Audio stream started successfully!");
                        
                        // Maintenir le stream vivant jusqu'à l'arrêt
                        loop {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            
                            // Vérifier si on doit arrêter (via un flag partagé)
                            if !*is_recording_capture.read() {
                                println!("🎤 CaptureThread: Recording stopped, exiting stream loop");
                                break;
                            }
                        }
                    } else {
                        eprintln!("❌ CaptureThread: Failed to create audio stream");
                    }
                }
            }
        });

        // Boucle principale pour traiter les données
        loop {
            tokio::select! {
                // Vérifier les commandes d'arrêt
                cmd = control_rx.recv() => {
                    match cmd {
                        Some(false) => {
                            println!("🎤 Received stop command, breaking capture loop");
                            break; // Commande d'arrêt reçue
                        }
                        None => {
                            println!("🎤 Control channel closed, breaking capture loop");
                            break; // Channel fermé
                        }
                        _ => {
                            // Ignorer les autres commandes
                        }
                    }
                }
                // Traiter les données audio
                audio_data = audio_rx.recv() => {
                    if let Some((data, sample_rate, channels)) = audio_data {
                        if *is_recording_stream.read() {
                            // println!("🎤 Sending {} samples to UDP (SR: {}Hz, CH: {})", data.len(), sample_rate, channels);
                            if let Err(e) = udp_client.send_audio_data(
                                user_id,
                                channel_id,
                                data,
                                sample_rate,
                                channels,
                            ).await {
                                eprintln!("Failed to send audio data: {}", e);
                            }
                        }
                    }
                }
            }
        }

        println!("Audio capture task stopped");
        Ok(())
    }

    /// Crée un stream audio typé
    fn create_stream<T>(
        device: &Device,
        config: &StreamConfig,
        audio_tx: mpsc::UnboundedSender<(Vec<f32>, u32, u8)>,
        sample_rate: u32,
        channels: u8,
    ) -> Result<cpal::Stream>
    where
        T: cpal::Sample + cpal::SizedSample + Send + 'static,
        f32: From<T>,
    {
        // Buffer pour accumuler les échantillons
        let mut sample_buffer = Vec::new();
        const BUFFER_SIZE: usize = 1024; // Environ 21ms à 48kHz
        
        let err_fn = |err| eprintln!("Audio stream error: {}", err);
        
        let stream = device.build_input_stream(
            config,
            move |data: &[T], _: &cpal::InputCallbackInfo| {
                // Convertir les échantillons vers f32
                let samples: Vec<f32> = data.iter().map(|&s| f32::from(s)).collect();
                sample_buffer.extend(samples);

                // Envoyer quand on a assez d'échantillons
                if sample_buffer.len() >= BUFFER_SIZE {
                    let audio_data = sample_buffer.drain(..BUFFER_SIZE).collect();
                    if let Err(_) = audio_tx.send((audio_data, sample_rate, channels)) {
                        // Channel fermé, ignore
                    }
                }
            },
            err_fn,
            None,
        )?;

        Ok(stream)
    }

    /// Arrête l'enregistrement audio
    pub fn stop_recording(&self) -> Result<()> {
        if !*self.is_recording.read() {
            return Ok(()); // Pas en cours d'enregistrement
        }

        // Envoyer signal d'arrêt
        if let Some(control_tx) = self.control_tx.read().as_ref() {
            let _ = control_tx.send(false);
        }
        
        *self.control_tx.write() = None;
        *self.is_recording.write() = false;
        
        println!("Audio recording stopped");
        Ok(())
    }

    /// Vérifie si l'enregistrement est en cours
    pub fn is_recording(&self) -> bool {
        *self.is_recording.read()
    }

    /// Obtient le nom du périphérique actuel
    pub fn get_device_name(&self) -> Option<String> {
        self.device_name.read().clone()
    }
}

impl Default for AudioCaptureManager {
    fn default() -> Self {
        Self::new()
    }
}

// Implémentation manuelle de Clone pour AudioCaptureManager
impl Clone for AudioCaptureManager {
    fn clone(&self) -> Self {
        Self {
            device_name: self.device_name.clone(),
            is_recording: Arc::new(RwLock::new(false)), // Nouvel état
            udp_client: self.udp_client.clone(),
            user_id: self.user_id.clone(),
            channel_id: self.channel_id.clone(),
            control_tx: Arc::new(RwLock::new(None)), // Nouveau channel
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_capture_manager_creation() {
        let manager = AudioCaptureManager::new();
        assert!(!manager.is_recording());
        assert!(manager.get_device_name().is_none());
    }

    #[test]
    fn test_set_device() {
        let manager = AudioCaptureManager::new();
        let device_name = "Test Device".to_string();
        
        manager.set_device(device_name.clone()).unwrap();
        assert_eq!(manager.get_device_name(), Some(device_name));
    }
}