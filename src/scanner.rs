use std::path::PathBuf;
use std::sync::mpsc::Sender;
use std::thread;
use walkdir::WalkDir;

/// Default large file threshold: 500 MB (500 * 1024 * 1024 bytes)
pub const LARGE_FILE_THRESHOLD: u64 = 500 * 1024 * 1024;

/// Detailed information of a sub-item inside a junk category (for preview panel)
#[derive(Debug, Clone)]
pub struct SubItem {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub is_dir: bool,
}

/// Scanning status of a category
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanStatus {
    Idle,
    Scanning,
    Completed,
    PermissionDenied,
    NotFound,
    Error(String),
}

/// Information for a system junk category
#[derive(Debug, Clone)]
pub struct CategoryInfo {
    pub id: usize,
    pub name: String,
    pub description: String,
    pub path: PathBuf,
    pub total_size: u64,
    pub item_count: usize,
    pub sub_items: Vec<SubItem>,
    pub status: ScanStatus,
    pub selected: bool,
}

/// Information for a large file in Downloads
#[derive(Debug, Clone)]
pub struct LargeFileInfo {
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub selected: bool,
}

/// Message sent from background scanner thread to UI thread
pub enum ScanMessage {
    CategoryStarted {
        category_id: usize,
    },
    CategoryProgress {
        category_id: usize,
        current_size: u64,
        items_count: usize,
    },
    CategoryFinished {
        category_id: usize,
        total_size: u64,
        item_count: usize,
        sub_items: Vec<SubItem>,
        status: ScanStatus,
    },
    LargeFileFound(LargeFileInfo),
    LargeFilesCompleted {
        total_found: usize,
        total_size: u64,
    },
    AllScansCompleted,
}

/// Initializes the default macOS junk categories
pub fn get_default_categories() -> Vec<CategoryInfo> {
    let home = crate::utils::get_user_home().unwrap_or_else(|| PathBuf::from("/"));

    vec![
        CategoryInfo {
            id: 0,
            name: "User Caches".to_string(),
            description: "Application and system cache (~/Library/Caches)".to_string(),
            path: home.join("Library/Caches"),
            total_size: 0,
            item_count: 0,
            sub_items: Vec::new(),
            status: ScanStatus::Idle,
            selected: true,
        },
        CategoryInfo {
            id: 1,
            name: "System & App Logs".to_string(),
            description: "Application and system logs (~/Library/Logs)".to_string(),
            path: home.join("Library/Logs"),
            total_size: 0,
            item_count: 0,
            sub_items: Vec::new(),
            status: ScanStatus::Idle,
            selected: true,
        },
        CategoryInfo {
            id: 2,
            name: "Trash Bin".to_string(),
            description: "User Trash bin (~/.Trash)".to_string(),
            path: home.join(".Trash"),
            total_size: 0,
            item_count: 0,
            sub_items: Vec::new(),
            status: ScanStatus::Idle,
            selected: true,
        },
        CategoryInfo {
            id: 3,
            name: "iOS Device Backups".to_string(),
            description: "iOS device backups (~/Library/.../MobileSync/Backup)".to_string(),
            path: home.join("Library/Application Support/MobileSync/Backup"),
            total_size: 0,
            item_count: 0,
            sub_items: Vec::new(),
            status: ScanStatus::Idle,
            selected: false, // iOS Backups unselected by default to prevent accidental data loss
        },
    ]
}

/// Spawns the scanning process in a background thread
pub fn start_full_scan(
    categories: Vec<CategoryInfo>,
    downloads_path: PathBuf,
    sender: Sender<ScanMessage>,
) {
    thread::spawn(move || {
        // 1. Scan each system junk category
        for cat in &categories {
            let cat_id = cat.id;
            let _ = sender.send(ScanMessage::CategoryStarted { category_id: cat_id });

            if !cat.path.exists() {
                let _ = sender.send(ScanMessage::CategoryFinished {
                    category_id: cat_id,
                    total_size: 0,
                    item_count: 0,
                    sub_items: Vec::new(),
                    status: ScanStatus::NotFound,
                });
                continue;
            }

            // Check root directory read permissions
            let read_dir = match std::fs::read_dir(&cat.path) {
                Ok(rd) => rd,
                Err(err) => {
                    let status = if err.kind() == std::io::ErrorKind::PermissionDenied {
                        ScanStatus::PermissionDenied
                    } else {
                        ScanStatus::Error(err.to_string())
                    };
                    let _ = sender.send(ScanMessage::CategoryFinished {
                        category_id: cat_id,
                        total_size: 0,
                        item_count: 0,
                        sub_items: Vec::new(),
                        status,
                    });
                    continue;
                }
            };

            let mut total_size: u64 = 0;
            let mut total_items: usize = 0;
            let mut sub_items: Vec<SubItem> = Vec::new();

            // Scan direct sub-items
            for entry_res in read_dir {
                let entry = match entry_res {
                    Ok(e) => e,
                    Err(_) => continue, // Skip unreadable/inaccessible files
                };

                let entry_path = entry.path();
                let file_name = entry.file_name().to_string_lossy().to_string();

                let metadata = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                let is_dir = metadata.is_dir();
                let mut sub_size: u64 = 0;

                if is_dir {
                    // Recursively scan sub-item using walkdir (safely without following symlinks)
                    for walk_entry in WalkDir::new(&entry_path)
                        .follow_links(false)
                        .min_depth(0)
                        .into_iter()
                        .filter_map(|e| e.ok())
                    {
                        // Safely get metadata for file size
                        if let Ok(meta) = walk_entry.path().symlink_metadata() {
                            if meta.is_file() {
                                sub_size += meta.len();
                                total_items += 1;
                            }
                        }
                    }
                } else {
                    sub_size = metadata.len();
                    total_items += 1;
                }

                total_size += sub_size;
                sub_items.push(SubItem {
                    name: file_name,
                    path: entry_path,
                    size: sub_size,
                    is_dir,
                });

                // Send progress update
                let _ = sender.send(ScanMessage::CategoryProgress {
                    category_id: cat_id,
                    current_size: total_size,
                    items_count: total_items,
                });
            }

            // Sort sub-items in descending order by size
            sub_items.sort_by_key(|a| std::cmp::Reverse(a.size));

            let _ = sender.send(ScanMessage::CategoryFinished {
                category_id: cat_id,
                total_size,
                item_count: total_items,
                sub_items,
                status: ScanStatus::Completed,
            });
        }

        // 2. Scan large files in Downloads (> 500MB)
        if downloads_path.exists() {
            let mut found_count = 0;
            let mut found_size: u64 = 0;

            for walk_entry in WalkDir::new(&downloads_path)
                .follow_links(false)
                .min_depth(1)
                .into_iter()
                .filter_map(|e| e.ok())
            {
                let path = walk_entry.path();
                if let Ok(meta) = path.symlink_metadata() {
                    if meta.is_file() {
                        let size = meta.len();
                        if size >= LARGE_FILE_THRESHOLD {
                            let name = walk_entry.file_name().to_string_lossy().to_string();
                            found_count += 1;
                            found_size += size;

                            let _ = sender.send(ScanMessage::LargeFileFound(LargeFileInfo {
                                name,
                                path: path.to_path_buf(),
                                size,
                                selected: false, // Unselected by default for safety
                            }));
                        }
                    }
                }
            }

            let _ = sender.send(ScanMessage::LargeFilesCompleted {
                total_found: found_count,
                total_size: found_size,
            });
        } else {
            let _ = sender.send(ScanMessage::LargeFilesCompleted {
                total_found: 0,
                total_size: 0,
            });
        }

        // 3. Complete entire scan process
        let _ = sender.send(ScanMessage::AllScansCompleted);
    });
}
