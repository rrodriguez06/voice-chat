/**
 * DOM utilities and helpers
 */

export const domUtils = {
  // Element creation and manipulation
  createElement(tag, className = '', attributes = {}) {
    const element = document.createElement(tag);
    if (className) element.className = className;
    
    Object.entries(attributes).forEach(([key, value]) => {
      element.setAttribute(key, value);
    });
    
    return element;
  },

  // Event handling
  addEvent(element, event, handler) {
    element.addEventListener(event, handler);
    return () => element.removeEventListener(event, handler);
  },

  // Show notification
  showNotification(message, type = 'info', duration = 3000) {
    const container = document.getElementById('notification-container') || 
                     this.createElement('div', 'notification-container');
    
    if (!container.parentNode) {
      document.body.appendChild(container);
    }
    
    const notification = this.createElement('div', 
      'notification notification-' + type);
    notification.textContent = message;
    
    container.appendChild(notification);
    
    // Auto remove
    setTimeout(() => {
      if (notification.parentNode) {
        container.removeChild(notification);
      }
    }, duration);
  },

  // Form utilities
  getFormData(form) {
    const formData = new FormData(form);
    const data = {};
    
    for (let [key, value] of formData.entries()) {
      data[key] = value;
    }
    
    return data;
  },

  // Toggle classes
  toggleClass(element, className) {
    element.classList.toggle(className);
  },

  // Find elements
  find(selector, parent = document) {
    return parent.querySelector(selector);
  },

  findAll(selector, parent = document) {
    return Array.from(parent.querySelectorAll(selector));
  },

  // Time formatting utility
  formatTime(timestamp) {
    const date = new Date(timestamp);
    return date.toLocaleTimeString();
  }
};

// Individual exports for convenience
export const createElement = domUtils.createElement.bind(domUtils);
export const $ = domUtils.find.bind(domUtils);
export const addListener = domUtils.addEvent.bind(domUtils);
export const formatTime = domUtils.formatTime.bind(domUtils);
export const storage = {
  get: (key) => localStorage.getItem(key),
  set: (key, value) => localStorage.setItem(key, value),
  remove: (key) => localStorage.removeItem(key)
};

// Export for global access
if (typeof window !== 'undefined') {
  window.domUtils = domUtils;
}

export default domUtils;
