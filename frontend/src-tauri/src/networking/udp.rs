use anyhow::{Result, Context};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::UdpSocket;
use uuid::Uuid;

/// Types de packets audio (identique au backend)
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

/// Header du packet audio - 32 bytes (identique au backend)
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
        Self {
            packet_type,
            user_id,
            channel_id,
            sequence,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros() as u64,
            payload_size,
            sample_rate,
            channels,
            reserved: [0; 3],
        }
    }

    /// Sérialise en bytes (compatible backend)
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        bincode::serialize(self).map_err(|e| anyhow::anyhow!("Serialization error: {}", e))
    }
}

/// Structure des packets audio (compatible avec le backend)
#[derive(Debug, Clone)]
pub struct AudioPacket {
    pub header: AudioHeader,
    pub payload: Bytes,
}

impl AudioPacket {
    /// Désérialise des bytes en packet
    pub fn from_bytes(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < 10 {
            return Err(anyhow::anyhow!("Packet too small"));
        }

        // Essayer de désérialiser le header avec bincode
        let mut cursor = std::io::Cursor::new(bytes);
        let header: AudioHeader = bincode::deserialize_from(&mut cursor)
            .map_err(|e| anyhow::anyhow!("Failed to deserialize header: {}", e))?;
        
        let header_size = cursor.position() as usize;
        
        // Le reste est le payload
        let payload_start = header_size;
        let payload_end = payload_start + header.payload_size as usize;
        
        if bytes.len() < payload_end {
            return Err(anyhow::anyhow!("Payload size mismatch: expected {} bytes, got {}", payload_end, bytes.len()));
        }

        let payload = Bytes::copy_from_slice(&bytes[payload_start..payload_end]);

        Ok(Self { header, payload })
    }
}

impl AudioPacket {
    /// Crée un nouveau packet audio
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
        
        Self {
            header,
            payload,
        }
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

    /// Sérialise le packet en bytes (compatible backend)
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let header_bytes = self.header.to_bytes()?;
        let mut bytes = Vec::with_capacity(header_bytes.len() + self.payload.len());
        bytes.extend_from_slice(&header_bytes);
        bytes.extend_from_slice(&self.payload);
        Ok(bytes)
    }
}

/// Client UDP pour l'audio
#[derive(Debug)]
pub struct AudioUdpClient {
    socket: UdpSocket,
    server_addr: SocketAddr,
    sequence: std::sync::atomic::AtomicU32,
}

impl Clone for AudioUdpClient {
    fn clone(&self) -> Self {
        // Créer un nouveau socket (impossible de cloner UdpSocket directement)
        // On va retourner une version simplifiée qui sera recréée à l'usage
        Self {
            socket: UdpSocket::from_std(std::net::UdpSocket::bind("0.0.0.0:0").unwrap()).unwrap(),
            server_addr: self.server_addr,
            sequence: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

impl AudioUdpClient {
    /// Crée un nouveau client UDP audio
    pub async fn new(server_addr: SocketAddr) -> Result<Self> {
        let socket = UdpSocket::bind("0.0.0.0:0").await
            .context("Failed to bind UDP socket")?;
            
        Ok(Self {
            socket,
            server_addr,
            sequence: std::sync::atomic::AtomicU32::new(0),
        })
    }

    /// Envoie un packet audio au serveur
    pub async fn send_audio_packet(&self, packet: AudioPacket) -> Result<()> {
        let bytes = packet.to_bytes()?;
        
        self.socket.send_to(&bytes, self.server_addr).await
            .context("Failed to send audio packet")?;
            
        Ok(())
    }

    /// Envoie des données audio brutes
    pub async fn send_audio_data(
        &self,
        user_id: Uuid,
        channel_id: Uuid,
        audio_data: Vec<f32>,
        sample_rate: u32,
        channels: u8,
    ) -> Result<()> {
        // Convertir f32 vers bytes (PCM 16-bit)
        println!("🎵 UdpClient: Converting {} f32 samples (SR: {}, CH: {})", 
            audio_data.len(), sample_rate, channels);
            
        let mut pcm_data = Vec::with_capacity(audio_data.len() * 2);
        for sample in audio_data {
            let sample_i16 = (sample.clamp(-1.0, 1.0) * 32767.0) as i16;
            pcm_data.extend_from_slice(&sample_i16.to_le_bytes());
        }
        
        println!("🎵 UdpClient: -> {} PCM bytes", pcm_data.len());

        let sequence = self.sequence.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let packet = AudioPacket::audio(
            user_id,
            channel_id,
            sequence,
            Bytes::from(pcm_data),
            sample_rate,
            channels,
        );

        self.send_audio_packet(packet).await
    }

    /// Obtient l'adresse locale du socket
    pub fn local_addr(&self) -> Result<SocketAddr> {
        self.socket.local_addr()
            .context("Failed to get local socket address")
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_packet_serialization() {
        let user_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let audio_data = vec![0.5, -0.3, 0.8, -0.1];
        
        let packet = AudioPacket::audio(
            user_id,
            channel_id,
            42,
            Bytes::from(audio_data.iter().map(|&f| (f * 127.0) as i8 as u8).collect::<Vec<u8>>()),
            48000,
            1,
        );

        let bytes = packet.to_bytes().unwrap();
        assert!(bytes.len() >= 32); // Au moins la taille du header
    }

    #[tokio::test]
    async fn test_udp_client_creation() {
        let server_addr: SocketAddr = "127.0.0.1:8082".parse().unwrap();
        let client = AudioUdpClient::new(server_addr).await;
        assert!(client.is_ok());
    }
}