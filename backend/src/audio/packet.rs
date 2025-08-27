use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Types de packets audio
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PacketType {
    /// Données audio PCM
    Audio = 0,
    /// Silence (pas d'audio à transmettre)
    Silence = 1,
    /// Début de transmission audio
    AudioStart = 2,
    /// Fin de transmission audio
    AudioStop = 3,
    /// Packet de synchronisation/heartbeat
    Sync = 4,
}

/// Header du packet audio - 32 bytes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioHeader {
    /// Type de packet
    pub packet_type: PacketType,
    /// ID de l'utilisateur qui envoie
    pub user_id: Uuid,
    /// ID du channel
    pub channel_id: Uuid,
    /// Numéro de séquence pour l'ordre des packets
    pub sequence: u32,
    /// Timestamp en microsecondes depuis UNIX_EPOCH
    pub timestamp: u64,
    /// Taille du payload en bytes
    pub payload_size: u16,
    /// Sample rate de l'audio (Hz)
    pub sample_rate: u32,
    /// Nombre de channels audio (mono=1, stereo=2)
    pub channels: u8,
    /// Réservé pour usage futur
    pub reserved: [u8; 3],
}

impl AudioHeader {
    pub fn new(
        packet_type: PacketType,
        user_id: Uuid,
        channel_id: Uuid,
        sequence: u32,
        payload_size: u16,
        sample_rate: u32,
        channels: u8,
    ) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        Self {
            packet_type,
            user_id,
            channel_id,
            sequence,
            timestamp,
            payload_size,
            sample_rate,
            channels,
            reserved: [0; 3],
        }
    }

    /// Sérialise le header en bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    /// Désérialise des bytes en header
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        bincode::deserialize(bytes)
            .map_err(|e| format!("Failed to deserialize header: {}", e))
    }

    /// Calcule l'âge du packet en microsecondes
    pub fn age_micros(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;
        now.saturating_sub(self.timestamp)
    }

    /// Vérifie si le packet est trop ancien (> 100ms)
    pub fn is_stale(&self) -> bool {
        self.age_micros() > 100_000 // 100ms en microsecondes
    }
}

/// Packet audio complet
#[derive(Debug, Clone)]
pub struct AudioPacket {
    /// Header avec métadonnées
    pub header: AudioHeader,
    /// Données audio en bytes
    pub payload: Bytes,
}

impl AudioPacket {
    pub fn new(
        packet_type: PacketType,
        user_id: Uuid,
        channel_id: Uuid,
        sequence: u32,
        payload: Bytes,
        sample_rate: u32,
        channels: u8,
    ) -> Self {
        let header = AudioHeader::new(
            packet_type,
            user_id,
            channel_id,
            sequence,
            payload.len() as u16,
            sample_rate,
            channels,
        );

        Self { header, payload }
    }

    /// Crée un packet audio avec données PCM
    pub fn audio(
        user_id: Uuid,
        channel_id: Uuid,
        sequence: u32,
        audio_data: Bytes,
        sample_rate: u32,
        channels: u8,
    ) -> Self {
        Self::new(
            PacketType::Audio,
            user_id,
            channel_id,
            sequence,
            audio_data,
            sample_rate,
            channels,
        )
    }

    /// Crée un packet de silence
    pub fn silence(
        user_id: Uuid,
        channel_id: Uuid,
        sequence: u32,
        sample_rate: u32,
        channels: u8,
    ) -> Self {
        Self::new(
            PacketType::Silence,
            user_id,
            channel_id,
            sequence,
            Bytes::new(),
            sample_rate,
            channels,
        )
    }

    /// Crée un packet de début de transmission
    pub fn audio_start(user_id: Uuid, channel_id: Uuid, sequence: u32) -> Self {
        Self::new(
            PacketType::AudioStart,
            user_id,
            channel_id,
            sequence,
            Bytes::new(),
            48000,
            1,
        )
    }

    /// Crée un packet de fin de transmission
    pub fn audio_stop(user_id: Uuid, channel_id: Uuid, sequence: u32) -> Self {
        Self::new(
            PacketType::AudioStop,
            user_id,
            channel_id,
            sequence,
            Bytes::new(),
            48000,
            1,
        )
    }

    /// Sérialise le packet complet en bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        let header_bytes = self.header.to_bytes();
        let mut packet_bytes = Vec::with_capacity(header_bytes.len() + self.payload.len());
        
        packet_bytes.extend_from_slice(&header_bytes);
        packet_bytes.extend_from_slice(&self.payload);
        
        packet_bytes
    }

    /// Désérialise des bytes en packet
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() < 10 {
            return Err("Packet too small".to_string());
        }

        // Essayer de désérialiser le header avec bincode
        // bincode va nous dire combien de bytes il a consommé
        let mut cursor = std::io::Cursor::new(bytes);
        let header: AudioHeader = bincode::deserialize_from(&mut cursor)
            .map_err(|e| format!("Failed to deserialize header: {}", e))?;
        
        let header_size = cursor.position() as usize;
        
        // Le reste est le payload
        let payload_start = header_size;
        let payload_end = payload_start + header.payload_size as usize;
        
        if bytes.len() < payload_end {
            return Err(format!("Payload size mismatch: expected {} bytes, got {}", payload_end, bytes.len()));
        }

        let payload = Bytes::copy_from_slice(&bytes[payload_start..payload_end]);

        Ok(Self { header, payload })
    }

    /// Taille totale du packet en bytes
    pub fn size(&self) -> usize {
        32 + self.payload.len() // Header + payload
    }

    /// Vérifie si le packet contient de l'audio
    pub fn has_audio(&self) -> bool {
        matches!(self.header.packet_type, PacketType::Audio)
    }

    /// Vérifie si le packet est un événement de contrôle
    pub fn is_control(&self) -> bool {
        matches!(
            self.header.packet_type,
            PacketType::AudioStart | PacketType::AudioStop | PacketType::Sync
        )
    }
}

/// Utilitaires pour la gestion des séquences
pub struct SequenceManager {
    next_sequence: u32,
}

impl SequenceManager {
    pub fn new() -> Self {
        Self { next_sequence: 0 }
    }

    pub fn next(&mut self) -> u32 {
        let seq = self.next_sequence;
        self.next_sequence = self.next_sequence.wrapping_add(1);
        seq
    }

    /// Vérifie si une séquence est dans l'ordre (avec tolérance pour le réordonnancement)
    pub fn is_in_order(&self, sequence: u32) -> bool {
        // Tolérance de 10 packets pour le réordonnancement
        let tolerance = 10u32;
        let expected = self.next_sequence;
        
        // Gestion du wrap-around
        if expected >= tolerance {
            sequence >= (expected - tolerance) && sequence <= (expected + tolerance)
        } else {
            sequence <= (expected + tolerance) || sequence >= (u32::MAX - tolerance + expected)
        }
    }
}

impl Default for SequenceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_packet_creation() {
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let audio_data = Bytes::from(vec![1, 2, 3, 4]);

        let packet = AudioPacket::audio(user_id, channel_id, 0, audio_data.clone(), 48000, 1);

        assert_eq!(packet.header.packet_type, PacketType::Audio);
        assert_eq!(packet.header.user_id, user_id);
        assert_eq!(packet.header.channel_id, channel_id);
        assert_eq!(packet.header.sequence, 0);
        assert_eq!(packet.header.payload_size, 4);
        assert_eq!(packet.payload, audio_data);
    }

    #[test]
    fn test_packet_serialization() {
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let packet = AudioPacket::silence(user_id, channel_id, 1, 48000, 1);

        let bytes = packet.to_bytes();
        let deserialized = AudioPacket::from_bytes(&bytes).unwrap();

        assert_eq!(deserialized.header.packet_type, packet.header.packet_type);
        assert_eq!(deserialized.header.user_id, packet.header.user_id);
        assert_eq!(deserialized.header.sequence, packet.header.sequence);
    }

    #[test]
    fn test_sequence_manager() {
        let mut seq_mgr = SequenceManager::new();
        
        assert_eq!(seq_mgr.next(), 0);
        assert_eq!(seq_mgr.next(), 1);
        assert_eq!(seq_mgr.next(), 2);
        
        assert!(seq_mgr.is_in_order(3));
        assert!(seq_mgr.is_in_order(12)); // Dans la tolérance
        assert!(!seq_mgr.is_in_order(100)); // Trop loin
    }
}