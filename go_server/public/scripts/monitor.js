import { httpClient } from "./httpClient.js";
import { VideoDecoder } from "./videoDecoder.js";

var self;
var ImageType = "webp"

let activeMonitor = null;

export function getActiveMonitor() {
    return activeMonitor;
}

export default class Monitor {
    constructor(canvas, socket, imagetype, keyboard) {

        if (canvas && canvas.canvas) {

            this.ctx = canvas;
            this.CanvasElement = canvas.canvas;
        } else if (canvas && canvas.getContext) {

            this.ctx = canvas.getContext('2d');
            this.CanvasElement = canvas;
        } else if (canvas && typeof canvas === 'object' && canvas.constructor.name === 'CanvasRenderingContext2D') {

            this.ctx = canvas;
            this.CanvasElement = canvas.canvas;
        } else {

            console.warn(' Invalid canvas parameter, attempting to find canvas by ID');
            this.CanvasElement = document.getElementById('DeskArea') || document.getElementById('Desk');
            this.ctx = this.CanvasElement ? this.CanvasElement.getContext('2d') : null;
        }

        this.Canvas = this.ctx;

        this.ImageType = imagetype || 'webp';
        this.CompressionLevel = 100;
        this.ScalingLevel = 1024;
        this.FrameRateTimer = 100;
        this.State = 1;
        this.PendingOperations = []
        this.tilesReceived = 0
        this.TilesDrawn = 0
        this.KillDraw = 0
        this.width = 0;
        this.height = 0;
        this.onPreDrawImage = null
        this.Socket = socket

        this.debugmode = localStorage.getItem('debug') === '1' ? 2 : 0
        this.displays = null
        this.selectedDisplay = null;
        this.onDisplayinfo = null;
        this.keyboard = keyboard
        this.onScreenSizeChange = null

        this.videoDecoder = new VideoDecoder(this.CanvasElement);
        this.isVideoMode = false;

        this.initEventHandler()

        self = this
        activeMonitor = this;
    }
    send(message) {
        this.Socket.send(message);
    }
    shortToStr(x) { return String.fromCharCode((x >> 8) & 0xFF, x & 0xFF); }
    rotX(x, y) {

        if (this.rotation == 0 || this.rotation == 1) return x;
        if (this.rotation == 2) return x - this.Canvas.canvas.width;
        if (this.rotation == 3) return x - this.Canvas.canvas.height;
    }
    rotY(x, y) {
        if (this.rotation == 0 || this.rotation == 3) return y;
        if (this.rotation == 1) return y - this.Canvas.canvas.width;
        if (this.rotation == 2) return y - this.Canvas.canvas.height;
    }

    ProcessCopyRectMsg = function (str) {
        const SX = ((str.charCodeAt(0) & 0xFF) << 8) + (str.charCodeAt(1) & 0xFF);
        const SY = ((str.charCodeAt(2) & 0xFF) << 8) + (str.charCodeAt(3) & 0xFF);
        const DX = ((str.charCodeAt(4) & 0xFF) << 8) + (str.charCodeAt(5) & 0xFF);
        const DY = ((str.charCodeAt(6) & 0xFF) << 8) + (str.charCodeAt(7) & 0xFF);
        const WIDTH = ((str.charCodeAt(8) & 0xFF) << 8) + (str.charCodeAt(9) & 0xFF);
        const HEIGHT = ((str.charCodeAt(10) & 0xFF) << 8) + (str.charCodeAt(11) & 0xFF);

        if (this.Canvas && this.Canvas.drawImage) {

            this.Canvas.drawImage(this.CanvasElement, SX, SY, WIDTH, HEIGHT, DX, DY, WIDTH, HEIGHT);
        }
    }


    processPendingPackets() {
        if (this.PendingOperations.length === 0) return false;

        for (let i = 0; i < this.PendingOperations.length; i++) {
            const Msg = this.PendingOperations[i];
            if (this.onPreDrawImage) { this.onPreDrawImage(); }


            const ctx = this.Canvas;
            if (!ctx) { console.warn('No drawing context'); return false; }

            if (Msg[1] === 1) {
                this.ProcessCopyRectMsg(Msg[2]);
            } else if (Msg[1] === 2) {
                ctx.drawImage(Msg[2], this.rotX(Msg[3], Msg[4]), this.rotY(Msg[3], Msg[4]));

                delete Msg[2];
            } else if (Msg[1] === 3) {
                const pattern = ctx.createPattern(Msg[2], 'repeat');
                if (pattern) {
                    ctx.fillStyle = pattern;
                    ctx.fillRect(this.rotX(Msg[3], Msg[4]), this.rotY(Msg[3], Msg[4]), this.CanvasElement.width, this.CanvasElement.height);
                }
            }

            this.PendingOperations.splice(i, 1);
            this.TilesDrawn++;
            if ((this.TilesDrawn === this.tilesReceived) && (this.KillDraw < this.TilesDrawn)) {
                this.KillDraw = this.TilesDrawn = this.tilesReceived = 0;
            }
            return true;
        }

        if (this.oldie && this.PendingOperations.length > 0) { this.TilesDrawn++; }
        return false;
    }



    async ProcessPictureMsg(view, X, Y, print = false) {

        if (view.length >= 10 && view[4] !== undefined) {
            const codecType = view[4];

            if (codecType >= 1 && codecType <= 4) {
                const isKeyframe = view[5] === 1;

                this.isVideoMode = true;


                const frameData = view.slice(10);

                if (frameData.length > 0) {

                    await this.videoDecoder.decodeVideoFrame(codecType, frameData, isKeyframe);
                    return;
                } else {
                    console.warn(' Empty video frame data');
                    return;
                }
            }
        }

        this.isVideoMode = false;


        if (!this.ScreenWidth || !this.ScreenHeight) {

            this.PendingOperations.push([++this.tilesReceived, 0, null, X, Y, view]);
            return;
        }
        const tile = new Image();
        const r = ++this.tilesReceived;
        const tdata = view.slice(8);
        let ptr = 0, strs = [];
        while ((tdata.byteLength - ptr) > 50000) {
            strs.push(String.fromCharCode.apply(null, Array.from(tdata.slice(ptr, ptr + 50000))));
            ptr += 50000;
        }
        if (ptr > 0) {
            strs.push(String.fromCharCode.apply(null, Array.from(tdata.slice(ptr))));
        } else {
            strs.push(String.fromCharCode.apply(null, Array.from(tdata)));
        }

        const dtype = this.ImageType || 'webp';
        switch (dtype) {
            case "jpeg":
                tile.src = 'data:image/jpeg;base64,' + btoa(strs.join(''));
                break;
            case "png":
                tile.src = 'data:image/png;base64,' + btoa(strs.join(''));
                break;
            case "tiff":
                tile.src = 'data:image/tiff;base64,' + btoa(strs.join(''));
                break;
            case "webp":
            default:
                tile.src = 'data:image/webp;base64,' + btoa(strs.join(''));
                break;
        }

        tile.onload = () => {
            if ((this.Canvas != null) && (this.KillDraw < r) && (this.State !== 0)) {
                this.PendingOperations.push([r, 2, tile, X, Y]);
                while (this.processPendingPackets()) { }
            } else {

                this.PendingOperations.push([r, 0]);
            }
        };

        tile.onerror = () => { console.log('DecodeTileError for tile ' + r); };
    }

    SendCompressionLevel(type, level, scaling, frametimer) {
        if (type !== undefined && type !== null) {
            this.ImageType = type;
        }
        if (level !== undefined && level !== null) {
            const parsedLevel = parseInt(level, 10);
            if (!Number.isNaN(parsedLevel)) {
                this.CompressionLevel = parsedLevel;
            }
        }
        if (scaling !== undefined && scaling !== null) {
            const parsedScaling = parseInt(scaling, 10);
            if (!Number.isNaN(parsedScaling)) {
                this.ScalingLevel = parsedScaling;
            }
        }
        if (frametimer !== undefined && frametimer !== null) {
            const parsedTimer = parseInt(frametimer, 10);
            if (!Number.isNaN(parsedTimer)) {
                this.FrameRateTimer = parsedTimer;
            }
        }

        const quality = Math.max(0, Math.min(this.CompressionLevel || 0, 100));
        const scalingValue = Math.max(1, Math.min(this.ScalingLevel || 1024, 65535));
        const frameTimerValue = Math.max(1, Math.min(this.FrameRateTimer || 100, 65535));
        const fps = Math.max(1, Math.min(255, Math.round(1000 / frameTimerValue)));

        const packet = new Uint8Array(10);
        packet[0] = 0x00;
        packet[1] = 0x05;
        packet[2] = 0x00;
        packet[3] = 0x0A;
        packet[4] = quality;
        packet[5] = fps;
        packet[6] = (scalingValue >> 8) & 0xff;
        packet[7] = scalingValue & 0xff;
        packet[8] = (frameTimerValue >> 8) & 0xff;
        packet[9] = frameTimerValue & 0xff;

        this.send(packet);
    }
    SendUnPause() {
        if (this.debugmode > 1) { console.log('SendUnPause'); }
        this.send(String.fromCharCode(0x00, 0x08, 0x00, 0x05, 0x00));
    }
    SendRefresh() {
        this.send(String.fromCharCode(0x00, 0x06, 0x00, 0x04));
    }
    SendPause() {
        if (this.debugmode > 1) { console.log('SendPause'); }
        this.send(String.fromCharCode(0x00, 0x08, 0x00, 0x05, 0x01));
    }
    SendRemoteInputLock(code) {
        this.send(String.fromCharCode(0x00, 87, 0x00, 0x05, code));
    }
    SendMouseEnable(state) {

        const buffer = new Uint8Array(5);

        buffer[0] = 0x00;
        buffer[1] = 0x90;

        buffer[2] = 0x00;
        buffer[3] = 0x05;

        buffer[4] = state;
        console.log("Sending MouseEnable:", buffer);

        this.send(buffer);
    }



    SetDisplay(number) { this.send(String.fromCharCode(0x00, 0x0C, 0x00, 0x06, number >> 8, number & 0xFF)); }
    onRemoteInputLockChanged = function (state) { QV('DeskInputLockedButton', state); QV('DeskInputUnLockedButton', !state); }

    ProcessScreenMsg(width, height) {
        if (this.debugmode > 0) { console.log('ScreenSize: ' + width + ' x ' + height); }
        const isValidDimensions = width && height && !isNaN(width) && !isNaN(height) && width > 0 && height > 0;
        if (!isValidDimensions) {
            console.log('Invalid screen dimensions received:', width, 'x', height);
            return;
        }

        if (typeof window.hideConnectionLoader === 'function') { window.hideConnectionLoader(); }

        if ((this.ScreenWidth === width) && (this.ScreenHeight === height)) return;

        if (this.CanvasElement) {
            this.Canvas.setTransform(1, 0, 0, 1, 0, 0);
            this.CanvasElement.width = width;
            this.CanvasElement.height = height;
        }
        this.rotation = 0;
        this.FirstDraw = true;
        this.ScreenWidth = this.width = width;
        this.ScreenHeight = this.height = height;
        this.KillDraw = this.tilesReceived;
        this.PendingOperations = [];

        this.SendCompressionLevel(this.ImageType, 100, 1024, 100);
        this.SendRemoteInputLock(2);

        try { this.onResize(); } catch (e) { console.warn('onResize failed after ProcessScreenMsg', e); }

        if (this.onScreenSizeChange) { this.onScreenSizeChange(this.ScreenWidth, this.ScreenHeight, this.CanvasId); }
    }

    onResize() {

        if (this.ScreenWidth == undefined || this.ScreenHeight == undefined ||
            this.ScreenWidth <= 0 || this.ScreenHeight <= 0 ||
            isNaN(this.ScreenWidth) || isNaN(this.ScreenHeight)) {
            console.log("onResize: Invalid dimensions - " + this.ScreenWidth + " x " + this.ScreenHeight);
            return false;
        }

        if (this.debugmode > 1) { console.log("onResize: " + this.ScreenWidth + " x " + this.ScreenHeight); }

        if (this.FirstDraw) {
            this.Canvas.canvas.width = this.ScreenWidth;
            this.Canvas.canvas.height = this.ScreenHeight;
            this.Canvas.fillRect(0, 0, this.ScreenWidth, this.ScreenHeight);

            if (this.onScreenSizeChange != null) { this.onScreenSizeChange(this, this.ScreenWidth, this.ScreenHeight, this.CanvasId); }
        }
        this.FirstDraw = false;
        const loader = document.getElementById("load-gif-box");
        if (loader) loader.style.display = "none";

        if (typeof window.hideConnectionLoader === 'function') {
            window.hideConnectionLoader();
        }

        console.log("resize completed");
        self.SendRefresh();

        return true;
    }

    initEventHandler() {

        document.getElementById("refresh").addEventListener("click", function () {
            self.SendRefresh()
            self.onResize()
            const params = new URLSearchParams(window.location.search);
            self.SetDisplay(params.get("view") || 0);
        });

        document.getElementById("EI").addEventListener("click", function () {
            document.getElementById("quality-control").style.display = "block";
        });

        document.getElementById("slider").addEventListener("input", function () {
            document.getElementById("sliderValue").textContent = this.value;
        });
        document.getElementById("update-compression").addEventListener("click", function () {
            let typeSelecter = document.getElementById("dropdown").value;
            let quality = document.getElementById("slider").value;
            ImageType = typeSelecter
            self.SendCompressionLevel(ImageType, quality, 1024, 50);

            document.getElementById("quality-control").style.display = "none";

        });
        document.getElementById("EnableMouse").addEventListener("change", function () {
            if (this.checked) {
                self.SendMouseEnable(1);
            } else {
                self.SendMouseEnable(0);
            }
        });




    }


}

export function updateEncoderSettings(encoder, quality) {
    if (httpClient) {
        httpClient.makeRequest('/encoder/settings', {
            method: 'POST',
            body: JSON.stringify({
                encoder: encoder,
                quality: parseInt(quality)
            })
        }).then(() => {

        }).catch(error => {
            showStatus(`Failed to update encoder settings: ${error.message}`, "error");
        });
    } else {
        showStatus("HTTP client not initialized", "error");
    }
}


export function toggleFullScreen() {
    if (!document.fullscreenElement) {
        document.documentElement.requestFullscreen();
    } else {
        document.exitFullscreen();
    }
}
export function setupQualityControl() {
    const dropdown = document.getElementById("encoder-dropdown");
    const slider = document.getElementById("quality-slider");
    const sliderValue = document.getElementById("slider-value");
    const updateBtn = document.getElementById("update-settings-btn");

    if (slider && sliderValue) {
        slider.addEventListener("input", () => {
            sliderValue.textContent = slider.value;
        });
    }

    if (updateBtn && dropdown && slider) {
        updateBtn.addEventListener("click", () => {
            const encoder = dropdown.value;
            const quality = slider.value;
            updateEncoderSettings(encoder, quality);
        });
    }
}
