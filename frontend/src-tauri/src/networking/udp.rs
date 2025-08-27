use anyhow::{Result, Context};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
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
    socket: Arc<UdpSocket>,
    server_addr: SocketAddr,
    sequence: std::sync::atomic::AtomicU32,
}

impl Clone for AudioUdpClient {
    fn clone(&self) -> Self {
        Self {
            socket: Arc::clone(&self.socket),
            server_addr: self.server_addr,
            sequence: std::sync::atomic::AtomicU32::new(0),
        }
    }
}

impl AudioUdpClient {
    /// Crée un nouveau client UDP audio
    pub async fn new(server_addr: SocketAddr) -> Result<Self> {
        // Utiliser le port 8083 pour que le backend puisse nous renvoyer l'audio sur le même port
        println!("🔗 AudioUdpClient: Attempting to bind to port 8083...");
        let socket = match UdpSocket::bind("0.0.0.0:8083").await {
            Ok(sock) => {
                println!("✅ AudioUdpClient: Successfully bound to port 8083");
                sock
            }
            Err(e) => {
                println!("❌ AudioUdpClient: Failed to bind to port 8083: {}", e);
                println!("🔗 AudioUdpClient: Trying dynamic port...");
                let sock = UdpSocket::bind("0.0.0.0:0").await
                    .context("Failed to bind UDP socket on any port")?;
                println!("✅ AudioUdpClient: Bound to dynamic port {:?}", sock.local_addr()?);
                sock
            }
        };
            
        Ok(Self {
            socket: Arc::new(socket),
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

    /// Obtient le socket partagé pour utilisation par d'autres composants
    pub fn get_shared_socket(&self) -> Arc<UdpSocket> {
        Arc::clone(&self.socket)
    }

    /// Démarre l'écoute des packets entrants sur le même socket
    pub async fn start_receiving(
        &self,
        user_id: Uuid,
        audio_tx: tokio::sync::mpsc::UnboundedSender<(Vec<f32>, u32, u8)>,
        mut stop_rx: tokio::sync::mpsc::UnboundedReceiver<bool>,
    ) -> Result<()> {
        println!("🔊 UdpClient: Starting to receive audio packets...");
        
        let socket = Arc::clone(&self.socket);
        let server_addr = self.server_addr;
        let mut buf = vec![0u8; 4096];
        
        loop {
            tokio::select! {
                // Vérifier les commandes d'arrêt
                cmd = stop_rx.recv() => {
                    if cmd.is_none() {
                        println!("🔊 UdpClient: Received stop command for receiving");
                        break;
                    }
                }
                // Recevoir des packets UDP
                result = socket.recv_from(&mut buf) => {
                    match result {
                        Ok((size, from)) => {
                            // Vérifier que le packet vient du serveur
                            if from.ip() == server_addr.ip() && from.port() == server_addr.port() {
                                // Essayer de désérialiser le packet audio
                                if let Ok(packet) = AudioPacket::from_bytes(&buf[..size]) {
                                    // Traiter les packets audio de type Audio
                                    if packet.header.packet_type == PacketType::Audio {
                                        // En mode normal, on reçoit l'audio d'autres utilisateurs
                                        // En mode loopback, on reçoit notre propre audio
                                        let is_own_packet = packet.header.user_id == user_id;
                                        
                                        println!("🔊 UdpClient: Received audio packet from user {} {} - Seq: {}, Payload: {} bytes, SR: {}Hz, CH: {}", 
                                            packet.header.user_id, 
                                            if is_own_packet { "(own)" } else { "(other)" },
                                            packet.header.sequence, packet.payload.len(),
                                            packet.header.sample_rate, packet.header.channels);
                                        
                                        // Convertir les bytes PCM en f32
                                        let audio_samples = Self::pcm_to_f32(&packet.payload);
                                        
                                        // Envoyer vers le lecteur audio avec métadonnées pour conversion
                                        if let Err(_) = audio_tx.send((audio_samples, packet.header.sample_rate, packet.header.channels)) {
                                            // Channel fermé, arrêter
                                            println!("🔊 UdpClient: Audio channel closed, stopping reception");
                                            break;
                                        }
                                    } else {
                                        println!("🔇 UdpClient: Ignoring non-audio packet type: {:?}", packet.header.packet_type);
                                    }
                                } else {
                                    println!("⚠️ UdpClient: Failed to parse packet from {}", from);
                                }
                            } else {
                                println!("🔇 UdpClient: Ignoring packet from unknown source: {}", from);
                            }
                        }
                        Err(e) => {
                            eprintln!("❌ UdpClient receive error: {}", e);
                            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                        }
                    }
                }
            }
        }
        
        println!("🔊 UdpClient: Stopped receiving audio packets");
        Ok(())
    }

    /// Convertit les bytes PCM en échantillons f32
    fn pcm_to_f32(pcm_data: &[u8]) -> Vec<f32> {
        let mut samples = Vec::with_capacity(pcm_data.len() / 2);
        
        for chunk in pcm_data.chunks_exact(2) {
            let sample_i16 = i16::from_le_bytes([chunk[0], chunk[1]]);
            let sample_f32 = sample_i16 as f32 / 32767.0;
            samples.push(sample_f32);
        }
        
        samples
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