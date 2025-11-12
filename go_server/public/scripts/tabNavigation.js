import { displayAgentDetails } from "./detailsTab.js";
import { displaySoftwareList } from "./installedApplicationTab.js";
import { initializeDesktopConnection } from "./main.js";
import { ChatClient } from "./chatClient.js";
import { updatePasteButtonState } from "./fileManagerTab.js";
import { httpClient } from "./httpClient.js";
import { getActiveMonitor } from "./monitor.js";
let cachedAgentDetails = null;
export function setCachedAgentDetails(agentDetails) {
    cachedAgentDetails = agentDetails;
}
let agentDetailsLoading = false;
let cachedSoftwareList = null;
let softwareListLoading = false;
export function setSoftwareListLoading(value) {
    softwareListLoading = value;
}

export function setCachedSoftwareList(value) {
    cachedSoftwareList = value;
}


export function setupTabNavigation() {
    document.querySelectorAll(".tab").forEach(tab => {
        tab.addEventListener("click", () => {

            document.getElementById("details-container").style.display = "none";
            document.getElementById("details-container").innerHTML = "";
            document.getElementById("desktop-view").style.display = "none";
            document.getElementById("menucir").style.display = "none";
            document.getElementById("tool").style.display = "none";
            const softwareContainer = document.getElementById("software-container");
            if (softwareContainer) {
                softwareContainer.style.display = "none";
            }

            document.querySelectorAll(".tab").forEach(t => t.classList.remove("selected"));
            tab.classList.add("selected");

            // Hide all panels first
            document.querySelector(".breadcrumb-container").style.display = "none";
            document.getElementById("filetransfer-view").classList.remove("show");
            document.getElementById("filetransfer-view").style.display = "none";
            document.getElementById("remote-file-list").style.display = "none";
            document.getElementById("file-table-header").style.display = "none";

            if (tab.id === "tab-desktop") {

                document.getElementById("desktop-view").style.display = "flex";
                const monitor = getActiveMonitor();
                if (monitor) {
                    monitor.onResize(2);
                }

                document.getElementById("filetransfer-view").style.display = "none";
                document.getElementById("filetransfer-view").classList.remove("show");
                document.querySelector(".breadcrumb-container").style.display = "none";
                document.getElementById("remote-file-list").style.display = "none";
                document.getElementById("file-table-header").style.display = "none";


                document.getElementById("menucir").style.display = "";
                document.getElementById("tool").style.display = "";
                document.getElementById("quality-control").style.display = "";


                if (!window.desktopConnected) {
                    const loader = document.getElementById("load-gif-box");
                    if (loader) loader.style.display = "flex";
                }

                if (typeof initializeDesktopConnection === 'function') {
                    initializeDesktopConnection();
                }

            } else if (tab.id === "tab-terminal") {

            } else if (tab.id === "tab-file") {

                if (window.desktopConnected) {
                    const loader = document.getElementById("load-gif-box");
                    if (loader) {
                        loader.style.display = "none";
                    }
                }

                document.getElementById("desktop-view").style.display = "none";
                document.getElementById("filetransfer-view").style.display = "block";

                document.getElementById("quality-control").style.display = "none";
                document.getElementById("menucir").style.display = "none";
                document.getElementById("tool").style.display = "none";
                document.getElementById("desktop-view").style.display = "none";

                document.getElementById("filetransfer-view").classList.add("show");

                if (window.isFileTabConnected) {

                    document.getElementById("btn-connect").style.display = "none";
                    document.getElementById("btn-disconnected").style.display = "none";
                    document.getElementById("btn-disconnect").style.display = "inline-block";
                    document.getElementById("btn-connected").style.display = "inline-block";

                    document.querySelector(".breadcrumb-container").style.display = "flex";
                    document.getElementById("remote-file-list").style.display = "block";
                    document.getElementById("file-table-header").style.display = "block";

                    document.getElementById("btn-disconnect").disabled = false;
                    document.getElementById("btn-connect").disabled = true;
                    const enableButtons = [
                        "btn-actions", "btn-back", "btn-select-all",
                        "btn-create-folder", "btn-upload", "btn-refresh", "btn-find", "btn-goto"
                    ];
                    const disableButtons = [
                        "btn-download", "btn-rename", "btn-delete", "btn-edit",
                        "btn-cut", "btn-copy", "btn-paste", "btn-zip", "btn-unzip", "btn-open"
                    ];

                    enableButtons.forEach(id => {
                        const btn = document.getElementById(id);
                        if (btn) btn.disabled = false;
                    });

                    disableButtons.forEach(id => {
                        const btn = document.getElementById(id);
                        if (btn) btn.disabled = true;
                    });

                    document.getElementById("sort-mode").disabled = false;
                } else {

                    document.getElementById("btn-connect").style.display = "inline-block";
                    document.getElementById("btn-disconnected").style.display = "inline-block";
                    document.getElementById("btn-disconnect").style.display = "none";
                    document.getElementById("btn-connected").style.display = "none";

                    document.getElementById("file-table-header").style.display = "none";
                    document.getElementById("remote-file-list").style.display = "none";
                    document.querySelector(".breadcrumb-container").style.display = "none";

                    document.getElementById("btn-connect").disabled = false;
                    document.getElementById("btn-disconnect").disabled = true;
                    const allButtons = [
                        "btn-back", "btn-select-all", "btn-rename", "btn-delete", "btn-edit",
                        "btn-create-folder", "btn-upload", "btn-download", "btn-cut",
                        "btn-copy", "btn-paste", "btn-zip", "btn-unzip", "btn-refresh",
                        "btn-find", "btn-goto", "btn-open", "btn-actions"
                    ];
                    allButtons.forEach(id => {
                        const btn = document.getElementById(id);
                        if (btn) btn.disabled = true;
                    });

                    document.getElementById("sort-mode").disabled = true;

                }


                updatePasteButtonState();

            } else if (tab.id == "details-section") {
                if (window.desktopConnected) {
                    const loader = document.getElementById("load-gif-box");
                    if (loader) {
                        loader.style.display = "none";
                    }
                }

                document.getElementById("remote-file-list").style.display = "none";
                document.querySelector(".breadcrumb-container").style.display = "none";
                document.getElementById("desktop-view").style.display = "none";
                document.getElementById("filetransfer-view").style.display = "none";

                if (cachedAgentDetails) {
                    displayAgentDetails(cachedAgentDetails);
                } else {

                    const tabArea = document.getElementById("details-container");
                    tabArea.style.display = "block";
                    tabArea.innerHTML = `
    <div style="padding: 40px; text-align: center;">
        <img src="img/progress_anim.gif" alt="Loading..." style="width: 50px; height: 50px; margin-bottom: 15px;">
        <div style="color: #666; font-size: 14px;">Loading system details...</div>
    </div>
        `;

                    if (httpClient && !agentDetailsLoading) {
                        agentDetailsLoading = true;
                        httpClient.getAgentDetails().then(agentDetails => {
                            cachedAgentDetails = agentDetails;
                            agentDetailsLoading = false;
                            if (document.getElementById("details-section").classList.contains("selected")) {
                                displayAgentDetails(agentDetails);
                            }
                        }).catch(error => {
                            agentDetailsLoading = false;
                            const tabArea = document.getElementById("details-container");
                            tabArea.innerHTML = `
                    <div style="padding: 40px; text-align: center; color: #d32f2f;">
                        <div style="margin-bottom: 10px;">Failed to load system details</div>
                        <div style="font-size: 12px; margin-bottom: 15px;">${error.message}</div>
                        <button onclick="location.reload()" style="padding: 8px 16px; background: #007bff; color: white; border: none; border-radius: 4px; cursor: pointer;">Retry</button>
                    </div>
                `;
                        });
                    } else if (!httpClient) {
                        const tabArea = document.getElementById("details-container");
                        tabArea.innerHTML = `
                <div style="padding: 40px; text-align: center; color: #d32f2f;">
                    <div>HTTP client not initialized</div>
                </div>
            `;
                    }
                }
            } else if (tab.id == "software-section") {

                if (window.desktopConnected) {
                    const loader = document.getElementById("load-gif-box");
                    if (loader) {
                        loader.style.display = "none";
                    }
                }

                document.getElementById("remote-file-list").style.display = "none";
                document.querySelector(".breadcrumb-container").style.display = "none";
                document.getElementById("desktop-view").style.display = "none";
                document.getElementById("filetransfer-view").style.display = "none";

                if (cachedSoftwareList) {
                    displaySoftwareList(cachedSoftwareList);
                } else {
                    const tabArea = document.getElementById("software-container");
                    tabArea.style.display = "block";
                    tabArea.innerHTML = `
    <div style="padding: 40px; text-align: center;">
        <img src="img/progress_anim.gif" alt="Loading..." style="width: 50px; height: 50px; margin-bottom: 15px;">
        <div style="color: #666; font-size: 14px;">Loading installed software...</div>
    </div>
        `;
                    if (httpClient && !softwareListLoading) {
                        setSoftwareListLoading(true);
                        httpClient.getInstalledSoftware().then(response => {

                            setCachedSoftwareList(response);
                            setSoftwareListLoading(false);
                            ve
                            if (document.getElementById("software-section").classList.contains("selected")) {
                                displaySoftwareList(response);
                            }
                        }).catch(error => {
                            setSoftwareListLoading(false);
                            const tabArea = document.getElementById("software-container");
                            tabArea.innerHTML = `
                    <div style="padding: 40px; text-align: center; color: #d32f2f;">
                        <div style="margin-bottom: 10px;">Failed to load installed software</div>
                        <div style="font-size: 12px; margin-bottom: 15px;">${error.message}</div>
                        <button onclick="refreshSoftwareList()" style="padding: 8px 16px; background: #007bff; color: white; border: none; border-radius: 4px; cursor: pointer;">Retry</button>
                    </div>
                `;
                        });
                    } else if (!httpClient) {
                        const tabArea = document.getElementById("software-container");
                        tabArea.innerHTML = `
                <div style="padding: 40px; text-align: center; color: #d32f2f;">
                    <div>HTTP client not initialized</div>
                </div>
            `;
                    }
                }
            } else if (tab.id == "chat-section") {
                if (window.desktopConnected) {
                    const loader = document.getElementById("load-gif-box");
                    if (loader) {
                        loader.style.display = "none";
                    }
                }

                document.getElementById("remote-file-list").style.display = "none";
                document.querySelector(".breadcrumb-container").style.display = "none";
                document.getElementById("desktop-view").style.display = "none";
                document.getElementById("filetransfer-view").style.display = "none";
                document.getElementById("details-container").style.display = "none";
                document.getElementById("software-container").style.display = "none";

                document.getElementById("chat-container").style.display = "block";

                if (window.currentDevice && !window.chatClient) {
                    const tenantID = window.tenantID;
                    const publicKey1 = window.currentDevice?.id || window.publicKey || "default";
                    window.chatClient = new ChatClient(tenantID, publicKey1);
                    window.chatClient.createChatUI(document.getElementById("chat-container"));
                    if (Notification.permission === 'default') {
                        window.chatClient.requestNotificationPermission();
                    }
                }
            }


        });
    });
}

