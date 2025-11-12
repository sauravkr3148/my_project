pub fn clean_text_for_transmission(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_ascii() && (*c as u8) >= 32 || *c == '\n' || *c == '\r' || *c == '\t')
        .collect()
}

/// Format bytes to human readable format
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}
