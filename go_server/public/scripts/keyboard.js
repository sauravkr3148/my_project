const ControlLeftKc = 17;
const AltGrKc = 225;

function isWindowsBrowser() {
    return navigator && !!(/win/i).exec(navigator.platform);
}

function haltEvent(e) {
    if (e.preventDefault) e.preventDefault();
    if (e.stopPropagation) e.stopPropagation();
    return false;
}

const directUnicodeChars = {
    "Semicolon": ";", "Equal": "=", "Comma": ",", "Minus": "-",
    "Period": ".", "Slash": "/", "Backquote": "`",
    "BracketLeft": "[", "Backslash": "\\", "BracketRight": "]", "Quote": "'"
};

export default class Keyboard {
    constructor(debug, socket) {
        this.isWindowsBrowser = isWindowsBrowser();
        this.debugmode = debug;
        this.pressedKeys = [];
        this.Socket = socket;
        this.InputType = { "KEY": 1, "MOUSE": 2, "CTRLALTDEL": 10, "TOUCH": 15, "KEYUNICODE": 85 };
        this.UseExtendedKeyFlag = true;
        this.extendedKeyTable = ['ShiftRight', 'AltRight', 'ControlRight', 'Home', 'End', 'Insert', 'Delete', 'PageUp', 'PageDown', 'NumpadDivide', 'NumpadEnter', 'NumLock', 'Pause'];
        this._altGrArmed = false;
        this.localKeyMap = true;
        this._keyDownList = {};
        this.KeyboardState = 0;
        this.remoteKeyMap = true;
        this.State = 3;
        this.KeyAction = { "NONE": 0, "DOWN": 1, "UP": 2, "SCROLL": 3, "EXUP": 4, "EXDOWN": 5, "DBLCLICK": 6 };

        this.initEventHandlers();
    }

    ShortToStr(x) {
        return String.fromCharCode((x >> 8) & 0xFF, x & 0xFF);
    }

    isDesktopViewActive() {
        const desktopView = document.getElementById("desktop-view");
        if (desktopView) {
            const visible = desktopView.style.display !== "none";
            if (visible) return true;
        }

        const canvas = document.getElementById("Desk") || document.getElementById("DeskArea");
        if (canvas) {
            const style = window.getComputedStyle(canvas);
            if (style && style.display !== "none" && style.visibility !== "hidden") return true;
        }

        const tab = document.getElementById("tab-desktop");
        return !!(tab && tab.classList && tab.classList.contains("selected"));
    }

    SendKeyMsgKC(action, kc, extendedKey) {
        if (typeof action == 'object') {
            for (var i in action) {
                this.SendKeyMsgKC(action[i][0], action[i][1], action[i][2]);
            }
        } else {
            if (action == 1 && this.pressedKeys.indexOf(kc) === -1) {
                this.pressedKeys.unshift(kc);
            } else if (action == 2) {
                const i = this.pressedKeys.indexOf(kc);
                if (i !== -1) this.pressedKeys.splice(i, 1);
            }

            const up = (action - 1) + (extendedKey ? 2 : 0);
            this.Socket.send(String.fromCharCode(0x00, this.InputType.KEY, 0x00, 0x06, up, kc));
        }
    }

    convertKeyCode(e) {
        if (e.code.startsWith('Key') && e.code.length === 4) return e.code.charCodeAt(3);
        if (e.code.startsWith('Digit') && e.code.length === 6) return e.code.charCodeAt(5);
        if (e.code.startsWith('Numpad') && e.code.length === 7) return e.code.charCodeAt(6) + 48;
        return null;
    }

    sendKeyComboToAgent(comboId) {
        const MSG_TYPE = 0x9999;
        const buffer = new Uint8Array(5);
        buffer[0] = (MSG_TYPE >> 8) & 0xFF;
        buffer[1] = MSG_TYPE & 0xFF;
        buffer[2] = 0x00;
        buffer[3] = 0x05;
        buffer[4] = comboId;
        this.Socket.send(buffer);
    }

    checkAltGr(obj, event, action) {
        if (this._altGrArmed) {
            this._altGrArmed = false;
            clearTimeout(this._altGrTimeout);
            if ((event.code === "AltRight") && ((event.timeStamp - this._altGrCtrlTime) < 50)) {
                this.SendKeyMsgKC(action, AltGrKc, false);
                return true;
            }
        }
        if ((event.code === "ControlLeft") && !(ControlLeftKc in this.pressedKeys)) {
            this._altGrArmed = true;
            this._altGrCtrlTime = event.timeStamp;
        }
        return false;
    }

    SendKeyMsg(action, event) {
        if (!event) event = window.event;

        let extendedKey = this.UseExtendedKeyFlag &&
            typeof event.code === 'string' &&
            (event.code.startsWith('Arrow') || this.extendedKeyTable.includes(event.code));

        if (this.isWindowsBrowser && this.checkAltGr(this.handler, event, action)) return;

        if (!extendedKey && event.code && !event.code.startsWith('NumPad') && !this.localKeyMap) {
            const kc = this.convertKeyCode(event);
            if (kc != null) {
                this.SendKeyMsgKC(action, kc, extendedKey);
                return;
            }
        }

        let kc = event.keyCode;
        if (kc === 0x3B) kc = 0xBA;
        else if (kc === 173) kc = 189;
        else if (kc === 61) kc = 187;
        this.SendKeyMsgKC(action, kc, extendedKey);
    }

    SendKeyUnicode(action, val) {
        if (this.State !== 3) return;
        this.Socket.send(String.fromCharCode(0x00, this.InputType.KEYUNICODE, 0x00, 0x07, action - 1) + this.ShortToStr(val));
    }

    keyup(e) {


        if (e.code === 'Backquote' && e.altKey && !e.ctrlKey && !e.metaKey) {
            this.sendKeyComboToAgent(0);
            return haltEvent(e);
        }

        if (e.code === 'CapsLock' && e.altKey) {
            this.sendKeyComboToAgent(1);
            return haltEvent(e);
        }

        if (e.code === 'CapsLock' && e.ctrlKey) {
            this.sendKeyComboToAgent(2);
            return haltEvent(e);
        }

        if (e.code === 'Space' && e.metaKey) {
            this.sendKeyComboToAgent(3);
            return haltEvent(e);
        }
        if (e.code === 'Backquote' && e.altKey && !e.ctrlKey && !e.metaKey) {
            return haltEvent(e);
        }

        if ((e.altKey || e.ctrlKey || e.metaKey) && directUnicodeChars[e.code]) return haltEvent(e);
        if (e.code === 'Backquote' && !e.altKey && !e.ctrlKey && !e.metaKey) {
            const char = e.shiftKey ? '~' : '`';
            this.SendKeyUnicode(this.KeyAction.UP, char.charCodeAt(0));
            return haltEvent(e);
        }
        if (directUnicodeChars[e.code] && !e.ctrlKey && !e.altKey && !e.metaKey) {
            const char = e.shiftKey ? e.key : directUnicodeChars[e.code];
            this.SendKeyUnicode(this.KeyAction.UP, char.charCodeAt(0));
            return haltEvent(e);
        }
        if (e.code === 'CapsLock' && (e.altKey || e.ctrlKey)) return haltEvent(e);
        if ((e.key !== 'Dead') && this.State === 3) {
            if (typeof e.key === 'string' && e.key.length === 1 && !e.ctrlKey && !e.altKey && !this.remoteKeyMap) {
                this.SendKeyUnicode(this.KeyAction.UP, e.key.charCodeAt(0));
            } else {
                this.SendKeyMsg(this.KeyAction.UP, e);
            }
        }
        return haltEvent(e);
    }

    keydown(e) {
        if (!this.isDesktopViewActive()) return;

        if ((e.altKey || e.ctrlKey || e.metaKey) && directUnicodeChars[e.code]) return haltEvent(e);

        if (e.code === 'Backquote' && !e.altKey && !e.ctrlKey && !e.metaKey) {
            const char = e.shiftKey ? '~' : '`';
            this.SendKeyUnicode(this.KeyAction.DOWN, char.charCodeAt(0));
            return haltEvent(e);
        }
        if (directUnicodeChars[e.code] && !e.ctrlKey && !e.altKey && !e.metaKey) {
            const char = e.shiftKey ? e.key : directUnicodeChars[e.code];
            this.SendKeyUnicode(this.KeyAction.DOWN, char.charCodeAt(0));
            return haltEvent(e);
        }

        if (e.code === 'CapsLock' && e.altKey) {
            this.sendKeyComboToAgent(1);
            return haltEvent(e);
        }
        if (e.code === 'CapsLock' && e.ctrlKey) {
            this.sendKeyComboToAgent(2);
            return haltEvent(e);
        }
        if (e.code === 'Space' && e.metaKey) {
            this.sendKeyComboToAgent(3);
            return haltEvent(e);
        }
        if (e.code === 'Backquote' && e.altKey && !e.ctrlKey && !e.metaKey) {
            this.sendKeyComboToAgent(0);
            return haltEvent(e);
        }


        if ((e.key !== 'Dead') && this.State === 3) {
            if (!(typeof e.key === 'string' && e.key.length === 1 && !e.ctrlKey && !e.altKey && !this.remoteKeyMap)) {
                this.SendKeyMsg(this.KeyAction.DOWN, e);
                return haltEvent(e);
            }
        }
    }

    keypress(e) {
        if (!this.isDesktopViewActive()) return;


        if (e.code === 'Backquote' && e.altKey && !e.ctrlKey && !e.metaKey) {
            this.sendKeyComboToAgent(0);
            return haltEvent(e);
        }


        if (e.code === 'CapsLock' && e.altKey) {
            this.sendKeyComboToAgent(1);
            return haltEvent(e);
        }

        if (e.code === 'CapsLock' && e.ctrlKey) {
            this.sendKeyComboToAgent(2);
            return haltEvent(e);
        }


        if (e.code === 'Space' && e.metaKey) {
            this.sendKeyComboToAgent(3);
            return haltEvent(e);
        }

        if (e.code === 'Backquote' && e.altKey && !e.ctrlKey && !e.metaKey) {
            return haltEvent(e);
        }
        if ((e.altKey || e.ctrlKey || e.metaKey) && directUnicodeChars[e.code]) return haltEvent(e);
        if (e.code === 'CapsLock' && (e.altKey || e.ctrlKey)) return haltEvent(e);
        if (directUnicodeChars[e.code] && !e.ctrlKey && !e.altKey && !e.metaKey) {
            this.SendKeyUnicode(this.KeyAction.DOWN, e.key.charCodeAt(0));
            return haltEvent(e);
        }
        if ((e.key !== 'Dead') && this.State === 3) {
            if (typeof e.key === 'string' && e.key.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey) {
                this.SendKeyUnicode(this.KeyAction.DOWN, e.key.charCodeAt(0));
            } else {
                this.SendKeyMsg(this.KeyAction.DOWN, e);
            }
        }
        if (e.key === 'Dead') return haltEvent(e);
        return true;
    }

    initEventHandlers() {
        const hiddenTrap = document.createElement("div");
        hiddenTrap.setAttribute("tabindex", "0");
        hiddenTrap.style.cssText = "position:fixed;top:-1000px;left:-1000px;width:0;height:0;opacity:0;pointer-events:none;";
        document.body.appendChild(hiddenTrap);
        window.addEventListener("load", () => hiddenTrap.focus({ preventScroll: true }));

        const forceBlur = () => {
            setTimeout(() => {
                const activeElement = document.activeElement;
                const isChatElement = activeElement && (
                    activeElement.id === 'chat-input' ||
                    activeElement.closest('#chat-window') ||
                    activeElement.closest('#chat-container') ||
                    activeElement.closest('.chat-input-container')
                );

                if (!isChatElement) {
                    if (activeElement && typeof activeElement.blur === 'function') {
                        activeElement.blur();
                    }
                    if (document.activeElement !== hiddenTrap) {
                        hiddenTrap.focus({ preventScroll: true });
                    }
                }
            }, 0);
        };

        document.addEventListener("keydown", (event) => {
            if (!this.isDesktopViewActive()) return;

            const isChatInput = event.target && (
                event.target.id === 'chat-input' ||
                event.target.closest('#chat-window') ||
                event.target.closest('#chat-container') ||
                event.target.closest('.chat-input-container')
            );

            if (!isChatInput) {
                forceBlur();
                this.keydown(event);
            }
        });

        document.addEventListener("keyup", (event) => {
            if (!this.isDesktopViewActive()) return;

            const isChatInput = event.target && (
                event.target.id === 'chat-input' ||
                event.target.closest('#chat-window') ||
                event.target.closest('#chat-container') ||
                event.target.closest('.chat-input-container')
            );

            if (!isChatInput) {
                forceBlur();
                this.keyup(event);
            }
        });

        document.addEventListener("keypress", (event) => {
            if (!this.isDesktopViewActive()) return;

            const isChatInput = event.target && (
                event.target.id === 'chat-input' ||
                event.target.closest('#chat-window') ||
                event.target.closest('#chat-container') ||
                event.target.closest('.chat-input-container')
            );

            if (!isChatInput) {
                forceBlur();
                this.keypress(event);
            }
        });
    }
}
