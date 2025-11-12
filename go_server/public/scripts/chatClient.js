export class ChatClient {
    constructor(tenantID, publicKey) {
        this.tenantID = tenantID;
        this.publicKey = publicKey;
        this.socket1 = null;
        this.isConnected = false;
        this.agentStatus = {};
        this.agentID = 'chat_agent';
        this.clientID = "js_client_" + String(Math.floor(Math.random() * 1000)).padStart(3, "0");
        this.requestNotificationPermission();
        this.connect();
    }
    async requestNotificationPermission() {
        if ('Notification' in window) {
            if (Notification.permission === 'default') {
                try {
                    const permission = await Notification.requestPermission();
                    if (permission === 'granted') {
                    }
                } catch (error) {
                    console.error('Error requesting notification permission:', error);
                }
            }
        }
    }
    connect() {
        const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
        const wsUrl = `${protocol}//${window.location.host}/ws/cli/chat_agent/${this.tenantID}/${this.publicKey}`;

        console.log('Connecting to chat WebSocket:', wsUrl);

        this.socket1 = new WebSocket(wsUrl);

        this.socket1.onopen = () => {
            console.log('Chat WebSocket connected');
            this.isConnected = true;

        };

        this.socket1.onmessage = (event) => {
            try {
                if (event.data instanceof ArrayBuffer || event.data instanceof Blob) {
                    console.log('Ignoring binary message in chat client');
                    return;
                }

                if (typeof event.data === 'string') {

                    if (event.data.length < 10 && /^[\x00-\x1F]/.test(event.data)) {

                        console.log('Ignoring control message in chat client');
                        return;
                    }

                    let data;
                    try {
                        data = JSON.parse(event.data);
                    } catch (parseError) {

                        console.log('Ignoring non-JSON message in chat client:', event.data.substring(0, 50));
                        return;
                    }


                    if (!data.type || (data.type !== 'chat_message' && data.type !== 'status_update')) {
                        console.log('Ignoring non-chat message:', data.type);
                        return;
                    }


                    this.handleMessage(data);
                }
            } catch (error) {
                console.error('Error processing chat message:', error);

                if (!this.errorCount) this.errorCount = 0;
                this.errorCount++;
                if (this.errorCount > 10) {
                    console.warn('Too many chat parsing errors, suppressing further error logs');
                    this.errorCount = 0;
                }
            }
        };

        this.socket1.onclose = () => {
            console.log('Chat WebSocket disconnected');
            this.isConnected = false;

        };

        this.socket1.onerror = (error) => {
            console.error('Chat WebSocket error:', error);
        };
    }

    handleMessage(data) {
        // console.log("Received message on the handlemessage():", data);
        switch (data.type) {
            case 'chat_message':
                const isSent = data.from === this.clientID;

                if (this.shouldShowNotification() && !isSent) {
                    this.showChatNotification(data);
                }
                this.displayMessage(data, isSent);
                break;

            case 'status_update':
                this.agentStatus[data.agent_id] = data.status;
                this.updateAgentStatusDisplay(data.agent_id, data.status);
                break;
        }
    }

    shouldShowNotification() {

        return true;
    }
    sendMessage(message, toAgent = this.publicKey) {
        if (!this.isConnected) {
            this.showNotification('Cannot send message - not connected to server', 'error');
            return;
        }

        const agentStatus = this.agentStatus[this.tenantID];
        if (agentStatus !== 'online') {
            this.showNotification('Cannot send message - agent is offline', 'error');
            return;
        }

        const chatMessage = {
            type: 'chat_message',
            from: this.clientID,
            to: toAgent,
            message: message,
            timestamp: Date.now(),
            agent_type: 'chat_agent'
        };

        this.socket1.send(JSON.stringify(chatMessage));
    }

    sendStatusUpdate(status) {

        return;

        if (!this.isConnected) return;

        const statusMessage = {
            type: 'status_update',
            agent_id: this.publicKey,
            status: status,
            agent_type: 'chat_agent'
        };

        this.socket1.send(JSON.stringify(statusMessage));
    }

    getDisplayName(from, isSent = false) {
        if (isSent) {
            return 'You';
        } else {
            return from || 'Unknown User';
        }
    }

    showChatNotification(data) {
        const notificationID = `${data.timestamp}_${data.from}_${data.message.substring(0, 20)}`;


        if (!this.shownNotifications) {
            this.shownNotifications = new Set();
        }

        if (this.shownNotifications.has(notificationID)) {
            console.log('Duplicate notification detected, skipping:', notificationID);
            return;
        }
        this.shownNotifications.add(notificationID);
        if (this.shownNotifications.size > 50) {
            const notificationsToDelete = Array.from(this.shownNotifications).slice(0, 25);
            notificationsToDelete.forEach(id => this.shownNotifications.delete(id));
        }
        const isChrome = /Chrome/.test(navigator.userAgent) && /Google Inc/.test(navigator.vendor);
        const isEdge = /Edg/.test(navigator.userAgent);

        const displayName = this.getDisplayName(data.from);

        if (Notification.permission === 'granted') {
            try {

                if (this.lastNotification) {
                    this.lastNotification.close();
                }

                const notification = new Notification(`New message from ${displayName}`, {
                    body: data.message.substring(0, 100),
                    icon: '/img/cachatto.ico',
                    tag: `chat-message-${data.from}`,
                    requireInteraction: false,
                    silent: false,
                    renotify: false
                });

                this.lastNotification = notification;

                notification.onclick = () => {

                    window.focus();

                    const chatTab = document.getElementById('chat-section');
                    if (chatTab) {
                        chatTab.click();
                    } else {
                        this.openChatWindow();
                    }

                    notification.close();
                };


                setTimeout(() => {
                    notification.close();
                }, 5000);

            } catch (error) {
                console.error('Error creating notification:', error);
                this.showCustomNotification(data);
            }
        } else if (Notification.permission === 'default') {

            Notification.requestPermission().then(permission => {

                if (permission === 'granted') {

                    setTimeout(() => this.showChatNotification(data), 100);
                } else {
                    console.log('Permission denied, using custom notification');
                    this.showCustomNotification(data);
                }
            }).catch(error => {
                console.error('Permission request failed:', error);
                this.showCustomNotification(data);
            });
        } else {

            console.log('Using custom notification due to denied permission');
            this.showCustomNotification(data);
        }
    }


    showCustomNotification(data) {

        const existingNotifications = document.querySelectorAll('.chat-notification');
        existingNotifications.forEach(notif => notif.remove());


        const displayName = this.getDisplayName(data.from);

        const notification = document.createElement('div');
        notification.className = 'chat-notification';
        notification.innerHTML = `
        <div class="notification-header">
            <strong>New Message from ${displayName}</strong>
            <button class="close-btn" onclick="this.parentElement.parentElement.remove()">&times;</button>
        </div>
        <div class="notification-body">${data.message.substring(0, 100)}</div>
        <div class="notification-actions">
            <button class="open-chat-btn">Open Chat</button>
        </div>
    `;

        notification.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        width: 300px;
        background: white;
        border: 1px solid #ccc;
        border-radius: 5px;
        padding: 10px;
        box-shadow: 0 4px 8px rgba(0,0,0,0.1);
        z-index: 10000;
        animation: slideIn 0.3s ease-out;
    `;

        if (!document.getElementById('notification-styles')) {
            const style = document.createElement('style');
            style.id = 'notification-styles';
            style.textContent = `
            @keyframes slideIn {
                from {
                    transform: translateX(100%);
                    opacity: 0;
                }
                to {
                    transform: translateX(0);
                    opacity: 1;
                }
            }
        `;
            document.head.appendChild(style);
        }

        document.body.appendChild(notification);

        notification.querySelector('.open-chat-btn').onclick = () => {
            this.openChatWindow();
            notification.remove();
        };

    }

    createChatUI(container) {
        if (container) {
            this.createChatInterface(container);
        } else {
            this.createChatWindow();
        }
    }
    createChatInterface(container) {

        container.innerHTML = `
        <div class="chat-messages" id="chat-messages"></div>
        <div class="chat-input-container">
            <input type="text" id="chat-input" placeholder="Type your message..." />
            <button id="send-btn" onclick="chatClient.sendCurrentMessage()">Send</button>
        </div>
    `;

        container.querySelector('#chat-input').addEventListener('keypress', (e) => {
            if (e.key === 'Enter') {
                this.sendCurrentMessage();
            }
        });
    }

    createChatWindow() {
        const chatWindow = document.createElement('div');
        chatWindow.id = 'chat-window';
        chatWindow.className = 'chat-window hidden';
        chatWindow.innerHTML = `
        <div class="chat-header">
            <h3>Chat with ${this.tenantID}</h3>
            <div class="header-controls">
                <div class="agent-status" id="agent-status">Offline</div>
                <button class="extend-btn" id="extend-btn" onclick="chatClient.toggleChatSize()" title="Extend/Minimize Chat">⛶</button>
                <button class="close-btn" onclick="chatClient.closeChatWindow()">&times;</button>
            </div>
        </div>
        <div class="chat-messages" id="chat-messages"></div>
        <div class="chat-input-container">
            <input type="text" id="chat-input" placeholder="Type your message..." />
            <button id="send-btn" onclick="chatClient.sendCurrentMessage()">Send</button>
        </div>
    `;

        chatWindow.style.cssText = `
        position: fixed;
        bottom: 20px;
        right: 20px;
        width: 350px;
        height: 400px;
        background: white;
        border: 1px solid #ccc;
        border-radius: 5px;
        display: flex;
        flex-direction: column;
        z-index: 9999;
        box-shadow: 0 4px 12px rgba(0,0,0,0.15);
        transition: all 0.3s ease;
    `;

        document.body.appendChild(chatWindow);
        document.getElementById('chat-input').addEventListener('keypress', (e) => {
            if (e.key === 'Enter') {
                this.sendCurrentMessage();
            }
        });

        this.isExtended = false;
    }

    toggleChatWindow() {
        const chatWindow = document.getElementById('chat-window');
        chatWindow.classList.toggle('hidden');
    }

    openChatWindow() {
        const chatWindow = document.getElementById('chat-window');
        chatWindow.classList.remove('hidden');
    }

    closeChatWindow() {
        const chatWindow = document.getElementById('chat-window');
        chatWindow.classList.add('hidden');
    }

    toggleChatSize() {
        const chatWindow = document.getElementById('chat-window');
        const extendBtn = document.getElementById('extend-btn');
        const inputContainer = chatWindow.querySelector('.chat-input-container');
        const messagesContainer = chatWindow.querySelector('.chat-messages');

        if (!this.isExtended) {
            chatWindow.style.cssText = `
            position: fixed;
            top: 0;
            left: 0;
            width: 100vw;
            height: 100vh;
            background: white;
            border: none;
            border-radius: 0;
            display: flex;
            flex-direction: column;
            z-index: 10000;
            box-shadow: none;
            transition: all 0.3s ease;
        `;

            if (inputContainer) {
                inputContainer.style.cssText = `
                position: fixed;
                bottom: 0;
                left: 0;
                right: 0;
                width: 100%;
                padding: 20px 30px;
                background: white;
                border-top: 2px solid #e0e0e0;
                box-shadow: 0 -2px 10px rgba(0,0,0,0.1);
                z-index: 10001;
                display: flex;
                gap: 10px;
                align-items: center;
                box-sizing: border-box;
            `;

                const input = inputContainer.querySelector('input');
                const button = inputContainer.querySelector('button');

                if (input) {
                    input.style.cssText = `
                    flex: 1;
                    padding: 15px 20px;
                    border: 2px solid #e0e0e0;
                    border-radius: 30px;
                    font-size: 16px;
                    outline: none;
                    transition: border-color 0.2s ease;
                `;
                }

                if (button) {
                    button.style.cssText = `
                    padding: 15px 25px;
                    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                    color: white;
                    border: none;
                    border-radius: 30px;
                    cursor: pointer;
                    font-weight: 600;
                    font-size: 16px;
                    min-width: 100px;
                    flex-shrink: 0;
                `;
                }
            }

            if (messagesContainer) {
                messagesContainer.style.cssText = `
                flex: 1;
                overflow-y: auto;
                padding: 20px 30px 120px 30px;
                background: #f8f9fa;
                scrollbar-width: thin;
                scrollbar-color: #ccc transparent;
                min-height: 0;
            `;
            }

            extendBtn.innerHTML = '🗗';
            extendBtn.title = 'Minimize Chat';
            this.isExtended = true;

            this.escapeKeyHandler = (e) => {
                if (e.key === 'Escape') {
                    this.toggleChatSize();
                }
            };
            document.addEventListener('keydown', this.escapeKeyHandler);
        } else {

            chatWindow.style.cssText = `
            position: fixed;
            bottom: 20px;
            right: 20px;
            width: 350px;
            height: 400px;
            background: white;
            border: 1px solid #ccc;
            border-radius: 5px;
            display: flex;
            flex-direction: column;
            z-index: 9999;
            box-shadow: 0 4px 12px rgba(0,0,0,0.15);
            transition: all 0.3s ease;
        `;

            if (inputContainer) {
                inputContainer.style.cssText = `
                display: flex;
                padding: 15px;
                border-top: 1px solid #e0e0e0;
                background: white;
                gap: 10px;
                align-items: center;
                flex-shrink: 0;
                position: relative;
                z-index: 1;
            `;

                const input = inputContainer.querySelector('input');
                const button = inputContainer.querySelector('button');

                if (input) {
                    input.style.cssText = `
                    flex: 1;
                    padding: 12px 15px;
                    border: 2px solid #e0e0e0;
                    border-radius: 25px;
                    font-size: 14px;
                    outline: none;
                    transition: border-color 0.2s ease;
                    min-width: 0;
                `;
                }

                if (button) {
                    button.style.cssText = `
                    padding: 12px 20px;
                    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
                    color: white;
                    border: none;
                    border-radius: 25px;
                    cursor: pointer;
                    font-weight: 600;
                    transition: all 0.2s ease;
                    min-width: 80px;
                    flex-shrink: 0;
                `;
                }
            }

            if (messagesContainer) {
                messagesContainer.style.cssText = `
                flex: 1;
                overflow-y: auto;
                padding: 15px;
                background: #f8f9fa;
                scrollbar-width: thin;
                scrollbar-color: #ccc transparent;
                min-height: 0;
            `;
            }

            extendBtn.innerHTML = '⛶';
            extendBtn.title = 'Extend Chat';
            this.isExtended = false;

            if (this.escapeKeyHandler) {
                document.removeEventListener('keydown', this.escapeKeyHandler);
                this.escapeKeyHandler = null;
            }
        }
    }

    sendCurrentMessage() {
        const input = document.getElementById('chat-input');
        const message = input.value.trim();
        if (message) {
            this.sendMessage(message);
            input.value = '';
        }
    }

    displayMessage(data, isSent = false) {
        const messagesContainer = document.getElementById('chat-messages');
        const messageDiv = document.createElement('div');
        messageDiv.className = `message ${isSent ? 'sent' : 'received'}`;

        const time = new Date(data.timestamp).toLocaleTimeString();

        const displayName = this.getDisplayName(data.from, isSent);

        messageDiv.innerHTML = `
            <div class="message-header">
                <strong>${displayName}</strong>
                <span class="time">${time}</span>
            </div>
            <div class="message-body">${data.message}</div>
        `;

        messagesContainer.appendChild(messageDiv);
        messagesContainer.scrollTop = messagesContainer.scrollHeight;
    }

    updateAgentStatusDisplay(agentId, status) {
        let statusElement = document.getElementById(`agent-status-${agentId}`);

        if (!statusElement) {
            statusElement = document.getElementById('agent-status');
        }

        if (statusElement) {
            statusElement.textContent = `${status}`;
            statusElement.className = `agent-status ${status}`;
        }

        const chatInput = document.getElementById('chat-input');
        const sendBtn = document.getElementById('send-btn');

        if (chatInput && sendBtn) {
            if (status === 'offline') {
                chatInput.disabled = true;
                chatInput.placeholder = 'Agent is offline - cannot send messages';
                sendBtn.disabled = true;
                sendBtn.style.opacity = '0.5';
            } else {
                chatInput.disabled = false;
                chatInput.placeholder = 'Type your message...';
                sendBtn.disabled = false;
                sendBtn.style.opacity = '1';
            }
        }
    }


    updateConnectionStatus(status) {
        console.log('Connection status:', status);
    }

    showNotification(message, type = 'info') {
        const notification = document.createElement('div');
        notification.className = `notification ${type}`;
        notification.textContent = message;
        notification.style.cssText = `
        position: fixed;
        top: 20px;
        right: 20px;
        padding: 10px 15px;
        background: ${type === 'error' ? '#f44336' : '#4CAF50'};
        color: white;
        border-radius: 4px;
        z-index: 10000;
        animation: slideIn 0.3s ease-out;
    `;

        document.body.appendChild(notification);

    }
}

let chatClient;