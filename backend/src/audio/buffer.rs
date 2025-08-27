use std::collections::VecDeque;
use crate::audio::AudioPacket;

/// Buffer circulaire pour stocker les packets audio
#[derive(Debug)]
pub struct CircularBuffer {
    buffer: VecDeque<AudioPacket>,
    capacity: usize,
    total_packets: usize,
    dropped_packets: usize,
}

impl CircularBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            buffer: VecDeque::with_capacity(capacity),
            capacity,
            total_packets: 0,
            dropped_packets: 0,
        }
    }

    /// Ajoute un packet au buffer
    pub fn push(&mut self, packet: AudioPacket) {
        self.total_packets += 1;

        // Si le buffer est plein, supprimer le plus ancien
        if self.buffer.len() >= self.capacity {
            self.buffer.pop_front();
            self.dropped_packets += 1;
        }

        self.buffer.push_back(packet);
    }

    /// Récupère le packet le plus ancien
    pub fn pop(&mut self) -> Option<AudioPacket> {
        self.buffer.pop_front()
    }

    /// Récupère tous les packets disponibles
    pub fn drain(&mut self) -> Vec<AudioPacket> {
        self.buffer.drain(..).collect()
    }

    /// Nombre de packets dans le buffer
    pub fn len(&self) -> usize {
        self.buffer.len()
    }

    /// Vérifie si le buffer est vide
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    /// Statistiques du buffer
    pub fn stats(&self) -> BufferStats {
        BufferStats {
            current_size: self.buffer.len(),
            capacity: self.capacity,
            total_packets: self.total_packets,
            dropped_packets: self.dropped_packets,
            drop_rate: if self.total_packets > 0 {
                (self.dropped_packets as f64 / self.total_packets as f64) * 100.0
            } else {
                0.0
            },
        }
    }

    /// Nettoie les packets trop anciens
    pub fn cleanup_stale(&mut self) {
        let initial_len = self.buffer.len();
        self.buffer.retain(|packet| !packet.header.is_stale());
        let removed = initial_len - self.buffer.len();
        self.dropped_packets += removed;
    }

    /// Ajuste la capacité dynamiquement selon la charge
    pub fn adjust_capacity(&mut self, new_capacity: usize) {
        if new_capacity != self.capacity {
            self.capacity = new_capacity;
            // Si la nouvelle capacité est plus petite, supprimer les packets les plus anciens
            while self.buffer.len() > new_capacity {
                if self.buffer.pop_front().is_some() {
                    self.dropped_packets += 1;
                }
            }
        }
    }

    /// Récupère la latence moyenne des packets dans le buffer
    pub fn average_latency_us(&self) -> f64 {
        if self.buffer.is_empty() {
            return 0.0;
        }

        let total_latency: u64 = self.buffer
            .iter()
            .map(|packet| packet.header.age_micros())
            .sum();
        
        total_latency as f64 / self.buffer.len() as f64
    }

    /// Détecte les packets manquants dans la séquence
    pub fn detect_missing_packets(&self) -> Vec<u32> {
        if self.buffer.len() < 2 {
            return Vec::new();
        }

        let mut missing = Vec::new();
        let mut sorted_packets: Vec<_> = self.buffer.iter().collect();
        sorted_packets.sort_by_key(|p| p.header.sequence);

        for window in sorted_packets.windows(2) {
            let current = window[0].header.sequence;
            let next = window[1].header.sequence;
            
            // Détecter les trous dans la séquence
            if next > current + 1 {
                for seq in (current + 1)..next {
                    missing.push(seq);
                }
            }
        }

        missing
    }

    /// Calcule le taux de jitter
    pub fn calculate_jitter(&self) -> f64 {
        if self.buffer.len() < 3 {
            return 0.0;
        }

        let mut intervals = Vec::new();
        let mut sorted_packets: Vec<_> = self.buffer.iter().collect();
        sorted_packets.sort_by_key(|p| p.header.timestamp);

        for window in sorted_packets.windows(2) {
            let interval = window[1].header.timestamp.saturating_sub(window[0].header.timestamp);
            intervals.push(interval as f64);
        }

        if intervals.len() < 2 {
            return 0.0;
        }

        // Calculer la variance des intervalles
        let mean: f64 = intervals.iter().sum::<f64>() / intervals.len() as f64;
        let variance: f64 = intervals
            .iter()
            .map(|x| (x - mean).powi(2))
            .sum::<f64>() / intervals.len() as f64;

        variance.sqrt()
    }
}

/// Statistiques d'un buffer
#[derive(Debug, Clone)]
pub struct BufferStats {
    pub current_size: usize,
    pub capacity: usize,
    pub total_packets: usize,
    pub dropped_packets: usize,
    pub drop_rate: f64,
}

/// Buffer audio spécialisé avec gestion de la latence
#[derive(Debug)]
pub struct AudioBuffer {
    /// Buffer principal pour les packets audio
    audio_buffer: CircularBuffer,
    /// Buffer pour les packets de contrôle
    control_buffer: CircularBuffer,
    /// Latence cible en microsecondes
    target_latency_us: u64,
}

impl AudioBuffer {
    pub fn new(capacity: usize, target_latency_ms: u64) -> Self {
        Self {
            audio_buffer: CircularBuffer::new(capacity),
            control_buffer: CircularBuffer::new(capacity / 4), // Plus petit pour les contrôles
            target_latency_us: target_latency_ms * 1000,
        }
    }

    /// Ajoute un packet au buffer approprié
    pub fn push(&mut self, packet: AudioPacket) {
        if packet.is_control() {
            self.control_buffer.push(packet);
        } else {
            self.audio_buffer.push(packet);
        }
    }

    /// Récupère les packets prêts à être traités
    pub fn get_ready_packets(&mut self) -> Vec<AudioPacket> {
        let mut packets = Vec::new();

        // Toujours traiter les packets de contrôle en priorité
        packets.extend(self.control_buffer.drain());

        // Pour l'audio, respecter la latence cible
        while let Some(packet) = self.audio_buffer.buffer.front() {
            if packet.header.age_micros() >= self.target_latency_us {
                if let Some(packet) = self.audio_buffer.pop() {
                    packets.push(packet);
                }
            } else {
                break;
            }
        }

        packets
    }

    /// Nettoie les packets expirés
    pub fn cleanup(&mut self) {
        self.audio_buffer.cleanup_stale();
        self.control_buffer.cleanup_stale();
    }

    /// Statistiques complètes
    pub fn stats(&self) -> AudioBufferStats {
        AudioBufferStats {
            audio_buffer: self.audio_buffer.stats(),
            control_buffer: self.control_buffer.stats(),
            target_latency_us: self.target_latency_us,
        }
    }

    /// Ajuste la latence cible dynamiquement
    pub fn adjust_latency(&mut self, new_latency_ms: u64) {
        self.target_latency_us = new_latency_ms * 1000;
    }

    /// Auto-ajuste la latence selon les conditions réseau
    pub fn auto_adjust_latency(&mut self) {
        let audio_stats = self.audio_buffer.stats();
        
        // Si le taux de perte est élevé, augmenter la latence
        if audio_stats.drop_rate > 5.0 {
            self.target_latency_us = (self.target_latency_us * 110 / 100).min(500_000); // Max 500ms
        }
        // Si pas de perte et buffer stable, réduire légèrement
        else if audio_stats.drop_rate < 1.0 && audio_stats.current_size < audio_stats.capacity / 2 {
            self.target_latency_us = (self.target_latency_us * 95 / 100).max(20_000); // Min 20ms
        }
    }

    /// Récupère les packets avec synchronisation inter-canal
    pub fn get_synchronized_packets(&mut self, channel_sync: &std::collections::HashMap<uuid::Uuid, u64>) -> Vec<AudioPacket> {
        let mut packets = Vec::new();

        // Traiter les contrôles en priorité
        packets.extend(self.control_buffer.drain());

        // Pour l'audio, synchroniser avec les autres canaux
        while let Some(packet) = self.audio_buffer.buffer.front() {
            let channel_latest = channel_sync.get(&packet.header.channel_id).copied().unwrap_or(0);
            
            // Ne traiter que si on n'est pas trop en avance sur les autres canaux
            let max_desync = 50_000; // 50ms max de désynchronisation
            if packet.header.timestamp <= channel_latest + max_desync || 
               packet.header.age_micros() >= self.target_latency_us {
                if let Some(packet) = self.audio_buffer.pop() {
                    packets.push(packet);
                }
            } else {
                break;
            }
        }

        packets
    }

    /// Analyse la qualité du flux audio
    pub fn analyze_quality(&self) -> AudioQualityMetrics {
        let audio_stats = self.audio_buffer.stats();
        let control_stats = self.control_buffer.stats();
        
        let missing_packets = self.audio_buffer.detect_missing_packets();
        let jitter = self.audio_buffer.calculate_jitter();
        let avg_latency = self.audio_buffer.average_latency_us();

        AudioQualityMetrics {
            packet_loss_rate: audio_stats.drop_rate as f32,
            jitter_ms: (jitter / 1000.0) as f32,
            average_latency_ms: (avg_latency / 1000.0) as f32,
            missing_packets_count: missing_packets.len(),
            buffer_health: self.calculate_buffer_health(),
            quality_score: self.calculate_quality_score(&audio_stats, jitter, missing_packets.len()),
        }
    }

    /// Calcule la santé du buffer (0.0 = critique, 1.0 = excellent)
    fn calculate_buffer_health(&self) -> f32 {
        let audio_stats = self.audio_buffer.stats();
        let usage_ratio = audio_stats.current_size as f32 / audio_stats.capacity as f32;
        
        // Optimal entre 30% et 70% d'utilisation
        if usage_ratio >= 0.3 && usage_ratio <= 0.7 {
            1.0
        } else if usage_ratio < 0.1 || usage_ratio > 0.9 {
            0.2 // Critique
        } else {
            0.6 // Acceptable
        }
    }

    /// Calcule un score de qualité global (0.0 = mauvais, 1.0 = excellent)
    fn calculate_quality_score(&self, audio_stats: &BufferStats, jitter: f64, missing_count: usize) -> f32 {
        let mut score = 1.0_f32;

        // Pénaliser les pertes de packets
        score -= (audio_stats.drop_rate as f32 / 100.0) * 0.3;

        // Pénaliser le jitter élevé
        let jitter_penalty = ((jitter / 1000.0) as f32 / 50.0).min(1.0) * 0.3; // Normaliser sur 50ms
        score -= jitter_penalty;

        // Pénaliser les packets manquants
        let missing_penalty = (missing_count as f32 / 10.0).min(1.0) * 0.2;
        score -= missing_penalty;

        // Pénaliser la mauvaise santé du buffer
        score *= self.calculate_buffer_health();

        score.max(0.0)
    }
}

/// Statistiques complètes du buffer audio
#[derive(Debug, Clone)]
pub struct AudioBufferStats {
    pub audio_buffer: BufferStats,
    pub control_buffer: BufferStats,
    pub target_latency_us: u64,
}

/// Métriques de qualité audio
#[derive(Debug, Clone)]
pub struct AudioQualityMetrics {
    pub packet_loss_rate: f32,
    pub jitter_ms: f32,
    pub average_latency_ms: f32,
    pub missing_packets_count: usize,
    pub buffer_health: f32,           // 0.0 = critique, 1.0 = excellent
    pub quality_score: f32,           // 0.0 = mauvais, 1.0 = excellent
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{AudioPacket, PacketType};
    use bytes::Bytes;
    use uuid::Uuid;

    #[test]
    fn test_circular_buffer() {
        let mut buffer = CircularBuffer::new(2);
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();

        // Ajouter des packets
        buffer.push(AudioPacket::audio(user_id, channel_id, 0, Bytes::new(), 48000, 1));
        buffer.push(AudioPacket::audio(user_id, channel_id, 1, Bytes::new(), 48000, 1));
        
        assert_eq!(buffer.len(), 2);

        // Ajouter un troisième packet (dépasse la capacité)
        buffer.push(AudioPacket::audio(user_id, channel_id, 2, Bytes::new(), 48000, 1));
        
        assert_eq!(buffer.len(), 2);
        assert_eq!(buffer.stats().dropped_packets, 1);
    }

    #[test]
    fn test_audio_buffer_separation() {
        let mut buffer = AudioBuffer::new(10, 50); // 50ms latence cible
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();

        // Ajouter un packet de contrôle et un packet audio
        buffer.push(AudioPacket::audio_start(user_id, channel_id, 0));
        buffer.push(AudioPacket::audio(user_id, channel_id, 1, Bytes::new(), 48000, 1));

        let ready = buffer.get_ready_packets();
        
        // Le packet de contrôle doit être retourné immédiatement
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].header.packet_type, PacketType::AudioStart);
    }
}