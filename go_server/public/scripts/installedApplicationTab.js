import { httpClient } from "./httpClient.js";
import { setCachedSoftwareList, setSoftwareListLoading } from "./tabNavigation.js";




export function displaySoftwareList(response) {
    const container = document.getElementById("software-container");
    if (!container) return;

    container.style.display = "block";

    const systemSoftware = response.system_software || [];
    const userSoftware = response.user_software || [];
    const totalCount = systemSoftware.length + userSoftware.length;

    if (totalCount === 0) {
        container.innerHTML = `
            <div style="padding: 40px; text-align: center; color: #666; font-style: italic;">
                <div>No installed software found</div>
            </div>
        `;
        return;
    }

    let html = `
        <div style="padding: 20px;">
            <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 20px;">
                <h3 style="margin: 0; color: #333;">Installed Software (${totalCount} items)</h3>
                <button onclick="refreshSoftwareList()" style="padding: 8px 16px; background: #007bff; color: white; border: none; border-radius: 4px; cursor: pointer;">
                    Refresh
                </button>
            </div>
    `;

    if (systemSoftware.length > 0) {
        html += `
            <div style="margin-bottom: 30px;">
                <h4 style="color: #333; margin-bottom: 15px; padding-bottom: 8px; border-bottom: 2px solid #007bff;">
                     System-wide Applications (${systemSoftware.length})
                </h4>
                <div style="display: grid; gap: 12px;">
        `;

        systemSoftware.forEach(software => {
            html += createSoftwareCard(software, '#ffffff');
        });

        html += `</div></div>`;
    }

    if (userSoftware.length > 0) {
        html += `
            <div style="margin-bottom: 20px;">
                <h4 style="color: #333; margin-bottom: 15px; padding-bottom: 8px; border-bottom: 2px solid #28a745;">
                    👤 User Applications (${userSoftware.length})
                </h4>
                <div style="display: grid; gap: 12px;">
        `;

        userSoftware.forEach(software => {
            html += createSoftwareCard(software, '#ffffff');
        });

        html += `</div></div>`;
    }

    html += `</div>`;
    container.innerHTML = html;
}

export function createSoftwareCard(software, backgroundColor) {
    return `
        <div style="background: ${backgroundColor}; border: 1px solid #dee2e6; border-radius: 6px; padding: 16px;">
            <div style="display: flex; justify-content: space-between; align-items: start;">
                <div style="flex: 1;">
                    <div style="font-weight: bold; color: #333; margin-bottom: 4px;">${software.name || 'Unknown'}</div>
                    <div style="color: #666; font-size: 14px; margin-bottom: 8px;">
                        Version: ${software.version || 'Unknown'}
                    </div>
                    ${software.publisher ? `<div style="color: #666; font-size: 14px;">Publisher: ${software.publisher}</div>` : ''}
                    ${software.install_date ? `<div style="color: #666; font-size: 14px;">Installed: ${software.install_date}</div>` : ''}
                    <div style="color: #888; font-size: 12px; margin-top: 4px;">
                        Scope: ${software.scope === 'system' ? ' System-wide' : '👤 User-specific'}
                    </div>
                </div>
                ${software.size ? `<div style="color: #666; font-size: 14px; text-align: right;">${software.size}</div>` : ''}
            </div>
        </div>
    `;
}

export function refreshSoftwareList() {
    setCachedSoftwareList(null);

    if (httpClient) {
        const tabArea = document.getElementById("software-container");
        tabArea.style.display = "block";
        tabArea.innerHTML = `
    <div style="padding: 40px; text-align: center;">
        <img src="img/progress_anim.gif" alt="Loading..." style="width: 50px; height: 50px; margin-bottom: 15px;">
        <div style="color: #666; font-size: 14px;">Refreshing installed software...</div>
    </div>
        `;

        setSoftwareListLoading(true);
        httpClient.getInstalledSoftware().then(response => {
            setCachedSoftwareList(response);
            setSoftwareListLoading(false);
            displaySoftwareList(response);
        }).catch(error => {
            softwareListLoading = false;
            tabArea.innerHTML = `
                <div style="padding: 40px; text-align: center; color: #d32f2f;">
                    <div style="margin-bottom: 10px;">Failed to refresh installed software</div>
                    <div style="font-size: 12px; margin-bottom: 15px;">${error.message}</div>
                    <button onclick="refreshSoftwareList()" style="padding: 8px 16px; background: #007bff; color: white; border: none; border-radius: 4px; cursor: pointer;">Retry</button>
                </div>
            `;
        });
    }
}

window.refreshSoftwareList = refreshSoftwareList;
