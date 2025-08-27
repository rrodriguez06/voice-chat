use anyhow::{Result, Context};
use bytes::Bytes;
use cpal::{
    traits::{DeviceTrait, HostTrait, StreamTrait},
    Device, SampleFormat, StreamConfig,
};
use std::sync::Arc;
use parking_lot::RwLock;
use tokio::sync::mpsc;
use tokio::net::UdpSocket;
use uuid::Uuid;
use crate::networking::{AudioPacket, PacketType};

/// Gestionnaire de lecture audio (haut-parleurs)
#[derive(Debug)]
pub struct AudioPlaybackManager {
    device_name: Arc<RwLock<Option<String>>>,
    is_playing: Arc<RwLock<bool>>,
    user_id: Arc<RwLock<Option<Uuid>>>,
    control_tx: Arc<RwLock<Option<mpsc::UnboundedSender<bool>>>>,
}

impl AudioPlaybackManager {
    pub fn new() -> Self {
        Self {
            device_name: Arc::new(RwLock::new(None)),
            is_playing: Arc::new(RwLock::new(false)),
            user_id: Arc::new(RwLock::new(None)),
            control_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Configure l'utilisateur
    pub fn set_user(&self, user_id: Uuid) {
        *self.user_id.write() = Some(user_id);
    }

    /// Configure le périphérique de lecture
    pub fn set_device(&self, device_name: String) -> Result<()> {
        self.stop_playback()?;
        *self.device_name.write() = Some(device_name);
        Ok(())
    }

    /// Démarre la lecture audio et l'écoute UDP
    pub async fn start_playback(&self, server_addr: std::net::SocketAddr) -> Result<()> {
        // println!("🔊 AudioPlaybackManager: Starting audio playback...");
        
        if *self.is_playing.read() {
            println!("⚠️ AudioPlaybackManager: Already playing, ignoring start request");
            return Ok(());
        }

        let device_name = self.device_name.read()
            .as_ref()
            .context("No audio device configured")?
            .clone();
        println!("🔊 AudioPlaybackManager: Using device: {}", device_name);

        let user_id = self.user_id.read()
            .context("No user ID configured")?;
        println!("🔊 AudioPlaybackManager: User ID: {}", user_id);

        // Créer un channel de contrôle
        let (control_tx, control_rx) = mpsc::unbounded_channel::<bool>();
        *self.control_tx.write() = Some(control_tx);

        // Créer un channel pour les données audio avec métadonnées
        let (audio_tx, audio_rx) = mpsc::unbounded_channel::<(Vec<f32>, u32, u8)>();

        let is_playing = self.is_playing.clone();
        
        // Utiliser le client UDP existant pour l'écoute au lieu de créer un nouveau socket
        // println!("🔊 AudioPlaybackManager: Using shared UDP client for audio reception...");
        let audio_tx_clone = audio_tx.clone();
        let user_id_clone = user_id;
        let control_rx_clone = control_rx;
        tokio::spawn(async move {
            // Ici, nous utiliserions le socket partagé du client UDP
            // Pour l'instant, utilisons l'ancienne méthode mais avec un port différent
            if let Err(e) = Self::start_udp_listener_fallback(
                server_addr,
                user_id_clone,
                audio_tx_clone,
                control_rx_clone,
            ).await {
                eprintln!("❌ UDP listener error: {}", e);
            }
        });

        // Démarrer la lecture dans un thread système (pas une tâche async)
        println!("🔊 AudioPlaybackManager: Starting playback task...");
        let device_name_clone = device_name;
        let is_playing_clone = is_playing.clone();
        let audio_rx_moved = audio_rx;
        
        std::thread::spawn(move || {
            if let Err(e) = Self::start_playback_task_sync(
                device_name_clone,
                is_playing_clone,
                audio_rx_moved,
            ) {
                eprintln!("❌ Audio playback error: {}", e);
            }
        });

        *self.is_playing.write() = true;
        // println!("✅ AudioPlaybackManager: Audio playback started successfully");
        Ok(())
    }

    /// Démarre la lecture audio en utilisant le socket partagé du client UDP
    pub async fn start_playback_with_shared_socket(
        &self,
        server_addr: std::net::SocketAddr,
        udp_socket: Arc<tokio::net::UdpSocket>,
    ) -> Result<()> {
        if *self.is_playing.read() {
            println!("⚠️ AudioPlaybackManager: Already playing, ignoring start request");
            return Ok(());
        }

        let device_name = self.device_name.read()
            .as_ref()
            .context("No audio device configured")?
            .clone();
        println!("🔊 AudioPlaybackManager: Using device: {}", device_name);
        println!("🔊 AudioPlaybackManager: Using shared UDP socket on {:?}", udp_socket.local_addr()?);

        let user_id = self.user_id.read()
            .context("No user ID configured")?;
        println!("🔊 AudioPlaybackManager: User ID: {}", user_id);

        // Créer un channel de contrôle
        let (control_tx, control_rx) = mpsc::unbounded_channel::<bool>();
        *self.control_tx.write() = Some(control_tx);

        // Créer un channel pour les données audio avec métadonnées
        let (audio_tx, audio_rx) = mpsc::unbounded_channel::<(Vec<f32>, u32, u8)>();

        // Démarrer l'UDP listener avec le socket partagé
        let audio_tx_clone = audio_tx.clone();
        let user_id_clone = user_id;
        let udp_socket_clone = Arc::clone(&udp_socket);
        let control_rx_clone = control_rx;
        tokio::spawn(async move {
            if let Err(e) = Self::start_udp_listener_with_shared_socket(
                server_addr,
                user_id_clone,
                audio_tx_clone,
                control_rx_clone,
                udp_socket_clone,
            ).await {
                eprintln!("❌ UDP listener error: {}", e);
            }
        });

        // Créer le receiver partagé pour le thread audio
        let is_playing = Arc::new(RwLock::new(true));

        // Démarrer la lecture dans un thread système (pas une tâche async)
        println!("🔊 AudioPlaybackManager: Starting playback task...");
        let device_name_clone = device_name;
        let is_playing_clone = is_playing.clone();
        let audio_rx_moved = audio_rx;
        
        std::thread::spawn(move || {
            if let Err(e) = Self::start_playback_task_sync(
                device_name_clone,
                is_playing_clone,
                audio_rx_moved,
            ) {
                eprintln!("❌ Audio playback error: {}", e);
            }
        });

        *self.is_playing.write() = true;
        // println!("✅ AudioPlaybackManager: Audio playback started successfully with shared socket");
        Ok(())
    }

    /// Tâche d'écoute UDP pour recevoir l'audio du backend
    async fn start_udp_listener(
        server_addr: std::net::SocketAddr,
        user_id: Uuid,
        audio_tx: mpsc::UnboundedSender<(Vec<f32>, u32, u8)>,
        control_rx: &mut mpsc::UnboundedReceiver<bool>,
    ) -> Result<()> {
        // println!("🔊 UdpListener: Starting UDP listener for playback...");
        
        // Essayer de se connecter au client UDP existant pour partager le socket
        // Si ça échoue, créer un nouveau socket
        let socket = match UdpSocket::bind("0.0.0.0:8083").await {
            Ok(sock) => {
                println!("🔊 UdpListener: Successfully bound to port 8083");
                sock
            }
            Err(e) => {
                println!("⚠️ UdpListener: Failed to bind to 8083 ({}), trying alternative port...", e);
                // Essayer un port différent si 8083 est occupé
                let sock = UdpSocket::bind("0.0.0.0:0").await
                    .context("Failed to bind UDP socket for playback on any port")?;
                println!("🔊 UdpListener: Using alternative port {:?}", sock.local_addr()?);
                sock
            }
        };
        
        println!("🔊 UdpListener: Listening on {:?}", socket.local_addr()?);
        
        let mut buf = vec![0u8; 4096];
        
        loop {
            tokio::select! {
                // Vérifier les commandes d'arrêt
                cmd = control_rx.recv() => {
                    if cmd.is_none() {
                        println!("🔊 UdpListener: Received stop command, shutting down");
                        break;
                    }
                }
                // Recevoir des packets UDP
                result = socket.recv_from(&mut buf) => {
                    match result {
                        Ok((size, from)) => {
                            if from.ip() == server_addr.ip() && from.port() == server_addr.port() {
                                // Essayer de désérialiser le packet audio
                                if let Ok(packet) = AudioPacket::from_bytes(&buf[..size]) {
                                    // Traiter les packets audio de type Audio
                                    if packet.header.packet_type == PacketType::Audio {
                                        // En mode normal, on reçoit l'audio d'autres utilisateurs
                                        // En mode loopback, on reçoit notre propre audio
                                        let is_own_packet = packet.header.user_id == user_id;
                                        
                                        println!("🔊 UdpListener: Received audio packet from user {} {} - Seq: {}, Payload: {} bytes, SR: {}Hz, CH: {}", 
                                            packet.header.user_id, 
                                            if is_own_packet { "(own)" } else { "(other)" },
                                            packet.header.sequence, packet.payload.len(),
                                            packet.header.sample_rate, packet.header.channels);
                                        
                                        // Convertir les bytes PCM en f32
                                        let audio_samples = Self::pcm_to_f32(&packet.payload);
                                        
                                        // Envoyer vers le lecteur audio avec métadonnées pour conversion
                                        if let Err(_) = audio_tx.send((audio_samples, packet.header.sample_rate, packet.header.channels)) {
                                            // Channel fermé, arrêter
                                            break;
                                        }
                                    } else {
                                        println!("🔇 UdpListener: Ignoring non-audio packet type: {:?}", packet.header.packet_type);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ UDP receive error: {}", e);
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                    }
                }
            }
        }

        println!("🔊 UdpListener: UDP listener stopped");
        Ok(())
    }

    /// Convertit les bytes PCM 16-bit en échantillons f32
    fn pcm_to_f32(pcm_data: &Bytes) -> Vec<f32> {
        let samples: Vec<f32> = pcm_data
            .chunks_exact(2)
            .map(|chunk| {
                let sample_i16 = i16::from_le_bytes([chunk[0], chunk[1]]);
                sample_i16 as f32 / 32767.0
            })
            .collect();
        
        println!("🔊 UdpListener: Converted {} PCM bytes -> {} f32 samples", 
            pcm_data.len(), samples.len());
        samples
    }
    
    /// Fallback UDP listener avec port dynamique si 8083 est occupé
    async fn start_udp_listener_fallback(
        server_addr: std::net::SocketAddr,
        user_id: Uuid,
        audio_tx: mpsc::UnboundedSender<(Vec<f32>, u32, u8)>,
        mut control_rx: mpsc::UnboundedReceiver<bool>,
    ) -> Result<()> {
        println!("🔊 UdpListener: Starting fallback UDP listener...");
        
        // Utiliser un port dynamique puisque 8083 est probablement occupé
        let socket = UdpSocket::bind("0.0.0.0:0").await
            .context("Failed to bind UDP socket for playback on any port")?;
        
        println!("🔊 UdpListener: Listening on {:?}", socket.local_addr()?);
        println!("⚠️ UdpListener: WARNING - Using different port than UDP client, audio routing may not work correctly");
        
        let mut buf = vec![0u8; 4096];
        
        loop {
            tokio::select! {
                // Vérifier les commandes d'arrêt
                cmd = control_rx.recv() => {
                    if cmd.is_none() {
                        println!("🔊 UdpListener: Received stop command, shutting down");
                        break;
                    }
                }
                // Recevoir des packets UDP
                result = socket.recv_from(&mut buf) => {
                    match result {
                        Ok((size, from)) => {
                            if from.ip() == server_addr.ip() && from.port() == server_addr.port() {
                                // Essayer de désérialiser le packet audio
                                if let Ok(packet) = AudioPacket::from_bytes(&buf[..size]) {
                                    // Traiter les packets audio de type Audio
                                    if packet.header.packet_type == PacketType::Audio {
                                        // En mode normal, on reçoit l'audio d'autres utilisateurs
                                        // En mode loopback, on reçoit notre propre audio
                                        let is_own_packet = packet.header.user_id == user_id;
                                        
                                        println!("🔊 UdpListener: Received audio packet from user {} {} - Seq: {}, Payload: {} bytes, SR: {}Hz, CH: {}", 
                                            packet.header.user_id, 
                                            if is_own_packet { "(own)" } else { "(other)" },
                                            packet.header.sequence, packet.payload.len(),
                                            packet.header.sample_rate, packet.header.channels);
                                        
                                        // Convertir les bytes PCM en f32
                                        let audio_samples = Self::pcm_to_f32(&packet.payload);
                                        
                                        // Envoyer vers le lecteur audio avec métadonnées pour conversion
                                        if let Err(_) = audio_tx.send((audio_samples, packet.header.sample_rate, packet.header.channels)) {
                                            // Channel fermé, arrêter
                                            break;
                                        }
                                    } else {
                                        println!("🔇 UdpListener: Ignoring non-audio packet type: {:?}", packet.header.packet_type);
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ UDP receive error: {}", e);
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                    }
                }
            }
        }
        
        println!("🔊 UdpListener: Stopped receiving audio packets");
        Ok(())
    }
    
    /// UDP listener utilisant un socket partagé (pas de conflit de port)
    async fn start_udp_listener_with_shared_socket(
        server_addr: std::net::SocketAddr,
        user_id: Uuid,
        audio_tx: mpsc::UnboundedSender<(Vec<f32>, u32, u8)>,
        mut control_rx: mpsc::UnboundedReceiver<bool>,
        udp_socket: Arc<tokio::net::UdpSocket>,
    ) -> Result<()> {
        println!("🔊 UdpListener: Starting UDP listener with shared socket on {:?}", udp_socket.local_addr()?);
        
        let mut buf = vec![0u8; 4096];
        
        loop {
            tokio::select! {
                // Vérifier les commandes d'arrêt
                cmd = control_rx.recv() => {
                    if cmd.is_none() {
                        println!("🔊 UdpListener: Received stop command, shutting down");
                        break;
                    }
                }
                // Recevoir des packets UDP
                result = udp_socket.recv_from(&mut buf) => {
                    match result {
                        Ok((size, from)) => {
                            println!("📡 UdpListener: Received {} bytes from {}", size, from);
                            if from.ip() == server_addr.ip() && from.port() == server_addr.port() {
                                // Essayer de désérialiser le packet audio
                                if let Ok(packet) = AudioPacket::from_bytes(&buf[..size]) {
                                    // Traiter les packets audio de type Audio
                                    if packet.header.packet_type == PacketType::Audio {
                                        // En mode normal, on reçoit l'audio d'autres utilisateurs
                                        // En mode loopback, on reçoit notre propre audio
                                        let is_own_packet = packet.header.user_id == user_id;
                                        
                                        println!("🔊 UdpListener: Received audio packet from user {} {} - Seq: {}, Payload: {} bytes, SR: {}Hz, CH: {}", 
                                            packet.header.user_id, 
                                            if is_own_packet { "(own)" } else { "(other)" },
                                            packet.header.sequence, packet.payload.len(),
                                            packet.header.sample_rate, packet.header.channels);
                                        
                                        // Convertir les bytes PCM en f32
                                        let audio_samples = Self::pcm_to_f32(&packet.payload);
                                        
                                        // Envoyer vers le lecteur audio avec métadonnées pour conversion
                                        if let Err(_) = audio_tx.send((audio_samples, packet.header.sample_rate, packet.header.channels)) {
                                            // Channel fermé, arrêter
                                            break;
                                        }
                                    } else {
                                        println!("🔇 UdpListener: Ignoring non-audio packet type: {:?}", packet.header.packet_type);
                                    }
                                } else {
                                    println!("❌ UdpListener: Failed to parse packet from {}", from);
                                }
                            } else {
                                println!("🚫 UdpListener: Ignoring packet from {} (expecting {}:{})", from, server_addr.ip(), server_addr.port());
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ UDP receive error: {}", e);
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                    }
                }
            }
        }
        
        println!("🔊 UdpListener: Stopped receiving audio packets (shared socket)");
        Ok(())
    }

    /// Convertit l'audio d'un format source vers un format de sortie
    fn convert_audio_format(
        input_samples: Vec<f32>,
        input_sample_rate: u32,
        input_channels: u8,
        output_sample_rate: u32,
        output_channels: usize,
    ) -> Vec<f32> {
        println!("🔄 Converting audio: {}Hz {}ch -> {}Hz {}ch", 
            input_sample_rate, input_channels, output_sample_rate, output_channels);

        let input_len = input_samples.len();
        let input_channels = input_channels as usize;
        
        // Étape 1: Convertir le sample rate (simple upsampling/downsampling)
        let resampled = if input_sample_rate != output_sample_rate {
            let ratio = output_sample_rate as f32 / input_sample_rate as f32;
            let new_len = (input_samples.len() as f32 * ratio) as usize;
            
            println!("🔄 Resampling with ratio: {:.2} ({} -> {} samples)", 
                ratio, input_samples.len(), new_len);
            
            if ratio > 1.0 {
                // Upsampling: interpolation linéaire simple
                let mut resampled = Vec::with_capacity(new_len);
                for i in 0..new_len {
                    let src_index = i as f32 / ratio;
                    let src_index_floor = src_index.floor() as usize;
                    let src_index_ceil = (src_index_floor + 1).min(input_samples.len() - 1);
                    let frac = src_index - src_index_floor as f32;
                    
                    if src_index_floor < input_samples.len() {
                        let sample = input_samples[src_index_floor] * (1.0 - frac) + 
                                   input_samples[src_index_ceil] * frac;
                        resampled.push(sample);
                    }
                }
                resampled
            } else {
                // Downsampling: prendre un échantillon sur N
                let step = 1.0 / ratio;
                input_samples.iter()
                    .enumerate()
                    .filter_map(|(i, &sample)| {
                        if ((i as f32 * step) as usize) < new_len {
                            Some(sample)
                        } else {
                            None
                        }
                    })
                    .collect()
            }
        } else {
            input_samples
        };
        
        // Étape 2: Convertir le nombre de canaux
        let converted = if input_channels != output_channels {
            println!("🔄 Converting channels: {} -> {}", input_channels, output_channels);
            
            let samples_per_input_frame = input_channels;
            let samples_per_output_frame = output_channels;
            let input_frames = resampled.len() / samples_per_input_frame;
            
            let mut converted = Vec::with_capacity(input_frames * samples_per_output_frame);
            
            for frame_idx in 0..input_frames {
                let input_frame_start = frame_idx * samples_per_input_frame;
                
                for out_ch in 0..output_channels {
                    if input_channels == 1 {
                        // Mono vers multi-channel: dupliquer le canal mono
                        if input_frame_start < resampled.len() {
                            converted.push(resampled[input_frame_start]);
                        } else {
                            converted.push(0.0);
                        }
                    } else if input_channels == 2 && output_channels > 2 {
                        // Stéréo vers multi-channel
                        if out_ch < 2 && input_frame_start + out_ch < resampled.len() {
                            converted.push(resampled[input_frame_start + out_ch]);
                        } else {
                            converted.push(0.0); // Canaux supplémentaires = silence
                        }
                    } else if input_channels > output_channels {
                        // Multi-channel vers moins de canaux: prendre les premiers canaux
                        if out_ch < input_channels && input_frame_start + out_ch < resampled.len() {
                            converted.push(resampled[input_frame_start + out_ch]);
                        } else {
                            converted.push(0.0);
                        }
                    } else {
                        // Par défaut: copier ou silence
                        if out_ch < input_channels && input_frame_start + out_ch < resampled.len() {
                            converted.push(resampled[input_frame_start + out_ch]);
                        } else {
                            converted.push(0.0);
                        }
                    }
                }
            }
            converted
        } else {
            resampled
        };
        
        println!("🔄 Conversion complete: {} -> {} samples", 
            input_len, converted.len());
        converted
    }

    /// Tâche de lecture audio (version synchrone pour thread)
    fn start_playback_task_sync(
        device_name: String,
        is_playing: Arc<RwLock<bool>>,
        audio_rx: mpsc::UnboundedReceiver<(Vec<f32>, u32, u8)>,
    ) -> Result<()> {
        println!("🔊 PlaybackTask: Starting audio playback task for device: {}", device_name);
        
        // Obtenir le device CPAL
        let host = cpal::default_host();
        let device = if device_name == "default" {
            host.default_output_device()
        } else {
            host.output_devices()
                .ok()
                .and_then(|mut devices| devices.find(|d| d.name().unwrap_or_default() == device_name))
        };

        let device = device.context("No output audio device found")?;
        println!("🔊 PlaybackTask: Found audio device: {:?}", device.name());

        // Configuration du stream
        let config = device.default_output_config()
            .context("Failed to get default output config")?;
        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        println!("🔊 PlaybackTask: Audio config - Sample rate: {}, Channels: {}", sample_rate, channels);

        // Buffer pour accumuler les échantillons
        let audio_rx = Arc::new(parking_lot::Mutex::new(audio_rx));

        // Créer le stream selon le format
        let stream = match config.sample_format() {
            SampleFormat::F32 => Self::create_output_stream::<f32>(&device, &config.into(), audio_rx, sample_rate, channels)?,
            SampleFormat::I16 => Self::create_output_stream_i16(&device, &config.into(), audio_rx, sample_rate, channels)?,
            SampleFormat::U16 => Self::create_output_stream_u16(&device, &config.into(), audio_rx, sample_rate, channels)?,
            _ => {
                return Err(anyhow::anyhow!("Unsupported sample format: {:?}", config.sample_format()));
            }
        };

        println!("🔊 PlaybackTask: Stream created successfully, starting playback...");
        stream.play().context("Failed to start audio stream")?;
        println!("✅ PlaybackTask: Audio stream started successfully!");

        // Maintenir le stream vivant dans ce thread (le stream ne sort jamais de ce thread)
        while *is_playing.read() {
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        println!("🔊 PlaybackTask: Playback task stopped");
        Ok(())
    }

    /// Crée un stream de sortie audio typé
    fn create_output_stream<T>(
        device: &Device,
        config: &StreamConfig,
        audio_rx: Arc<parking_lot::Mutex<mpsc::UnboundedReceiver<(Vec<f32>, u32, u8)>>>,
        output_sample_rate: u32,
        output_channels: usize,
    ) -> Result<cpal::Stream>
    where
        T: cpal::Sample + cpal::SizedSample + Send + 'static,
        f32: Into<T>,
    {
        let mut output_buffer = Vec::<f32>::new();
        let mut last_packet_time = std::time::Instant::now();
        const AUDIO_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100); // 100ms timeout
        
        let err_fn = |err| eprintln!("Audio output stream error: {}", err);
        
        let stream = device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                let now = std::time::Instant::now();
                
                // Essayer de recevoir de nouveaux échantillons
                let mut received_new_packet = false;
                while let Ok((samples, input_sr, input_ch)) = audio_rx.lock().try_recv() {
                    received_new_packet = true;
                    last_packet_time = now;
                    
                    let converted_samples = Self::convert_audio_format(
                        samples,
                        input_sr,
                        input_ch,
                        output_sample_rate,
                        output_channels,
                    );
                    output_buffer.extend(converted_samples);
                }
                
                // Si timeout atteint sans nouveaux paquets, vider le buffer pour éviter les boucles
                if !received_new_packet && now.duration_since(last_packet_time) > AUDIO_TIMEOUT {
                    if !output_buffer.is_empty() {
                        println!("🔇 Audio timeout reached, clearing buffer to avoid loop");
                        output_buffer.clear();
                    }
                }

                // Remplir le buffer de sortie
                for (i, sample) in data.iter_mut().enumerate() {
                    if i < output_buffer.len() {
                        *sample = output_buffer[i].into();
                    } else {
                        *sample = T::EQUILIBRIUM; // Silence
                    }
                }

                // Retirer les échantillons utilisés
                if output_buffer.len() >= data.len() {
                    output_buffer.drain(..data.len());
                }
            },
            err_fn,
            None,
        )?;

        Ok(stream)
    }

    /// Stream pour i16
    fn create_output_stream_i16(
        device: &Device,
        config: &StreamConfig,
        audio_rx: Arc<parking_lot::Mutex<mpsc::UnboundedReceiver<(Vec<f32>, u32, u8)>>>,
        output_sample_rate: u32,
        output_channels: usize,
    ) -> Result<cpal::Stream> {
        let mut output_buffer = Vec::<f32>::new();
        let mut last_packet_time = std::time::Instant::now();
        const AUDIO_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100); // 100ms timeout
        
        let err_fn = |err| eprintln!("Audio output stream error: {}", err);
        
        let stream = device.build_output_stream(
            config,
            move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                let now = std::time::Instant::now();
                
                // Essayer de recevoir de nouveaux échantillons
                let mut received_new_packet = false;
                while let Ok((samples, input_sr, input_ch)) = audio_rx.lock().try_recv() {
                    received_new_packet = true;
                    last_packet_time = now;
                    
                    let converted_samples = Self::convert_audio_format(
                        samples,
                        input_sr,
                        input_ch,
                        output_sample_rate,
                        output_channels,
                    );
                    output_buffer.extend(converted_samples);
                }
                
                // Si timeout atteint sans nouveaux paquets, vider le buffer pour éviter les boucles
                if !received_new_packet && now.duration_since(last_packet_time) > AUDIO_TIMEOUT {
                    if !output_buffer.is_empty() {
                        println!("🔇 Audio timeout reached (i16), clearing buffer to avoid loop");
                        output_buffer.clear();
                    }
                }

                // Remplir le buffer de sortie
                for (i, sample) in data.iter_mut().enumerate() {
                    if i < output_buffer.len() {
                        *sample = (output_buffer[i].clamp(-1.0, 1.0) * 32767.0) as i16;
                    } else {
                        *sample = 0; // Silence
                    }
                }

                // Retirer les échantillons utilisés
                if output_buffer.len() >= data.len() {
                    output_buffer.drain(..data.len());
                }
            },
            err_fn,
            None,
        )?;

        Ok(stream)
    }

    /// Stream pour u16
    fn create_output_stream_u16(
        device: &Device,
        config: &StreamConfig,
        audio_rx: Arc<parking_lot::Mutex<mpsc::UnboundedReceiver<(Vec<f32>, u32, u8)>>>,
        output_sample_rate: u32,
        output_channels: usize,
    ) -> Result<cpal::Stream> {
        let mut output_buffer = Vec::<f32>::new();
        let mut last_packet_time = std::time::Instant::now();
        const AUDIO_TIMEOUT: std::time::Duration = std::time::Duration::from_millis(100); // 100ms timeout
        
        let err_fn = |err| eprintln!("Audio output stream error: {}", err);
        
        let stream = device.build_output_stream(
            config,
            move |data: &mut [u16], _: &cpal::OutputCallbackInfo| {
                let now = std::time::Instant::now();
                
                // Essayer de recevoir de nouveaux échantillons
                let mut received_new_packet = false;
                while let Ok((samples, input_sr, input_ch)) = audio_rx.lock().try_recv() {
                    received_new_packet = true;
                    last_packet_time = now;
                    
                    let converted_samples = Self::convert_audio_format(
                        samples,
                        input_sr,
                        input_ch,
                        output_sample_rate,
                        output_channels,
                    );
                    output_buffer.extend(converted_samples);
                }
                
                // Si timeout atteint sans nouveaux paquets, vider le buffer pour éviter les boucles
                if !received_new_packet && now.duration_since(last_packet_time) > AUDIO_TIMEOUT {
                    if !output_buffer.is_empty() {
                        println!("🔇 Audio timeout reached (u16), clearing buffer to avoid loop");
                        output_buffer.clear();
                    }
                }

                // Remplir le buffer de sortie
                for (i, sample) in data.iter_mut().enumerate() {
                    if i < output_buffer.len() {
                        *sample = ((output_buffer[i].clamp(-1.0, 1.0) + 1.0) * 32767.5) as u16;
                    } else {
                        *sample = 32768; // Silence pour unsigned
                    }
                }

                // Retirer les échantillons utilisés
                if output_buffer.len() >= data.len() {
                    output_buffer.drain(..data.len());
                }
            },
            err_fn,
            None,
        )?;

        Ok(stream)
    }

    /// Arrête la lecture audio
    pub fn stop_playback(&self) -> Result<()> {
        if !*self.is_playing.read() {
            return Ok(());
        }

        // Envoyer signal d'arrêt
        if let Some(control_tx) = self.control_tx.read().as_ref() {
            let _ = control_tx.send(false);
        }
        
        *self.control_tx.write() = None;
        *self.is_playing.write() = false;
        
        println!("🔊 AudioPlaybackManager: Audio playback stopped");
        Ok(())
    }

    /// Vérifie si la lecture est en cours
    pub fn is_playing(&self) -> bool {
        *self.is_playing.read()
    }

    /// Obtient le nom du périphérique actuel
    pub fn get_device_name(&self) -> Option<String> {
        self.device_name.read().clone()
    }
}

impl Default for AudioPlaybackManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for AudioPlaybackManager {
    fn clone(&self) -> Self {
        Self {
            device_name: self.device_name.clone(),
            is_playing: Arc::new(RwLock::new(false)),
            user_id: self.user_id.clone(),
            control_tx: Arc::new(RwLock::new(None)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pcm_to_f32_conversion() {
        // Test de conversion PCM 16-bit vers f32
        let pcm_data = Bytes::from(vec![0x00, 0x00, 0xFF, 0x7F, 0x00, 0x80]); // 0, 32767, -32768
        let samples = AudioPlaybackManager::pcm_to_f32(&pcm_data);
        
        assert_eq!(samples.len(), 3);
        assert_eq!(samples[0], 0.0);
        assert!((samples[1] - 1.0).abs() < 0.001);
        assert!((samples[2] - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn test_playback_manager_creation() {
        let manager = AudioPlaybackManager::new();
        assert!(!manager.is_playing());
        assert!(manager.get_device_name().is_none());
    }
}