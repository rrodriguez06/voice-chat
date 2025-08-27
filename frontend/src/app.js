/**
 * Application Router - Navigation and State Management
 */

// Import Tauri functions
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

class VoiceChatApp {
  constructor() {
    this.currentPage = null;
    this.appState = {
      user: null,
      currentChannel: null,
      channels: [],
      audioSettings: {
        inputDevice: null,
        outputDevice: null,
        inputLevel: 0.8,
        async handleChannelUpdated(eventData) {
    console.log('🔄 Channel updated event received:', eventData);
    
    // Toujours rafraîchir la liste des channels pour mettre à jour les comptes d'utilisateurs
    console.log('📡 Refreshing channels list to update user counts...');
    await this.refreshChannelsList();
    
    // Si on est dans le channel qui a été mis à jour, on recharge aussi ses informations
    if (this.appState.currentChannel && eventData.channelId === this.appState.currentChannel.id) {
      console.log('📡 Also refreshing current channel data...');
      
      const mainPage = this.pages.get('main');
      if (mainPage && mainPage.refreshChannelData) {
        await mainPage.refreshChannelData();
        console.log('✅ Current channel data refreshed successfully');
      } else {
        console.log('ℹ️ Main page not available or no refresh method');
      }
    } else {
      console.log('ℹ️ Event not for current channel, but channels list updated');
    }
  },
        micEnabled: true,
        speakerEnabled: true
      },
      connectionStatus: 'disconnected' // 'connecting', 'connected', 'disconnected', 'error'
    };
    
    this.pages = new Map();
    this.eventListeners = new Map();
    
    this.initialize();
  }

  /**
   * Initialize the application
   */
  async initialize() {
    try {
      console.log('Initializing Voice Chat App...');
      
      // Initialize Tauri app first
      console.log('Initializing Tauri app...');
      await invoke('initialize_app');
      console.log('Tauri app initialized successfully');
      
      // Import page components
      console.log('Loading components...');
      await this.loadComponents();
      console.log('Components loaded successfully');
      
      // Setup event listeners
      console.log('Setting up event listeners...');
      this.setupEventListeners();
      console.log('Event listeners setup complete');
      
      // Check if user is already logged in
      console.log('Checking for saved user session...');
      const savedUser = await this.loadUserSession();
      
      if (savedUser) {
        console.log('Found saved user session, reconnecting to backend...');
        await this.reconnectFromSession(savedUser);
      } else {
        console.log('No saved user session, navigating to login page');
        this.navigateTo('login');
      }
      
      console.log('App initialized successfully');
    } catch (error) {
      console.error('Failed to initialize app:', error);
      this.showInitializationError(error);
    }
  }

  /**
   * Load page components
   */
  async loadComponents() {
    try {
      console.log('Starting component loading...');
      
      // Import login page
      console.log('Importing LoginPage...');
      const LoginPage = (await import('./pages/LoginPage.js')).default;
      console.log('LoginPage imported successfully');
      
      console.log('Creating LoginPage instance...');
      this.pages.set('login', new LoginPage(this));
      console.log('LoginPage instance created');

      // Import main page
      console.log('Importing MainPage...');
      const MainPage = (await import('./pages/MainPage.js')).default;
      console.log('MainPage imported successfully');
      
      console.log('Creating MainPage instance...');
      this.pages.set('main', new MainPage(this));
      console.log('MainPage instance created');

      console.log('All components loaded successfully');
    } catch (error) {
      console.error('Failed to load components:', error);
      throw error;
    }
  }

  /**
   * Setup global event listeners
   */
  setupEventListeners() {
    // Listen for app-wide events
    document.addEventListener('app:login', (e) => {
      this.handleLogin(e.detail);
    });

    document.addEventListener('app:logout', () => {
      this.handleLogout();
    });

    document.addEventListener('app:join-channel', (e) => {
      this.handleJoinChannel(e.detail);
    });

    document.addEventListener('app:leave-channel', () => {
      this.handleLeaveChannel();
    });

    document.addEventListener('app:audio-settings', () => {
      this.showAudioSettings();
    });

    document.addEventListener('app:refresh-channels', () => {
      this.handleRefreshChannels();
    });

    // Listen for Tauri events
    this.setupTauriEventListeners();

    // Handle window events
    window.addEventListener('beforeunload', () => {
      this.cleanup();
    });
  }

  /**
   * Setup Tauri-specific event listeners
   */
  async setupTauriEventListeners() {
    try {
      // Utiliser l'API Tauri globale directement
      if (typeof window !== 'undefined' && window.__TAURI__) {
        console.log('🔧 Setting up Tauri event listeners...');
        
        // Listen for backend events using the global Tauri API
        await listen('connection-status', (event) => {
          console.log('📡 Event: connection-status', event.payload);
          this.handleConnectionStatusChange(event.payload);
        });

        await listen('channel-update', (event) => {
          console.log('📡 Event: channel-update', event.payload);
          this.handleChannelUpdate(event.payload);
        });

        await listen('user-joined', (event) => {
          console.log('📡 Event: user-joined', event.payload);
          this.handleChannelUpdated(event.payload);
        });

        await listen('user-left', (event) => {
          console.log('📡 Event: user-left', event.payload);
          this.handleChannelUpdated(event.payload);
        });

        await listen('channel_users', (event) => {
          console.log('📡 Event: channel_users', event.payload);
          this.handleChannelUsers(event.payload);
        });

        await listen('audio-level', (event) => {
          console.log('📡 Event: audio-level', event.payload);
          this.handleAudioLevel(event.payload);
        });

        console.log('✅ Tauri event listeners setup complete');
      } else {
        console.warn('⚠️ Tauri not available (running in browser mode)');
      }

    } catch (error) {
      console.warn('⚠️ Failed to setup Tauri event listeners:', error);
    }
  }

  /**
   * Navigate to a specific page
   */
  async navigateTo(pageName, params = {}) {
    try {
      console.log(`Navigating to: ${pageName}`);
      
      const page = this.pages.get(pageName);
      if (!page) {
        throw new Error(`Page not found: ${pageName}`);
      }

      // Hide current page
      if (this.currentPage) {
        await this.currentPage.hide();
      }

      // Show new page
      this.currentPage = page;
      await page.show(params);

      // Update document title
      this.updateTitle(pageName);

    } catch (error) {
      console.error(`Navigation failed: ${error.message}`);
      this.showNotification('Navigation failed', 'error');
    }
  }

  /**
   * Update document title
   */
  updateTitle(pageName) {
    const titles = {
      login: 'Voice Chat - Login',
      main: this.appState.currentChannel 
        ? `Voice Chat - ${this.appState.currentChannel.name}`
        : 'Voice Chat'
    };

    document.title = titles[pageName] || 'Voice Chat';
  }

  /**
   * Handle user login
   */
  async handleLogin(loginData) {
    try {
      console.log('🔄 Starting login process...', loginData);
      
      this.setConnectionStatus('connecting');
      this.showNotification('Connecting to server...', 'info');
      
      // Call backend login
      console.log('📡 Calling tauriAPI.connectToServer...');
      const result = await window.tauriAPI.connectToServer(loginData);
      console.log('📡 Server connection result:', result);
      
      if (result.success) {
        console.log('✅ Connection successful, updating app state...');
        this.appState.user = result.user;
        this.appState.channels = result.channels || [];
        
        // Save session avec l'URL du serveur
        const serverUrl = localStorage.getItem('lastServerUrl');
        await this.saveUserSession(result.user, serverUrl);
        
        this.setConnectionStatus('connected');
        this.showNotification('Connected successfully!', 'success');
        
        console.log('🔄 Navigating to main page...');
        // Navigate to main page
        await this.navigateTo('main');
        console.log('✅ Navigation complete');
      } else {
        console.error('❌ Server connection failed:', result.error);
        throw new Error(result.error || 'Login failed');
      }
      
    } catch (error) {
      console.error('❌ Login process failed:', error);
      this.setConnectionStatus('error');
      this.showNotification(`Connection failed: ${error.message}`, 'error');
    }
  }

  /**
   * Handle user logout
   */
  async handleLogout() {
    try {
      console.log('Handling logout...');
      
      // Leave current channel
      if (this.appState.currentChannel) {
        await this.handleLeaveChannel();
      }
      
      // Disconnect from server
      await window.tauriAPI.disconnectFromServer();
      
      // Clear state
      this.appState.user = null;
      this.appState.channels = [];
      this.appState.currentChannel = null;
      this.setConnectionStatus('disconnected');
      
      // Clear saved session
      await this.clearUserSession();
      
      this.showNotification('Disconnected', 'info');
      
      // Navigate to login
      await this.navigateTo('login');
      
    } catch (error) {
      console.error('Logout failed:', error);
      this.showNotification('Logout failed', 'error');
    }
  }

  /**
   * Handle joining a channel
   */
  async handleJoinChannel(channelData) {
    try {
      console.log('🏠 Frontend: Starting join channel process...', channelData);
      
      // Leave current channel first
      if (this.appState.currentChannel) {
        console.log('🚪 Leaving current channel first...');
        await this.handleLeaveChannel();
      }
      
      // Join new channel
      console.log('📡 Calling tauriAPI.joinChannel...', channelData.id);
      const result = await window.tauriAPI.joinChannel(channelData.id);
      console.log('📡 Join channel result:', result);
      
      if (result.success) {
        console.log('✅ Successfully joined channel, refreshing data...');
        this.showNotification(`Joined ${channelData.name}`, 'success');
        
        // Petit délai pour permettre au backend de mettre à jour ses données
        console.log('⏳ Waiting for backend to update...');
        await new Promise(resolve => setTimeout(resolve, 100));
        
        // Refresh channels to get updated user counts
        console.log('🔄 Refreshing channels list to get updated user counts...');
        await this.refreshChannelsList();
        
        // Find the updated channel data
        const updatedChannel = this.appState.channels.find(c => c.id === channelData.id);
        if (updatedChannel) {
          console.log('📋 Using updated channel data:', updatedChannel);
          console.log('👥 Channel user count:', updatedChannel.userCount || 0);
          console.log('👥 Channel users:', updatedChannel.users || []);
          this.appState.currentChannel = updatedChannel;
          
          // Update main page with fresh data
          const mainPage = this.pages.get('main');
          if (mainPage) {
            await mainPage.updateChannel(updatedChannel);
          }
        } else {
          console.warn('⚠️ Could not find updated channel data, using original');
          this.appState.currentChannel = channelData;
          
          const mainPage = this.pages.get('main');
          if (mainPage) {
            await mainPage.updateChannel(channelData);
          }
        }
        
        // Update title
        this.updateTitle('main');
        
        console.log('🎯 Join channel process completed successfully');
      } else {
        throw new Error(result.error || 'Failed to join channel');
      }
      
    } catch (error) {
      console.error('❌ Failed to join channel:', error);
      this.showNotification(error.message, 'error');
    }
  }

  /**
   * Handle leaving a channel
   */
  async handleLeaveChannel() {
    try {
      if (!this.appState.currentChannel) {
        return;
      }
      
      console.log('Leaving channel...', this.appState.currentChannel);
      
      const result = await window.tauriAPI.leaveChannel();
      
      if (result.success) {
        const channelName = this.appState.currentChannel.name;
        this.appState.currentChannel = null;
        this.showNotification(`Left ${channelName}`, 'info');
        
        // Refresh channels list to update user counts
        console.log('🔄 Refreshing channels list after leaving channel...');
        await this.refreshChannelsList();
        
        // Update main page
        const mainPage = this.pages.get('main');
        if (mainPage) {
          await mainPage.updateChannel(null);
        }
        
        // Update title
        this.updateTitle('main');
      }
      
    } catch (error) {
      console.error('Failed to leave channel:', error);
      this.showNotification('Failed to leave channel', 'error');
    }
  }

  /**
   * Handle connection status changes
   */
  handleConnectionStatusChange(status) {
    console.log('Connection status changed:', status);
    this.setConnectionStatus(status.status);
    
    if (status.status === 'disconnected' && this.appState.user) {
      this.showNotification('Connection lost', 'error');
      // Could implement auto-reconnect here
    }
  }

  /**
   * Handle channel updates
   */
  handleChannelUpdate(channelData) {
    console.log('Channel update received:', channelData);
    
    // Update channels list
    const channelIndex = this.appState.channels.findIndex(c => c.id === channelData.id);
    if (channelIndex >= 0) {
      this.appState.channels[channelIndex] = channelData;
    } else {
      this.appState.channels.push(channelData);
    }
    
    // Update current channel if needed
    if (this.appState.currentChannel && this.appState.currentChannel.id === channelData.id) {
      this.appState.currentChannel = channelData;
    }
    
    // Update main page
    const mainPage = this.pages.get('main');
    if (mainPage) {
      mainPage.updateChannels(this.appState.channels);
      if (this.appState.currentChannel) {
        mainPage.updateChannel(this.appState.currentChannel);
      }
    }
  }

  /**
   * Handle channel updated event (generic handler for user join/leave)
   */
  async handleChannelUpdated(eventData) {
    console.log('🔄 Channel updated event received:', eventData);
    
    // Si on est dans le channel qui a été mis à jour, on recharge ses informations
    if (this.appState.currentChannel && eventData.channelId === this.appState.currentChannel.id) {
      console.log('� Refreshing channel data from API...');
      
      const mainPage = this.pages.get('main');
      if (mainPage && mainPage.refreshChannelData) {
        await mainPage.refreshChannelData();
        console.log('✅ Channel data refreshed successfully');
      } else {
        console.log('ℹ️ Main page not available or no refresh method');
      }
    } else {
      console.log('ℹ️ Event not for current channel, ignoring');
    }
  }

  /**
   * Handle channel users list
   */
  handleChannelUsers(data) {
    console.log('Channel users update:', data);
    
    // Update current channel users if needed
    if (this.appState.currentChannel && data.channelId === this.appState.currentChannel.id) {
      const mainPage = this.pages.get('main');
      if (mainPage) {
        // Convert user IDs to user objects - for now just create minimal objects
        const users = data.users.map(userId => ({
          id: userId,
          username: `User ${userId.slice(0, 8)}...`, // Temporary username
          isSpeaking: false,
          micEnabled: true,
          speakerEnabled: true
        }));
        
        mainPage.updateUsersList(users);
      }
    }
  }

  /**
   * Handle audio level updates
   */
  handleAudioLevel(levelData) {
    // Update audio controls
    const mainPage = this.pages.get('main');
    if (mainPage) {
      mainPage.updateAudioLevels(levelData);
    }
  }

  /**
   * Set connection status
   */
  setConnectionStatus(status) {
    this.appState.connectionStatus = status;
    
    // Update all pages
    this.pages.forEach(page => {
      if (page.updateConnectionStatus) {
        page.updateConnectionStatus(status);
      }
    });
  }

  /**
   * Show audio settings modal
   */
  async showAudioSettings() {
    const mainPage = this.pages.get('main');
    if (mainPage) {
      await mainPage.showAudioSettings();
    }
  }

  /**
   * Save user session
   */
  async saveUserSession(user, serverUrl = null) {
    try {
      // Si pas d'URL fournie, essayer de la récupérer du localStorage
      const currentServerUrl = serverUrl || localStorage.getItem('lastServerUrl') || null;
      
      localStorage.setItem('voice-chat-user', JSON.stringify({
        username: user.username,
        serverId: user.serverId,
        serverUrl: currentServerUrl,
        savedAt: Date.now()
      }));
    } catch (error) {
      console.warn('Failed to save user session:', error);
    }
  }

  /**
   * Load user session
   */
  async loadUserSession() {
    try {
      const saved = localStorage.getItem('voice-chat-user');
      if (!saved) return null;
      
      const session = JSON.parse(saved);
      
      // Check if session is not too old (24 hours)
      const maxAge = 24 * 60 * 60 * 1000;
      if (Date.now() - session.savedAt > maxAge) {
        await this.clearUserSession();
        return null;
      }
      
      return session;
    } catch (error) {
      console.warn('Failed to load user session:', error);
      return null;
    }
  }

  /**
   * Clear user session
   */
  async clearUserSession() {
    try {
      localStorage.removeItem('voice-chat-user');
    } catch (error) {
      console.warn('Failed to clear user session:', error);
    }
  }

  /**
   * Reconnect from saved session
   */
  async reconnectFromSession(savedUser) {
    try {
      console.log('🔄 Attempting to reconnect from session...', savedUser);
      
      if (!savedUser.serverUrl) {
        console.warn('⚠️ No server URL in saved session, redirecting to login');
        this.navigateTo('login');
        return;
      }

      this.setConnectionStatus('connecting');
      this.showNotification('Reconnecting to server...', 'info');

      // Reconnecter au serveur
      console.log('📡 Calling tauriAPI.connectToServer for reconnection...');
      const result = await window.tauriAPI.connectToServer({
        username: savedUser.username,
        serverUrl: savedUser.serverUrl
      });
      
      console.log('📡 Reconnection result:', result);

      if (result.success) {
        console.log('✅ Reconnection successful, updating app state...');
        this.appState.user = result.user;
        this.appState.channels = result.channels || [];
        
        // Sauvegarder la session mise à jour
        await this.saveUserSession(result.user, savedUser.serverUrl);
        
        this.setConnectionStatus('connected');
        this.showNotification('Reconnected successfully!', 'success');
        
        console.log('🔄 Navigating to main page...');
        await this.navigateTo('main');
        console.log('✅ Reconnection complete');
      } else {
        console.warn('❌ Reconnection failed:', result.error);
        this.setConnectionStatus('disconnected');
        this.showNotification('Reconnection failed, please login again', 'error');
        
        // Supprimer la session invalide et rediriger vers login
        await this.clearUserSession();
        this.navigateTo('login');
      }
    } catch (error) {
      console.error('❌ Reconnection error:', error);
      this.setConnectionStatus('disconnected');
      this.showNotification('Reconnection failed, please login again', 'error');
      
      // Supprimer la session invalide et rediriger vers login
      await this.clearUserSession();
      this.navigateTo('login');
    }
  }

  /**
   * Show notification
   */
  showNotification(message, type = 'info') {
    if (window.domUtils) {
      window.domUtils.showNotification(message, type);
    } else {
      console.warn('domUtils not available, showing alert instead:', message);
      alert(message);
    }
  }

  /**
   * Show initialization error
   */
  showInitializationError(error) {
    console.error('Initialization error:', error);
    
    // Hide loading screen
    const loadingScreen = document.getElementById('loading-screen');
    if (loadingScreen) {
      loadingScreen.style.display = 'none';
    }
    
    // Show error message
    const container = document.getElementById('page-container');
    if (container) {
      container.innerHTML = `
        <div class="error-page">
          <div class="error-content">
            <h2>Initialization Failed</h2>
            <p>The application failed to start properly.</p>
            <details>
              <summary>Error Details</summary>
              <pre>${error.message || error}</pre>
            </details>
            <button onclick="location.reload()" class="btn btn-primary">
              Retry
            </button>
          </div>
        </div>
      `;
      container.style.display = 'block';
    }
  }

  /**
   * Get current app state
   */
  getState() {
    return { ...this.appState };
  }

  /**
   * Update app state
   */
  updateState(updates) {
    Object.assign(this.appState, updates);
  }

  /**
   * Handle refresh channels request from MainPage
   */
  async handleRefreshChannels() {
    try {
      console.log('🔄 App: Handling refresh channels request...');
      await this.refreshChannelsList();
      
      // Si on a un channel actuel, on met à jour sa vue avec les nouvelles données
      if (this.appState.currentChannel) {
        const updatedChannel = this.appState.channels.find(c => c.id === this.appState.currentChannel.id);
        if (updatedChannel) {
          console.log('📋 Updating current channel view with fresh data...');
          this.appState.currentChannel = updatedChannel;
          
          const mainPage = this.pages.get('main');
          if (mainPage) {
            await mainPage.updateChannel(updatedChannel);
          }
        }
      }
      
      console.log('✅ Channels refresh completed');
    } catch (error) {
      console.error('❌ Failed to refresh channels:', error);
    }
  }

  /**
   * Handle refresh channels request
   */
  async handleRefreshChannels() {
    try {
      console.log('🔄 Handling refresh channels request...');
      await this.refreshChannelsList();
      console.log('✅ Channels refreshed successfully');
    } catch (error) {
      console.error('❌ Failed to handle refresh channels:', error);
    }
  }

  /**
   * Refresh channels list from backend
   */
  async refreshChannelsList() {
    try {
      console.log('🔄 Refreshing channels list...');
      const result = await window.tauriAPI.getChannels();
      
      if (result.success && result.channels) {
        console.log('📋 Updated channels raw data:', result.channels);
        this.appState.channels = result.channels;
        
        // Afficher le détail de chaque channel pour debug
        result.channels.forEach(channel => {
          console.log(`📋 Channel "${channel.name}": ${channel.userCount || channel.user_count || 0} users`, channel.users || []);
        });
        
        // Update main page channels list
        const mainPage = this.pages.get('main');
        if (mainPage) {
          mainPage.updateChannels(this.appState.channels);
        }
      } else {
        console.error('❌ Failed to get channels:', result.error);
      }
    } catch (error) {
      console.error('❌ Failed to refresh channels:', error);
    }
  }

  /**
   * Cleanup on app exit
   */
  async cleanup() {
    try {
      console.log('Cleaning up...');
      
      // Leave current channel
      if (this.appState.currentChannel) {
        await window.tauriAPI.leaveChannel();
      }
      
      // Disconnect from server
      if (this.appState.user) {
        await window.tauriAPI.disconnectFromServer();
      }
      
      // Cleanup pages
      this.pages.forEach(page => {
        if (page.cleanup) {
          page.cleanup();
        }
      });
      
    } catch (error) {
      console.error('Cleanup failed:', error);
    }
  }
}

// Initialize app when DOM is ready
document.addEventListener('DOMContentLoaded', async () => {
  try {
    console.log('DOM loaded, starting app...');
    
    // Import DOM utilities first
    console.log('Importing DOM utilities...');
    const domUtilsModule = await import('./utils/dom.js');
    window.domUtils = domUtilsModule.default;
    console.log('DOM utilities loaded');
    
    // Import Tauri API
    console.log('Importing Tauri API...');
    const tauriModule = await import('./utils/tauri.js');
    window.tauriAPI = tauriModule.default;
    console.log('Tauri API loaded');
    
    // Initialize app
    console.log('Creating app instance...');
    window.voiceChatApp = new VoiceChatApp();
    console.log('App instance created');
    
  } catch (error) {
    console.error('Failed to start app:', error);
    
    // Show error manually if domUtils not available
    const loadingScreen = document.getElementById('loading-screen');
    if (loadingScreen) {
      loadingScreen.innerHTML = `
        <div class="loading-content">
          <h2 style="color: #ff4444;">Startup Error</h2>
          <p style="color: #ff4444;">Failed to load application</p>
          <details style="margin-top: 10px;">
            <summary>Error Details</summary>
            <pre style="background: #333; color: #fff; padding: 10px; border-radius: 4px; margin-top: 5px;">${error.message || error}</pre>
          </details>
          <button onclick="location.reload()" style="margin-top: 10px; padding: 8px 16px; background: #5865f2; color: white; border: none; border-radius: 4px; cursor: pointer;">
            Retry
          </button>
        </div>
      `;
    }
  }
});

export default VoiceChatApp;