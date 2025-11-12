import WebSocketClient from './websocket.js';
import Keyboard from './keyboard.js';
import Mouse from './mouse.js'
import Monitor from './monitor.js'
import topBar from './top.js'

function getToken(tenantID, publicKey, agentType = "c_agent") {
    return `ws/cli/${agentType}/${tenantID}/${publicKey}`
}


export default class KVM {
    constructor(tenantID, publicKey, agentType = "c_agent") {
        this.agentType = agentType;
        this.CanvasId = document.getElementById("Desk");
        this.Ctx = this.CanvasId.getContext("2d");

        this.Url = new URL(getToken(tenantID, publicKey, agentType), window.location.href).href
        console.log(`Connecting to ${agentType}:`, this.Url);

        this.Socket = new WebSocketClient(this.Url, { autoReconnect: true }, this);
        this.Debugmode = localStorage.getItem('debug') === '1' ? 1 : 0;
        this.Imagetype = 1;
        this.webComponents(tenantID)
        this.IODevices()

    }

    webComponents(name) {
        new topBar(name)
    }

    IODevices() {

        this.Mouse = new Mouse(this.CanvasId, this.Socket)
        this.Monitor = new Monitor(this.CanvasId, this.Socket, this.Imagetype);
        this.Keyboard = new Keyboard(this.Debugmode, this.Socket, this.CanvasId)
    }
}
