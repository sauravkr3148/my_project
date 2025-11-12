use crate::{
    error::{Error, Result},
    filesystem::operations as fs_ops,
    system::info as system_info,
};
use futures_util::SinkExt;
use log::{debug, error};
use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::Mutex;

use tokio_tungstenite::{tungstenite::Message, MaybeTlsStream, WebSocketStream};
type WebSocketWriter = Arc<
    Mutex<
        futures_util::stream::SplitSink<
            WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
            Message,
        >,
    >,
>;

/// Handles incoming WebSocket messages and routes them to appropriate handlers
#[derive(Clone)]
pub struct MessageHandler;

impl MessageHandler {
    pub fn new() -> Self {
        Self
    }

    /// Handle incoming text messages
    pub async fn handle_text_message(&self, text: &str, writer: &WebSocketWriter) -> Result<()> {
        debug!("Received text message: {}", text);

        let msg: Value = serde_json::from_str(text).map_err(|e| Error::Json(e))?;

        let msg_type = msg["type"].as_str().unwrap_or_default();

        match msg_type {
            "list_remote" => handle_list_remote(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "rename" => handle_rename(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "delete" => handle_delete(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "create_folder" => handle_folder_creation(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "upload_file" => handle_upload_file(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "download_file" => handle_download_file(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "paste_file" => handle_paste_file(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "edit_file" => handle_edit_file(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "save_file" => handle_save_file(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "zip_file" => handle_zip_file(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "unzip_file" => handle_unzip_file(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "open_file" => handle_open_file(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "get_agent_details" => handle_get_agent_details(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            "get_installed_software" => handle_get_installed_software(&msg, writer)
                .await
                .map_err(|e| Error::Network(e)),
            _ => {
                error!("Unknown message type: {}", msg_type);
                Ok(())
            }
        }
    }

    pub async fn handle_binary_message(&self, data: &[u8], writer: &WebSocketWriter) -> Result<()> {
        if data.len() >= 6 {
            let msg_type = u16::from_be_bytes([data[0], data[1]]);
            match msg_type {
                73 => {
                    if data.len() == 6 && data[4] == 0 && data[5] == 1 {
                        debug!("Received unpause command");
                    }
                }
                74 => {
                    if data.len() == 6 && data[4] == 0 && data[5] == 1 {
                        debug!("Received pause command");
                    }
                }
                _ => {
                    debug!("Unknown binary message type: {}", msg_type);
                }
            }
        }
        Ok(())
    }
}

async fn handle_list_remote(
    msg: &Value,
    writer: &WebSocketWriter,
) -> std::result::Result<(), String> {
    let path = msg["path"].as_str().unwrap_or("");
    let request_id = msg["request_id"].as_str();

    let mut response = if path.is_empty() {
        // Return drives in the format expected by frontend
        let drives = fs_ops::get_drives();
        let entries: Vec<serde_json::Value> = drives
            .into_iter()
            .map(|drive| {
                json!({
                    "name": drive,
                    "is_dir": true,
                    "size": 0,
                    "date": "Drive"
                })
            })
            .collect();

        json!({
            "entries": entries,
            "path": path
        })
    } else {
        // List directory contents in the expected format
        match std::fs::read_dir(path) {
            Ok(entries) => {
                let entries: Vec<serde_json::Value> = entries
                    .filter_map(|res| res.ok())
                    .filter_map(|entry| {
                        let metadata = entry.metadata().ok()?;
                        let modified = metadata.modified().ok().and_then(|time| {
                            let datetime: chrono::DateTime<chrono::Local> = time.into();
                            Some(datetime.format("%d/%m/%Y, %H:%M:%S").to_string())
                        });

                        Some(json!({
                            "name": entry.file_name().to_string_lossy(),
                            "is_dir": metadata.is_dir(),
                            "size": metadata.len(),
                            "date": modified.unwrap_or_else(|| "Unknown".to_string())
                        }))
                    })
                    .collect();

                json!({
                    "entries": entries,
                    "path": path
                })
            }
            Err(e) => {
                json!({
                    "entries": [],
                    "path": path,
                    "error": format!("Failed to list directory: {}", e)
                })
            }
        }
    };

    if let Some(req_id) = request_id {
        response["request_id"] = json!(req_id);
    }

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("List remote successfully");

    Ok(())
}
async fn handle_rename(msg: &Value, writer: &WebSocketWriter) -> std::result::Result<(), String> {
    let request_id = msg["request_id"].as_str();
    let response = match fs_ops::handle_rename(msg) {
        Ok(_) => {
            let mut response_json = json!({
                "type": "rename_result",
                "status": "success",
                "old_path": msg["old_path"],
                "new_path": msg["new_path"]
            });
            if let Some(req_id) = request_id {
                response_json["request_id"] = json!(req_id);
            }
            response_json
        }
        Err(e) => {
            let mut error_json = json!({
                "type": "error",
                "message": format!("Rename failed: {}", e)
            });
            if let Some(req_id) = request_id {
                error_json["request_id"] = json!(req_id);
            }
            error_json
        }
    };

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("Rename successfully {:?}", msg);

    Ok(())
}

async fn handle_delete(msg: &Value, writer: &WebSocketWriter) -> std::result::Result<(), String> {
    let request_id = msg["request_id"].as_str();
    let response = match fs_ops::handle_delete(msg) {
        Ok(_) => {
            let mut response_json = json!({
                "type": "delete_result",
                "status": "success"
            });
            if let Some(req_id) = request_id {
                response_json["request_id"] = json!(req_id);
            }
            response_json
        }
        Err(e) => {
            let mut error_json = json!({
                "type": "error",
                "message": format!("Delete failed: {}", e)
            });
            if let Some(req_id) = request_id {
                error_json["request_id"] = json!(req_id);
            }
            error_json
        }
    };

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("File deleted successfully: {:?}", msg);

    Ok(())
}

async fn handle_folder_creation(
    msg: &Value,
    writer: &WebSocketWriter,
) -> std::result::Result<(), String> {
    let request_id = msg["request_id"].as_str();
    let response = match fs_ops::handle_folder_creation(msg) {
        Ok(_) => {
            let mut response_json = json!({
                "type": "create_folder_result",
                "status": "success"
            });
            if let Some(req_id) = request_id {
                response_json["request_id"] = json!(req_id);
            }
            response_json
        }
        Err(e) => {
            let mut error_json = json!({
                "type": "error",
                "message": format!("Folder creation failed: {}", e)
            });
            if let Some(req_id) = request_id {
                error_json["request_id"] = json!(req_id);
            }
            error_json
        }
    };

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("Folder created successfully: {:?}", msg);

    Ok(())
}

async fn handle_upload_file(
    msg: &Value,
    writer: &WebSocketWriter,
) -> std::result::Result<(), String> {
    let filename = msg["filename"].as_str().unwrap_or("");
    let path = msg["path"].as_str().unwrap_or("");
    let content_base64 = msg["content_base64"].as_str().unwrap_or("");
    let request_id = msg["request_id"].as_str();

    let response = match fs_ops::handle_upload_file(path, filename, content_base64) {
        Ok(_) => {
            let full_path = if path.is_empty() {
                filename.to_string()
            } else {
                format!("{}/{}", path.trim_end_matches('/'), filename)
            };

            let mut response_json = json!({
                "type": "upload_file_result",
                "status": "success",
                "path": full_path
            });
            if let Some(req_id) = request_id {
                response_json["request_id"] = json!(req_id);
            }
            response_json
        }
        Err(e) => {
            let mut error_json = json!({
                "type": "error",
                "message": format!("Upload failed: {}", e)
            });
            if let Some(req_id) = request_id {
                error_json["request_id"] = json!(req_id);
            }
            error_json
        }
    };

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("File uploaded successfully: {}", filename);
    Ok(())
}
async fn handle_paste_file(
    msg: &Value,
    writer: &WebSocketWriter,
) -> std::result::Result<(), String> {
    let request_id = msg["request_id"].as_str();

    let response = match fs_ops::handle_paste_multiple(msg) {
        Ok(_) => {
            let mut response = json!({
                "paste_file_result": "success"
            });

            if let Some(id) = request_id {
                response["request_id"] = json!(id);
            }

            response
        }
        Err(e) => {
            let mut response = json!({
                "error": format!("{}", e)
            });

            if let Some(id) = request_id {
                response["request_id"] = json!(id);
            }

            response
        }
    };

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("File pasted successfully: {:?}", msg);

    Ok(())
}

async fn handle_download_file(
    msg: &Value,
    writer: &WebSocketWriter,
) -> std::result::Result<(), String> {
    let path = msg["path"].as_str().unwrap_or("");
    let request_id = msg["request_id"].as_str();

    let mut response = match fs_ops::handle_download_file(path) {
        Ok(response) => {
            // Convert to object to allow mutation
            let mut obj = response.as_object().cloned().unwrap_or_default();

            if let Some(data) = obj.remove("data") {
                obj.insert("content".to_string(), data);
            }

            Value::Object(obj)
        }
        Err(err_msg) => json!({
            "type": "error",
            "action": "download_file",
            "message": format!("{}", err_msg)
        }),
    };

    if let Some(req_id) = request_id {
        response["request_id"] = json!(req_id);
    }

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("File downloaded successfully: {:?}", msg);

    Ok(())
}

async fn handle_edit_file(
    msg: &Value,
    writer: &WebSocketWriter,
) -> std::result::Result<(), String> {
    let path = msg["path"].as_str().unwrap_or("");
    let request_id = msg["request_id"].as_str();

    let mut response = match fs_ops::handle_edit_file(path) {
        Ok(response) => response,
        Err(e) => json!({
            "type": "error",
            "message": format!("Edit failed: {}", e)
        }),
    };

    if let Some(req_id) = request_id {
        response["request_id"] = json!(req_id);
    }

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("File edited successfully: {:?}", msg);

    Ok(())
}

async fn handle_save_file(
    msg: &Value,
    writer: &WebSocketWriter,
) -> std::result::Result<(), String> {
    let request_id = msg["request_id"].as_str();
    let path = msg["path"].as_str().unwrap_or("");
    let content = msg["content"].as_str().unwrap_or("");

    let response = match std::fs::write(path, content) {
        Ok(_) => {
            let mut response_json = json!({
                "type": "save_file_result",
                "status": "success",
                "path": path
            });
            if let Some(req_id) = request_id {
                response_json["request_id"] = json!(req_id);
            }
            response_json
        }
        Err(e) => {
            let mut error_json = json!({
                "type": "error",
                "message": format!("Save failed: {}", e)
            });
            if let Some(req_id) = request_id {
                error_json["request_id"] = json!(req_id);
            }
            error_json
        }
    };

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("File saved successfully: {:?}", msg);

    Ok(())
}

async fn handle_zip_file(msg: &Value, writer: &WebSocketWriter) -> std::result::Result<(), String> {
    let paths: Vec<String> = msg["target_list"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|p| p.as_str().map(|s| s.to_string()))
        .collect();

    let zip_name = msg["zip_name"]
        .as_str()
        .unwrap_or("archive.zip")
        .to_string();

    let request_id = msg["request_id"].as_str();

    let mut response = if paths.is_empty() {
        json!({
            "type": "error",
            "message": "No files selected for zip"
        })
    } else {
        match fs_ops::handle_zip_files(&paths, &zip_name) {
            Ok(result) => result,
            Err(e) => json!({
                "type": "error",
                "message": format!("Zip failed: {}", e)
            }),
        }
    };

    if let Some(req_id) = request_id {
        response["request_id"] = json!(req_id);
    }

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("File zipped successfully: {:?}", msg);

    Ok(())
}

async fn handle_unzip_file(
    msg: &Value,
    writer: &WebSocketWriter,
) -> std::result::Result<(), String> {
    let source = msg["source"].as_str().unwrap_or("");
    let target = msg["target"].as_str().unwrap_or("");
    let request_id = msg["request_id"].as_str();

    let mut response = if source.is_empty() || target.is_empty() {
        json!({
            "type": "error",
            "message": "Source or target path missing for unzip."
        })
    } else {
        match fs_ops::handle_unzip_file(source, target) {
            Ok(result) => result,
            Err(e) => json!({
                "type": "error",
                "message": format!("Unzip failed: {}", e)
            }),
        }
    };

    if let Some(req_id) = request_id {
        response["request_id"] = json!(req_id);
    }

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("File unzipped successfully: {:?}", msg);

    Ok(())
}

async fn handle_open_file(
    msg: &Value,
    writer: &WebSocketWriter,
) -> std::result::Result<(), String> {
    // Extract request_id from the message
    let request_id = msg["request_id"].as_str().unwrap_or("");

    let mut raw_path = msg["path"].as_str().unwrap_or("").to_string();

    if raw_path.is_empty() {
        debug!("Empty path received for open_file");

        let error_response = json!({
            "request_id": request_id,
            "success": false,
            "error": "Empty path received"
        });

        let mut writer_guard = writer.lock().await;
        writer_guard
            .send(Message::Text(error_response.to_string()))
            .await
            .map_err(|e| format!("Failed to send error response: {}", e))?;
        return Ok(());
    }

    // Clean up double slashes and convert to Windows path format
    while raw_path.contains("//") {
        raw_path = raw_path.replace("//", "/");
    }
    let path = raw_path.replace("/", "\\");

    debug!("Opening path: {}", path);

    let result = {
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("explorer.exe")
                .arg(&path)
                .spawn()
        }

        #[cfg(not(target_os = "windows"))]
        {
            std::process::Command::new("xdg-open").arg(&path).spawn()
        }
    };

    let response = match result {
        Ok(_) => {
            debug!("Explorer launched for: {}", path);

            json!({
                "request_id": request_id,
                "success": true,
                "message": "File opened successfully"
            })
        }
        Err(e) => {
            debug!("Failed to open path: {}", e);

            json!({
                "request_id": request_id,
                "success": false,
                "error": format!("Failed to open path: {}", e)
            })
        }
    };

    let mut writer_guard = writer.lock().await;
    writer_guard
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("File opened successfully: {:?}", msg);

    Ok(())
}

async fn handle_get_agent_details(
    msg: &Value,
    writer: &WebSocketWriter,
) -> std::result::Result<(), String> {
    let request_id = msg["request_id"].as_str();

    let mut response = system_info::get_agent_details();

    if let Some(req_id) = request_id {
        response["request_id"] = json!(req_id);
    }

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("Agent details fetched successfully");
    debug!("Agent details: {:?}", response);

    Ok(())
}

async fn handle_get_installed_software(
    msg: &Value,
    writer: &WebSocketWriter,
) -> std::result::Result<(), String> {
    let request_id = msg["request_id"].as_str();

    let mut response = system_info::get_installed_software();

    if let Some(req_id) = request_id {
        response["request_id"] = json!(req_id);
    }

    let mut writer = writer.lock().await;
    writer
        .send(Message::Text(response.to_string()))
        .await
        .map_err(|e| format!("Failed to send response: {}", e))?;
    println!("Installed software fetched successfully:");

    Ok(())
}
