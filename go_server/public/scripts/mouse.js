
var KeyAction = { "NONE": 0, "DOWN": 1, "UP": 2, "SCROLL": 3, "EXUP": 4, "EXDOWN": 5, "DBLCLICK": 6 };
var MouseButton = {
    "NONE": 0x00,
    "LEFT": 0x02,
    "RIGHT": 0x08,
    "MIDDLE": 0x20,
    "BACK": 0x05,
    "FORWARD": 0x06
};
var SwapMouse = false
var ReverseMouseWheel = false
var InputType = { "KEY": 1, "MOUSE": 2, "CTRLALTDEL": 10, "TOUCH": 15, "KEYUNICODE": 85 };
var Alternate = 0;
var mouseCursors = ['default', 'progress', 'crosshair', 'pointer', 'help', 'text', 'no-drop', 'move', 'nesw-resize', 'ns-resize', 'nwse-resize', 'w-resize', 'alias', 'wait', 'none', 'not-allowed', 'col-resize', 'row-resize', 'copy', 'zoom-in', 'zoom-out'];
var socket
var prev = false;
var move;
var lastMouseMove;

const DOM_DELTA_PIXEL = 0;
const DOM_DELTA_LINE = 1;
const DOM_DELTA_PAGE = 2;
const EDGE_THRESHOLD_PX = 8;

function haltEvent(e) { if (e.preventDefault) e.preventDefault(); if (e.stopPropagation) e.stopPropagation(); return false; }

function GetPositionOfControl(Control) {
    var Position = Array(2);
    Position[0] = Position[1] = 0;
    while (Control) { Position[0] += Control.offsetLeft; Position[1] += Control.offsetTop; Control = Control.offsetParent; }
    return Position;
}

function doubleToByteArray(num) {
    let buffer = new ArrayBuffer(8);
    let view = new DataView(buffer);
    view.setFloat64(0, num, true);
    return new Uint8Array(buffer);
}

function int16ToBytes(value) {
    const clamped = Math.max(-32768, Math.min(32767, Math.round(value)));
    return [((clamped >> 8) & 0xFF), (clamped & 0xFF)];
}

function normalizeWheelAxis(deltaValue, wheelDeltaValue, detailValue, deltaMode) {
    if (typeof deltaValue === "number" && deltaValue !== 0) {
        let factor;
        switch (deltaMode) {
            case DOM_DELTA_LINE:
                factor = 1;
                break;
            case DOM_DELTA_PAGE:
                factor = 10;
                break;
            default:
                factor = 1 / 120;
                break;
        }
        return deltaValue * factor;
    }
    if (typeof wheelDeltaValue === "number" && wheelDeltaValue !== 0) {
        return -wheelDeltaValue / 120;
    }
    if (typeof detailValue === "number" && detailValue !== 0) {
        return detailValue;
    }
    return 0;
}

function SendMouseMsg(Action, event) {

    var CanvasId = document.getElementById("Desk")
    var Canvas = CanvasId.getContext("2d")
    if (Action != null) {
        if (!event) { var event = window.event; }
        var ScaleFactorHeight = (Canvas.canvas.height / CanvasId.clientHeight);
        var ScaleFactorWidth = (Canvas.canvas.width / CanvasId.clientWidth);
        var Offsets = GetPositionOfControl(Canvas.canvas);
        var X = ((event.pageX - Offsets[0]) * ScaleFactorWidth);
        var Y = ((event.pageY - Offsets[1]) * ScaleFactorHeight);
        if (event.addx) { X += event.addx; }
        if (event.addy) { Y += event.addy; }


        if (X >= 0 && X <= Canvas.canvas.width && Y >= 0 && Y <= Canvas.canvas.height) {
            const xInt = Math.max(0, Math.min(65535, Math.round(X)));
            const yInt = Math.max(0, Math.min(65535, Math.round(Y)));
            let buttonValue = MouseButton.NONE;
            let deltaY = 0;
            let deltaX = 0;

            if (Action === KeyAction.UP || Action === KeyAction.DOWN) {
                if (typeof event.button === 'number') {
                    switch (event.button) {
                        case 0:
                            buttonValue = MouseButton.LEFT;
                            break;
                        case 1:
                            buttonValue = MouseButton.MIDDLE;
                            break;
                        case 2:
                            buttonValue = MouseButton.RIGHT;
                            break;
                        case 3:
                            buttonValue = MouseButton.BACK;
                            break;
                        case 4:
                            buttonValue = MouseButton.FORWARD;
                            break;
                        default:
                            buttonValue = MouseButton.NONE;
                    }
                } else if (event.which) {
                    buttonValue = (event.which === 1)
                        ? MouseButton.LEFT
                        : (event.which === 2)
                            ? MouseButton.MIDDLE
                            : (event.which === 3)
                                ? MouseButton.RIGHT
                                : MouseButton.NONE;
                }
            } else if (Action === KeyAction.SCROLL) {
                const deltaMode = typeof event.deltaMode === "number" ? event.deltaMode : DOM_DELTA_PIXEL;
                const wheelDeltaY = typeof event.wheelDeltaY === "number"
                    ? event.wheelDeltaY
                    : (typeof event.wheelDelta === "number" ? event.wheelDelta : 0);
                deltaY = normalizeWheelAxis(event.deltaY, wheelDeltaY, event.detail, deltaMode);
                const wheelDeltaX = typeof event.wheelDeltaX === "number" ? event.wheelDeltaX : 0;
                deltaX = normalizeWheelAxis(event.deltaX, wheelDeltaX, 0, deltaMode);
                if (ReverseMouseWheel) {
                    deltaY = -deltaY;
                    deltaX = -deltaX;
                }
            }

            if (SwapMouse) {
                if (buttonValue === MouseButton.LEFT) {
                    buttonValue = MouseButton.RIGHT;
                } else if (buttonValue === MouseButton.RIGHT) {
                    buttonValue = MouseButton.LEFT;
                }
            }

            let buffer = null;

            if (Action === KeyAction.DBLCLICK) {
                buffer = new ArrayBuffer(10);
                let view = new DataView(buffer);
                view.setUint8(0, 0x00);
                view.setUint8(1, InputType.MOUSE);
                view.setUint8(2, 0x00);
                view.setUint8(3, 0x0A);
                view.setUint8(4, 0x00);
                view.setUint8(5, 0x88);
                view.setUint8(6, (xInt >> 8) & 0xFF);
                view.setUint8(7, xInt & 0xFF);
                view.setUint8(8, (yInt >> 8) & 0xFF);
                view.setUint8(9, yInt & 0xFF);
            } else if (Action === KeyAction.SCROLL) {
                const [deltaYHigh, deltaYLow] = int16ToBytes(deltaY);
                const [deltaXHigh, deltaXLow] = int16ToBytes(deltaX);

                buffer = new ArrayBuffer(14);
                let view = new DataView(buffer);
                view.setUint8(0, 0x00);
                view.setUint8(1, InputType.MOUSE);
                view.setUint8(2, 0x00);
                view.setUint8(3, 0x0E);
                view.setUint8(4, 0x00);
                view.setUint8(5, 0x00);
                view.setUint8(6, (xInt >> 8) & 0xFF);
                view.setUint8(7, xInt & 0xFF);
                view.setUint8(8, (yInt >> 8) & 0xFF);
                view.setUint8(9, yInt & 0xFF);
                view.setUint8(10, deltaYHigh);
                view.setUint8(11, deltaYLow);
                view.setUint8(12, deltaXHigh);
                view.setUint8(13, deltaXLow);
            } else {
                buffer = new ArrayBuffer(10);
                let view = new DataView(buffer);
                view.setUint8(0, 0x00);
                view.setUint8(1, InputType.MOUSE);
                view.setUint8(2, 0x00);
                view.setUint8(3, 0x0A);
                view.setUint8(4, 0x00);
                const buttonByte = (Action === KeyAction.DOWN)
                    ? buttonValue
                    : ((buttonValue * 2) & 0xFF);
                view.setUint8(5, buttonByte);
                view.setUint8(6, (xInt >> 8) & 0xFF);
                view.setUint8(7, xInt & 0xFF);
                view.setUint8(8, (yInt >> 8) & 0xFF);
                view.setUint8(9, yInt & 0xFF);
            }

            socket.send(buffer);
        }
    }
}

export default class Mouse {
    constructor(canvasId, sock) {
        this.canvasId = canvasId;
        this.CanvasId = canvasId;
        socket = sock;
        this.mouseCursors = mouseCursors;
        this.KeyAction = KeyAction;
        this.remoteCursor = canvasId.style.cursor || "default";
        this.isEdgeCursorActive = false;
        this.activeEdgeCursor = null;
        this.edgeThreshold = EDGE_THRESHOLD_PX;
        this.xMouseCursorActive = true;

        this.mousedown = this.mousedown.bind(this);
        this.mouseup = this.mouseup.bind(this);
        this.mousemove = this.mousemove.bind(this);
        this.mousewheel = this.mousewheel.bind(this);
        this.doubleclick = this.doubleclick.bind(this);
        this.handleMouseLeave = this.handleMouseLeave.bind(this);
        this.showResizeIndicator = this.showResizeIndicator.bind(this);
        this.hideResizeIndicator = this.hideResizeIndicator.bind(this);

        this.initEventHandler();
    }

    initEventHandler() {
        this.canvasId.addEventListener("mousedown", this.mousedown);
        this.canvasId.addEventListener("mouseup", this.mouseup);
        this.canvasId.addEventListener("mousemove", this.mousemove);
        this.canvasId.addEventListener("wheel", this.mousewheel);
        this.canvasId.addEventListener("dblclick", this.doubleclick);
        this.canvasId.addEventListener("mouseleave", this.handleMouseLeave);

        this.resizeIndicator = document.createElement("div");
        this.resizeIndicator.className = "mouse-resize-indicator";
        Object.assign(this.resizeIndicator.style, {
            position: "fixed",
            pointerEvents: "none",
            zIndex: 9999,
            fontSize: "18px",
            lineHeight: "18px",
            transform: "translate(12px, 12px)",
            userSelect: "none",
            display: "none",
        });
        document.body.appendChild(this.resizeIndicator);
    }

    mousedown(event) {
        this.updateResizeCursor(event);
        SendMouseMsg(KeyAction.DOWN, event);
    }

    mouseup(event) {
        this.updateResizeCursor(event);
        SendMouseMsg(KeyAction.UP, event);
    }

    mousemove(event) {
        this.updateResizeCursor(event);
        SendMouseMsg(KeyAction.NONE, event);
    }

    mousewheel(event) {
        haltEvent(event);
        this.updateResizeCursor(event);
        SendMouseMsg(KeyAction.SCROLL, event);
    }

    doubleclick(event) {
        SendMouseMsg(KeyAction.DBLCLICK, event);
    }

    handleMouseLeave() {
        if (this.isEdgeCursorActive) {
            this.isEdgeCursorActive = false;
            this.activeEdgeCursor = null;
            this.canvasId.style.cursor = this.remoteCursor;
        }
        this.hideResizeIndicator();
    }

    updateResizeCursor(event) {
        if (!this.canvasId) {
            return;
        }
        const rect = this.canvasId.getBoundingClientRect();
        const x = event.clientX - rect.left;
        const y = event.clientY - rect.top;
        const threshold = this.edgeThreshold;

        const nearLeft = x >= 0 && x <= threshold;
        const nearRight = x >= rect.width - threshold && x <= rect.width;
        const nearTop = y >= 0 && y <= threshold;
        const nearBottom = y >= rect.height - threshold && y <= rect.height;

        let nextCursor = null;
        let indicatorSymbol = null;
        if ((nearLeft && nearTop) || (nearRight && nearBottom)) {
            nextCursor = "nwse-resize";
            indicatorSymbol = "↘️";
        } else if ((nearRight && nearTop) || (nearLeft && nearBottom)) {
            nextCursor = "nesw-resize";
            indicatorSymbol = "↙️";
        } else if (nearLeft || nearRight) {
            nextCursor = "ew-resize";
            indicatorSymbol = "↔️";
        } else if (nearTop || nearBottom) {
            nextCursor = "ns-resize";
            indicatorSymbol = "↕️";
        }

        if (nextCursor) {
            if (!this.isEdgeCursorActive || this.activeEdgeCursor !== nextCursor) {
                this.isEdgeCursorActive = true;
                this.activeEdgeCursor = nextCursor;
                this.canvasId.style.cursor = nextCursor;
            }
            this.showResizeIndicator(indicatorSymbol, event.clientX, event.clientY);
        } else if (this.isEdgeCursorActive) {
            this.isEdgeCursorActive = false;
            this.activeEdgeCursor = null;
            this.canvasId.style.cursor = this.remoteCursor;
            this.hideResizeIndicator();
        } else {
            this.hideResizeIndicator();
        }
    }

    showResizeIndicator(symbol, clientX, clientY) {
        if (!this.resizeIndicator) {
            return;
        }
        if (!symbol) {
            this.hideResizeIndicator();
            return;
        }
        this.resizeIndicator.textContent = symbol;
        this.resizeIndicator.style.left = `${clientX}px`;
        this.resizeIndicator.style.top = `${clientY}px`;
        this.resizeIndicator.style.display = "block";
    }

    hideResizeIndicator() {
        if (this.resizeIndicator) {
            this.resizeIndicator.style.display = "none";
        }
    }

    setRemoteCursor(cursorName) {
        const resolved = cursorName || "default";
        this.remoteCursor = resolved;
        if (!this.isEdgeCursorActive) {
            this.hideResizeIndicator();
            this.canvasId.style.cursor = resolved;
        }
    }
}

