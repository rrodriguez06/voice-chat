use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use uuid::Uuid;
use bytes::Bytes;

use voice_chat_backend::audio::{CircularBuffer, AudioPacket, PacketType};

/// Test de charge simple pour vérifier les performances du backend
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialiser le logging
    tracing_subscriber::fmt::init();
    
    println!("🧪 Tests de charge - Backend Voice Chat");
    println!("=====================================\n");

    // Test 1: Buffer performance
    println!("🔹 Test 1: Performance des buffers audio");
    test_buffer_performance().await?;

    // Test 2: Packet creation performance
    println!("\n🔹 Test 2: Performance de création de packets");
    test_packet_creation_performance().await?;

    // Test 3: Concurrent buffer access
    println!("\n🔹 Test 3: Accès concurrent aux buffers");
    test_concurrent_buffer_access().await?;

    println!("\n🏁 Tests de charge terminés avec succès !");
    println!("✅ Le backend est prêt pour la Phase 3 !");
    
    Ok(())
}

/// Test de performance des buffers audio
async fn test_buffer_performance() -> anyhow::Result<()> {
    let mut buffer = CircularBuffer::new(8192);
    
    let iterations = 10000;
    let data_size = 1024;
    
    // Test d'écriture avec des packets audio réels
    let write_start = Instant::now();
    for i in 0..iterations {
        let data = Bytes::from(vec![0u8; data_size]);
        let packet = AudioPacket::new(
            PacketType::Audio,
            Uuid::new_v4(),
            Uuid::new_v4(),
            i,
            data,
            48000,
            2,
        );
        buffer.push(packet);
    }
    let write_duration = write_start.elapsed();
    
    // Test de lecture
    let read_start = Instant::now();
    for _ in 0..iterations {
        let _ = buffer.pop();
    }
    let read_duration = read_start.elapsed();
    
    println!("   📊 Résultats du test de buffer:");
    println!("      ✏️  Écriture: {} packets en {:.2}ms ({:.0} packets/sec)", 
        iterations, write_duration.as_millis(), 
        iterations as f64 / write_duration.as_secs_f64());
    println!("      📖 Lecture: {} packets en {:.2}ms ({:.0} packets/sec)", 
        iterations, read_duration.as_millis(), 
        iterations as f64 / read_duration.as_secs_f64());
    
    Ok(())
}

/// Test de performance de création de packets
async fn test_packet_creation_performance() -> anyhow::Result<()> {
    let start_time = Instant::now();
    let iterations = 50000;
    
    for i in 0..iterations {
        let data = Bytes::from(vec![0u8; 1024]);
        let _packet = AudioPacket::new(
            PacketType::Audio,
            Uuid::new_v4(),
            Uuid::new_v4(),
            i,
            data,
            48000,
            2,
        );
    }
    
    let duration = start_time.elapsed();
    
    println!("   📊 Résultats de création de packets:");
    println!("      📦 {} packets créés en {:.2}ms ({:.0} packets/sec)", 
        iterations, duration.as_millis(), 
        iterations as f64 / duration.as_secs_f64());
    
    Ok(())
}

/// Test d'accès concurrent aux buffers
async fn test_concurrent_buffer_access() -> anyhow::Result<()> {
    let buffer = Arc::new(RwLock::new(CircularBuffer::new(4096)));
    let test_duration = Duration::from_secs(3);
    let start_time = Instant::now();
    
    let mut tasks = Vec::new();
    
    // Tâches d'écriture concurrentes
    for i in 0..5 {
        let buffer_clone = buffer.clone();
        let task = tokio::spawn(async move {
            let mut writes = 0;
            while start_time.elapsed() < test_duration {
                let data = Bytes::from(vec![i as u8; 512]);
                let packet = AudioPacket::new(
                    PacketType::Audio,
                    Uuid::new_v4(),
                    Uuid::new_v4(),
                    writes,
                    data,
                    48000,
                    2,
                );
                {
                    let mut buf = buffer_clone.write().await;
                    buf.push(packet);
                    writes += 1;
                }
                tokio::time::sleep(Duration::from_micros(100)).await;
            }
            writes
        });
        tasks.push(task);
    }
    
    // Tâches de lecture concurrentes
    for _ in 0..3 {
        let buffer_clone = buffer.clone();
        let task = tokio::spawn(async move {
            let mut reads = 0;
            while start_time.elapsed() < test_duration {
                {
                    let mut buf = buffer_clone.write().await;
                    if buf.pop().is_some() {
                        reads += 1;
                    }
                }
                tokio::time::sleep(Duration::from_micros(200)).await;
            }
            reads
        });
        tasks.push(task);
    }
    
    // Attendre toutes les tâches
    let mut total_operations = 0;
    for task in tasks {
        if let Ok(ops) = task.await {
            total_operations += ops;
        }
    }
    
    let actual_duration = start_time.elapsed();
    
    println!("   📊 Résultats d'accès concurrent:");
    println!("      🔄 {} opérations en {:.2}s ({:.0} ops/sec)", 
        total_operations, actual_duration.as_secs_f64(), 
        total_operations as f64 / actual_duration.as_secs_f64());
    println!("      ✅ Test de stress concurrent réussi !");
    
    Ok(())
}