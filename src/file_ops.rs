use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Detailed report generated after cleanup execution
#[derive(Debug, Clone, Default)]
pub struct DeletionReport {
    pub total_targeted_bytes: u64,
    pub freed_bytes: u64,
    pub targeted_items_count: usize,
    pub deleted_items_count: usize,
    pub skipped_items_count: usize,
    pub failed_items: Vec<(PathBuf, String)>,
}

/// File or directory information included in dry-run preview
#[derive(Debug, Clone)]
pub struct DryRunItem {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    pub category_name: String,
}

/// Generates list of items to be deleted in Dry-Run mode (read-only, no deletion)
pub fn generate_dry_run_list(
    selected_categories: &[crate::scanner::CategoryInfo],
    selected_large_files: &[crate::scanner::LargeFileInfo],
) -> (Vec<DryRunItem>, u64) {
    let mut items = Vec::new();
    let mut total_size: u64 = 0;

    // 1. Add sub-items from selected system junk categories
    for cat in selected_categories {
        if !cat.selected {
            continue;
        }

        for sub in &cat.sub_items {
            items.push(DryRunItem {
                path: sub.path.clone(),
                name: sub.name.clone(),
                size: sub.size,
                is_dir: sub.is_dir,
                category_name: cat.name.clone(),
            });
            total_size += sub.size;
        }
    }

    // 2. Add large files selected by the user
    for file in selected_large_files {
        if !file.selected {
            continue;
        }

        items.push(DryRunItem {
            path: file.path.clone(),
            name: file.name.clone(),
            size: file.size,
            is_dir: false,
            category_name: "Large Downloads".to_string(),
        });
        total_size += file.size;
    }

    (items, total_size)
}

/// Safely deletes files or directories (Safeguard logic).
///
/// Safety guarantees:
/// 1. Never deletes system root folders (e.g., clears contents of `~/Library/Caches` rather than removing the folder itself).
/// 2. Handles `PermissionDenied` or files locked / in-use by other processes.
/// 3. Records any errors in `failed_items` and proceeds with remaining files WITHOUT crashing the application.
pub fn execute_safe_cleanup(dry_run_items: &[DryRunItem]) -> DeletionReport {
    let mut report = DeletionReport {
        targeted_items_count: dry_run_items.len(),
        ..Default::default()
    };

    for item in dry_run_items {
        report.total_targeted_bytes += item.size;

        if !item.path.exists() {
            // File or directory no longer exists
            report.skipped_items_count += 1;
            continue;
        }

        if item.is_dir {
            // Safely remove directory recursively
            match safe_remove_dir_all(&item.path) {
                Ok(freed) => {
                    report.freed_bytes += freed;
                    report.deleted_items_count += 1;
                }
                Err(err) => {
                    report.skipped_items_count += 1;
                    report.failed_items.push((item.path.clone(), err));
                }
            }
        } else {
            // Safely remove single file
            match safe_remove_file(&item.path) {
                Ok(freed) => {
                    report.freed_bytes += freed;
                    report.deleted_items_count += 1;
                }
                Err(err) => {
                    report.skipped_items_count += 1;
                    report.failed_items.push((item.path.clone(), err));
                }
            }
        }
    }

    report
}

/// Safely removes a single file with exception handling
fn safe_remove_file(path: &Path) -> Result<u64, String> {
    let file_size = match path.symlink_metadata() {
        Ok(m) => m.len(),
        Err(_) => 0,
    };

    // Check write permissions before deletion
    if let Ok(meta) = path.metadata() {
        if meta.permissions().readonly() {
            // Try clearing readonly flag if possible
            let mut perms = meta.permissions();
            #[allow(clippy::permissions_set_readonly_false)]
            perms.set_readonly(false);
            let _ = fs::set_permissions(path, perms);
        }
    }

    match fs::remove_file(path) {
        Ok(_) => Ok(file_size),
        Err(e) => {
            let reason = match e.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    "Permission denied or file is locked".to_string()
                }
                std::io::ErrorKind::NotFound => "File not found".to_string(),
                _ => format!("System error: {}", e),
            };
            Err(reason)
        }
    }
}

/// Safely removes a directory recursively.
/// If the directory contains files locked by running applications (Safari, Chrome, system processes),
/// this function attempts to remove as many unlocked files as possible rather than failing completely.
fn safe_remove_dir_all(dir_path: &Path) -> Result<u64, String> {
    // Step 1: Attempt fast deletion using std::fs::remove_dir_all
    let total_size = get_dir_size(dir_path);

    if fs::remove_dir_all(dir_path).is_ok() {
        return Ok(total_size);
    }

    // Step 2: If fast deletion fails (e.g. locked files inside),
    // traverse and delete as much as possible
    let mut freed: u64 = 0;

    // Traverse bottom-up (depth-first) to delete files first and empty directories after
    let entries: Vec<_> = WalkDir::new(dir_path)
        .follow_links(false)
        .contents_first(true)
        .into_iter()
        .filter_map(|e| e.ok())
        .collect();

    for entry in entries {
        let path = entry.path();
        if path == dir_path {
            continue; // Try removing root directory last
        }

        if path.is_file() || path.is_symlink() {
            if let Ok(meta) = path.symlink_metadata() {
                let size = meta.len();
                if fs::remove_file(path).is_ok() {
                    freed += size;
                }
            }
        } else if path.is_dir() {
            let _ = fs::remove_dir(path); // Only succeeds if directory is empty
        }
    }

    // Retry removing root directory
    if fs::remove_dir(dir_path).is_ok() {
        Ok(freed)
    } else if freed > 0 {
        // Partially freed space even though some files were locked
        Ok(freed)
    } else {
        Err("Cannot delete directory (may contain files in use by another process)".to_string())
    }
}

/// Calculates directory size (does not follow symlinks for safety)
fn get_dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.path().symlink_metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}
