# Voice Chat Backend

Backend Rust pour une application de chat vocal alternative à Discord, optimisée pour la faible latence et la qualité audio.

## Architecture

Le backend est structuré en modules pour une architecture claire et maintenable :

```
src/
├── main.rs              # Point d'entrée avec gestion des signaux
├── lib.rs               # Modules et exports publics
├── server.rs            # Serveur HTTP principal avec routage
├── config.rs            # Configuration chargée depuis settings.toml
├── error.rs             # Types d'erreurs personnalisés
├── models/              # Structures de données
│   ├── user.rs          # User, CreateUserRequest, UserResponse
│   ├── channel.rs       # Channel, CreateChannelRequest, etc.
│   ├── message.rs       # Messages WebSocket (Client/Server)
│   └── audio.rs         # Responses pour statistiques audio
├── services/            # Logique métier
│   ├── user_service.rs  # Gestion des utilisateurs en mémoire
│   ├── channel_service.rs # Gestion des channels avec permissions
│   └── audio_service.rs # Service audio avec intégration UDP
├── handlers/            # Handlers HTTP et WebSocket
│   ├── api.rs           # Endpoints REST pour users/channels/audio
│   └── websocket.rs     # Validation des messages WebSocket
├── networking/          # Couche réseau
│   ├── websocket.rs     # Serveur WebSocket pour signalisation
│   └── udp.rs           # Serveur UDP pour streaming audio
└── audio/               # Infrastructure audio
    ├── packet.rs        # Structures AudioPacket avec headers
    ├── buffer.rs        # Buffer circulaire pour packets audio
    ├── router.rs        # Routeur audio avec statistiques
    └── mixer.rs         # Mixeur audio (préparé pour développement)
```

## Fonctionnalités Implémentées

### 🔧 Gestion des Utilisateurs
- **Création/consultation** via API REST
- **Stockage en mémoire** avec DashMap (thread-safe)
- **Authentication basique** par pseudo unique

### 🏠 Gestion des Channels
- **CRUD complet** via API REST
- **Système de permissions** (owner/member)
- **Limites configurables** d'utilisateurs par channel
- **Join/Leave** avec validation des droits

### 🎵 Infrastructure Audio (NOUVEAU)
- **Serveur UDP** dédié pour streaming audio faible latence
- **Protocol custom** avec headers (user_id, channel_id, timestamp, type)
- **Buffer circulaire** avec gestion automatique des packets expirés
- **Routeur audio** avec statistiques temps réel
- **Support multiple types** : Audio, AudioStart, AudioStop, Silence

### 📊 Monitoring et Métriques
- **Statistiques par channel** : packets reçus/routés, latence, perte
- **Métriques globales** : clients connectés, channels actifs
- **Endpoints dédiés** pour monitoring : `/api/audio/config`, `/api/channels/:id/audio/stats`

### 🌐 Networking
- **WebSocket** sur port 3001 pour signalisation temps réel
- **HTTP REST** sur port 3000 pour opérations CRUD
- **UDP** sur port 3002 pour streaming audio
- **CORS** configuré pour développement local

## Configuration

Le serveur est configuré via `settings.toml` :

```toml
[server]
host = "127.0.0.1"
port = 3000
websocket_port = 3001

[audio]
udp_port = 3002
sample_rate = 48000
channels = 2
buffer_size = 1024
max_packet_size = 1500

[limits]
max_users_per_channel = 10
max_channels = 100
```

## API Endpoints

### Users
- `POST /api/users` - Créer un utilisateur
- `GET /api/users/:id` - Récupérer un utilisateur

### Channels
- `GET /api/channels` - Liste des channels
- `POST /api/channels` - Créer un channel
- `GET /api/channels/:id` - Détails d'un channel
- `GET /api/channels/:id/audio/stats` - Statistiques audio du channel

### Audio
- `GET /api/audio/config` - Configuration audio du serveur

### WebSocket (`ws://localhost:3001`)
Messages supportés :
```json
// Client -> Server
{"Authenticate": {"username": "pseudo"}}
{"JoinChannel": {"channel_id": "uuid"}}
{"LeaveChannel": {"channel_id": "uuid"}}
{"StartAudio": {"channel_id": "uuid"}}
{"StopAudio": {"channel_id": "uuid"}}

// Server -> Client
{"UserJoined": {"user": {...}, "channel_id": "uuid"}}
{"UserLeft": {"user_id": "uuid", "channel_id": "uuid"}}
{"AudioStarted": {"user_id": "uuid", "channel_id": "uuid"}}
```

## Protocol Audio UDP

### Structure des Packets

```rust
pub struct AudioHeader {
    pub user_id: Uuid,           // Utilisateur source
    pub channel_id: Uuid,        // Channel de destination
    pub sequence_number: u32,    // Numéro de séquence
    pub timestamp: u64,          // Timestamp en microsecondes
    pub packet_type: PacketType, // Audio, AudioStart, AudioStop, Silence
    pub sample_rate: u32,        // Taux d'échantillonnage
    pub channels: u16,           // Nombre de canaux audio
}

pub struct AudioPacket {
    pub header: AudioHeader,
    pub payload: Bytes,          // Données audio encodées
}
```

### Flux de Données Audio

1. **Authentification** : Clients s'authentifient via WebSocket
2. **Join Channel** : Clients rejoignent un channel audio
3. **Streaming** : Clients envoient packets UDP au serveur
4. **Routage** : Serveur route vers les autres utilisateurs du channel
5. **Buffering** : Gestion automatique de la latence et packets perdus

## Démarrage

```bash
# Installation des dépendances
cargo build

# Lancement du serveur
cargo run

# Tests
cargo test

# Vérification
curl http://localhost:3000/health
```

## Développement

### Ajout de Nouvelles Fonctionnalités

1. **Models** : Ajouter structures dans `src/models/`
2. **Services** : Logique métier dans `src/services/`
3. **Handlers** : Endpoints dans `src/handlers/api.rs`
4. **Routing** : Déclarer routes dans `src/server.rs`

### Architecture Audio

Le système audio est conçu pour être extensible :

- **AudioPacket** : Format standardisé pour tous les types audio
- **AudioRouter** : Routage intelligent avec métriques
- **AudioBuffer** : Gestion optimisée de la latence
- **AudioMixer** : Prêt pour mixage multi-sources (à développer)

## Prochaines Étapes

### Phase 2.2 - Routage Audio Avancé
- [ ] Implémentation complète du mixage audio
- [ ] Gestion de la qualité adaptive (bitrate)
- [ ] Optimisations zero-copy
- [ ] Tests de charge et monitoring

### Phase 2.3 - Performance
- [ ] Pool de threads pour traitement audio
- [ ] Optimisations mémoire (allocations)
- [ ] Métriques détaillées (Prometheus/Grafana)
- [ ] Tests de latence réseau

## Tests

```bash
# Tests unitaires
cargo test

# Tests d'intégration spécifiques
cargo test --test integration

# Tests avec logs
RUST_LOG=debug cargo test
```

## Monitoring

Le serveur expose des métriques via les endpoints API :
- Statistiques par channel (packets, latence, utilisateurs)
- Configuration audio globale
- Métriques de performance du routeur

## Architecture Réseau

```
Client ──HTTP──> [Port 3000] Backend REST API
Client ──WS───> [Port 3001] Backend WebSocket (signalisation)
Client ──UDP──> [Port 3002] Backend Audio Server (streaming)
```

Cette architecture sépare clairement :
- **REST** : Opérations CRUD (channels, users)
- **WebSocket** : Signalisation temps réel (join/leave)
- **UDP** : Streaming audio faible latence

## Performance

- **Concurrent** : Architecture async avec Tokio
- **Thread-safe** : DashMap pour stockage partagé
- **Low-latency** : UDP direct pour audio
- **Scalable** : Design préparé pour clustering (Phase 3)