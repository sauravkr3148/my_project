import KVM from './kvm.js';
import {
    setupFileTransferButtons,
} from './fileManagerTab.js';
import { initializeHTTPClient } from './httpClient.js';
import { setupTabNavigation } from './tabNavigation.js';

export function loadKVM(tenantID, publicKey) {
    window.tenantID = tenantID;
    window.publicKey = publicKey;

    initializeHTTPClient(tenantID, publicKey, 'file_agent');

    setTimeout(function () {
        document.getElementById("refresh").click();
    }, 2000);
}

export function initializeDesktopConnection() {
    // console.log("tenantID:", window.tenantID);
    // console.log("publicKey:", window.publicKey);
    if (!window.desktopKVM && window.tenantID && window.publicKey) {
        window.desktopKVM = new KVM(window.tenantID, window.publicKey, "c_agent");
    }
}

export { setupTabNavigation, setupFileTransferButtons };
window.setupTabNavigation = setupTabNavigation;
window.setupFileTransferButtons = setupFileTransferButtons;