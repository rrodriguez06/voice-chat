use bytes::Bytes;
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;
use crate::audio::AudioPacket;

/// Contrôles de volume et effets par utilisateur
#[derive(Debug, Clone)]
pub struct UserAudioControls {
    pub volume: f32,                    // 0.0 à 2.0 (200% max)
    pub muted: bool,
    pub solo: bool,                     // Si true, seul cet utilisateur est audible
    pub pan: f32,                       // -1.0 (gauche) à 1.0 (droite)
    pub high_pass_enabled: bool,
    pub noise_suppression: f32,         // 0.0 à 1.0
    pub echo_cancellation: bool,
    pub voice_activity_threshold: f32,  // Seuil de détection de voix
}

impl Default for UserAudioControls {
    fn default() -> Self {
        Self {
            volume: 1.0,
            muted: false,
            solo: false,
            pan: 0.0,
            high_pass_enabled: true,
            noise_suppression: 0.3,
            echo_cancellation: true,
            voice_activity_threshold: 0.1,
        }
    }
}

/// Configuration de mixage pour un channel
#[derive(Debug, Clone)]
pub struct ChannelMixConfig {
    pub master_volume: f32,
    pub auto_gain_control: bool,
    pub compression_enabled: bool,
    pub compression_ratio: f32,
    pub noise_gate_enabled: bool,
    pub noise_gate_threshold: f32,
    pub max_concurrent_voices: usize,
}

impl Default for ChannelMixConfig {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            auto_gain_control: true,
            compression_enabled: true,
            compression_ratio: 4.0,
            noise_gate_enabled: true,
            noise_gate_threshold: -40.0, // dB
            max_concurrent_voices: 8,
        }
    }
}

/// Statistiques de mixage
#[derive(Debug, Clone)]
pub struct MixingStats {
    pub active_voices: usize,
    pub total_samples_processed: u64,
    pub peak_level: f32,
    pub rms_level: f32,
    pub clipping_detected: bool,
    pub processing_time_us: u64,
}

impl Default for MixingStats {
    fn default() -> Self {
        Self {
            active_voices: 0,
            total_samples_processed: 0,
            peak_level: 0.0,
            rms_level: 0.0,
            clipping_detected: false,
            processing_time_us: 0,
        }
    }
}

/// Mixeur audio avancé
#[derive(Debug)]
pub struct AudioMixer {
    /// Contrôles par utilisateur et par channel
    user_controls: HashMap<(Uuid, Uuid), UserAudioControls>, // (user_id, channel_id)
    /// Configuration par channel
    channel_configs: HashMap<Uuid, ChannelMixConfig>,
    /// Buffer de mixage temporaire
    mix_buffer: Vec<f32>,
    /// Statistiques de mixage
    stats: HashMap<Uuid, MixingStats>,
    /// Historique de niveaux pour AGC
    level_history: HashMap<Uuid, Vec<f32>>,
    /// Sample rate de sortie
    sample_rate: u32,
    /// Nombre de channels de sortie
    channels: u8,
}

impl AudioMixer {
    pub fn new(sample_rate: u32, channels: u8) -> Self {
        Self {
            user_controls: HashMap::new(),
            channel_configs: HashMap::new(),
            mix_buffer: Vec::new(),
            stats: HashMap::new(),
            level_history: HashMap::new(),
            sample_rate,
            channels,
        }
    }

    /// Configure les contrôles audio pour un utilisateur dans un channel
    pub fn set_user_controls(&mut self, user_id: Uuid, channel_id: Uuid, controls: UserAudioControls) {
        self.user_controls.insert((user_id, channel_id), controls);
        tracing::debug!("Updated controls for user {} in channel {}", user_id, channel_id);
    }

    /// Récupère les contrôles d'un utilisateur dans un channel
    pub fn get_user_controls(&self, user_id: &Uuid, channel_id: &Uuid) -> UserAudioControls {
        self.user_controls.get(&(*user_id, *channel_id))
            .cloned()
            .unwrap_or_default()
    }

    /// Configure un channel de mixage
    pub fn set_channel_config(&mut self, channel_id: Uuid, config: ChannelMixConfig) {
        self.channel_configs.insert(channel_id, config);
        tracing::debug!("Updated config for channel {}", channel_id);
    }

    /// Récupère la configuration d'un channel
    pub fn get_channel_config(&self, channel_id: &Uuid) -> ChannelMixConfig {
        self.channel_configs.get(channel_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Mixe plusieurs packets audio avec contrôles avancés
    pub fn mix_packets_advanced(&mut self, packets: Vec<AudioPacket>, channel_id: Uuid) -> Option<Bytes> {
        let start_time = Instant::now();
        
        if packets.is_empty() {
            return None;
        }

        let config = self.get_channel_config(&channel_id);
        
        // Filtrer les packets valides et appliquer les contrôles utilisateur
        let mut active_packets = Vec::new();
        let mut has_solo = false;

        // Vérifier s'il y a des utilisateurs en mode solo
        for packet in &packets {
            if packet.has_audio() && !packet.payload.is_empty() {
                let controls = self.get_user_controls(&packet.header.user_id, &channel_id);
                if controls.solo {
                    has_solo = true;
                    break;
                }
            }
        }

        // Filtrer selon les contrôles
        for packet in packets {
            if !packet.has_audio() || packet.payload.is_empty() {
                continue;
            }

            let controls = self.get_user_controls(&packet.header.user_id, &channel_id);
            
            // Ignorer si muté
            if controls.muted {
                continue;
            }

            // Si mode solo activé, ne garder que les utilisateurs en solo
            if has_solo && !controls.solo {
                continue;
            }

            // Vérifier la limite de voix concurrentes
            if active_packets.len() >= config.max_concurrent_voices {
                break;
            }

            active_packets.push((packet, controls.clone()));
        }

        if active_packets.is_empty() {
            return None;
        }

        let reference = &active_packets[0].0;
        let sample_count = reference.payload.len() / 2; // 16-bit samples

        if sample_count == 0 {
            return None;
        }

        // Redimensionner le buffer de mixage si nécessaire
        if self.mix_buffer.len() < sample_count * self.channels as usize {
            self.mix_buffer.resize(sample_count * self.channels as usize, 0.0);
        } else {
            // Réinitialiser le buffer
            self.mix_buffer.fill(0.0);
        }

        let mut stats = MixingStats::default();
        stats.active_voices = active_packets.len();

        // Mixer chaque packet avec ses contrôles
        for (packet, controls) in &active_packets {
            if let Ok(samples) = self.bytes_to_samples(&packet.payload) {
                self.apply_audio_processing(&samples, controls, &config, &mut stats);
            }
        }

        // Post-traitement global
        self.apply_global_processing(&config, &mut stats);

        // Convertir en sortie
        let output = self.mix_buffer_to_bytes(sample_count);

        // Mettre à jour les statistiques
        stats.processing_time_us = start_time.elapsed().as_micros() as u64;
        self.stats.insert(channel_id, stats);

        output
    }

    /// Applique le traitement audio pour un utilisateur
    fn apply_audio_processing(
        &mut self,
        samples: &[i16],
        controls: &UserAudioControls,
        config: &ChannelMixConfig,
        stats: &mut MixingStats,
    ) {
        for (i, &sample) in samples.iter().enumerate() {
            if i >= self.mix_buffer.len() {
                break;
            }

            let mut processed_sample = sample as f32;

            // Appliquer le volume utilisateur
            processed_sample *= controls.volume;

            // Appliquer le volume maître du channel
            processed_sample *= config.master_volume;

            // Gate de bruit
            if config.noise_gate_enabled {
                let sample_db = 20.0 * (processed_sample.abs() / i16::MAX as f32).log10();
                if sample_db < config.noise_gate_threshold {
                    processed_sample = 0.0;
                }
            }

            // Suppression de bruit basique
            if controls.noise_suppression > 0.0 {
                let noise_factor = 1.0 - controls.noise_suppression;
                if processed_sample.abs() < (i16::MAX as f32 * 0.1) {
                    processed_sample *= noise_factor;
                }
            }

            // Filtre passe-haut basique
            if controls.high_pass_enabled {
                // Implémentation simplifiée - en production, utiliser un vrai filtre
                if processed_sample.abs() < (i16::MAX as f32 * 0.05) {
                    processed_sample *= 0.5;
                }
            }

            // Accumuler dans le buffer de mixage
            self.mix_buffer[i] += processed_sample;

            // Mettre à jour les statistiques
            let abs_sample = processed_sample.abs();
            if abs_sample > stats.peak_level {
                stats.peak_level = abs_sample;
            }

            stats.total_samples_processed += 1;
        }
    }

    /// Applique le traitement global au mix
    fn apply_global_processing(&mut self, config: &ChannelMixConfig, stats: &mut MixingStats) {
        let mut rms_sum = 0.0;

        for sample in self.mix_buffer.iter_mut() {
            // Compression dynamique basique
            if config.compression_enabled && sample.abs() > (i16::MAX as f32 * 0.7) {
                let threshold = i16::MAX as f32 * 0.7;
                let excess = sample.abs() - threshold;
                let compressed_excess = excess / config.compression_ratio;
                let sign = if *sample >= 0.0 { 1.0 } else { -1.0 };
                *sample = sign * (threshold + compressed_excess);
            }

            // Détection de saturation
            if sample.abs() >= i16::MAX as f32 {
                stats.clipping_detected = true;
                *sample = sample.signum() * (i16::MAX as f32 - 1.0);
            }

            rms_sum += *sample * *sample;
        }

        // Calculer le RMS
        stats.rms_level = (rms_sum / self.mix_buffer.len() as f32).sqrt();

        // Contrôle automatique de gain (AGC)
        if config.auto_gain_control {
            let target_rms = i16::MAX as f32 * 0.3; // 30% du maximum
            if stats.rms_level > 0.0 {
                let gain_adjustment = target_rms / stats.rms_level;
                let clamped_gain = gain_adjustment.clamp(0.1, 2.0);
                
                for sample in self.mix_buffer.iter_mut() {
                    *sample *= clamped_gain;
                }
            }
        }
    }

    /// Convertit le buffer de mixage en bytes de sortie
    fn mix_buffer_to_bytes(&self, sample_count: usize) -> Option<Bytes> {
        let mut output_bytes = Vec::with_capacity(sample_count * 2);
        
        for i in 0..sample_count {
            if i < self.mix_buffer.len() {
                let sample = self.mix_buffer[i].clamp(i16::MIN as f32, i16::MAX as f32) as i16;
                output_bytes.extend_from_slice(&sample.to_le_bytes());
            } else {
                output_bytes.extend_from_slice(&0i16.to_le_bytes());
            }
        }

        Some(Bytes::from(output_bytes))
    }

    /// Convertit des bytes en samples 16-bit
    fn bytes_to_samples(&self, bytes: &[u8]) -> Result<Vec<i16>, &'static str> {
        if bytes.len() % 2 != 0 {
            return Err("Invalid sample data: odd number of bytes");
        }

        let mut samples = Vec::with_capacity(bytes.len() / 2);
        for chunk in bytes.chunks_exact(2) {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            samples.push(sample);
        }

        Ok(samples)
    }

    /// Active/désactive le mute pour un utilisateur
    pub fn set_user_muted(&mut self, user_id: Uuid, channel_id: Uuid, muted: bool) {
        let key = (user_id, channel_id);
        let mut controls = self.user_controls.get(&key).cloned()
            .unwrap_or_default();
        controls.muted = muted;
        self.user_controls.insert(key, controls);
    }

    /// Active/désactive le mode solo pour un utilisateur
    pub fn set_user_solo(&mut self, user_id: Uuid, channel_id: Uuid, solo: bool) {
        let key = (user_id, channel_id);
        let mut controls = self.user_controls.get(&key).cloned()
            .unwrap_or_default();
        controls.solo = solo;
        self.user_controls.insert(key, controls);
    }

    /// Définit le volume pour un utilisateur
    pub fn set_user_volume(&mut self, user_id: Uuid, channel_id: Uuid, volume: f32) {
        let key = (user_id, channel_id);
        let mut controls = self.user_controls.get(&key).cloned()
            .unwrap_or_default();
        controls.volume = volume.clamp(0.0, 2.0);
        self.user_controls.insert(key, controls);
    }

    /// Définit le panoramique pour un utilisateur
    pub fn set_user_pan(&mut self, user_id: Uuid, channel_id: Uuid, pan: f32) {
        let key = (user_id, channel_id);
        let mut controls = self.user_controls.get(&key).cloned()
            .unwrap_or_default();
        controls.pan = pan.clamp(-1.0, 1.0);
        self.user_controls.insert(key, controls);
    }

    /// Supprime les contrôles d'un utilisateur
    pub fn remove_user(&mut self, user_id: &Uuid, channel_id: &Uuid) {
        self.user_controls.remove(&(*user_id, *channel_id));
    }

    /// Réinitialise tous les contrôles utilisateur pour un channel
    pub fn reset_channel_users(&mut self, channel_id: &Uuid) {
        self.user_controls.retain(|(_, ch_id), _| ch_id != channel_id);
    }

    /// Récupère les statistiques de mixage pour un channel
    pub fn get_stats(&self, channel_id: &Uuid) -> Option<&MixingStats> {
        self.stats.get(channel_id)
    }

    /// Récupère les statistiques globales du mixeur
    pub fn global_stats(&self) -> MixerGlobalStats {
        MixerGlobalStats {
            total_users: self.user_controls.len(),
            active_channels: self.channel_configs.len(),
            sample_rate: self.sample_rate,
            channels: self.channels,
            buffer_size: self.mix_buffer.len(),
        }
    }

    /// Mixe avec l'ancienne API pour compatibilité
    pub fn mix_packets(&self, packets: Vec<AudioPacket>) -> Option<Bytes> {
        if packets.is_empty() {
            return None;
        }

        // Utiliser le mixage basique pour la compatibilité
        let audio_packets: Vec<_> = packets
            .into_iter()
            .filter(|p| p.has_audio() && !p.payload.is_empty())
            .collect();

        if audio_packets.is_empty() {
            return None;
        }

        let reference = &audio_packets[0];
        let sample_count = reference.payload.len() / 2;

        if sample_count == 0 {
            return None;
        }

        let mut mixed_samples = vec![0i32; sample_count];

        for packet in &audio_packets {
            if let Ok(samples) = self.bytes_to_samples(&packet.payload) {
                for (i, &sample) in samples.iter().enumerate() {
                    if i < mixed_samples.len() {
                        mixed_samples[i] = mixed_samples[i].saturating_add(sample as i32);
                    }
                }
            }
        }

        // Normalisation simple
        let max_amplitude = mixed_samples.iter().map(|&s| s.abs()).max().unwrap_or(1);
        let normalization_factor = if max_amplitude > i16::MAX as i32 {
            i16::MAX as f32 / max_amplitude as f32
        } else {
            1.0
        };

        let mut output_bytes = Vec::with_capacity(sample_count * 2);
        for &sample in &mixed_samples {
            let normalized = (sample as f32 * normalization_factor) as i16;
            output_bytes.extend_from_slice(&normalized.to_le_bytes());
        }

        Some(Bytes::from(output_bytes))
    }
}

impl Default for AudioMixer {
    fn default() -> Self {
        Self::new(48000, 1)
    }
}

/// Statistiques globales du mixeur
#[derive(Debug, Clone)]
pub struct MixerGlobalStats {
    pub total_users: usize,
    pub active_channels: usize,
    pub sample_rate: u32,
    pub channels: u8,
    pub buffer_size: usize,
}

/// Types de fade
#[derive(Debug, Clone, Copy)]
pub enum FadeType {
    In,
    Out,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::AudioPacket;

    #[test]
    fn test_mixer_creation() {
        let mixer = AudioMixer::new(48000, 2);
        assert_eq!(mixer.sample_rate, 48000);
        assert_eq!(mixer.channels, 2);
    }

    #[test]
    fn test_user_controls() {
        let mut mixer = AudioMixer::default();
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();

        // Test contrôles par défaut
        let controls = mixer.get_user_controls(&user_id, &channel_id);
        assert_eq!(controls.volume, 1.0);
        assert!(!controls.muted);

        // Test modification de volume
        mixer.set_user_volume(user_id, channel_id, 0.5);
        let controls = mixer.get_user_controls(&user_id, &channel_id);
        assert_eq!(controls.volume, 0.5);

        // Test mute
        mixer.set_user_muted(user_id, channel_id, true);
        let controls = mixer.get_user_controls(&user_id, &channel_id);
        assert!(controls.muted);
    }

    #[test]
    fn test_channel_config() {
        let mut mixer = AudioMixer::default();
        let channel_id = Uuid::new_v4();

        // Test configuration par défaut
        let config = mixer.get_channel_config(&channel_id);
        assert_eq!(config.master_volume, 1.0);
        assert!(config.auto_gain_control);

        // Test modification de configuration
        let mut new_config = ChannelMixConfig::default();
        new_config.master_volume = 0.8;
        mixer.set_channel_config(channel_id, new_config);

        let config = mixer.get_channel_config(&channel_id);
        assert_eq!(config.master_volume, 0.8);
    }

    #[test]
    fn test_empty_packets() {
        let mut mixer = AudioMixer::default();
        let channel_id = Uuid::new_v4();
        
        assert!(mixer.mix_packets_advanced(vec![], channel_id).is_none());
    }

    #[test]
    fn test_bytes_conversion() {
        let mixer = AudioMixer::default();
        
        let bytes = vec![0x00, 0x01, 0xFF, 0x7F];
        let samples = mixer.bytes_to_samples(&bytes).unwrap();
        assert_eq!(samples.len(), 2);
        assert_eq!(samples[0], 256);
        assert_eq!(samples[1], 32767);

        // Test données invalides
        let invalid_bytes = vec![0x00, 0x01, 0xFF];
        assert!(mixer.bytes_to_samples(&invalid_bytes).is_err());
    }
}