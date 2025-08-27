/**
 * Tauri API utilities for frontend-backend communication
 */

// Check if we're in Tauri environment
const isTauri = typeof window !== 'undefined' && window.__TAURI__;

let invoke;
if (isTauri) {
  invoke = window.__TAURI__.core.invoke;
} else {
  // Mock invoke for web development
  invoke = async (command, args) => {
    console.warn('Mock Tauri command:', command, args);
    throw new Error('Tauri not available in browser mode');
  };
}

/**
 * Backend connection utilities
 */
export const tauriAPI = {
  // Server connection
  async connectToServer(serverData) {
    try {
      console.log('🔌 Connecting to server:', serverData);
      
      if (!isTauri) {
        // Simulate connection for web mode
        console.log('🌐 Running in web mode, simulating connection...');
        return {
          success: true,
          user: { username: serverData.username, id: '1' },
          channels: []
        };
      }

      console.log('📡 Calling connect_to_server command...');
      const result = await invoke('connect_to_server', { 
        serverUrl: serverData.serverUrl, 
        username: serverData.username 
      });
      
      console.log('📡 Backend response:', result);
      
      // Si la connexion est réussie, démarrer aussi la connexion WebSocket
      if (result.success) {
        console.log('🔗 Starting WebSocket connection...');
        
        // Utiliser l'URL WebSocket retournée par le backend
        const wsUrl = isTauri ? 
            serverData.serverUrl.replace('http://', 'ws://').replace('https://', 'wss://').replace(/:\d+/, ':8080/ws') : 
            `${serverData.serverUrl.replace('http://', 'ws://').replace('https://', 'wss://')}:8080/ws`;
        
        console.log('🔗 Using WebSocket URL:', wsUrl);
        
        try {
          await this.startWebSocket(wsUrl);
          console.log('✅ WebSocket connection started successfully');
        } catch (wsError) {
          console.warn('⚠️ WebSocket connection failed but server connection succeeded:', wsError);
          // Continue même si WebSocket échoue
        }
      }
      
      return result;
      
    } catch (error) {
      console.error('❌ Connection error:', error);
      return {
        success: false,
        error: error.toString()
      };
    }
  },

  // WebSocket connection
  async startWebSocket(wsUrl) {
    try {
      console.log('🔗 Starting WebSocket connection to:', wsUrl);
      
      if (!isTauri) {
        console.log('🌐 Running in web mode, simulating WebSocket...');
        return { success: true };
      }

      console.log('📡 Calling start_websocket command...');
      await invoke('start_websocket', { wsUrl });
      
      console.log('✅ WebSocket connection command completed');
      return { success: true };
      
    } catch (error) {
      console.error('❌ WebSocket connection error:', error);
      return {
        success: false,
        error: error.toString()
      };
    }
  },

  async disconnectFromServer() {
    try {
      console.log('🔌 Disconnecting from server...');
      
      if (!isTauri) {
        console.log('🌐 Running in web mode, simulating disconnection...');
        return { success: true };
      }

      console.log('📡 Calling disconnect_user command...');
      await invoke('disconnect_user');
      
      console.log('✅ Successfully disconnected from server');
      return { success: true };
    } catch (error) {
      console.error('❌ Disconnection error:', error);
      return { success: false, error: error.toString() };
    }
  },

  // WebSocket management
  async stopWebSocket() {
    try {
      console.log('🔌 Stopping WebSocket connection...');
      
      if (!isTauri) {
        console.log('🌐 Running in web mode, simulating WebSocket stop...');
        return { success: true };
      }
      
      await invoke('stop_websocket');
      console.log('✅ WebSocket connection stopped');
      return { success: true };
    } catch (error) {
      console.error('❌ WebSocket stop error:', error);
      return { success: false, error: error.toString() };
    }
  },

  // Channel management
  async joinChannel(channelId) {
    try {
      console.log('🏠 TauriAPI: Joining channel:', channelId);
      
      if (!isTauri) return { success: true };
      
      await invoke('join_channel', { channelId });
      console.log('✅ TauriAPI: Successfully joined channel');
      return { success: true };
    } catch (error) {
      console.error('❌ TauriAPI: Failed to join channel:', error);
      return { success: false, error: error.toString() };
    }
  },

  async getChannels() {
    try {
      console.log('📋 TauriAPI: Getting channels list...');
      
      if (!isTauri) {
        return { 
          success: true, 
          channels: [
            { id: '1', name: 'General', userCount: 0, users: [] }
          ]
        };
      }
      
      const channels = await invoke('get_channels');
      console.log('📋 TauriAPI: Got channels:', channels);
      return { success: true, channels };
    } catch (error) {
      console.error('❌ TauriAPI: Failed to get channels:', error);
      return { success: false, error: error.toString() };
    }
  },

  async leaveChannel() {
    try {
      if (!isTauri) return { success: true };
      
      await invoke('leave_current_channel');
      return { success: true };
    } catch (error) {
      console.error('Failed to leave channel:', error);
      return { success: false, error: error.toString() };
    }
  },

  // Audio device management
  async getAudioDevices() {
    try {
      if (!isTauri) {
        // Mock devices for web mode
        return {
          input: [
            { name: 'Default Microphone', id: 'default_input', is_default: true }
          ],
          output: [
            { name: 'Default Speakers', id: 'default_output', is_default: true }
          ]
        };
      }
      
      const result = await invoke('scan_audio_devices');
      console.log('Raw audio devices result:', result);
      
      // Transform the result to match expected format
      return {
        input: result.input_devices || [],
        output: result.output_devices || []
      };
    } catch (error) {
      console.error('Failed to get audio devices:', error);
      return {
        input: [],
        output: []
      };
    }
  },

  async selectInputDevice(deviceId) {
    try {
      if (!isTauri) return { success: true };
      
      await invoke('select_input_device', { deviceId });
      return { success: true };
    } catch (error) {
      console.error('Failed to select input device:', error);
      return { success: false, error: error.toString() };
    }
  },

  async selectOutputDevice(deviceId) {
    try {
      if (!isTauri) return { success: true };
      
      await invoke('select_output_device', { deviceId });
      return { success: true };
    } catch (error) {
      console.error('Failed to select output device:', error);
      return { success: false, error: error.toString() };
    }
  },

  async testAudioPlayback() {
    try {
      if (!isTauri) {
        // Simulate test audio for web mode
        console.log('Test audio played (simulated)');
        return { success: true };
      }
      
      await invoke('play_test_sound');
      return { success: true };
    } catch (error) {
      console.error('Failed to play test sound:', error);
      return { success: false, error: error.toString() };
    }
  },

  // Audio control
  async playTestSound() {
    try {
      if (!isTauri) {
        console.log('Playing test sound (mock)');
        return { success: true };
      }
      
      await invoke('play_test_sound');
      return { success: true };
    } catch (error) {
      console.error('Failed to play test sound:', error);
      return { success: false, error: error.toString() };
    }
  },

  // Audio capture control
  async startAudioCapture() {
    try {
      if (!isTauri) {
        console.log('Starting audio capture (mock)');
        return { success: true };
      }
      
      await invoke('start_audio_capture');
      return { success: true };
    } catch (error) {
      console.error('Failed to start audio capture:', error);
      return { success: false, error: error.toString() };
    }
  },

  async stopAudioCapture() {
    try {
      if (!isTauri) {
        console.log('Stopping audio capture (mock)');
        return { success: true };
      }
      
      await invoke('stop_audio_capture');
      return { success: true };
    } catch (error) {
      console.error('Failed to stop audio capture:', error);
      return { success: false, error: error.toString() };
    }
  }
};

// Export for global access
if (typeof window !== 'undefined') {
  window.tauriAPI = tauriAPI;
}

export default tauriAPI;
