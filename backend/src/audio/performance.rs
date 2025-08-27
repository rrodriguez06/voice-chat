use std::sync::Arc;
use tokio::sync::{RwLock, mpsc, Semaphore, broadcast};
use tokio::task::JoinHandle;
use uuid::Uuid;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use bytes::Bytes;
use crate::audio::{AudioPacket, AudioMixer};

/// Configuration du pool de threads pour le traitement audio
#[derive(Debug, Clone)]
pub struct AudioThreadPoolConfig {
    pub max_workers: usize,
    pub queue_size: usize,
    pub worker_timeout: Duration,
    pub enable_priority_queue: bool,
    pub max_concurrent_mixes: usize,
}

impl Default for AudioThreadPoolConfig {
    fn default() -> Self {
        Self {
            max_workers: num_cpus::get().max(4),
            queue_size: 1000,
            worker_timeout: Duration::from_millis(100),
            enable_priority_queue: true,
            max_concurrent_mixes: 16,
        }
    }
}

/// Priorité de traitement audio
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AudioProcessingPriority {
    Low = 0,
    Normal = 1,
    High = 2,
    Critical = 3,
}

/// Tâche de traitement audio
#[derive(Debug)]
pub struct AudioProcessingTask {
    pub id: Uuid,
    pub channel_id: Uuid,
    pub packets: Vec<AudioPacket>,
    pub priority: AudioProcessingPriority,
    pub timestamp: Instant,
    pub response_sender: tokio::sync::oneshot::Sender<Option<Bytes>>,
}

/// Statistiques du pool de threads
#[derive(Debug, Clone)]
pub struct ThreadPoolStats {
    pub active_workers: usize,
    pub queued_tasks: usize,
    pub completed_tasks: u64,
    pub failed_tasks: u64,
    pub average_processing_time_us: u64,
    pub peak_queue_size: usize,
}

/// Pool de threads optimisé pour le traitement audio en temps réel
#[derive(Debug)]
pub struct AudioThreadPool {
    config: AudioThreadPoolConfig,
    task_sender: mpsc::Sender<AudioProcessingTask>,
    workers: Vec<JoinHandle<()>>,
    mixer: Arc<RwLock<AudioMixer>>,
    semaphore: Arc<Semaphore>,
    stats: Arc<RwLock<ThreadPoolStats>>,
    shutdown_sender: Option<broadcast::Sender<()>>,
}

impl AudioThreadPool {
    /// Crée un nouveau pool de threads audio
    pub async fn new(config: AudioThreadPoolConfig, mixer: Arc<RwLock<AudioMixer>>) -> Self {
        let (task_sender, task_receiver) = mpsc::channel(config.queue_size);
        let (shutdown_sender, _shutdown_receiver) = broadcast::channel(1);
        
        let semaphore = Arc::new(Semaphore::new(config.max_concurrent_mixes));
        let stats = Arc::new(RwLock::new(ThreadPoolStats {
            active_workers: 0,
            queued_tasks: 0,
            completed_tasks: 0,
            failed_tasks: 0,
            average_processing_time_us: 0,
            peak_queue_size: 0,
        }));

        let task_receiver = Arc::new(tokio::sync::Mutex::new(task_receiver));
        let mut workers = Vec::new();
        
        // Créer les workers
        for worker_id in 0..config.max_workers {
            let worker = Self::spawn_worker(
                worker_id,
                task_receiver.clone(),
                shutdown_sender.subscribe(),
                mixer.clone(),
                semaphore.clone(),
                stats.clone(),
                config.clone(),
            );
            workers.push(worker);
        }

        Self {
            config,
            task_sender,
            workers,
            mixer,
            semaphore,
            stats,
            shutdown_sender: Some(shutdown_sender),
        }
    }

    /// Spawn un worker thread
    fn spawn_worker(
        worker_id: usize,
        task_receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<AudioProcessingTask>>>,
        mut shutdown_receiver: broadcast::Receiver<()>,
        mixer: Arc<RwLock<AudioMixer>>,
        semaphore: Arc<Semaphore>,
        stats: Arc<RwLock<ThreadPoolStats>>,
        config: AudioThreadPoolConfig,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            tracing::debug!("Audio worker {} started", worker_id);
            
            {
                let mut stats_guard = stats.write().await;
                stats_guard.active_workers += 1;
            }

            loop {
                let task = tokio::select! {
                    // Traitement des tâches
                    task = async {
                        let mut receiver = task_receiver.lock().await;
                        receiver.recv().await
                    } => {
                        match task {
                            Some(task) => task,
                            None => {
                                tracing::debug!("Worker {} stopping: task channel closed", worker_id);
                                break;
                            }
                        }
                    }
                    
                    // Signal d'arrêt
                    _ = shutdown_receiver.recv() => {
                        tracing::debug!("Worker {} stopping: shutdown signal received", worker_id);
                        break;
                    }
                    
                    // Timeout pour éviter les blocages
                    _ = tokio::time::sleep(config.worker_timeout) => {
                        continue;
                    }
                };

                let _permit = semaphore.acquire().await.unwrap();
                Self::process_task(task, &mixer, &stats).await;
            }

            {
                let mut stats_guard = stats.write().await;
                stats_guard.active_workers = stats_guard.active_workers.saturating_sub(1);
            }

            tracing::debug!("Audio worker {} stopped", worker_id);
        })
    }

    /// Traite une tâche audio
    async fn process_task(
        task: AudioProcessingTask,
        mixer: &Arc<RwLock<AudioMixer>>,
        stats: &Arc<RwLock<ThreadPoolStats>>,
    ) {
        let start_time = Instant::now();
        
        // Vérifier si la tâche n'est pas trop ancienne (dépassement de délai)
        let age = start_time.duration_since(task.timestamp);
        if age > Duration::from_millis(500) { // Timeout de 500ms pour l'audio temps réel
            tracing::warn!("Dropping audio task {} due to timeout: {:?}", task.id, age);
            let _ = task.response_sender.send(None);
            
            let mut stats_guard = stats.write().await;
            stats_guard.failed_tasks += 1;
            return;
        }

        // Traitement du mixage
        let result = {
            let mut mixer_guard = mixer.write().await;
            mixer_guard.mix_packets_advanced(task.packets, task.channel_id)
        };

        // Envoyer le résultat
        let _ = task.response_sender.send(result);

        // Mettre à jour les statistiques
        let processing_time = start_time.elapsed();
        let mut stats_guard = stats.write().await;
        stats_guard.completed_tasks += 1;
        
        // Mettre à jour la moyenne mobile du temps de traitement
        let new_time_us = processing_time.as_micros() as u64;
        if stats_guard.completed_tasks == 1 {
            stats_guard.average_processing_time_us = new_time_us;
        } else {
            // Moyenne mobile avec facteur d'oubli
            stats_guard.average_processing_time_us = 
                (stats_guard.average_processing_time_us * 9 + new_time_us) / 10;
        }

        tracing::trace!(
            "Processed audio task {} in {:?} (priority: {:?})", 
            task.id, 
            processing_time,
            task.priority
        );
    }

    /// Soumet une tâche de mixage audio pour traitement asynchrone
    pub async fn submit_mix_task(
        &self,
        channel_id: Uuid,
        packets: Vec<AudioPacket>,
        priority: AudioProcessingPriority,
    ) -> Result<tokio::sync::oneshot::Receiver<Option<Bytes>>, &'static str> {
        let (response_sender, response_receiver) = tokio::sync::oneshot::channel();
        
        let task = AudioProcessingTask {
            id: Uuid::new_v4(),
            channel_id,
            packets,
            priority,
            timestamp: Instant::now(),
            response_sender,
        };

        // Mettre à jour les statistiques de file d'attente
        {
            let mut stats_guard = self.stats.write().await;
            stats_guard.queued_tasks += 1;
            if stats_guard.queued_tasks > stats_guard.peak_queue_size {
                stats_guard.peak_queue_size = stats_guard.queued_tasks;
            }
        }

        match self.task_sender.try_send(task) {
            Ok(_) => Ok(response_receiver),
            Err(mpsc::error::TrySendError::Full(_)) => {
                // File d'attente pleine - diminuer les statistiques
                let mut stats_guard = self.stats.write().await;
                stats_guard.queued_tasks = stats_guard.queued_tasks.saturating_sub(1);
                stats_guard.failed_tasks += 1;
                
                Err("Thread pool queue is full")
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                let mut stats_guard = self.stats.write().await;
                stats_guard.queued_tasks = stats_guard.queued_tasks.saturating_sub(1);
                stats_guard.failed_tasks += 1;
                
                Err("Thread pool is shut down")
            }
        }
    }

    /// Soumet une tâche avec gestion des priorités
    pub async fn submit_priority_mix(
        &self,
        channel_id: Uuid,
        packets: Vec<AudioPacket>,
        priority: AudioProcessingPriority,
        timeout: Duration,
    ) -> Result<Option<Bytes>, &'static str> {
        let receiver = self.submit_mix_task(channel_id, packets, priority).await?;
        
        match tokio::time::timeout(timeout, receiver).await {
            Ok(Ok(result)) => {
                // Décrémenter le compteur de file d'attente
                let mut stats_guard = self.stats.write().await;
                stats_guard.queued_tasks = stats_guard.queued_tasks.saturating_sub(1);
                Ok(result)
            }
            Ok(Err(_)) => Err("Task processing was cancelled"),
            Err(_) => Err("Task processing timed out"),
        }
    }

    /// Récupère les statistiques du pool de threads
    pub async fn get_stats(&self) -> ThreadPoolStats {
        self.stats.read().await.clone()
    }

    /// Récupère l'état de santé du pool
    pub async fn health_check(&self) -> PoolHealthStatus {
        let stats = self.get_stats().await;
        
        let queue_utilization = if self.config.queue_size > 0 {
            (stats.queued_tasks as f32 / self.config.queue_size as f32) * 100.0
        } else {
            0.0
        };

        let worker_utilization = if self.config.max_workers > 0 {
            (stats.active_workers as f32 / self.config.max_workers as f32) * 100.0
        } else {
            0.0
        };

        let average_latency_ms = stats.average_processing_time_us as f32 / 1000.0;

        PoolHealthStatus {
            is_healthy: queue_utilization < 90.0 && worker_utilization > 0.0 && average_latency_ms < 50.0,
            queue_utilization_percent: queue_utilization,
            worker_utilization_percent: worker_utilization,
            average_latency_ms,
            error_rate_percent: if stats.completed_tasks + stats.failed_tasks > 0 {
                (stats.failed_tasks as f32 / (stats.completed_tasks + stats.failed_tasks) as f32) * 100.0
            } else {
                0.0
            },
        }
    }

    /// Ajuste dynamiquement la configuration du pool
    pub async fn adjust_performance(&mut self, _load_metrics: &LoadMetrics) {
        let health = self.health_check().await;
        
        if !health.is_healthy {
            tracing::warn!("Audio thread pool unhealthy: {:?}", health);
            
            // Si la file d'attente est saturée, augmenter le nombre de workers
            if health.queue_utilization_percent > 85.0 && self.workers.len() < self.config.max_workers * 2 {
                tracing::info!("Scaling up audio workers due to high queue utilization");
                // En production, implémenter l'ajout dynamique de workers
            }
            
            // Si la latence est trop élevée, ajuster les timeouts
            if health.average_latency_ms > 100.0 {
                tracing::warn!("High audio processing latency: {:.2}ms", health.average_latency_ms);
                // En production, implémenter l'ajustement des paramètres
            }
        }
    }

    /// Arrête proprement le pool de threads
    pub async fn shutdown(mut self) -> Result<(), &'static str> {
        tracing::info!("Shutting down audio thread pool...");
        
        // Fermer la file de tâches
        drop(self.task_sender);
        
        // Envoyer le signal d'arrêt aux workers
        if let Some(shutdown_sender) = self.shutdown_sender.take() {
            let _ = shutdown_sender.send(());
        }

        // Attendre que tous les workers se terminent
        for worker in self.workers {
            if let Err(e) = worker.await {
                tracing::error!("Worker failed to shut down cleanly: {}", e);
            }
        }

        tracing::info!("Audio thread pool shut down complete");
        Ok(())
    }
}

/// État de santé du pool de threads
#[derive(Debug, Clone)]
pub struct PoolHealthStatus {
    pub is_healthy: bool,
    pub queue_utilization_percent: f32,
    pub worker_utilization_percent: f32,
    pub average_latency_ms: f32,
    pub error_rate_percent: f32,
}

/// Métriques de charge système
#[derive(Debug, Clone)]
pub struct LoadMetrics {
    pub cpu_usage_percent: f32,
    pub memory_usage_mb: u64,
    pub active_connections: usize,
    pub packets_per_second: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_thread_pool_creation() {
        let config = AudioThreadPoolConfig::default();
        let mixer = Arc::new(RwLock::new(AudioMixer::default()));
        
        let pool = AudioThreadPool::new(config.clone(), mixer).await;
        
        let stats = pool.get_stats().await;
        assert_eq!(stats.active_workers, config.max_workers);
        assert_eq!(stats.completed_tasks, 0);
        
        let _ = pool.shutdown().await;
    }

    #[tokio::test]
    async fn test_health_check() {
        let config = AudioThreadPoolConfig::default();
        let mixer = Arc::new(RwLock::new(AudioMixer::default()));
        
        let pool = AudioThreadPool::new(config, mixer).await;
        
        let health = pool.health_check().await;
        assert!(health.worker_utilization_percent > 0.0);
        
        let _ = pool.shutdown().await;
    }

    #[tokio::test]
    async fn test_task_submission() {
        let config = AudioThreadPoolConfig {
            max_workers: 2,
            queue_size: 10,
            ..Default::default()
        };
        let mixer = Arc::new(RwLock::new(AudioMixer::default()));
        
        let pool = AudioThreadPool::new(config, mixer).await;
        
        let channel_id = Uuid::new_v4();
        let packets = vec![];
        
        let result = pool.submit_priority_mix(
            channel_id,
            packets,
            AudioProcessingPriority::Normal,
            Duration::from_millis(100),
        ).await;
        
        // Devrait retourner None car pas de packets audio
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
        
        let _ = pool.shutdown().await;
    }
}