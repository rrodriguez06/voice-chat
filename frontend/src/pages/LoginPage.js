import domUtils, { $, addListener, storage } from '../utils/dom.js';
import tauriAPI from '../utils/tauri.js';

/**
 * Login Page Component
 */
export default class LoginPage {
  constructor(app) {
    this.app = app;
    this.isConnecting = false;
  }

  /**
   * Show the login page
   */
  async show(params = {}) {
    console.log('Showing login page');
    
    const container = document.getElementById('page-container');
    container.innerHTML = this.render();
    
    // Hide loading screen and show page
    document.getElementById('loading-screen').style.display = 'none';
    container.style.display = 'block';
    document.body.classList.add('app-ready');
    
    this.bindEvents();
    this.loadSavedData();
    
    // Pre-fill form if params provided
    if (params.username) {
      const usernameInput = document.getElementById('username');
      if (usernameInput) {
        usernameInput.value = params.username;
      }
    }
    
    if (params.serverAddress) {
      const serverInput = document.getElementById('serverUrl');
      if (serverInput) {
        serverInput.value = params.serverAddress;
      }
    }
  }

  /**
   * Hide the login page
   */
  async hide() {
    console.log('Hiding login page');
    this.unbindEvents();
    
    const container = document.getElementById('page-container');
    container.style.display = 'none';
  }

  render() {
    return `
      <div class="login-page">
        <div class="login-card">
          <!-- Header -->
          <div class="login-header">
            <div class="login-logo">
              <div class="logo-icon">🎤</div>
            </div>
            <h1 class="login-title">Voice Chat</h1>
            <p class="login-subtitle">Connect to your voice server</p>
          </div>
          
          <!-- Form -->
          <form class="login-form" id="loginForm">
            <div class="form-group">
              <label for="username" class="form-label">Username</label>
              <input type="text" id="username" class="form-input" placeholder="Enter your username" required maxlength="32">
            </div>
            
            <div class="form-group">
              <label for="serverUrl" class="form-label">Server URL</label>
              <input type="url" id="serverUrl" class="form-input" placeholder="http://localhost:8080" required>
            </div>
            
            <div class="form-actions">
              <button type="submit" class="btn btn-primary btn-lg" id="connectBtn">Connect</button>
              <button type="button" class="btn btn-secondary" id="settingsBtn">Audio Settings</button>
            </div>
          </form>
          
          <!-- Status -->
          <div class="login-status" id="loginStatus"></div>
        </div>
      </div>
    `;
  }

  bindEvents() {
    const form = $('#loginForm');
    const connectBtn = $('#connectBtn');
    const settingsBtn = $('#settingsBtn');
    const usernameInput = $('#username');
    const serverUrlInput = $('#serverUrl');
    
    // Form submission
    if (form) {
      addListener(form, 'submit', (e) => {
        e.preventDefault();
        this.handleConnect();
      });
    }
    
    // Settings button
    if (settingsBtn) {
      addListener(settingsBtn, 'click', () => {
        this.showAudioSettings();
      });
    }
    
    // Input validation
    if (usernameInput) {
      addListener(usernameInput, 'blur', () => {
        this.validateUsername(usernameInput.value);
      });
    }
    
    if (serverUrlInput) {
      addListener(serverUrlInput, 'blur', () => {
        this.validateServerUrl(serverUrlInput.value);
      });
    }
  }

  unbindEvents() {
    // Remove event listeners if needed
    // DOM elements will be removed anyway
  }

  async handleConnect() {
    if (this.isConnecting) return;
    
    const usernameInput = $('#username');
    const serverUrlInput = $('#serverUrl');
    const connectBtn = $('#connectBtn');
    const statusDiv = $('#loginStatus');
    
    const username = usernameInput?.value.trim();
    const serverUrl = serverUrlInput?.value.trim() || 'http://localhost:8080';
    
    if (!username) {
      this.showStatus('Please enter a username', 'error');
      return;
    }
    
    if (!this.validateUsername(username)) {
      this.showStatus('Username contains invalid characters', 'error');
      return;
    }
    
    this.isConnecting = true;
    connectBtn.disabled = true;
    connectBtn.textContent = 'Connecting...';
    
    try {
      this.showStatus('Connecting to server...', 'info');
      
      // Save connection data
      storage.set('lastUsername', username);
      storage.set('lastServerUrl', serverUrl);
      
      this.showStatus('Connected successfully!', 'success');
      
      // Emit login event for app to handle
      const loginEvent = new CustomEvent('app:login', {
        detail: {
          username,
          serverUrl
        }
      });
      document.dispatchEvent(loginEvent);
      
    } catch (error) {
      console.error('Connection failed:', error);
      this.showStatus(`Connection failed: ${error.message}`, 'error');
    } finally {
      this.isConnecting = false;
      connectBtn.disabled = false;
      connectBtn.textContent = 'Connect';
    }
  }

  validateUsername(username) {
    if (!username) return false;
    
    // Allow alphanumeric, spaces, hyphens, underscores
    const validPattern = /^[a-zA-Z0-9\s\-_]+$/;
    const isValid = validPattern.test(username) && username.length >= 2 && username.length <= 32;
    
    const usernameInput = $('#username');
    if (usernameInput) {
      if (isValid) {
        usernameInput.classList.remove('input-error');
      } else {
        usernameInput.classList.add('input-error');
      }
    }
    
    return isValid;
  }

  validateServerUrl(url) {
    if (!url) return true; // Empty is ok, will use default
    
    try {
      new URL(url);
      const serverUrlInput = $('#serverUrl');
      if (serverUrlInput) {
        serverUrlInput.classList.remove('input-error');
      }
      return true;
    } catch {
      const serverUrlInput = $('#serverUrl');
      if (serverUrlInput) {
        serverUrlInput.classList.add('input-error');
      }
      return false;
    }
  }

  showStatus(message, type = 'info') {
    const statusDiv = $('#loginStatus');
    if (!statusDiv) return;
    
    statusDiv.textContent = message;
    statusDiv.className = `login-status status-${type}`;
    
    // Auto-clear success messages
    if (type === 'success') {
      setTimeout(() => {
        statusDiv.textContent = '';
        statusDiv.className = 'login-status';
      }, 3000);
    }
  }

  loadSavedData() {
    const lastUsername = storage.get('lastUsername');
    const lastServerUrl = storage.get('lastServerUrl');
    
    if (lastUsername) {
      const usernameInput = $('#username');
      if (usernameInput) usernameInput.value = lastUsername;
    }
    
    if (lastServerUrl) {
      const serverUrlInput = $('#serverUrl');
      if (serverUrlInput) serverUrlInput.value = lastServerUrl;
    }
  }

  async showAudioSettings() {
    console.log('Opening audio settings modal');
    
    // Create modal HTML
    const modalHTML = `
      <div class="modal-overlay" id="audioSettingsModal">
        <div class="modal card">
          <div class="modal-header">
            <h3>Audio Settings</h3>
            <button class="btn btn-sm" id="closeSettingsBtn">✕</button>
          </div>
          
          <div class="modal-content">
            <div class="form-group">
              <label class="form-label">Test Audio</label>
              <div class="audio-test-controls">
                <button type="button" class="btn btn-secondary" id="testAudioBtn">
                  Play Test Sound
                </button>
              </div>
            </div>
            
            <div class="audio-devices" id="audioDevicesList">
              <p>Loading audio devices...</p>
            </div>
          </div>
        </div>
      </div>
    `;
    
    // Add modal to page
    document.body.insertAdjacentHTML('beforeend', modalHTML);
    
    const modal = $('#audioSettingsModal');
    const closeBtn = $('#closeSettingsBtn');
    const testBtn = $('#testAudioBtn');
    
    // Bind events
    if (closeBtn) {
      addListener(closeBtn, 'click', () => this.closeAudioSettings());
    }
    
    if (testBtn) {
      addListener(testBtn, 'click', () => this.testAudio());
    }
    
    // Close on outside click
    if (modal) {
      addListener(modal, 'click', (e) => {
        if (e.target === modal) {
          this.closeAudioSettings();
        }
      });
    }
    
    // Load and display audio devices
    await this.loadAudioDevicesSettings();
  }

  async loadAudioDevicesSettings() {
    const devicesList = $('#audioDevicesList');
    if (!devicesList) return;
    
    try {
      console.log('Loading audio devices...');
      const devices = await tauriAPI.getAudioDevices();
      console.log('Audio devices loaded:', devices);
      
      let devicesHTML = '';
      
      // Input devices
      if (devices.input && devices.input.length > 0) {
        devicesHTML += `
          <div class="device-section">
            <h4>Input Devices (Microphones)</h4>
            ${devices.input.map(device => `
              <div class="device-item">
                <span class="device-name">${device.name}</span>
                <button class="btn btn-sm btn-outline" data-device-id="${device.id}" data-device-type="input">
                  Select
                </button>
              </div>
            `).join('')}
          </div>
        `;
      }
      
      // Output devices
      if (devices.output && devices.output.length > 0) {
        devicesHTML += `
          <div class="device-section">
            <h4>Output Devices (Speakers)</h4>
            ${devices.output.map(device => `
              <div class="device-item">
                <span class="device-name">${device.name}</span>
                <button class="btn btn-sm btn-outline" data-device-id="${device.id}" data-device-type="output">
                  Select
                </button>
              </div>
            `).join('')}
          </div>
        `;
      }
      
      if (!devicesHTML) {
        devicesHTML = '<p class="error-message">No audio devices found</p>';
      }
      
      devicesList.innerHTML = devicesHTML;
      
      // Bind device selection events
      const deviceButtons = devicesList.querySelectorAll('button[data-device-id]');
      deviceButtons.forEach(button => {
        addListener(button, 'click', () => {
          const deviceId = button.getAttribute('data-device-id');
          const deviceType = button.getAttribute('data-device-type');
          
          if (deviceType === 'input') {
            this.selectInputDevice(deviceId);
          } else if (deviceType === 'output') {
            this.selectOutputDevice(deviceId);
          }
        });
      });
      
    } catch (error) {
      console.error('Failed to load audio devices:', error);
      devicesList.innerHTML = `
        <div class="error-message">
          Failed to load audio devices: ${error.message}
        </div>
      `;
    }
  }

  closeAudioSettings() {
    const modal = $('#audioSettingsModal');
    if (modal) modal.remove();
  }

  async testAudio() {
    try {
      console.log('Testing audio...');
      const result = await tauriAPI.testAudioPlayback();
      if (result.success) {
        this.showStatus('Test sound played', 'success');
      } else {
        throw new Error(result.error || 'Unknown error');
      }
    } catch (error) {
      console.error('Audio test failed:', error);
      this.showStatus('Failed to play test sound: ' + error.message, 'error');
    }
  }

  async selectInputDevice(deviceId) {
    try {
      console.log('Selecting input device:', deviceId);
      const result = await tauriAPI.selectInputDevice(deviceId);
      if (result.success) {
        this.showStatus('Input device selected', 'success');
        // Update app state if available
        if (this.app) {
          this.app.updateState({ 
            audioSettings: { 
              ...this.app.getState().audioSettings, 
              inputDevice: deviceId 
            }
          });
        }
      } else {
        throw new Error(result.error || 'Failed to select device');
      }
    } catch (error) {
      console.error('Failed to select input device:', error);
      this.showStatus('Failed to select input device: ' + error.message, 'error');
    }
  }

  async selectOutputDevice(deviceId) {
    try {
      console.log('Selecting output device:', deviceId);
      const result = await tauriAPI.selectOutputDevice(deviceId);
      if (result.success) {
        this.showStatus('Output device selected', 'success');
        // Update app state if available
        if (this.app) {
          this.app.updateState({ 
            audioSettings: { 
              ...this.app.getState().audioSettings, 
              outputDevice: deviceId 
            }
          });
        }
      } else {
        throw new Error(result.error || 'Failed to select device');
      }
    } catch (error) {
      console.error('Failed to select output device:', error);
      this.showStatus('Failed to select output device: ' + error.message, 'error');
    }
  }

  destroy() {
    // Cleanup any event listeners or timers if needed
    this.container.innerHTML = '';
  }
}