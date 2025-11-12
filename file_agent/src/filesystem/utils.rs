use std::path::Path;

/// Extract filename from path
pub fn extract_filename(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

/// Validate file path
pub fn is_valid_path(path: &str) -> bool {
    !path.is_empty() && Path::new(path).exists()
}

/// Get file extension
pub fn get_file_extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
}
