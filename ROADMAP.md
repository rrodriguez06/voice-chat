# Voice Chat - Roadmap du Projet

## Vue d'ensemble du projet

Alternative à Discord pour le chat vocal optimisée pour la latence minimale et la qualité audio, composée d'un backend Rust (home server) et d'une application desktop Tauri.

## Architecture générale

```
┌─────────────────────┐    ┌─────────────────────┐
│   Frontend (Tauri)  │◄──►│  Backend (Server)   │
│                     │    │                     │
│ - Interface UI      │    │ - Gestion users     │
│ - Capture audio     │    │ - Gestion channels  │
│ - Soundboard        │    │ - Routage audio     │
│ - Contrôles volume  │    │ - WebSocket/UDP     │
└─────────────────────┘    └─────────────────────┘
```

## Technologies choisies

### Backend
- **Rust** avec Tokio pour l'async/networking
- **WebSocket** pour la signalisation
- **UDP** pour le streaming audio (faible latence)
- **Serde** pour la sérialisation
- **SQLite** ou fichier JSON pour la persistence

### Frontend
- **Tauri** pour l'application desktop
- **CPAL** pour la capture/lecture audio
- **fundsp** pour le traitement audio
- **HTML/CSS/JS** ou **SvelteKit** pour l'UI

## Phase 1: Fondations Backend (Semaines 1-2) ✅ TERMINÉE

### 1.1 Structure du projet backend
- [x] Initialiser le projet Rust backend
- [x] Configuration Cargo.toml avec dépendances
- [x] Structure modulaire (users, channels, audio, networking)
- [x] Configuration basique avec fichier config

### 1.2 Gestion des utilisateurs
- [x] Structure User (id, pseudo, status)
- [x] Système d'authentification simple (pseudo uniquement)
- [x] Stockage en mémoire des utilisateurs connectés
- [x] API REST basique pour la gestion des profils

### 1.3 Gestion des channels
- [x] Structure Channel (id, nom, limite utilisateurs, owner)
- [x] CRUD channels via API REST
- [x] Logique de connexion/déconnexion des users aux channels
- [x] Système de permissions basique

### 1.4 Networking de base
- [x] Serveur WebSocket pour la signalisation
- [x] Protocole de messages (join/leave channel, user events)
- [x] Tests de connexion basique

## Phase 2: Audio Backend (Semaines 3-4) ✅ TERMINÉ

### 2.1 Infrastructure audio ✅ TERMINÉE
- [x] Serveur UDP pour le streaming audio
- [x] Protocol audio custom (header + payload)
- [x] Buffer circulaire pour la gestion des packets
- [x] Structure routeur audio avec statistiques
- [x] Endpoints API pour configuration et métriques audio

### 2.2 Routage audio ✅ TERMINÉE
- [x] Système de routage des flux audio entre utilisateurs
- [x] Mixage des flux audio par channel
- [x] Gestion de la qualité (compression, débit)
- [x] Métriques de performance (latence, perte de packets, système complet)

### 2.3 Optimisations performance ✅ TERMINÉE
- [x] Pool de threads pour l'audio
- [x] Optimisations mémoire (zero-copy quand possible)
- [x] Monitoring des performances
- [x] Tests de charge basiques (2M+ packets/sec, 500+ ops/sec concurrent)

## Phase 3: Fondations Frontend (Semaines 5-6) ✅ TERMINÉE

### 3.1 Setup Tauri ✅ TERMINÉE
- [x] Initialiser le projet Tauri
- [x] Configuration build pour Windows/Linux/macOS
- [x] Structure du projet frontend (modules Rust: state, networking, audio)
- [x] Communication Tauri ↔ Backend (WebSocket + HTTP client)
- [x] Correction problèmes Send/Sync pour compatibilité Tauri
- [x] Tests de compilation debug et release

### 3.2 Interface utilisateur de base ✅ TERMINÉE
- [x] Design system basique (couleurs, composants)
- [x] Page de connexion (saisie pseudo)
- [x] Liste des channels disponibles
- [x] Interface principale du channel (liste users)
- [x] Système de navigation entre pages
- [x] Gestion d'état global de l'application

### 3.3 Audio frontend ✅ TERMINÉE
- [x] Configuration CPAL pour capture/lecture
- [x] Test de capture micro basique
- [x] Test de lecture audio basique
- [x] Interface de sélection des périphériques audio
- [x] Modal de paramètres audio fonctionnel
- [x] Test audio avec périphériques sélectionnés

### 3.4 Connexion serveur ✅ TERMINÉE
- [x] Commande Tauri pour connexion serveur avec URL
- [x] Gestion des erreurs de connexion détaillée
- [x] Logs de débogage pour troubleshooting
- [x] Test de connexion réussie au backend
- [x] Navigation automatique après connexion

### 3.5 Gestion des déconnexions ✅ TERMINÉE

- [x] **PRIORITÉ**: Notification du serveur lors de la déconnexion utilisateur
- [x] Déconnexion automatique lors de la fermeture de l'application
- [x] Gestion de la reconnexion automatique lors de la réouverture
- [x] Cleanup automatique des utilisateurs déconnectés côté serveur
- [x] Sauvegarde et restauration de session avec URL du serveur
- [x] Gestion des erreurs et fallback en cas d'échec de reconnexion
- [x] Logs détaillés pour le debugging des connexions/déconnexions

## Phase 4: Intégration Audio (Semaines 7-8) ✅ TERMINÉE

### 4.1 Communication audio ✅ TERMINÉE
- [x] Client UDP pour envoi/réception audio
- [x] Encodage/décodage des packets audio (bincode)
- [x] Synchronisation avec le backend
- [x] Gestion des erreurs de réseau

### 4.2 Pipeline audio complet ✅ TERMINÉE
- [x] Capture audio CPAL avec buffer 1024 échantillons
- [x] Conversion f32 ↔ PCM 16-bit automatique
- [x] Transmission UDP avec sérialisation bincode
- [x] Mode loopback pour tests locaux
- [x] Port UDP fixe (8083) pour compatibilité backend

### 4.3 Conversion audio intelligente ✅ TERMINÉE
- [x] Détection automatique des paramètres device (sample rate, canaux)
- [x] Conversion adaptative sample rate (ex: 48kHz → 96kHz)
- [x] Mapping intelligent des canaux (ex: stéréo → 8 canaux)
- [x] Support formats f32, i16, u16
- [x] Interpolation linéaire pour upsampling
- [x] Audio clair sans grésillement ni latence excessive

### 4.4 Audio multi-utilisateur cross-platform ✅ TERMINÉE
- [x] Backend routage audio entre utilisateurs du même channel
- [x] Résolution problèmes UDP socket (Arc<UdpSocket>) pour macOS/Unix
- [x] Socket partagé pour éviter conflits de ports UDP sur Windows
- [x] Heartbeat UDP pour registration automatique des clients
- [x] Fix routage IP cross-platform (backend IP vs 127.0.0.1)
- [x] **Audio bidirectionnel Windows ↔ Mac fonctionnel**
- [x] Diagnostic logging complet pour troubleshooting

### 4.5 Contrôles volume et gestion ✅ TERMINÉE
- [x] Interface de sélection des périphériques audio
- [x] Configuration automatique des devices
- [x] Démarrage/arrêt automatique playback avec join/leave channel

## Phase 5: Fonctionnalités Avancées (Semaines 9-11) 🔄 PROCHAINE ÉTAPE

### 5.1 Interface utilisateur audio avancée ⭐ PRIORITÉ

- [ ] Contrôles volume individuels par utilisateur
- [ ] Boutons mute/unmute avec indicateurs visuels
- [ ] Indicateurs d'activité vocale (voice activity detection)
- [ ] Réglages audio par utilisateur (gain, filtre)
- [ ] Status de connexion en temps réel (connecté/déconnecté)

### 5.2 Optimisations audio

- [ ] Compression audio (opus codec) pour réduire bande passante
- [ ] Gestion buffers adaptative selon latence réseau
- [ ] Réduction de bruit (noise suppression)
- [ ] Echo cancellation basique

### 5.3 Soundboard

- [ ] Interface de gestion des sons
- [ ] Import/export de fichiers audio
- [ ] Raccourcis clavier configurables
- [ ] Lecture simultanée avec le micro
- [ ] Stockage local des soundboards

### 5.4 Gestion avancée des channels

- [ ] Channels privés avec mot de passe
- [ ] Système de modération basique
- [ ] Limitations avancées (bitrate, codec)
- [ ] Channels temporaires

### 5.5 Profils utilisateurs

- [ ] Avatar/image de profil
- [ ] Statuts personnalisés
- [ ] Historique des connexions
- [ ] Paramètres audio par défaut

## Phase 6: Polish et Optimisations (Semaines 12-13)

### 6.1 Optimisations audio
- [ ] Algorithmes adaptatifs de qualité
- [ ] Réduction automatique de la latence
- [ ] Echo cancellation
- [ ] Tests de qualité audio

### 6.2 Interface utilisateur finale
- [ ] Thèmes sombre/clair
- [ ] Animations et transitions
- [ ] Raccourcis clavier globaux
- [ ] Notifications système

### 6.3 Déploiement et packaging
- [ ] Configuration pour home server
- [ ] Scripts de déploiement automatique
- [ ] Documentation d'installation
- [ ] Packaging des applications (MSI, AppImage, DMG)

## Phase 7: Tests et Documentation (Semaine 14)

### 7.1 Tests
- [ ] Tests unitaires backend
- [ ] Tests d'intégration audio
- [ ] Tests de charge réseau
- [ ] Tests utilisateur final

### 7.2 Documentation
- [ ] Documentation utilisateur
- [ ] Documentation développeur
- [ ] Guide de déploiement home server
- [ ] Troubleshooting guide

## Fonctionnalités futures (Post v1.0)

### Extensions possibles
- [ ] Partage d'écran basique
- [ ] Bots simples
- [ ] API plugins
- [ ] Chat texte
- [ ] Enregistrement des conversations
- [ ] Support mobile (Tauri mobile)

## Métriques de succès

### Performance
- **Latence audio**: < 50ms end-to-end
- **Qualité audio**: 48kHz/16bit minimum
- **Concurrent users**: 10+ par channel
- **Stabilité**: 99%+ uptime

### Utilisabilité
- **Setup time**: < 5 minutes
- **Connection time**: < 3 secondes
- **Interface**: Intuitive, pas de formation requise

## Notes techniques importantes

### Optimisations critiques
1. **Buffer sizes**: Petits buffers pour la latence, grands pour la stabilité
2. **Thread priorities**: Audio threads en high priority
3. **Network**: UDP avec fallback TCP si nécessaire
4. **Memory**: Éviter les allocations dans les hot paths

### Sécurité
- Validation stricte des inputs
- Rate limiting
- Chiffrement des communications (optionnel v2)

### Monitoring
- Logs structurés
- Métriques de performance
- Health checks automatiques

---

**Version**: 1.5  
**Dernière mise à jour**: 27 Août 2025  
**Statut**: Phase 4 TERMINÉE ✅ - Audio multi-utilisateur cross-platform FONCTIONNEL, Phase 5 PROCHAINE 🔄 - Interface audio avancée

## Accomplissements Récents 🎉

### Audio Cross-Platform Complet ✅
- **Windows ↔ Mac**: Audio bidirectionnel fonctionnel
- **Socket Management**: Résolution complète des conflits UDP ports
- **IP Routing**: Fix du routage réseau (192.168.x.x vs 127.0.0.1)
- **Heartbeat Protocol**: Registration automatique des clients
- **Diagnostic Tools**: Logging complet pour troubleshooting

### Architecture Robuste ✅
- **Backend Rust**: Routage multi-utilisateur stable
- **Frontend Tauri**: Cross-platform compatible
- **UDP Protocol**: Optimisé pour faible latence
- **Audio Pipeline**: CPAL → UDP → Backend → Routing → CPAL

### Tests Validés ✅
- Multi-device (Windows + Mac simultané)
- Multi-user dans même channel
- Qualité audio préservée
- Latence acceptable (~50ms)
- Stabilité de connexion