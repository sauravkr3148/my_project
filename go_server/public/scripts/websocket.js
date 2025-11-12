
var cmd, X, Y, cmdsize;

function QV(x, y) {
    try {
        QS(x).display = (y ? '' : 'none');
    } catch (x) { }
}

async function blobToBytes(blob) {
    const arrayBuffer = await blob.arrayBuffer();
    return new Uint8Array(arrayBuffer);
}

var connected_status = "green";
var reconnect_failed = "red";
var no_agent = "yellow";
var reconnecting = "yellow";

var status = "";
var reconnect_counter = 0;

const context = new (window.AudioContext || window.webkitAudioContext)();
var nextPacketTime = context.currentTime;
var MIN_SPLIT_SIZE = 0.02;
var maxLatency = 0.3;
var SampleArray = window.Int16Array;
var maxSampleValue = 32768;

var format = {
    channels: 2,
    rate: 192000
};

var packetQueue = [];

function joinAudioPackets(packets) {
    if (packets.length <= 1) return packets[0];
    var totalLength = 0;
    packets.forEach(packet => totalLength += packet.length);
    var joined = new SampleArray(totalLength);
    var offset = 0;
    packets.forEach(packet => {
        joined.set(packet, offset);
        offset += packet.length;
    });
    return joined;
}

function splitAudioPacket(data) {
    var minValue = Number.MAX_VALUE;
    var optimalSplitLength = data.length;
    var samples = Math.floor(data.length / format.channels);
    var minSplitSamples = Math.floor(format.rate * MIN_SPLIT_SIZE);
    var start = Math.max(format.channels * minSplitSamples, format.channels * (samples - minSplitSamples));
    for (var offset = start; offset < data.length; offset += format.channels) {
        var totalValue = 0;
        for (var channel = 0; channel < format.channels; channel++) {
            totalValue += Math.abs(data[offset + channel]);
        }
        if (totalValue <= minValue) {
            optimalSplitLength = offset + format.channels;
            minValue = totalValue;
        }
    }

    if (optimalSplitLength === data.length) return [data];

    const firstSplit = new SampleArray(data.buffer.slice(0, optimalSplitLength * format.bytesPerSample));
    const secondSplit = new SampleArray(data.buffer.slice(optimalSplitLength * format.bytesPerSample));
    console.log("Returning split packets:", firstSplit.length, secondSplit.length);
    return [firstSplit, secondSplit];
}

function pushAudioPacket(data) {
    packetQueue.push(new SampleArray(data));
}

function shiftAudioPacket() {
    var data = joinAudioPackets(packetQueue);
    if (!data) return null;
    packetQueue = splitAudioPacket(data);
    console.log(packetQueue);
    return packetQueue.shift();
}

function toAudioBuffer(data) {
    var samples = data.length / format.channels;
    var packetTime = context.currentTime;
    if (nextPacketTime < packetTime) nextPacketTime = packetTime;
    var audioBuffer = context.createBuffer(format.channels, samples, format.rate);
    for (var channel = 0; channel < format.channels; channel++) {
        var audioData = audioBuffer.getChannelData(channel);
        var offset = channel;
        for (var i = 0; i < samples; i++) {
            audioData[i] = data[offset] / maxSampleValue;
            offset += format.channels;
        }
    }
    return audioBuffer;
}

function playReceivedAudio(data) {
    pushAudioPacket(new SampleArray(data));
    var packet = shiftAudioPacket();
    if (!packet) {
        console.log("shifting");
        return;
    }
    var packetTime = context.currentTime;
    if (nextPacketTime < packetTime) nextPacketTime = packetTime;
    var source = context.createBufferSource();
    source.connect(context.destination);
    if (!source.start) source.start = source.noteOn;
    source.buffer = toAudioBuffer(packet);
    source.start(nextPacketTime);
    nextPacketTime += packet.length / format.channels / format.rate;
}

let audioContext = new (window.AudioContext || window.webkitAudioContext)();
let bufferQueue = [];

function append_pcm_data(pcmData) {
    const data = new Float16Array(pcmData.buffer || pcmData);
    bufferQueue.push(...data);
    playRaw16BitLittleEndianPCM();
}

function playRaw16BitLittleEndianPCM(sampleRate = 44800, channels = 2) {
    const frameCount = sampleRate * channels * 0.5;
    if (bufferQueue.length >= frameCount) {
        const audioBuffer = audioContext.createBuffer(channels, frameCount / channels, sampleRate);
        for (let channel = 0; channel < channels; channel++) {
            const channelData = audioBuffer.getChannelData(channel);
            for (let i = 0; i < channelData.length; i++) {
                channelData[i] = bufferQueue[i * channels + channel] || 0;
            }
        }
        const source = audioContext.createBufferSource();
        source.buffer = audioBuffer;
        source.connect(audioContext.destination);
        source.start();
        bufferQueue = bufferQueue.slice(frameCount);
    }
}

export default class WebSocketClient {
    constructor(url, options = {}, obj) {
        this.url = url;
        this.reconnectInterval = options.reconnectInterval || 5000;
        this.autoReconnect = options.autoReconnect !== false;
        this.eventListeners = {};
        this.socket = null;
        this.reconnectTimeout = null;
        this.obj = obj;
        this.stopInput = false;
        this.RemoteInputLock = null;
        this.status = null;
        this.agentDisconnectedShown = false;
        this.gracefulClose = false;
        this.connect();
        this.WebSocket_Monitor();
    }

    async connect() {
        this.socket = new WebSocket(this.url);

        this.socket.onopen = () => {
            console.log('WebSocket connected:', this.url);
            window.websocket = this.socket;
            status = connected_status;

            const handshake = new Uint8Array([0, 73, 0, 6, 0, 1]);
            this.socket.send(handshake);
            this.emit('open');
        };

        let xMouseCursorCurrent = 'default';

        this.socket.onmessage = async (event) => {
            if (event.data instanceof Blob) {
                blobToBytes(event.data).then(view => {

                    if (view.length === 1 && view[0] === 0x63) {

                        if (!this.gracefulClose && !this.agentDisconnectedShown) {
                            this.showPopupMessage("Agent Disconnected", 2000);
                            if (this.obj && this.obj.onAgentDisconnected) {
                                this.obj.onAgentDisconnected();
                            }
                            this.agentDisconnectedShown = true;
                        }
                        return;
                    }


                    cmd = (view[0] << 8) + view[1];
                    cmdsize = (view[2] << 8) + view[3];

                    if (cmd === 7) {
                        const width = (view[4] << 8) + view[5];
                        const height = (view[6] << 8) + view[7];
                        console.log("Received screen resolution:", width, height);
                        this.obj.Monitor.ProcessScreenMsg(width, height);
                        this.send(String.fromCharCode(0x00, 0x0E, 0x00, 0x04));
                        return;
                    }

                    if ((cmd === 3) || (cmd === 4)) {

                        if (cmd === 3 && view.length >= 10) {
                            const codecType = view[4];

                            if (codecType >= 1 && codecType <= 4) {
                                X = 0;
                                Y = 0;
                            } else {

                                X = (view[4] << 8) + view[5];
                                Y = (view[6] << 8) + view[7];
                            }
                        } else {

                            X = (view[4] << 8) + view[5];
                            Y = (view[6] << 8) + view[7];
                        }
                    }

                    switch (cmd) {
                        case 59:
                            this.agentDisconnectedShown = false;
                            this.showPopupMessage("Agent connected", 5000);
                            document.getElementById("refresh").click();
                            this.obj.Monitor.SendRefresh();
                            this.obj.Monitor.onResize();


                            break;

                        case 3:
                            if (this.obj.Monitor.FirstDraw && !this.obj.Monitor.onResize()) status = no_agent;
                            this.obj.Monitor.ProcessPictureMsg(view, X, Y);
                            this.lastscreen = new Date();
                            break;

                        case 11:
                            let selectedDisplay = 0, displays = {}, dcount = (view[4] << 8) + view[5];
                            if (dcount > 0) {
                                selectedDisplay = (view[6 + (dcount * 2)] << 8) + view[7 + (dcount * 2)];
                                for (let i = 0; i < dcount; i++) {
                                    let disp = (view[6 + (i * 2)] << 8) + view[7 + (i * 2)];
                                    displays[disp] = (disp === 65535) ? 'All Displays' : 'Display ' + disp;
                                }
                            }
                            this.obj.Monitor.displays = displays;
                            this.obj.Monitor.selectedDisplay = selectedDisplay;
                            if (this.obj.Monitor.onDisplayinfo != null) {
                                this.obj.Monitor.onDisplayinfo(obj, displays, selectedDisplay);
                            }
                            break;

                        case 18:
                            if ((cmdsize !== 5) || (this.obj.Keyboard.KeyboardState === view[4])) break;
                            this.obj.Keyboard.KeyboardState = view[4];
                            if (this.obj.Keyboard.onKeyboardStateChanged) {
                                this.obj.Keyboard.onKeyboardStateChanged(this.obj.event.KeyboardState);
                            }
                            break;

                        case 27:
                            view = view.slice(8);
                            cmd = view[0];
                            X = (view[4] << 8) + view[5];
                            Y = (view[6] << 8) + view[7];
                            this.obj.Monitor.ProcessPictureMsg(view, X, Y, true);
                            break;

                        case 82:
                            if ((cmdsize < 4) || (((cmdsize - 4) % 10) !== 0)) break;
                            let screenCount = ((cmdsize - 4) / 10), screenInfo = {}, ptr = 4;
                            for (let i = 0; i < screenCount; i++) {
                                screenInfo[(view[ptr] << 8) + view[ptr + 1]] = {
                                    x: (view[ptr + 2] << 8) + view[ptr + 3],
                                    y: (view[ptr + 4] << 8) + view[ptr + 5],
                                    w: (view[ptr + 6] << 8) + view[ptr + 7],
                                    h: (view[ptr + 8] << 8) + view[ptr + 9]
                                };
                                ptr += 10;
                            }
                            break;

                        case 87:
                            if (cmdsize !== 5) break;
                            let lock = (view[4] !== 0);
                            if (this.RemoteInputLock === null || this.RemoteInputLock !== lock) {
                                this.RemoteInputLock = lock;
                                if (this.onRemoteInputLockChanged) {
                                    this.onRemoteInputLockChanged(lock);
                                }
                            }
                            break;

                        case 88:
                            if ((cmdsize !== 5) || this.stopInput) break;
                            let cursorNum = view[4];
                            if (cursorNum > this.obj.Mouse.mouseCursors.length) cursorNum = 0;
                            xMouseCursorCurrent = this.obj.Mouse.mouseCursors[cursorNum];
                            if (this.obj.Mouse && typeof this.obj.Mouse.setRemoteCursor === "function") {
                                this.obj.Mouse.setRemoteCursor(xMouseCursorCurrent);
                            } else if (this.obj.Mouse.xMouseCursorActive && this.obj.Mouse.CanvasId) {
                                this.obj.Mouse.CanvasId.style.cursor = xMouseCursorCurrent;
                            }
                            break;

                        case 90:
                            let canvas = document.getElementById("desktop-view");
                            if (canvas && canvas.style.display === "block") {
                                canvas.style.display = "none";
                            }
                            break;

                        case 92:
                            view = view.slice(2);
                            console.log(view);
                            append_pcm_data(view);
                            break;

                        default:

                    }
                });
            }
        };

        this.socket.onerror = (error) => {
            console.error('WebSocket error:', error);
            this.emit('error', error);
        };

        this.socket.onclose = (event) => {
            console.warn('WebSocket closed:', event);
            this.emit('close', event);
            if (this.autoReconnect && event.code !== 1000) {
                this.reconnect();
            }
        };
    }

    WebSocket_Monitor() {
        setInterval(() => {
            const element = document.getElementById("conn_status");
            if (element) {
                element.style.backgroundColor = status;
            } else {
                console.warn('Element with ID "conn_status" not found.');
            }
        }, 10000);
    }
    showPopupMessage(message, duration = 5000) {
        const popup = document.createElement("div");
        popup.textContent = message;
        popup.style.position = "fixed";
        popup.style.top = "20px";
        popup.style.right = "20px";
        popup.style.backgroundColor = "rgba(0, 0, 0, 0.8)";
        popup.style.color = "#fff";
        popup.style.padding = "10px 20px";
        popup.style.borderRadius = "8px";
        popup.style.zIndex = 9999;
        popup.style.fontFamily = "Arial, sans-serif";
        popup.style.fontSize = "14px";
        popup.style.boxShadow = "0 0 10px rgba(0,0,0,0.5)";
        popup.style.opacity = "1";
        popup.style.transition = "opacity 0.5s ease";

        document.body.appendChild(popup);

        setTimeout(() => {
            popup.style.opacity = "0";
            setTimeout(() => {
                if (popup.parentNode) popup.parentNode.removeChild(popup);
            }, 500);
        }, duration);
    }


    reconnect() {
        if (this.reconnectTimeout) return;
        console.log(`Reconnecting in ${this.reconnectInterval / 1000} seconds...`);
        this.reconnectTimeout = setTimeout(() => {
            if (reconnect_counter > 10) {
                status = reconnect_failed;
                alert("server is away contact system support.");
                window.close();
            } else {
                status = reconnecting;
            }
            this.reconnectTimeout = null;
            this.connect();
        }, this.reconnectInterval);
        reconnect_counter += 1;
    }

    send(data) {
        if (this.socket.readyState === WebSocket.OPEN) {
            this.socket.send(data);
        } else {
            console.warn('WebSocket is not open. Message not sent.');
        }
    }

    close() {
        this.gracefulClose = true;
        this.autoReconnect = false;
        this.socket.close();
    }

    on(event, callback) {
        if (!this.eventListeners[event]) {
            this.eventListeners[event] = [];
        }
        this.eventListeners[event].push(callback);
    }

    emit(event, data) {
        if (this.eventListeners[event]) {
            this.eventListeners[event].forEach(callback => callback(data));
        }
    }
}
