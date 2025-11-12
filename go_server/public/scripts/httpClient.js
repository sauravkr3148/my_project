
export class HTTPClient {
    constructor(tenantID, publicKey, agentType = 'file_agent') {
        this.tenantID = tenantID;
        this.publicKey = publicKey;
        this.agentType = agentType;
        this.baseURL = window.location.origin;
    }

    async makeRequest(endpoint, options = {}) {
        const url = `${this.baseURL}/api/v1${endpoint}/${this.agentType}/${this.tenantID}/${this.publicKey}`;
        // console.log('Making HTTP request:', {
        //     url: url,
        //     method: options.method || 'GET',
        //     hasBody: !!options.body
        // });

        try {
            const response = await fetch(url, {
                headers: {
                    'Content-Type': 'application/json',
                    ...options.headers
                },
                ...options
            });

            // console.log('HTTP response:', {
            //     status: response.status,
            //     statusText: response.statusText,
            //     ok: response.ok
            // });

            if (!response.ok) {
                const errorText = await response.text();
                console.error('HTTP error response:', errorText);
                throw new Error(`HTTP ${response.status}: ${response.statusText}${errorText ? ' - ' + errorText : ''}`);
            }

            return response;
        } catch (error) {
            console.error('HTTP request failed:', error);
            throw error;
        }
    }

    async listFiles(path) {
        const response = await this.makeRequest('/files/list', {
            method: 'POST',
            body: JSON.stringify({ path: path || '', show_hidden: false })
        });
        return await response.json();
    }

    async downloadFile(filePath) {
        try {
            const url = `${this.baseURL}/api/v1/files/download/${this.agentType}/${this.tenantID}/${this.publicKey}?path=${encodeURIComponent(filePath)}`;
            const response = await fetch(url);
            if (!response.ok) {
                throw new Error(`HTTP error! status: ${response.status}`);
            }
            return await response.blob();
        } catch (error) {
            console.error('Download failed:', error);
            throw error;
        }
    }
    async uploadFile(file, path) {
        const formData = new FormData();
        formData.append('file', file);
        formData.append('path', path);

        const response = await this.makeRequest('/files/upload', {
            method: 'POST',
            body: formData,
            headers: {}
        });
        return response.json();
    }

    async deleteFile(path) {
        const response = await this.makeRequest('/files/delete', {
            method: 'DELETE',
            body: JSON.stringify({ path })
        });
        return await response.json();
    }

    async createDirectory(path) {
        const response = await this.makeRequest('/files/mkdir', {
            method: 'POST',
            body: JSON.stringify({ path })
        });

        if (!response.ok) {
            throw new Error(`HTTP ${response.status}`);
        }
        return;
    }


    async renameFile(oldPath, newName) {
        const response = await this.makeRequest('/files/rename', {
            method: 'PUT',
            body: JSON.stringify({ old_path: oldPath, new_name: newName })
        });
        return await response.json();
    }

    async editFile(filePath) {
        const response = await this.makeRequest('/files/edit', {
            method: 'POST',
            body: JSON.stringify({ path: filePath })
        });
        return response.json();
    }

    async saveFile(filePath, content) {
        const response = await this.makeRequest('/files/save', {
            method: 'POST',
            body: JSON.stringify({ path: filePath, content })
        });
        return await response.json();
    }

    async zipFiles(paths, outputName) {
        const response = await this.makeRequest('/files/zip', {
            method: 'POST',
            body: JSON.stringify({ target_list: paths, zip_name: outputName })
        });
        return await response.json();
    }

    async unzipFile(filePath, outputPath) {
        const response = await this.makeRequest('/files/unzip', {
            method: 'POST',
            body: JSON.stringify({ source: filePath, target: outputPath })
        });
        return await response.json();
    }

    async openFile(filePath) {
        const response = await this.makeRequest('/files/open', {
            method: 'POST',
            body: JSON.stringify({ path: filePath })
        });
        return await response.json();
    }


    async pasteFiles(sourcePaths, destinationPath, mode = 'copy') {
        const response = await this.makeRequest('/files/paste', {
            method: 'POST',
            body: JSON.stringify({
                from_list: sourcePaths,
                to: destinationPath,
                mode: mode
            })
        });
        return response.json();
    }
    async getAgentDetails() {
        const response = await this.makeRequest('/agent/details', {
            method: 'GET'
        });
        return await response.json();
    }
    async getInstalledSoftware() {
        const response = await this.makeRequest('/agent/software', {
            method: 'GET'
        });
        return await response.json();
    }
}
export let httpClient;

export function initializeHTTPClient(tenantID, publicKey, agentType = 'file_agent') {
    if (!tenantID || !publicKey) {
        console.error('Cannot initialize HTTP client: missing tenantID or publicKey');
        return;
    }
    httpClient = new HTTPClient(tenantID, publicKey, agentType);
}

window.initializeHTTPClient = initializeHTTPClient; 