use std::path::{Path, PathBuf};

/// Formats byte size into human-readable string (B, KB, MB, GB, TB).
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    const TB: u64 = 1024 * GB;

    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Shortens display path by replacing $HOME with `~`
pub fn format_path_for_display(path: &Path) -> String {
    let home = dirs::home_dir();
    if let Some(home_path) = home {
        if let Ok(relative) = path.strip_prefix(&home_path) {
            format!("~/{}", relative.display())
        } else {
            path.display().to_string()
        }
    } else {
        path.display().to_string()
    }
}

/// Truncates a string if it exceeds the maximum display length on TUI
pub fn truncate_string(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else if max_len <= 3 {
        "...".chars().take(max_len).collect()
    } else {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    }
}

/// Safely retrieves the user's home directory path
pub fn get_user_home() -> Option<PathBuf> {
    dirs::home_dir()
}
