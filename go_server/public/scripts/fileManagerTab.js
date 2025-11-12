import { httpClient } from "./httpClient.js";

const RESTRICTED_BASE_PATH = "C:\\Program Files (x86)\\e-Jan Networks";
window.selectedFilePath = null;
window.remoteHistory = [];
window.clipboardFile = null;
window.clipboardMode = null;
window.selectedItems = new Set();
window.clipboardFileList = [];
window.isFileTabConnected = false;
let isSelectAllMode = true;
let currentEditingFilePath = null;

export function renderRemoteFiles(msg) {
    const listUl = document.getElementById("remote-list-ul");
    if (!listUl) return;
    listUl.innerHTML = msg.entries.map((e, i) => `
        <li data-index="${i}" data-path="${msg.path}/${e.name}" data-isdir="${e.is_dir}">
            ${e.is_dir ? "📁" : "📄"} ${e.name}
        </li>
    `).join("");
    msg.entries.forEach((entry, i) => {
        const el = document.querySelector(`li[data-index="${i}"]`);
        if (el) {
            el.addEventListener("click", () => {
                document.querySelectorAll("li").forEach(li => li.classList.remove("selected"));
                el.classList.add("selected");
                el.setAttribute("data-selected", "true");
                window.currentRemotePath = msg.path;
            });
            if (entry.is_dir) {
                el.addEventListener("click", () => {
                    window.remoteHistory.push(msg.path);
                    window.navigateToRemoteFolder(`${msg.path}/${entry.name}`);
                });
            }
            //  Styling
            el.style.cursor = "pointer";
            el.style.padding = "4px";
            el.addEventListener("mouseenter", () => el.style.backgroundColor = "#f0f8ff");
            el.addEventListener("mouseleave", () => {
                if (!el.classList.contains("selected")) el.style.backgroundColor = "transparent";
            });
        }

    });
    document.getElementById("delete-remote")?.addEventListener("click", async () => {
        const selected = document.querySelector("li.selected");
        if (selected) await jobManager.addDeleteJob(selected.dataset.path, true);
    });

    document.getElementById("create-remote-dir")?.addEventListener("click", async () => {
        const name = prompt("Enter folder name:");
        if (name) {
            const parent = msg.path || "C:/";
            try {
                await httpClient.createDirectory(`${parent}/${name}`);
                await requestRemoteFileList(parent);
            } catch (error) {
                showStatus(`Failed to create directory: ${error.message}`, "error");
            }
        }
    });

    document.getElementById("receive")?.addEventListener("click", () => {
        const selected = document.querySelector("li.selected");
        if (selected) {
            const path = selected.dataset.path;
            const to = "";
            jobManager.addTransferJob(path, to, false, false);
        }
    });

    document.getElementById("remote-path").value = msg.path;
    window.currentRemotePath = msg.path;
    document.getElementById("file-list-header").style.display = "flex";
}
export async function requestRemoteFileList(path) {
    try {
        if (!httpClient) {
            showStatus("HTTP client not initialized", "error");

            return;
        }
        let restrictedPath;
        if (!path || path === "") {
            restrictedPath = RESTRICTED_BASE_PATH;
        } else {
            const normalizedPath = path.replace(/\\/g, '/').replace(/\/+/g, '/');
            const normalizedBasePath = RESTRICTED_BASE_PATH.replace(/\\/g, '/').replace(/\/+/g, '/');

            if (normalizedPath.startsWith(normalizedBasePath)) {
                restrictedPath = path;
            } else {
                restrictedPath = RESTRICTED_BASE_PATH;
                showStatus("Access restricted to designated folder only", "warning");
            }
        }

        const result = await httpClient.listFiles(restrictedPath);
        displayRemoteFileList(result.entries, result.path);
        renderBreadcrumb(result.path);
    } catch (error) {
        console.error('Failed to get file list:', error);
        showStatus(`Failed to get file list: ${error.message}`, "error");
    }
}


export function setupNavigateToRemoteFolder() {
    window.navigateToRemoteFolder = async function (path) {
        try {
            await requestRemoteFileList(path);
        } catch (error) {
            console.error("Navigation failed:", error);
            showStatus(`Navigation failed: ${error.message}`, "error");
        }
    };
}

export async function renameRemoteFile(oldPath, newName) {
    try {
        await httpClient.renameFile(oldPath, newName);
        await requestRemoteFileList(window.currentRemotePath);
    } catch (error) {
        console.error('Rename failed:', error);
        showStatus(`Rename failed: ${error.message}`, "error");
    }
}


export function setupFileTransferButtons() {
    document.getElementById("btn-connect")?.addEventListener("click", () => {
        window.requestRemoteFileList(RESTRICTED_BASE_PATH);

        document.querySelector(".breadcrumb-container").style.display = "flex";
        document.getElementById("remote-file-list").style.display = "block";
        document.getElementById("file-table-header").style.display = "block";

        const enableButtons = [
            "btn-disconnect", "btn-actions", "btn-back", "btn-select-all",
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

        document.getElementById("btn-connect").style.display = "none";
        document.getElementById("btn-disconnected").style.display = "none";

        document.getElementById("btn-disconnect").style.display = "inline-block";
        document.getElementById("btn-connected").style.display = "inline-block";
        window.isFileTabConnected = true;
    });

    document.getElementById("btn-disconnect")?.addEventListener("click", () => {
        window.isFileTabConnected = false;
        document.querySelector(".breadcrumb-container").style.display = "none";
        document.getElementById("remote-file-list").style.display = "none";
        document.getElementById("file-table-header").style.display = "none";
        const allButtons = [
            "btn-disconnect", "btn-actions", "btn-back", "btn-select-all",
            "btn-rename", "btn-delete", "btn-edit", "btn-create-folder",
            "btn-upload", "btn-download", "btn-cut", "btn-copy",
            "btn-paste", "btn-zip", "btn-unzip", "btn-refresh",
            "btn-find", "btn-goto"
        ];
        allButtons.forEach(id => {
            const btn = document.getElementById(id);
            if (btn) btn.disabled = true;
        });

        document.getElementById("sort-mode").disabled = true;

        document.getElementById("btn-connect").style.display = "inline-block";
        document.getElementById("btn-disconnected").style.display = "inline-block";
        document.getElementById("btn-connect").disabled = false;
        document.getElementById("btn-disconnect").style.display = "none";
        document.getElementById("btn-connected").style.display = "none";
    });

    document.getElementById("btn-back")?.addEventListener("click", () => {
        if (window.remoteHistory.length > 0) {
            const previousPath = window.remoteHistory.pop();
            if (isPathAllowed(previousPath)) {
                window.currentRemotePath = previousPath;
                navigateToPath(previousPath);
            } else {

                window.currentRemotePath = RESTRICTED_BASE_PATH;
                navigateToPath(RESTRICTED_BASE_PATH);
            }
        } else {

            window.currentRemotePath = RESTRICTED_BASE_PATH;
            navigateToPath(RESTRICTED_BASE_PATH);
        }
    });

    document.getElementById("btn-select-all")?.addEventListener("click", () => {
        const fileItems = document.querySelectorAll('.file-item');
        const btn = document.getElementById("btn-select-all");

        if (isSelectAllMode) {

            window.selectedItems = new Set();
            window.selectedFileItem = null;

            fileItems.forEach(item => {
                item.classList.add("selected");
                const checkbox = item.querySelector("input[type='checkbox']");
                if (checkbox) checkbox.checked = true;

                const path = item.dataset.path;
                const isDir = item.dataset.isDir === "true";

                if (path) {
                    window.selectedItems.add(path);

                    if (!window.selectedFileItem) {
                        window.selectedFileItem = item;
                    }
                }
            });

            btn.textContent = "Select None";
        } else {

            fileItems.forEach(item => {
                item.classList.remove("selected");
                const checkbox = item.querySelector("input[type='checkbox']");
                if (checkbox) checkbox.checked = false;
            });

            window.selectedItems = new Set();
            window.selectedFileItem = null;
            btn.textContent = "Select All";
        }

        isSelectAllMode = !isSelectAllMode;

        updateActionButtons();
    });

    document.getElementById("btn-rename").addEventListener("click", () => {
        const selected = document.querySelector(".file-item.selected");
        if (!selected) {
            showStatus("Please select a file or folder to rename.", "error");
            return;
        }

        const nameElem = selected.querySelector(".file-item-name");
        const oldName = nameElem.textContent;

        showMeshDialog({
            title: "Rename",
            message: `Rename "${oldName}" to:`,
            input: true,
            defaultValue: oldName,
            onSubmit: (newName) => {
                if (newName && newName !== oldName) {

                    const selectedPath = window.selectedFilePath || selected.dataset.path;
                    const parentPath = selectedPath.substring(0, selectedPath.lastIndexOf("/"));
                    const newPath = `${parentPath}/${newName}`;
                    renameRemoteFile(selectedPath, newPath);
                } else {
                    showStatus("Invalid or same name. Rename cancelled.", "error");
                }
            }
        });
    });

    document.getElementById("btn-delete").addEventListener("click", async () => {
        const selectedItems = Array.from(document.querySelectorAll(".file-item input:checked"))
            .map(cb => cb.closest(".file-item"));

        if (selectedItems.length === 0) {
            showStatus("Please select files or folders to delete.", "error");
            return;
        }

        const textarea = document.getElementById("meshDialogTextArea");
        if (textarea) textarea.style.display = "none";

        showMeshDialog({
            title: "Delete",
            message: `Are you sure you want to delete ${selectedItems.length} item(s)?`,
            onSubmit: async (val, recursive) => {
                try {
                    for (const item of selectedItems) {
                        const filePath = item.dataset.path;
                        if (filePath) {
                            await httpClient.deleteFile(filePath);
                        }
                    }
                    await requestRemoteFileList(window.currentRemotePath);
                } catch (error) {
                    showStatus(`Error deleting files: ${error.message}`, "error");
                }
            }
        });
    });

    document.getElementById("btn-edit")?.addEventListener("click", async () => {
        if ((!window.selectedFileItem || !window.selectedFileItem.dataset) &&
            window.selectedItems && window.selectedItems.size === 1) {
            const selectedPath = [...window.selectedItems][0];
            const el = document.querySelector(`[data-path="${selectedPath}"]`);
            if (el) {
                window.selectedFileItem = el;
            }
        }

        const fileItem = window.selectedFileItem;
        const isDir = fileItem?.dataset?.isDir === "true";
        const filePath = fileItem?.dataset?.path;

        console.log("Selected file for edit:", filePath, "| isDir:", isDir);


        if (fileItem && !isDir && filePath) {
            try {
                const fileContent = await httpClient.editFile(filePath);
                currentEditingFilePath = filePath;
                document.getElementById("editTextarea").value = fileContent.content || "";
                document.getElementById("editDialogTitle").innerText = filePath.split("/").pop();
                document.getElementById("editDialog").style.display = "block";
            } catch (error) {
                showStatus(`Error opening file for edit: ${error.message}`, "error");
            }
        } else {
            showStatus("Please select a file to edit", "error");
        }
    });

    document.getElementById("btn-create-folder").addEventListener("click", () => {
        const textarea = document.getElementById("meshDialogTextArea");
        if (textarea) textarea.style.display = "none";

        showMeshDialog({
            title: "New Folder",
            message: "Enter folder name:",
            input: true,
            defaultValue: "",
            onSubmit: async (folderName) => {
                if (!folderName) {
                    showStatus("Folder name cannot be empty", "error");
                    return;
                }

                const fullPath = `${window.currentRemotePath}/${folderName}`;
                try {
                    await httpClient.createDirectory(fullPath);
                    await requestRemoteFileList(window.currentRemotePath);
                } catch (error) {
                    showStatus(`Error creating folder: ${error.message}`, "error");
                }
            }
        });
    });

    document.getElementById("btn-upload").addEventListener("click", () => {
        if (!window.currentRemotePath) {
            showStatus("No remote path selected", "error");
            return;
        }

        const uploadDialog = document.getElementById("uploadDialog");
        if (uploadDialog) {
            uploadDialog.style.display = "block";
            const fileInput = document.getElementById("uploadFileInput");
            if (fileInput) fileInput.value = "";
        } else {
            console.error('Upload dialog not found');
            showStatus("Upload dialog not available", "error");
        }
    });


    document.getElementById("uploadOkBtn").addEventListener("click", async () => {
        const fileInput = document.getElementById("uploadFileInput");
        const file = fileInput?.files?.[0];

        if (!file) {
            showStatus("Please select a file to upload", "error");
            return;
        }

        if (!window.currentRemotePath) {
            showStatus("No destination path selected", "error");
            return;
        }

        try {
            console.log('Starting upload:', file.name, 'to:', window.currentRemotePath);

            await httpClient.uploadFile(file, window.currentRemotePath);

            closeUploadDialog();

            fileInput.value = "";

            await requestRemoteFileList(window.currentRemotePath);
        } catch (error) {
            console.error('Upload failed:', error);
            showStatus(`Upload failed: ${error.message}`, "error");

        }
    });

    const btnDownload = document.getElementById("btn-download");
    if (!btnDownload) return;


    btnDownload.addEventListener("click", async () => {
        const selectedItems = [...document.querySelectorAll(".file-item input:checked")]
            .map(cb => cb.closest(".file-item"))
            .filter(item => item && item.dataset.isDir === "false");

        if (selectedItems.length === 0) {
            showStatus("Please select at least one file to download.", "warn");
            return;
        }

        try {

            for (const item of selectedItems) {
                const filePath = item.dataset.path;
                if (filePath) {
                    try {
                        await downloadFile(filePath);
                    } catch (error) {
                        console.error(`Failed to download ${filePath}:`, error);
                        showStatus(`Failed to download ${filePath}: ${error.message}`, "error");
                    }
                }
            }

        } catch (error) {
            showStatus(`Download failed: ${error.message}`, "error");
        }
    });


    document.getElementById("btn-cut").addEventListener("click", () => {
        const selectedPaths = Array.from(window.selectedItems || []);
        console.log('Cut operation - selected items:', selectedPaths);

        if (selectedPaths.length === 0) {
            showStatus("Please select files/folders to cut", "warn");
            return;
        }

        window.clipboardFileList = selectedPaths;
        window.clipboardMode = "cut";
        console.log("Clipboard set for cut:", window.clipboardFileList, window.clipboardMode);

        const pasteBtn = document.getElementById("btn-paste");
        if (pasteBtn) pasteBtn.disabled = false;

        updateActionButtons();
    });


    document.getElementById("btn-copy").addEventListener("click", () => {
        const selectedPaths = Array.from(window.selectedItems || []);
        console.log('Copy operation - selected items:', selectedPaths);

        if (selectedPaths.length === 0) {
            showStatus("Please select files/folders to copy", "warn");
            return;
        }

        window.clipboardFileList = selectedPaths;
        window.clipboardMode = "copy";
        console.log("Clipboard set for copy:", window.clipboardFileList, window.clipboardMode);

        const pasteBtn = document.getElementById("btn-paste");
        if (pasteBtn) pasteBtn.disabled = false;

        updateActionButtons();
    });


    document.getElementById("btn-paste").addEventListener("click", () => {

        if (!window.clipboardFileList || window.clipboardFileList.length === 0) {
            showStatus("No files in clipboard", "warn");
            return;
        }

        if (!window.currentRemotePath) {
            showStatus("No destination path selected", "error");
            return;
        }

        const textarea = document.getElementById("meshDialogTextArea");
        if (textarea) textarea.style.display = "none";

        showMeshDialog({
            title: "Paste",
            message: `Do you want to ${window.clipboardMode} ${window.clipboardFileList.length} item(s) here?`,
            onSubmit: () => pasteRemoteFile()
        });
    });



    document.getElementById("btn-zip").addEventListener("click", () => {
        const selectedItems = Array.from(document.querySelectorAll(".file-item input:checked"))
            .map(cb => cb.closest(".file-item"));

        if (selectedItems.length === 0) {
            showStatus("Please select files/folders to zip.", "error");
            return;
        }

        const paths = selectedItems.map(item => item.dataset.path);
        showMeshDialog({
            title: "Zip Filename",
            message: "Enter name for the zip file:",
            input: true,
            defaultValue: "archive.zip",
            onSubmit: async (zipName) => {
                if (!zipName) {
                    showStatus("Please provide a zip file name", "error");
                    return;
                }

                try {
                    const outputPath = `${window.currentRemotePath}/${zipName}`;
                    await httpClient.zipFiles(paths, outputPath);
                    await requestRemoteFileList(window.currentRemotePath);
                } catch (error) {
                    showStatus(`Error creating zip: ${error.message}`, "error");
                }
            }
        });
    });

    document.getElementById("btn-unzip").addEventListener("click", () => {
        const selectedItems = Array.from(document.querySelectorAll(".file-item input:checked"))
            .map(cb => cb.closest(".file-item"));

        if (selectedItems.length !== 1) {
            showStatus("Please select exactly one ZIP file to unzip.", "error");
            return;
        }

        const zipPath = selectedItems[0].dataset.path;
        showMeshDialog({
            title: "Unzip To Folder",
            message: "Enter destination folder path:",
            input: true,
            defaultValue: window.currentRemotePath + "/",
            onSubmit: async (destPath) => {
                if (!destPath) {
                    showStatus("Destination path is required", "error");
                    return;
                }

                try {
                    await httpClient.unzipFile(zipPath, destPath);
                    await requestRemoteFileList(window.currentRemotePath);
                } catch (error) {
                    showStatus(`Error unzipping: ${error.message}`, "error");
                }
            }
        });
    });

    document.getElementById("btn-refresh").addEventListener("click", () => {

        const currentPath = window.currentRemotePath || "C:/";
        requestRemoteFileList(currentPath);
    });

    document.getElementById("btn-find").addEventListener("click", () => {
        showMeshDialog({
            title: "Find Files",
            message: "Enter filename/foldername to filter:",
            input: true,
            defaultValue: "",
            onSubmit: (filter) => {
                if (!filter) {

                    return;
                }

                const items = document.querySelectorAll(".file-item");
                let matched = 0;


                items.forEach(item => {
                    item.classList.remove("selected");
                    const checkbox = item.querySelector("input[type='checkbox']");
                    if (checkbox) checkbox.checked = false;
                });
                window.selectedItems.clear();

                items.forEach(item => {
                    const name = item.querySelector(".file-item-name").textContent;
                    if (matchWildcard(name, filter)) {
                        item.classList.add("selected");
                        const checkbox = item.querySelector("input[type='checkbox']");
                        if (checkbox) checkbox.checked = true;
                        window.selectedItems.add(item.dataset.path);
                        matched++;
                        // Scroll to first match
                        if (matched === 1) {
                            item.scrollIntoView({ behavior: "smooth", block: "center" });
                        }
                    }
                });

                if (matched > 0) {

                } else {

                }

                updateActionButtons();
            }
        });
    });

    document.getElementById("btn-goto").addEventListener("click", () => {
        const textarea = document.getElementById("meshDialogTextArea");
        if (textarea) textarea.style.display = "none";
        showMeshDialog({
            title: "Go To Folder",
            message: "Enter path (e.g. C:/Users):",
            input: true,
            defaultValue: "C:/",
            onSubmit: async (path) => {
                if (path) {
                    try {

                        const normalizedPath = path.replace(/\\/g, '/').replace(/\/+/g, '/');
                        await requestRemoteFileList(normalizedPath);
                    } catch (error) {
                        showStatus(`Invalid path: ${error.message}`, "error");
                    }
                }
            }
        });
    });

    document.getElementById("btn-open")?.addEventListener("click", () => {
        const selectedPaths = Array.from(window.selectedItems || []);
        if (selectedPaths.length === 1) {
            openOpenDialog();
        } else {
            showStatus("Please select a single folder to open", "error");
        }
    });

    document.getElementById("sort-mode")?.addEventListener("change", () => {
        const sortMode = document.getElementById("sort-mode").value;
        if (!window.remoteFileList) return;

        let sorted = [...window.remoteFileList];

        if (sortMode.includes("type")) {

            sorted.sort((a, b) => {

                if (a.is_dir && !b.is_dir) return -1;
                if (!a.is_dir && b.is_dir) return 1;


                if (a.is_dir && b.is_dir) {
                    return a.name.localeCompare(b.name);
                }


                const extA = a.name.split('.').pop().toLowerCase() || '';
                const extB = b.name.split('.').pop().toLowerCase() || '';

                if (extA !== extB) {
                    return extA.localeCompare(extB);
                }


                return a.name.localeCompare(b.name);
            });
        } else if (sortMode.includes("name")) {
            sorted.sort((a, b) => a.name.localeCompare(b.name));
        } else if (sortMode.includes("size")) {
            sorted.sort((a, b) => (a.size || 0) - (b.size || 0));
        } else if (sortMode.includes("date")) {
            sorted.sort((a, b) => {
                const dateA = new Date(a.date || "");
                const dateB = new Date(b.date || "");
                return dateA - dateB;
            });
        }

        displayRemoteFileList(sorted, window.currentRemotePath);
    });

    document.getElementById("select-all-checkbox").addEventListener("change", (e) => {
        const checkboxes = document.querySelectorAll('.file-item-checkbox input[type="checkbox"]');
        checkboxes.forEach(cb => cb.checked = e.target.checked);
    });
}


export function isPathAllowed(path) {
    const normalizedPath = path.replace(/\\/g, '/').replace(/\/+/g, '/');
    const normalizedBasePath = RESTRICTED_BASE_PATH.replace(/\\/g, '/').replace(/\/+/g, '/');
    return normalizedPath.startsWith(normalizedBasePath);
}


export function matchWildcard(str, pattern) {
    const regex = new RegExp("^" + pattern.replace(/\./g, "\\.").replace(/\*/g, ".*") + "$", "i");
    return regex.test(str);
}
export function ensureEndsWithSlash(str) {
    return str.endsWith("/") ? str : str + "/";
}

export function displayRemoteFileList(entries, path) {
    window.remoteFileList = entries;

    const fileList = document.getElementById("remote-file-list");

    window.currentRemotePath = path;
    renderBreadcrumb(path);

    if (!entries || entries.length === 0) {
        fileList.innerHTML = '<div class="loading-message">No files found in this directory</div>';
        return;
    }

    const sortMode = document.getElementById("sort-mode")?.value || "type";
    let sorted = [...entries];

    if (sortMode === "type") {
        sorted.sort((a, b) => {

            if (a.is_dir && !b.is_dir) return -1;
            if (!a.is_dir && b.is_dir) return 1;

            if (a.is_dir && b.is_dir) {
                return a.name.localeCompare(b.name);
            }

            const extA = a.name.split('.').pop().toLowerCase() || '';
            const extB = b.name.split('.').pop().toLowerCase() || '';

            if (extA !== extB) {
                return extA.localeCompare(extB);
            }


            return a.name.localeCompare(b.name);
        });
    }

    fileList.innerHTML = sorted.map(file => {
        const filePath = path === "" ? file.name : (file.path || `${path}/${file.name}`);
        return `
            <div class="file-item" data-path="${filePath}" data-is-dir="${file.is_dir}" onclick="selectFile(event, this, ${file.is_dir})">
                <div class="file-item-checkbox">
                    <input type="checkbox" onclick="onCheckboxClick(event, this)">
                </div>
                <div class="file-item-icon">
                    ${file.is_dir ? '📁' : '📄'}
                </div>
                <div class="file-item-name">${file.name}</div>
                <div class="file-item-size">${!file.is_dir && file.size ? formatFileSize(file.size) : ''}</div>
                <div class="file-item-date">${!file.is_dir && file.date ? file.date : ''}</div>
            </div>
        `;
    }).join('');
    hideStatus();
    updatePasteButtonState();

    const selectAllBtn = document.getElementById("btn-select-all");
    if (selectAllBtn) {
        selectAllBtn.textContent = "Select All";
        isSelectAllMode = true;
    }

    window.selectedItems = new Set();
    window.selectedFilePath = null;
    window.selectedFileItem = null;


    const pasteBtn = document.getElementById("btn-paste");
    if (pasteBtn) {
        const hasClipboardData = (window.clipboardFileList && window.clipboardFileList.length > 0 && window.clipboardMode);
        pasteBtn.disabled = !hasClipboardData;

    }

    updateActionButtons();
}

export function onCheckboxClick(event, checkbox) {
    event.stopPropagation();
    const fileItem = checkbox.closest(".file-item");
    const isDir = fileItem?.dataset?.isDir === "true";
    selectFile(event, fileItem, isDir);
    updateActionButtons();

}

export function selectFile(event, element, isDir) {
    if (!window.selectedItems) window.selectedItems = new Set();

    const filePath = element.dataset?.path;
    const checkbox = element.querySelector("input[type='checkbox']");
    const isCheckboxClick = event.target.tagName === "INPUT" && event.target.type === "checkbox";

    if (!isCheckboxClick && !event.ctrlKey && !event.shiftKey) {

        document.querySelectorAll(".file-item.selected").forEach(item => {
            item.classList.remove("selected");
            const cb = item.querySelector("input[type='checkbox']");
            if (cb) cb.checked = false;
        });
        window.selectedItems.clear();
    }

    if (window.selectedItems.has(filePath)) {
        window.selectedItems.delete(filePath);
        element.classList.remove("selected");
        if (checkbox) checkbox.checked = false;
    } else {
        window.selectedItems.add(filePath);
        element.classList.add("selected");
        if (checkbox) checkbox.checked = true;
    }

    window.selectedFilePath = filePath;
    window.selectedFileItem = element;

    updateActionButtons();

    if (!isCheckboxClick && isDir && event.detail === 2) {
        setTimeout(() => {
            window.remoteHistory.push(window.currentRemotePath);
            navigateToPath(filePath);
        }, 100);
    }
}

export function updateActionButtons() {
    const selectedPaths = Array.from(window.selectedItems || []);
    const selectedCount = selectedPaths.length;
    const currentPath = window.currentRemotePath || "";

    const buttons = {
        rename: document.getElementById("btn-rename"),
        edit: document.getElementById("btn-edit"),
        open: document.getElementById("btn-open"),
        delete: document.getElementById("btn-delete"),
        download: document.getElementById("btn-download"),
        cut: document.getElementById("btn-cut"),
        copy: document.getElementById("btn-copy"),
        paste: document.getElementById("btn-paste"),
        zip: document.getElementById("btn-zip"),
        unzip: document.getElementById("btn-unzip"),
        selectAll: document.getElementById("btn-select-all"),
        refresh: document.getElementById("btn-refresh"),
        goto: document.getElementById("btn-goto")
    };

    Object.keys(buttons).forEach(key => {
        if (buttons[key] && !['selectAll', 'refresh', 'goto'].includes(key)) {
            buttons[key].disabled = true;
        }
    });

    ['selectAll', 'refresh', 'goto'].forEach(key => {
        if (buttons[key]) buttons[key].disabled = false;
    });

    if (selectedCount === 0) {

        if (window.clipboardFileList && window.clipboardFileList.length > 0 && window.clipboardMode && buttons.paste) {
            buttons.paste.disabled = false;
            console.log(" Paste re-enabled from NO-SELECTION state");
        }
        return;
    }

    if (selectedCount === 1) {
        const path = selectedPaths[0];
        const el = document.querySelector(`[data-path="${path}"]`);
        const isDir = el?.dataset?.isDir === "true";

        console.log(`Single selection: ${path}, isDir: ${isDir}`);

        if (isDir) {

            const enableForFolder = ['rename', 'delete', 'open', 'zip'];
            enableForFolder.forEach(key => {
                if (buttons[key]) buttons[key].disabled = false;
            });
        } else {

            const enableForFile = ['rename', 'edit', 'open', 'delete', 'download', 'cut', 'copy', 'zip'];
            enableForFile.forEach(key => {
                if (buttons[key]) buttons[key].disabled = false;
            });

            if (path.toLowerCase().endsWith(".zip")) {
                if (buttons.unzip) buttons.unzip.disabled = false;
            }
        }
    }

    if (selectedCount > 1) {

        let fileCount = 0, folderCount = 0;

        selectedPaths.forEach(path => {
            const el = document.querySelector(`[data-path="${path}"]`);
            if (el?.dataset?.isDir === "true") {
                folderCount++;
            } else {
                fileCount++;
            }
        });

        console.log(`Multiple selection: ${fileCount} files, ${folderCount} folders`);

        if (fileCount > 0 && folderCount === 0) {
            const enableForFiles = ['delete', 'download', 'cut', 'copy', 'zip'];
            enableForFiles.forEach(key => {
                if (buttons[key]) buttons[key].disabled = false;
            });
        } else if (fileCount === 0 && folderCount > 0) {

            const enableForFolders = ['delete', 'zip'];
            enableForFolders.forEach(key => {
                if (buttons[key]) buttons[key].disabled = false;
            });
        } else {

            const enableForMixed = ['delete', 'zip'];
            enableForMixed.forEach(key => {
                if (buttons[key]) buttons[key].disabled = false;
            });
        }
    }


    if (((window.clipboardFile && window.clipboardMode) || (window.clipboardFileList && window.clipboardFileList.length > 0 && window.clipboardMode)) && buttons.paste) {
        if (buttons.paste) {
            buttons.paste.disabled = false;
        } else {
            // console.warn("Paste button not found in DOM!");
        }
    }

    console.log("Selected count:", selectedCount, "Selected paths:", selectedPaths, "Current path:", currentPath);
}
export function navigateToPath(path) {
    requestRemoteFileList(path);
    updatePasteButtonState();
}

export async function downloadFile(filePath) {
    try {
        const blob = await httpClient.downloadFile(filePath);

        const filename = filePath.split(/[\\/]/).pop();
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.style.display = 'none';
        a.href = url;
        a.download = filename;
        document.body.appendChild(a);
        a.click();
        window.URL.revokeObjectURL(url);
        a.remove();

    } catch (error) {
        console.error('Download failed:', error);
        showStatus(`Download failed: ${error.message}`, "error");
    }
}

export async function deleteRemoteFile(filePath) {
    if (confirm(`Are you sure you want to delete: ${filePath}?`)) {
        try {
            await httpClient.deleteFile(filePath);
            await requestRemoteFileList(window.currentRemotePath);
        } catch (error) {
            console.error('Delete failed:', error);
            showStatus(`Delete failed: ${error.message}`, "error");
        }
    }
}

export async function pasteRemoteFile() {
    console.log('Paste operation starting:', {
        clipboardFileList: window.clipboardFileList,
        clipboardMode: window.clipboardMode,
        currentRemotePath: window.currentRemotePath
    });

    console.log('Clipboard file list type:', typeof window.clipboardFileList);
    console.log('Clipboard file list value:', window.clipboardFileList);
    console.log('Current remote path type:', typeof window.currentRemotePath);
    console.log('Current remote path value:', window.currentRemotePath);

    if (!window.clipboardFileList || window.clipboardFileList.length === 0) {
        showStatus("No files in clipboard", "error");
        return;
    }

    if (!window.currentRemotePath) {
        showStatus("No destination path selected", "error");
        return;
    }

    try {
        const operation = window.clipboardMode === 'cut' ? 'Moving' : 'Copying';
        console.log('About to call pasteFiles with:', {
            sourcePaths: window.clipboardFileList,
            destinationPath: window.currentRemotePath,
            mode: window.clipboardMode
        });

        const result = await httpClient.pasteFiles(
            window.clipboardFileList,
            window.currentRemotePath,
            window.clipboardMode
        );

        console.log('Paste operation result:', result);

        const pastTense = window.clipboardMode === 'cut' ? 'moved' : 'copied';

        if (window.clipboardMode === 'cut') {
            window.clipboardFileList = [];
            window.clipboardMode = null;
            const pasteBtn = document.getElementById("btn-paste");
            if (pasteBtn) pasteBtn.disabled = true;
            updateActionButtons();
        }

        await requestRemoteFileList(window.currentRemotePath);
    } catch (error) {
        console.error('Paste operation failed:', error);
        showStatus(`Error ${window.clipboardMode === 'cut' ? 'moving' : 'copying'} files: ${error.message}`, "error");
    }
}

export async function createRemoteFolder(folderPath) {
    try {
        await httpClient.createDirectory(folderPath);
        setTimeout(async () => {
            await requestRemoteFileList(window.currentRemotePath);
        }, 100);
    } catch (error) {
        console.error('Create folder failed:', error);
        showStatus(`Create folder failed: ${error.message}`, "error");
    }
}
export function handleFileDownload(msg) {
    console.log('File download handled via HTTP client');
}

export function updatePasteButtonState() {
    const btn = document.getElementById("btn-paste");
    if (btn && window.clipboardFileList && window.clipboardFileList.length > 0 && window.clipboardMode) {
        btn.disabled = false;
        console.log(" Paste re-enabled from updatePasteButtonState");
    }
}

export function updateConnectionStatus(connected) {
    const statusElement = document.getElementById("conn_status");
    statusElement.className = `conn_status ${connected ? 'connected' : 'disconnected'}`;
}

export function showStatus(message, type) {
    const statusElement = document.getElementById("status-message");
    statusElement.textContent = message;
    statusElement.className = `status-message ${type}`;
    statusElement.style.display = "block";
}

export function hideStatus() {
    const statusElement = document.getElementById("status-message");
    statusElement.style.display = "none";
}

export function formatFileSize(bytes) {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}

export function refresh() {
    if (window.fileTabConnected && window.currentRemotePath) {
        requestRemoteFileList(window.currentRemotePath);
    } else if (window.fileTabConnected) {
        requestRemoteFileList("C:\Program Files (x86)\e-Jan Networks");
    }
}

export const handler = {
    async send_files(id, path, to, include_hidden, is_remote) {
        try {
            if (!is_remote) {
                await httpClient.downloadFile(path);
            }
            jobManager.updateJobStatus(id, { finished: true });
        } catch (error) {
            console.error('Send files failed:', error);
        }
    },

    async delete_path(id, path, is_remote) {
        try {
            await httpClient.deleteFile(path);
            jobManager.updateJobStatus(id, { finished: true });
        } catch (error) {
            console.error('Delete path failed:', error);
        }
    },

    async create_dir(id, path, is_remote) {
        try {
            await httpClient.createDirectory(path);
            jobManager.updateJobStatus(id, { finished: true });
        } catch (error) {
            console.error('Create directory failed:', error);
        }
    },

    async read_remote_dir(path, show_hidden = false) {
        try {
            await requestRemoteFileList(path);
        } catch (error) {
            console.error('Read remote directory failed:', error);
        }
    }
};

export function closeEditDialog() {
    document.getElementById("editDialog").style.display = "none";
    currentEditingFilePath = null;
}

export async function saveEditedFile() {
    const content = document.getElementById("editTextarea").value;
    if (!currentEditingFilePath) {
        showStatus("No file is being edited", "error");
        return;
    }

    try {
        await httpClient.saveFile(currentEditingFilePath, content);
        closeEditDialog();
        await requestRemoteFileList(window.currentRemotePath);
    } catch (error) {
        showStatus(`Error saving file: ${error.message}`, "error");
    }
}

export function makeDialogDraggable(dialogId, headerId) {
    const dialog = document.getElementById(dialogId);
    const header = document.getElementById(headerId);
    let offsetX = 0, offsetY = 0, isDragging = false;

    header.addEventListener("mousedown", (e) => {
        isDragging = true;
        offsetX = e.clientX - dialog.offsetLeft;
        offsetY = e.clientY - dialog.offsetTop;
        document.body.style.userSelect = "none";
    });

    document.addEventListener("mouseup", () => {
        isDragging = false;
        document.body.style.userSelect = "";
    });

    document.addEventListener("mousemove", (e) => {
        if (isDragging) {
            dialog.style.position = "absolute";
            dialog.style.left = `${e.clientX - offsetX}px`;
            dialog.style.top = `${e.clientY - offsetY}px`;
        }
    });
}

makeDialogDraggable("uploadDialogBox", "uploadDialogHeader");
makeDialogDraggable("openDialogBox", "openDialogHeader");
makeDialogDraggable("customInputDialogBox", "customInputDialogHeader");
makeDialogDraggable("meshDialogBox", "meshDialogHeader");

export function openOpenDialog() {
    document.getElementById("openDialog").style.display = "block";
}

export function closeOpenDialog() {
    document.getElementById("openDialog").style.display = "none";
}

export async function sendOpenRequest() {
    if (!window.selectedFileItem || !window.selectedFileItem.dataset || window.selectedFileItem.dataset.isDir !== "true") {
        showStatus("Please select a folder to open", "error");
        return;
    }

    const selectedPath = window.selectedFileItem.dataset.path;
    console.log(" Sending open request for:", selectedPath);

    try {
        await httpClient.openFile(selectedPath);
        closeOpenDialog();
    } catch (error) {
        console.error('Open failed:', error);
        showStatus(`Open failed: ${error.message}`, "error");
    }
}

let customInputCallback = null;
export function showCustomInputDialog({ title, message, defaultValue = "", onSubmit }) {
    document.getElementById("customInputDialogTitle").textContent = title;
    document.getElementById("customInputDialogMessage").textContent = message;
    const input = document.getElementById("customInputDialogInput");
    input.value = defaultValue;
    customInputCallback = onSubmit;

    document.getElementById("customInputDialog").style.display = "block";
    input.focus();
}

export function closeCustomInputDialog() {
    document.getElementById("customInputDialog").style.display = "none";
    customInputCallback = null;
}

export function submitCustomInputDialog() {
    const input = document.getElementById("customInputDialogInput");
    const value = input.value;
    if (customInputCallback) {
        customInputCallback(value);
    }
    closeCustomInputDialog();
}

let meshDialogCallback = null;

export function showMeshDialog({ title, message, input = false, checkbox = false, checkboxLabel = "", defaultValue = "", onSubmit }) {
    document.getElementById("meshDialogTitle").textContent = title;
    document.getElementById("meshDialogMessage").textContent = message;

    const inputBox = document.getElementById("meshDialogInput");
    const checkboxBox = document.getElementById("meshDialogCheckboxContainer");
    const checkboxEl = document.getElementById("meshDialogCheckbox");
    const checkboxLabelEl = document.getElementById("meshDialogCheckboxLabel");

    inputBox.style.display = input ? "block" : "none";
    checkboxBox.style.display = checkbox ? "block" : "none";

    if (input) inputBox.value = defaultValue;
    if (checkbox) {
        checkboxEl.checked = false;
        checkboxLabelEl.textContent = checkboxLabel;
    }

    document.getElementById("meshDialog").style.display = "block";
    meshDialogCallback = () => {
        const val = input ? inputBox.value : null;
        const checked = checkbox ? checkboxEl.checked : null;
        onSubmit(val, checked);
        closeMeshDialog();
    };
}

export function closeMeshDialog() {
    document.getElementById("meshDialog").style.display = "none";
    meshDialogCallback = null;
}

document.getElementById("meshDialogOkBtn").onclick = () => {
    if (meshDialogCallback) meshDialogCallback();
};

export function closeUploadDialog() {
    document.getElementById("uploadDialog").style.display = "none";
    document.getElementById("uploadFileInput").value = "";
}
export function renderBreadcrumb(path) {
    const container = document.getElementById("current-path");
    if (!container) return;

    const restrictedBasePath = RESTRICTED_BASE_PATH.replace(/\\/g, '/').replace(/\/+/g, '/');
    const normalizedPath = path.replace(/\\/g, '/').replace(/\/+/g, '/');

    let relativePath = "";
    if (normalizedPath.startsWith(restrictedBasePath)) {
        relativePath = normalizedPath.substring(restrictedBasePath.length);
        if (relativePath.startsWith('/')) {
            relativePath = relativePath.substring(1);
        }
    }

    const segments = relativePath ? relativePath.split("/").filter(Boolean) : [];
    let accumulated = restrictedBasePath;

    const folderName = restrictedBasePath.split('/').pop() || "Restricted Folder";
    const parts = [`<span class="breadcrumb-root" data-path="${restrictedBasePath}">${folderName}</span>`];

    segments.forEach((seg, index) => {
        accumulated += "/" + seg;
        parts.push(`<span class="breadcrumb-segment" data-path="${accumulated}">${seg}</span>`);
    });

    container.innerHTML = parts.join(' <span class="breadcrumb-separator">/</span> ');

    document.querySelector(".breadcrumb-root")?.addEventListener("click", () => {
        window.remoteHistory.push(window.currentRemotePath);
        window.currentRemotePath = restrictedBasePath;
        navigateToPath(restrictedBasePath);
    });

    document.querySelectorAll(".breadcrumb-segment").forEach(span => {
        span.style.cursor = "pointer";
        span.addEventListener("click", () => {
            const targetPath = span.dataset.path;
            window.remoteHistory.push(window.currentRemotePath);
            window.currentRemotePath = targetPath;
            navigateToPath(targetPath);
        });
    });
}


export function renderJobTable() {
    const container = document.getElementById("job-table");
    if (!container) return;
    container.innerHTML = jobManager.jobs.map(job => `
        <div class="job">
            <div><b>${job.type.toUpperCase()}</b> ${job.path} → ${job.to || ""}</div>
            <div>
                ${job.finished ? " Finished" : " In Progress"}
                ${job.total_size ? ` - ${job.finished_size}/${job.total_size}` : ""}
            </div>
        </div>
    `).join("");
}

export const jobManager = {
    jobs: [],
    jobMap: {},

    getNextJobId() {
        return Date.now();
    },

    async addTransferJob(path, to, is_remote, include_hidden) {
        const id = this.getNextJobId();
        const job = {
            id, type: "transfer", path, to, is_remote, include_hidden,
            finished: false, finished_size: 0, total_size: 0, speed: 0, file_num: 0, entries: []
        };
        this.jobs.push(job);
        this.jobMap[id] = job;
        renderJobTable();

        try {

            if (!is_remote) {
                await httpClient.downloadFile(path);
                this.updateJobStatus(id, { finished: true });
            }
        } catch (error) {
            console.error('Transfer job failed:', error);
            showStatus(`Transfer failed: ${error.message}`, "error");
        }
    },

    async addDeleteJob(path, is_remote) {
        const id = this.getNextJobId();
        const job = { id, type: "delete", path, is_remote };
        this.jobs.push(job);
        this.jobMap[id] = job;
        renderJobTable();
        try {
            await httpClient.deleteFile(path);
            this.updateJobStatus(id, { finished: true });

            const parentPath = path.substring(0, path.lastIndexOf("/")) || "C:/";
            if (is_remote) {
                await requestRemoteFileList(parentPath);
            }
        } catch (error) {
            console.error('Delete job failed:', error);
            showStatus(`Delete failed: ${error.message}`, "error");
        }
    },

    updateJobStatus(id, update) {
        const job = this.jobMap[id];
        if (!job) return;
        Object.assign(job, update);
        renderJobTable();
    },

    clearAll() {
        this.jobs = [];
        this.jobMap = {};
        renderJobTable();
    }
};








window.closeUploadDialog = closeUploadDialog;
window.onCheckboxClick = onCheckboxClick;
window.selectFile = selectFile;
window.saveEditedFile = saveEditedFile;
window.closeEditDialog = closeEditDialog;
window.sendOpenRequest = sendOpenRequest;
window.closeOpenDialog = closeOpenDialog;
window.showCustomInputDialog = showCustomInputDialog;
window.closeCustomInputDialog = closeCustomInputDialog;
window.submitCustomInputDialog = submitCustomInputDialog;
window.closeMeshDialog = closeMeshDialog;
window.refresh = refresh;