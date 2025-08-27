# API Documentation - Voice Chat Backend

## Table des matières

1. [Vue d'ensemble](#vue-densemble)
2. [Authentification](#authentification)
3. [Endpoints principaux](#endpoints-principaux)
4. [APIs avancées](#apis-avancées)
5. [Métriques et monitoring](#métriques-et-monitoring)
6. [WebSocket](#websocket)
7. [Exemples d'utilisation](#exemples-dutilisation)

## Vue d'ensemble

Le backend Voice Chat expose une API REST complète pour la gestion des utilisateurs, channels, audio streaming et monitoring. L'API est construite avec Axum et supporte WebSocket pour les communications temps réel.

**Base URL**: `http://localhost:3000`

### Formats de réponse

Toutes les réponses sont au format JSON. Les erreurs incluent un code d'erreur et un message descriptif.

```json
{
  "success": true,
  "data": { ... }
}
```

```json
{
  "success": false,
  "error": "Message d'erreur",
  "error_code": "ERROR_CODE",
  "timestamp": 1234567890
}
```

## Authentification

Pour le moment, l'API utilise un système d'authentification basique. Dans une implémentation production, vous devriez utiliser JWT ou OAuth.

## Endpoints principaux

### Users

#### `POST /api/users`
Créer un nouvel utilisateur.

**Body**:
```json
{
  "username": "john_doe",
  "display_name": "John Doe"
}
```

**Réponse**:
```json
{
  "success": true,
  "user": {
    "id": "uuid",
    "username": "john_doe",
    "display_name": "John Doe",
    "is_active": true,
    "created_at": "2024-01-01T00:00:00Z"
  }
}
```

#### `GET /api/users/:id`
Récupérer un utilisateur par ID.

### Channels

#### `GET /api/channels`
Lister tous les channels publics.

#### `POST /api/channels`
Créer un nouveau channel.

**Body**:
```json
{
  "name": "General",
  "description": "Channel général",
  "max_users": 10,
  "is_private": false
}
```

#### `GET /api/channels/:id`
Récupérer les détails d'un channel.

#### `GET /api/channels/:id/audio/stats`
Récupérer les statistiques audio d'un channel.

### Audio

#### `GET /api/audio/config`
Récupérer la configuration audio actuelle.

**Réponse**:
```json
{
  "sample_rate": 48000,
  "channels": 2,
  "bit_depth": 16,
  "buffer_size": 1024,
  "latency_target_ms": 50
}
```

### Health Check

#### `GET /health`
Vérification de santé simple.

**Réponse**: `"OK"`

## APIs avancées

Les APIs avancées sont accessibles sous `/api/advanced/` et offrent des fonctionnalités étendues.

### Utilisateurs avancés

#### `GET /api/advanced/users`
Liste paginée des utilisateurs avec filtrage.

**Query Parameters**:
- `page`: Numéro de page (défaut: 1)
- `limit`: Éléments par page (défaut: 20, max: 100)
- `sort_by`: Champ de tri
- `order`: Ordre (asc/desc)

**Réponse**:
```json
{
  "data": [...],
  "pagination": {
    "current_page": 1,
    "total_pages": 5,
    "total_items": 100,
    "items_per_page": 20,
    "has_next": true,
    "has_previous": false
  }
}
```

#### `PUT /api/advanced/users/:id`
Mettre à jour un utilisateur.

#### `DELETE /api/advanced/users/:id`
Supprimer un utilisateur.

#### `GET /api/advanced/users/:id/statistics`
Statistiques détaillées d'un utilisateur.

### Channels avancés

#### `GET /api/advanced/channels`
Liste filtrée des channels.

**Query Parameters**:
- `name`: Filtrer par nom
- `channel_type`: Filtrer par type
- `min_users`: Nombre minimum d'utilisateurs
- `max_users`: Nombre maximum d'utilisateurs
- `include_private`: Inclure les channels privés

#### `GET /api/advanced/channels/:id/statistics`
Statistiques détaillées d'un channel.

**Réponse**:
```json
{
  "channel_id": "uuid",
  "current_users": 5,
  "total_connections": 100,
  "uptime_seconds": 1800,
  "audio_stats": {
    "packets_sent": 10000,
    "packets_received": 9950,
    "bytes_transferred": 5000000,
    "average_latency_ms": 45.0,
    "packet_loss_rate": 0.5,
    "audio_quality_score": 0.95,
    "jitter_ms": 2.0
  },
  "performance_stats": {
    "cpu_usage_percent": 5.0,
    "memory_usage_mb": 50,
    "active_streams": 5,
    "processing_latency_us": 500
  }
}
```

### Configuration audio avancée

#### `PUT /api/advanced/audio/config`
Mettre à jour la configuration audio.

#### `POST /api/advanced/audio/config/reset`
Remettre la configuration audio par défaut.

### Statistiques globales

#### `GET /api/advanced/statistics/server`
Statistiques globales du serveur.

**Réponse**:
```json
{
  "total_users": 25,
  "active_users": 15,
  "total_channels": 5,
  "active_channels": 3,
  "uptime_seconds": 3600,
  "total_audio_packets": 1000000,
  "total_data_transferred_mb": 500.0,
  "system_stats": {
    "cpu_usage_percent": 25.0,
    "memory_usage_mb": 512,
    "network_throughput_mbps": 10.0,
    "active_connections": 15
  }
}
```

### Administration

#### `GET /api/advanced/admin/health-check`
Vérification de santé complète.

#### `POST /api/advanced/admin/cleanup`
Nettoyer les ressources expirées.

#### `POST /api/advanced/admin/reset`
Réinitialiser l'état du serveur.

## Métriques et monitoring

Les métriques sont accessibles sous `/api/metrics/`.

### Métriques actuelles

#### `GET /api/metrics/current`
Métriques en temps réel.

**Réponse**:
```json
{
  "success": true,
  "data": {
    "timestamp": 1234567890,
    "audio_metrics": {
      "active_channels": 3,
      "active_users": 15,
      "average_latency_ms": 45.0,
      "packet_loss_percentage": 0.5,
      "audio_quality_score": 0.95
    },
    "performance_metrics": {
      "cpu_usage_percent": 25.0,
      "memory_usage_mb": 512,
      "thread_pool_utilization": 60.0
    },
    "network_metrics": {
      "packets_received_per_second": 1000,
      "packets_sent_per_second": 950,
      "connection_count": 15
    },
    "system_health": {
      "overall_health_score": 0.95,
      "uptime_seconds": 3600
    }
  }
}
```

### Historique des métriques

#### `GET /api/metrics/history`
Historique des métriques.

**Query Parameters**:
- `limit`: Nombre de points (défaut: 100)
- `period`: Période en secondes (défaut: 3600)

### Rapport de santé

#### `GET /api/metrics/health`
Rapport de santé complet.

### Alertes

#### `GET /api/metrics/alerts`
Alertes actives.

**Query Parameters**:
- `severity`: Niveau minimum (info, warning, error, critical)
- `component`: Composant spécifique
- `limit`: Nombre maximum d'alertes

### Résumé des métriques

#### `GET /api/metrics/summary`
Résumé simplifié des métriques principales.

### Métriques par composant

#### `GET /api/metrics/component/:component`
Métriques pour un composant spécifique (audio, performance, network).

## WebSocket

### Connexion

**URL**: `ws://localhost:3000/ws`

### Messages clients

#### Rejoindre un channel
```json
{
  "type": "JoinChannel",
  "channel_id": "uuid",
  "user_id": "uuid"
}
```

#### Quitter un channel
```json
{
  "type": "LeaveChannel",
  "channel_id": "uuid"
}
```

#### Message audio
```json
{
  "type": "AudioData",
  "channel_id": "uuid",
  "data": "base64_encoded_audio"
}
```

### Messages serveur

#### Utilisateur rejoint
```json
{
  "type": "UserJoined",
  "user_id": "uuid",
  "channel_id": "uuid"
}
```

#### Utilisateur quitte
```json
{
  "type": "UserLeft",
  "user_id": "uuid",
  "channel_id": "uuid"
}
```

#### Données audio
```json
{
  "type": "AudioData",
  "user_id": "uuid",
  "channel_id": "uuid",
  "data": "base64_encoded_audio"
}
```

## Exemples d'utilisation

### Créer un utilisateur et rejoindre un channel

1. **Créer un utilisateur**:
```bash
curl -X POST http://localhost:3000/api/users \
  -H "Content-Type: application/json" \
  -d '{"username": "alice", "display_name": "Alice"}'
```

2. **Créer un channel**:
```bash
curl -X POST http://localhost:3000/api/channels \
  -H "Content-Type: application/json" \
  -d '{"name": "General", "description": "Channel général"}'
```

3. **Se connecter via WebSocket** et envoyer un message JoinChannel.

### Surveiller les métriques

1. **Vérifier la santé**:
```bash
curl http://localhost:3000/api/metrics/health
```

2. **Obtenir les métriques actuelles**:
```bash
curl http://localhost:3000/api/metrics/current
```

3. **Obtenir les alertes**:
```bash
curl http://localhost:3000/api/metrics/alerts?severity=warning
```

### Administration

1. **Statistiques globales**:
```bash
curl http://localhost:3000/api/advanced/statistics/server
```

2. **Nettoyer les ressources**:
```bash
curl -X POST http://localhost:3000/api/advanced/admin/cleanup
```

## Codes d'erreur

- `USER_NOT_FOUND`: Utilisateur introuvable
- `CHANNEL_NOT_FOUND`: Channel introuvable
- `INVALID_INPUT`: Données d'entrée invalides
- `CHANNEL_FULL`: Channel plein
- `PERMISSION_DENIED`: Permission refusée
- `TIMEOUT`: Timeout de la requête
- `INTERNAL_ERROR`: Erreur interne du serveur

## Limites et quotas

- **Utilisateurs par channel**: Configurable (défaut: 10)
- **Channels par serveur**: Pas de limite fixe
- **Taille des messages WebSocket**: 64KB max
- **Rate limiting**: À implémenter selon les besoins

## Notes de performance

- Les métriques sont collectées toutes les 5 secondes
- L'historique des métriques est conservé 24h
- Les connexions WebSocket sont automatiquement nettoyées après déconnexion
- Le serveur UDP audio utilise un pool de threads pour optimiser les performances

## Support

Pour plus d'informations, consultez le code source et les tests dans le repository.