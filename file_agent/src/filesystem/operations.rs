use crate::error::{Error, Result};
use base64::{engine::general_purpose, Engine as _};
use serde_json::{json, Value};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use walkdir::WalkDir;
use zip::{write::FileOptions, ZipArchive, ZipWriter};

/// Get available drives on the system
pub fn get_drives() -> Vec<String> {
    #[cfg(windows)]
    {
        let mut drives = Vec::new();
        // On Windows, check drives A-Z
        for drive_letter in b'A'..=b'Z' {
            let drive_path = format!("{}:\\", drive_letter as char);
            if Path::new(&drive_path).exists() {
                drives.push(format!("{}:", drive_letter as char));
            }
        }
        drives
    }
    #[cfg(unix)]
    {
        vec!["/".to_string()]
    }
    #[cfg(not(any(windows, unix)))]
    {
        vec!["/".to_string()]
    }
}

/// Handle file/folder rename operation
pub fn handle_rename(msg: &Value) -> Result<()> {
    let old_path = msg["old_path"]
        .as_str()
        .ok_or(Error::FileSystem("Missing old_path".to_string()))?;
    let new_name = msg["new_name"]
        .as_str()
        .ok_or(Error::FileSystem("Missing new_name".to_string()))?;

    if old_path.is_empty() || new_name.is_empty() {
        return Err(Error::FileSystem("Empty path or name provided".to_string()));
    }

    let old_path = Path::new(old_path);
    if let Some(parent) = old_path.parent() {
        let new_path = parent.join(new_name);
        std::fs::rename(old_path, new_path)
            .map_err(|e| Error::FileSystem(format!("Failed to rename: {}", e)))?;
    } else {
        return Err(Error::FileSystem(
            "Cannot determine parent directory".to_string(),
        ));
    }
    Ok(())
}

/// Handle file/folder deletion
pub fn handle_delete(msg: &Value) -> Result<()> {
    let path = msg["path"]
        .as_str()
        .ok_or(Error::FileSystem("Missing path".to_string()))?;

    if path.is_empty() {
        return Err(Error::FileSystem("Empty path provided".to_string()));
    }

    let path = Path::new(path);
    if path.is_file() {
        fs::remove_file(path)
            .map_err(|e| Error::FileSystem(format!("Failed to delete file: {}", e)))?;
    } else if path.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|e| Error::FileSystem(format!("Failed to delete directory: {}", e)))?;
    } else {
        return Err(Error::FileSystem("Path does not exist".to_string()));
    }
    Ok(())
}

/// Handle folder creation
pub fn handle_folder_creation(msg: &Value) -> Result<()> {
    // Support both formats: path + folder_name OR just path
    let path = if let Some(folder_name) = msg["folder_name"].as_str() {
        let base_path = msg["path"]
            .as_str()
            .ok_or(Error::FileSystem("Missing path".to_string()))?;
        Path::new(base_path)
            .join(folder_name)
            .to_string_lossy()
            .to_string()
    } else {
        msg["path"]
            .as_str()
            .ok_or(Error::FileSystem("Missing path".to_string()))?
            .to_string()
    };

    if path.is_empty() {
        return Err(Error::FileSystem("Empty path provided".to_string()));
    }

    fs::create_dir_all(&path)
        .map_err(|e| Error::FileSystem(format!("Failed to create folder: {}", e)))?;
    Ok(())
}

/// Handle file editing (read file content)
pub fn handle_edit_file(path: &str) -> Result<Value> {
    if path.is_empty() {
        return Err(Error::FileSystem("Empty path provided".to_string()));
    }

    let content = fs::read_to_string(path)
        .map_err(|e| Error::FileSystem(format!("Failed to read file: {}", e)))?;

    Ok(json!({
        "type": "edit_file",
        "path": path,
        "content": content
    }))
}

/// Handle file download (encode file as base64)
pub fn handle_download_file(path: &str) -> Result<Value> {
    if path.is_empty() {
        return Err(Error::FileSystem("Empty path provided".to_string()));
    }

    let mut file =
        File::open(path).map_err(|e| Error::FileSystem(format!("Failed to open file: {}", e)))?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| Error::FileSystem(format!("Failed to read file: {}", e)))?;

    let encoded = general_purpose::STANDARD.encode(&buffer);
    let filename = Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    Ok(json!({
        "type": "download_file",
        "filename": filename,
        "content": encoded
    }))
}

/// Handle file upload
pub fn handle_upload_file(dir_path: &str, filename: &str, base64_content: &str) -> Result<()> {
    if dir_path.is_empty() || filename.is_empty() || base64_content.is_empty() {
        return Err(Error::FileSystem("Missing required parameters".to_string()));
    }

    let decoded = general_purpose::STANDARD
        .decode(base64_content)
        .map_err(|e| Error::FileSystem(format!("Failed to decode base64: {}", e)))?;

    let file_path = Path::new(dir_path).join(filename);
    let mut file = File::create(&file_path)
        .map_err(|e| Error::FileSystem(format!("Failed to create file: {}", e)))?;

    file.write_all(&decoded)
        .map_err(|e| Error::FileSystem(format!("Failed to write file: {}", e)))?;

    Ok(())
}

/// Handle paste operation
pub fn handle_paste_multiple(msg: &Value) -> Result<()> {
    // Support both old and new parameter formats for compatibility
    let source_paths = msg["source_paths"]
        .as_array()
        .or_else(|| msg["from_list"].as_array())
        .ok_or(Error::FileSystem(
            "Missing source_paths or from_list".to_string(),
        ))?;

    let target_path = msg["target_path"]
        .as_str()
        .or_else(|| msg["to"].as_str())
        .ok_or(Error::FileSystem("Missing target_path or to".to_string()))?;

    let operation = msg["operation"]
        .as_str()
        .or_else(|| msg["mode"].as_str())
        .unwrap_or("copy");

    for source_value in source_paths {
        let source_path = source_value
            .as_str()
            .ok_or(Error::FileSystem("Invalid source path".to_string()))?;
        let source = Path::new(source_path);
        let filename = extract_filename(source_path);
        let target = Path::new(target_path).join(&filename);

        if source.is_file() {
            if operation == "move" || operation == "cut" {
                fs::rename(source, &target)
                    .map_err(|e| Error::FileSystem(format!("Failed to move file: {}", e)))?;
            } else {
                fs::copy(source, &target)
                    .map_err(|e| Error::FileSystem(format!("Failed to copy file: {}", e)))?;
            }
        } else if source.is_dir() {
            copy_dir_all(source, &target)?;
            if operation == "move" || operation == "cut" {
                fs::remove_dir_all(source).map_err(|e| {
                    Error::FileSystem(format!("Failed to remove source directory: {}", e))
                })?;
            }
        }
    }
    Ok(())
}
// Add separate copy and cut handlers for compatibility
pub fn handle_copy_files(msg: &Value) -> Result<()> {
    let mut copy_msg = msg.clone();
    copy_msg["operation"] = json!("copy");
    handle_paste_multiple(&copy_msg)
}

pub fn handle_cut_files(msg: &Value) -> Result<()> {
    let mut cut_msg = msg.clone();
    cut_msg["operation"] = json!("cut");
    handle_paste_multiple(&cut_msg)
}
fn extract_filename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)
        .map_err(|e| Error::FileSystem(format!("Failed to create directory: {}", e)))?;
    for entry in fs::read_dir(src)
        .map_err(|e| Error::FileSystem(format!("Failed to read directory: {}", e)))?
    {
        let entry = entry.map_err(|e| Error::FileSystem(format!("Failed to read entry: {}", e)))?;
        let ty = entry
            .file_type()
            .map_err(|e| Error::FileSystem(format!("Failed to get file type: {}", e)))?;
        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))
                .map_err(|e| Error::FileSystem(format!("Failed to copy file: {}", e)))?;
        }
    }
    Ok(())
}

/// Handle zip operation
pub fn handle_zip_files(paths: &[String], zip_name: &str) -> Result<Value> {
    if paths.is_empty() {
        return Err(Error::FileSystem("No input paths provided".to_string()));
    }

    if zip_name.is_empty() {
        return Err(Error::FileSystem("No zip name provided".to_string()));
    }

    let parent = Path::new(&paths[0]).parent().unwrap_or(Path::new("."));
    let target = parent.join(zip_name);

    let file = File::create(&target)
        .map_err(|e| Error::FileSystem(format!("Cannot create zip file: {}", e)))?;
    let mut zip = ZipWriter::new(file);
    let options = FileOptions::default().compression_method(zip::CompressionMethod::Stored);

    for path_str in paths {
        let path = Path::new(path_str);

        if path.is_file() {
            add_file_to_zip(&mut zip, path, &options)?;
        } else if path.is_dir() {
            add_directory_to_zip(&mut zip, path, &options)?;
        }
    }

    zip.finish()
        .map_err(|e| Error::FileSystem(format!("Failed to finalize zip: {}", e)))?;

    Ok(json!({
        "type": "zip_file_result",
        "path": target.display().to_string()
    }))
}

fn add_file_to_zip(zip: &mut ZipWriter<File>, path: &Path, options: &FileOptions) -> Result<()> {
    let name = path.file_name().unwrap().to_string_lossy();
    let content = fs::read(path)
        .map_err(|e| Error::FileSystem(format!("Failed to read file {}: {}", path.display(), e)))?;

    zip.start_file(name, *options)
        .map_err(|e| Error::FileSystem(format!("Failed to start zip file entry: {}", e)))?;
    zip.write_all(&content)
        .map_err(|e| Error::FileSystem(format!("Failed to write to zip: {}", e)))?;

    Ok(())
}

fn add_directory_to_zip(
    zip: &mut ZipWriter<File>,
    path: &Path,
    options: &FileOptions,
) -> Result<()> {
    let base_name = path.file_name().unwrap().to_string_lossy();
    let mut has_entries = false;

    for entry_result in WalkDir::new(path).min_depth(1) {
        has_entries = true;
        let entry =
            entry_result.map_err(|e| Error::FileSystem(format!("Walk directory error: {}", e)))?;
        let entry_path = entry.path();

        let rel_path = entry_path
            .strip_prefix(path)
            .map_err(|e| Error::FileSystem(format!("Failed to strip prefix: {}", e)))?;
        let zip_path = format!("{}/{}", base_name, rel_path.to_string_lossy());

        if entry_path.is_file() {
            zip.start_file(&zip_path, *options)
                .map_err(|e| Error::FileSystem(format!("Failed to start zip file entry: {}", e)))?;

            let mut f = File::open(entry_path)
                .map_err(|e| Error::FileSystem(format!("Failed to open file: {}", e)))?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)
                .map_err(|e| Error::FileSystem(format!("Failed to read file: {}", e)))?;

            zip.write_all(&buffer)
                .map_err(|e| Error::FileSystem(format!("Failed to write to zip: {}", e)))?;
        } else if entry_path.is_dir() {
            zip.add_directory(format!("{}/", zip_path), *options)
                .map_err(|e| Error::FileSystem(format!("Failed to add directory to zip: {}", e)))?;
        }
    }

    // If folder is completely empty
    if !has_entries {
        zip.add_directory(format!("{}/", base_name), *options)
            .map_err(|e| {
                Error::FileSystem(format!("Failed to add empty directory to zip: {}", e))
            })?;
    }

    Ok(())
}

/// Handle unzip operation
pub fn handle_unzip_file(source: &str, target: &str) -> Result<Value> {
    if source.is_empty() || target.is_empty() {
        return Err(Error::FileSystem(
            "Source or target path is empty".to_string(),
        ));
    }

    let zip_file = File::open(source)
        .map_err(|e| Error::FileSystem(format!("Failed to open zip file: {}", e)))?;
    let mut archive = ZipArchive::new(zip_file)
        .map_err(|e| Error::FileSystem(format!("Failed to read zip archive: {}", e)))?;

    fs::create_dir_all(target)
        .map_err(|e| Error::FileSystem(format!("Failed to create target directory: {}", e)))?;

    let zip_name = Path::new(source)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy();
    let base_folder = Path::new(target).join(zip_name.to_string());

    fs::create_dir_all(&base_folder)
        .map_err(|e| Error::FileSystem(format!("Failed to create base folder: {}", e)))?;

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| Error::FileSystem(format!("Failed to access zip entry {}: {}", i, e)))?;

        let outpath = base_folder.join(file.mangled_name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)
                .map_err(|e| Error::FileSystem(format!("Failed to create directory: {}", e)))?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p).map_err(|e| {
                    Error::FileSystem(format!("Failed to create parent directory: {}", e))
                })?;
            }

            let mut outfile = File::create(&outpath)
                .map_err(|e| Error::FileSystem(format!("Failed to create output file: {}", e)))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| Error::FileSystem(format!("Failed to extract file: {}", e)))?;
        }
    }

    Ok(json!({
        "type": "unzip_file_result",
        "path": base_folder.display().to_string()
    }))
}
