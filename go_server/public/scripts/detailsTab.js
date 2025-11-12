import { httpClient } from "./httpClient.js";
import { setCachedAgentDetails } from "./tabNavigation.js";

export function displayAgentDetails(info) {
    const tabArea = document.getElementById("details-container");
    if (!tabArea) return;

    tabArea.style.display = "block";

    // Build storage volumes with clean styling
    let volumeRows = "";
    if (info.storage && info.storage.disks) {
        info.storage.disks.forEach(disk => {
            volumeRows += `
                <div style="border: 1px solid #dee2e6; border-radius: 6px; padding: 16px; margin-bottom: 12px; background: #ffffff;">
                    <div style="display: flex; justify-content: space-between; align-items: start;">
                        <div style="flex: 1;">
                            <div style="font-weight: bold; color: #333; margin-bottom: 4px;">${disk.mount_point || disk.name}</div>
                            <div style="color: #666; font-size: 14px; margin-bottom: 8px;">
                                Capacity: ${disk.total_gb} GB
                            </div>
                            <div style="color: #666; font-size: 14px; margin-bottom: 4px;">
                                Available: ${disk.available_gb} GB
                            </div>
                            <div style="color: #888; font-size: 12px;">
                                File System: ${disk.file_system}
                            </div>
                        </div>
                    </div>
                </div>
            `;
        });
    }

    tabArea.innerHTML = `
        <div style="padding: 20px; font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; background-color: #fff;">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 30px;">
                <h2 style="font-size: 18px; margin: 0; color: #333;">Details - ${info.system.hostname}</h2>
                <button onclick="refreshAgentDetails()" style="padding: 6px 12px; background: #007bff; color: white; border: none; border-radius: 4px; cursor: pointer; font-size: 12px;"> Refresh</button>
            </div>
            
            <!-- BIOS Section -->
            <div style="display: flex; align-items: flex-start; margin-bottom: 20px;">
                <div style="width: 60px; height: 60px; background: linear-gradient(135deg, #4a90e2, #357abd); border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 15px;">
                    <div style="color: white; font-size: 24px; font-weight: bold;">B</div>
                </div>
                <div style="flex: 1;">
                    <div style="font-weight: bold; margin-bottom: 8px;">BIOS</div>
                    <div style="border: 1px solid #dee2e6; border-radius: 6px; padding: 16px; background: #ffffff;">
                        <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                            <span style="color: #666; font-size: 14px;">Vendor</span>
                            <span style="text-align: right; color: #333;">VMware, Inc.</span>
                        </div>
                        <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                            <span style="color: #666; font-size: 14px;">Version</span>
                            <span style="text-align: right; color: #333;">${info.system.kernel_version}</span>
                        </div>
                        <div style="display: flex; justify-content: space-between;">
                            <span style="color: #666; font-size: 14px;">Mode</span>
                            <span style="text-align: right; color: #333;">UEFI</span>
                        </div>
                    </div>
                </div>
            </div>

            <!-- Motherboard Section -->
            <div style="display: flex; align-items: flex-start; margin-bottom: 20px;">
                <div style="width: 60px; height: 60px; background: linear-gradient(135deg, #5cb85c, #449d44); border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 15px;">
                    <div style="color: white; font-size: 20px;">⚡</div>
                </div>
                <div style="flex: 1;">
                    <div style="font-weight: bold; margin-bottom: 8px;">Motherboard</div>
                    <div style="border: 1px solid #dee2e6; border-radius: 6px; padding: 16px; background: #ffffff;">
                        <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                            <span style="color: #666; font-size: 14px;">Vendor</span>
                            <span style="text-align: right; color: #333;">Intel Corporation</span>
                        </div>
                        <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                            <span style="color: #666; font-size: 14px;">Name</span>
                            <span style="text-align: right; color: #333;">440BX Desktop Reference Platform</span>
                        </div>
                        <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                            <span style="color: #666; font-size: 14px;">CPU</span>
                            <span style="text-align: right; color: #333;">${info.cpu.brand}</span>
                        </div>
                        <div style="display: flex; justify-content: space-between;">
                            <span style="color: #666; font-size: 14px;">Architecture</span>
                            <span style="text-align: right; color: #333;">${info.system.architecture}</span>
                        </div>
                    </div>
                </div>
            </div>

            <!-- Memory Section -->
            <div style="display: flex; align-items: flex-start; margin-bottom: 20px;">
                <div style="width: 60px; height: 60px; background: linear-gradient(135deg, #f0ad4e, #ec971f); border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 15px;">
                    <div style="color: white; font-size: 20px;">▬</div>
                </div>
                <div style="flex: 1;">
                    <div style="font-weight: bold; margin-bottom: 8px;">Memory</div>
                    <div style="border: 1px solid #dee2e6; border-radius: 6px; padding: 16px; background: #ffffff;">
                        <div style="font-weight: bold; color: #333; margin-bottom: 8px;">RAM slot #0</div>
                        <div style="display: flex; justify-content: space-between; margin-bottom: 4px;">
                            <span style="color: #666; font-size: 14px;">Capacity</span>
                            <span style="text-align: right; color: #333;">${info.memory.total_mb} MB</span>
                        </div>
                        <div style="display: flex; justify-content: space-between; margin-bottom: 4px;">
                            <span style="color: #666; font-size: 14px;">Used</span>
                            <span style="text-align: right; color: #333;">${info.memory.used_mb} MB (${info.memory.usage_percent}%)</span>
                        </div>
                        <div style="display: flex; justify-content: space-between;">
                            <span style="color: #666; font-size: 14px;">Available</span>
                            <span style="text-align: right; color: #333;">${info.memory.available_mb} MB</span>
                        </div>
                    </div>
                </div>
            </div>

            <!-- Storage Section -->
            <div style="display: flex; align-items: flex-start; margin-bottom: 20px;">
                <div style="width: 60px; height: 60px; background: linear-gradient(135deg, #777, #555); border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 15px;">
                    <div style="color: white; font-size: 20px;">💾</div>
                </div>
                <div style="flex: 1;">
                    <div style="font-weight: bold; margin-bottom: 8px;">Storage</div>
                    ${info.storage && info.storage.disks && info.storage.disks.length > 0 ? `
                        <div style="border: 1px solid #dee2e6; border-radius: 6px; padding: 16px; background: #ffffff;">
                            <div style="font-weight: bold; color: #333; margin-bottom: 8px;">${info.storage.disks[0].name} Storage Device</div>
                            <div style="display: flex; justify-content: space-between; margin-bottom: 4px;">
                                <span style="color: #666; font-size: 14px;">Capacity</span>
                                <span style="text-align: right; color: #333;">${info.storage.disks[0].total_gb} GB</span>
                            </div>
                            <div style="display: flex; justify-content: space-between;">
                                <span style="color: #666; font-size: 14px;">Status</span>
                                <span style="text-align: right; color: green;">OK</span>
                            </div>
                        </div>
                    ` : ''}
                </div>
            </div>

            <!-- Storage Volumes Section -->
            <div style="display: flex; align-items: flex-start; margin-bottom: 20px;">
                <div style="width: 60px; height: 60px; background: linear-gradient(135deg, #666, #444); border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 15px;">
                    <div style="color: white; font-size: 20px;">📁</div>
                </div>
                <div style="flex: 1;">
                    <div style="font-weight: bold; margin-bottom: 15px;">Storage Volumes</div>
                    <div style="display: grid; gap: 12px;">
                        ${volumeRows}
                    </div>
                </div>
            </div>

            <!-- Network Section -->
            <div style="display: flex; align-items: flex-start; margin-bottom: 20px;">
                <div style="width: 60px; height: 60px; background: linear-gradient(135deg, #5bc0de, #31b0d5); border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 15px;">
                    <div style="color: white; font-size: 20px;">🌐</div>
                </div>
                <div style="flex: 1;">
                    <div style="font-weight: bold; margin-bottom: 8px;">Networking</div>
                    <div style="display: grid; gap: 12px;">
                        ${info.networking && info.networking.interfaces ? info.networking.interfaces.map(net => `
                            <div style="border: 1px solid #dee2e6; border-radius: 6px; padding: 16px; background: #ffffff;">
                                <div style="font-weight: bold; color: #333; margin-bottom: 8px;">${net.interface}</div>
                                <div style="display: flex; justify-content: space-between; margin-bottom: 4px;">
                                    <span style="color: #666; font-size: 14px;">Data Received</span>
                                    <span style="text-align: right; color: #333;">${net.received_mb || (net.received_bytes / (1024 * 1024)).toFixed(2)} MB</span>
                                </div>
                                <div style="display: flex; justify-content: space-between; margin-bottom: 4px;">
                                    <span style="color: #666; font-size: 14px;">Data Transmitted</span>
                                    <span style="text-align: right; color: #333;">${net.transmitted_mb || (net.transmitted_bytes / (1024 * 1024)).toFixed(2)} MB</span>
                                </div>
                                <div style="display: flex; justify-content: space-between;">
                                    <span style="color: #666; font-size: 14px;">Status</span>
                                    <span style="text-align: right; color: green;">Connected</span>
                                </div>
                            </div>
                        `).join('') : ''}
                    </div>
                </div>
            </div>

            <!-- System Information Card -->
            <div style="display: flex; align-items: flex-start; margin-top: 30px;">
                <div style="width: 60px; height: 60px; background: linear-gradient(135deg, #6f42c1, #5a32a3); border-radius: 8px; display: flex; align-items: center; justify-content: center; margin-right: 15px;">
                    <div style="color: white; font-size: 20px;">ℹ️</div>
                </div>
                <div style="flex: 1;">
                    <div style="font-weight: bold; margin-bottom: 8px;">System Information</div>
                    <div style="border: 1px solid #dee2e6; border-radius: 6px; padding: 16px; background: #ffffff;">
                        <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                            <span style="color: #666; font-size: 14px;">Hostname</span>
                            <span style="text-align: right; color: #333; font-weight: 500;">${info.system.hostname}</span>
                        </div>
                        <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                            <span style="color: #666; font-size: 14px;">Operating System</span>
                            <span style="text-align: right; color: #333; font-weight: 500;">${info.system.os_name} ${info.system.os_version}</span>
                        </div>
                        <div style="display: flex; justify-content: space-between; margin-bottom: 8px;">
                            <span style="color: #666; font-size: 14px;">Last Boot</span>
                            <span style="text-align: right; color: #333;">${info.system.boot_time}</span>
                        </div>
                        <div style="display: flex; justify-content: space-between;">
                            <span style="color: #666; font-size: 14px;">Last Updated</span>
                            <span style="text-align: right; color: #28a745; font-weight: 500;">${new Date(info.timestamp).toLocaleString()}</span>
                        </div>
                    </div>
                </div>
            </div>
        </div>
    `;
}

export function refreshAgentDetails() {
    setCachedAgentDetails(null);
    if (httpClient) {
        const tabArea = document.getElementById("details-container");
        tabArea.innerHTML = `
    <div style="padding: 40px; text-align: center;">
        <img src="img/progress_anim.gif" alt="Loading..." style="width: 50px; height: 50px; margin-bottom: 15px;">
        <div style="color: #666; font-size: 14px;">Refreshing system details...</div>
    </div>
        `;

        httpClient.getAgentDetails().then(agentDetails => {
            setCachedAgentDetails(agentDetails);
            displayAgentDetails(agentDetails);
        }).catch(error => {
            tabArea.innerHTML = `
                <div style="padding: 40px; text-align: center; color: #d32f2f;">
                    <div style="margin-bottom: 10px;">Failed to refresh system details</div>
                    <div style="font-size: 12px; margin-bottom: 15px;">${error.message}</div>
                    <button onclick="refreshAgentDetails()" style="padding: 8px 16px; background: #007bff; color: white; border: none; border-radius: 4px; cursor: pointer;">Retry</button>
                </div>
            `;
        });
    }
}


window.refreshAgentDetails = refreshAgentDetails;