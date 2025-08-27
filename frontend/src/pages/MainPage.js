import domUtils, { createElement, $, addListener } from '../utils/dom.js';
import tauriAPI from '../utils/tauri.js';

/**
 * Main App Page Component
 */
export default class MainPage {
  constructor(app) {
    this.app = app;
    this.currentChannel = null;
    this.unsubscribeState = null;
  }

  /**
   * Show the main page
   */
  async show(params = {}) {
    console.log('Showing main page');
    
    const container = document.getElementById('page-container');
    container.innerHTML = this.render();
    
    // Hide loading screen and show page
    document.getElementById('loading-screen').style.display = 'none';
    container.style.display = 'block';
    document.body.classList.add('app-ready');
    
    this.bindEvents();
    this.setupStateListeners();
    this.loadInitialData();
  }

  /**
   * Hide the main page
   */
  async hide() {
    console.log('Hiding main page');
    this.unbindEvents();
    this.cleanupStateListeners();
    
    const container = document.getElementById('page-container');
    container.style.display = 'none';
  }

  render() {
    return `
      <div class="main-page">
        <div class="app-layout">
          <!-- Sidebar -->
          <aside class="sidebar">
            <div class="sidebar-header">
              <div class="server-info">
                <h2 class="server-name">Voice Chat Server</h2>
                <div class="user-info" id="userInfo"></div>
              </div>
            </div>
            
            <div class="sidebar-content">
              <div class="channels-section">
                <div class="section-header">
                  <h3 class="section-title">Channels</h3>
                  <button class="btn btn-sm btn-secondary" id="refreshChannelsBtn" title="Refresh channels">↻</button>
                </div>
                <div class="channels-list" id="channelsList"></div>
              </div>
            </div>
            
            <!-- Audio controls at bottom -->
            <div class="audio-controls" id="audioControls"></div>
          </aside>
          
          <!-- Main content -->
          <main class="main-content">
            <div class="channel-view" id="channelView">
              <!-- Channel content will be inserted here -->
            </div>
          </main>
        </div>
      </div>
    `;
  }

  bindEvents() {
    const refreshBtn = $('#refreshChannelsBtn');
    
    if (refreshBtn) {
      addListener(refreshBtn, 'click', () => {
        this.refreshChannels();
      });
    }
  }

  unbindEvents() {
    // Remove event listeners if needed
    // DOM elements will be removed anyway
  }

  cleanupStateListeners() {
    if (this.unsubscribeState) {
      this.unsubscribeState();
      this.unsubscribeState = null;
    }
  }

  async loadInitialData() {
    try {
      // Load initial data from app state
      const appState = this.app.getState();
      
      this.renderUserInfo(appState.user);
      this.renderAudioControls();
      this.updateChannels(appState.channels);
      
      if (appState.currentChannel) {
        this.updateChannel(appState.currentChannel);
      } else {
        this.renderEmptyState();
      }
      
    } catch (error) {
      console.error('Failed to load initial data:', error);
    }
  }

  renderEmptyState() {
    const channelView = $('#channelView');
    if (!channelView) return;
    
    channelView.innerHTML = `
      <div class="empty-state">
        <div class="empty-icon">💬</div>
        <h3>Select a channel to start</h3>
        <p class="text-muted">Choose a voice channel from the sidebar to join the conversation</p>
      </div>
    `;
  }

  renderUserInfo(user) {
    const userInfoDiv = $('#userInfo');
    if (!userInfoDiv) return;
    
    if (user) {
      userInfoDiv.innerHTML = `
        <div class="user-card">
          <div class="user-avatar">${user.username.charAt(0).toUpperCase()}</div>
          <div class="user-details">
            <div class="username">${user.username}</div>
            <div class="user-status text-muted">Online</div>
          </div>
          <button class="btn btn-sm btn-secondary" id="logoutBtn" title="Disconnect">⚙️</button>
        </div>
      `;
      
      // Add logout event
      const logoutBtn = $('#logoutBtn');
      if (logoutBtn) {
        addListener(logoutBtn, 'click', () => {
          const logoutEvent = new CustomEvent('app:logout');
          document.dispatchEvent(logoutEvent);
        });
      }
    } else {
      userInfoDiv.innerHTML = '<p class="text-muted">Not connected</p>';
    }
  }

  renderAudioControls() {
    const audioControlsDiv = $('#audioControls');
    if (!audioControlsDiv) return;
    
    const appState = this.app.getState();
    const audioSettings = appState.audioSettings;
    
    audioControlsDiv.innerHTML = `
      <div class="audio-controls-grid">
        <button class="btn btn-icon audio-control ${audioSettings.micEnabled ? 'active' : 'muted'}" 
                id="micToggle" title="Toggle Microphone">
          🎤
        </button>
        <button class="btn btn-icon audio-control ${audioSettings.speakerEnabled ? 'active' : 'muted'}" 
                id="speakerToggle" title="Toggle Speaker">
          🔊
        </button>
        <button class="btn btn-icon audio-control" id="settingsToggle" title="Audio Settings">
          ⚙️
        </button>
      </div>
    `;
    
    // Bind audio control events
    const micToggle = $('#micToggle');
    const speakerToggle = $('#speakerToggle');
    const settingsToggle = $('#settingsToggle');
    
    if (micToggle) {
      addListener(micToggle, 'click', () => {
        this.toggleMicrophone();
      });
    }
    
    if (speakerToggle) {
      addListener(speakerToggle, 'click', () => {
        this.toggleSpeaker();
      });
    }
    
    if (settingsToggle) {
      addListener(settingsToggle, 'click', () => {
        const audioSettingsEvent = new CustomEvent('app:audio-settings');
        document.dispatchEvent(audioSettingsEvent);
      });
    }
  }

  updateChannels(channels) {
    const channelsList = $('#channelsList');
    if (!channelsList) return;
    
    if (!channels || channels.length === 0) {
      channelsList.innerHTML = `
        <div class="empty-channels">
          <p class="text-muted">No channels available</p>
        </div>
      `;
      return;
    }
    
    channelsList.innerHTML = channels.map(channel => `
      <div class="channel-item ${this.currentChannel?.id === channel.id ? 'active' : ''}" 
           data-channel-id="${channel.id}">
        <div class="channel-icon">🔊</div>
        <div class="channel-info">
          <div class="channel-name">${channel.name}</div>
          <div class="channel-users text-muted">${channel.userCount || 0} users</div>
        </div>
      </div>
    `).join('');
    
    // Bind channel click events
    const channelItems = channelsList.querySelectorAll('.channel-item');
    channelItems.forEach(item => {
      addListener(item, 'click', () => {
        const channelId = item.dataset.channelId;
        const channel = channels.find(c => c.id === channelId);
        if (channel) {
          this.handleJoinChannel(channel);
        }
      });
    });
  }

  updateChannel(channel) {
    this.currentChannel = channel;
    
    const channelView = $('#channelView');
    if (!channelView) return;
    
    if (!channel) {
      this.renderEmptyState();
      return;
    }
    
    channelView.innerHTML = `
      <div class="channel-content">
        <div class="channel-header">
          <div>
            <h2 class="channel-title"># ${channel.name}</h2>
            <div class="channel-meta text-muted">${channel.userCount || 0} users connected</div>
          </div>
          <button class="btn btn-danger" id="leaveChannelBtn">Leave Channel</button>
        </div>
        
        <div class="channel-users">
          <h3 class="users-title">Users in Channel</h3>
          <div class="users-list" id="usersList">
            <!-- Users will be populated here -->
          </div>
        </div>
      </div>
    `;
    
    // Bind leave channel event
    const leaveBtn = $('#leaveChannelBtn');
    if (leaveBtn) {
      addListener(leaveBtn, 'click', () => {
        const leaveEvent = new CustomEvent('app:leave-channel');
        document.dispatchEvent(leaveEvent);
      });
    }
    
    // Update users list
    this.updateUsersList(channel.users || []);
    
    // Update channels list to show active state
    this.updateChannels(this.app.getState().channels);
  }

  updateUsersList(users) {
    const usersList = $('#usersList');
    if (!usersList) return;
    
    if (!users || users.length === 0) {
      usersList.innerHTML = '<p class="text-muted">No users in this channel</p>';
      return;
    }
    
    const currentUser = this.app.getState().user;
    
    usersList.innerHTML = users.map(user => `
      <div class="user-item ${user.id === currentUser?.id ? 'current-user' : ''}">
        <div class="user-avatar">${user.username.charAt(0).toUpperCase()}</div>
        <div class="user-info">
          <div class="user-name">${user.username}${user.id === currentUser?.id ? ' (You)' : ''}</div>
          <div class="user-status text-muted">${user.isSpeaking ? 'Speaking...' : 'Connected'}</div>
        </div>
        <div class="user-actions">
          ${user.micEnabled ? '🎤' : '🔇'}
          ${user.speakerEnabled ? '🔊' : '🔇'}
        </div>
      </div>
    `).join('');
  }

  handleJoinChannel(channel) {
    const joinEvent = new CustomEvent('app:join-channel', {
      detail: channel
    });
    document.dispatchEvent(joinEvent);
  }

  refreshChannels() {
    // Request channels refresh from app
    console.log('Refreshing channels...');
    // The app will handle this and update the channels via updateChannels()
  }

  async toggleMicrophone() {
    const currentState = this.app.getState().audioSettings;
    const newMicState = !currentState.micEnabled;
    
    console.log(`🎤 Toggling microphone: ${currentState.micEnabled} -> ${newMicState}`);
    
    try {
      if (newMicState) {
        // Démarrer la capture audio
        console.log('🎤 Starting audio capture...');
        const result = await window.tauriAPI.startAudioCapture();
        if (result.success) {
          console.log('✅ Audio capture started successfully');
          this.app.updateState({
            audioSettings: {
              ...currentState,
              micEnabled: true
            }
          });
        } else {
          console.error('❌ Failed to start audio capture:', result.error);
          alert('Failed to start microphone: ' + (result.error || 'Unknown error'));
          return;
        }
      } else {
        // Arrêter la capture audio
        console.log('🎤 Stopping audio capture...');
        const result = await window.tauriAPI.stopAudioCapture();
        if (result.success) {
          console.log('✅ Audio capture stopped successfully');
          this.app.updateState({
            audioSettings: {
              ...currentState,
              micEnabled: false
            }
          });
        } else {
          console.error('❌ Failed to stop audio capture:', result.error);
          // Continuer quand même avec le changement d'état
          this.app.updateState({
            audioSettings: {
              ...currentState,
              micEnabled: false
            }
          });
        }
      }
      
      this.renderAudioControls();
    } catch (error) {
      console.error('❌ Error toggling microphone:', error);
      alert('Error with microphone: ' + error.message);
    }
  }

  toggleSpeaker() {
    const currentState = this.app.getState().audioSettings;
    this.app.updateState({
      audioSettings: {
        ...currentState,
        speakerEnabled: !currentState.speakerEnabled
      }
    });
    this.renderAudioControls();
  }

  setupStateListeners() {
    // Listen for state changes and update UI accordingly
    // This would be implemented based on the state management system
  }

  updateConnectionStatus(status) {
    // Update UI based on connection status
    const statusIndicator = $('.connection-status');
    if (statusIndicator) {
      statusIndicator.textContent = status;
      statusIndicator.className = `connection-status status-${status}`;
    }
  }

  updateAudioLevels(levelData) {
    // Update audio level indicators
    // This would show speaking indicators for users
  }

  addUser(userData) {
    if (this.currentChannel && userData.channelId === this.currentChannel.id) {
      // Add user to current channel
      const updatedUsers = [...(this.currentChannel.users || []), userData];
      this.updateUsersList(updatedUsers);
    }
  }

  removeUser(userId) {
    if (this.currentChannel) {
      // Remove user from current channel
      const updatedUsers = (this.currentChannel.users || []).filter(u => u.id !== userId);
      this.updateUsersList(updatedUsers);
    }
  }

  async showAudioSettings() {
    // Show audio settings modal
    // This could be delegated to the app or handled here
    const audioSettingsEvent = new CustomEvent('app:audio-settings');
    document.dispatchEvent(audioSettingsEvent);
  }

  /**
   * Refresh channel data from API (called when WebSocket events are received)
   */
  async refreshChannelData() {
    try {
      console.log('🔄 MainPage: Refreshing channel data...');
      
      if (!this.currentChannel) {
        console.log('ℹ️ No current channel to refresh');
        return;
      }

      // Demander à l'app de rafraîchir la liste des channels
      // Cela va mettre à jour les données du channel et les utilisateurs
      const refreshEvent = new CustomEvent('app:refresh-channels');
      document.dispatchEvent(refreshEvent);
      
      // Attendre un peu pour que les données soient rafraîchies
      await new Promise(resolve => setTimeout(resolve, 100));
      
      // Récupérer les données mises à jour du channel courant
      const appState = this.app.getState();
      const updatedChannel = appState.channels.find(ch => ch.id === this.currentChannel.id);
      
      if (updatedChannel) {
        console.log('🔄 [DEBUG] Updated channel data:', updatedChannel);
        console.log('🔄 [DEBUG] Updated users list:', updatedChannel.users);
        
        // Mettre à jour l'affichage avec les nouvelles données
        this.updateChannel(updatedChannel);
        console.log('✅ Channel display updated with fresh data');
      } else {
        console.log('⚠️ Could not find updated channel data');
      }
      
      console.log('✅ Channel data refresh completed');
    } catch (error) {
      console.error('❌ Failed to refresh channel data:', error);
    }
  }
}