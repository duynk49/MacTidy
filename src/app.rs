use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver};

use crate::file_ops::{execute_safe_cleanup, generate_dry_run_list, DeletionReport, DryRunItem};
use crate::scanner::{
    get_default_categories, start_full_scan, CategoryInfo, LargeFileInfo, ScanMessage, ScanStatus,
};

/// Main application tabs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    SystemJunk,
    LargeFiles,
}

/// Screen display modes (State Machine)
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppMode {
    Normal,
    DryRunPreview,
    Deleting,
    ReportModal,
}

/// Global application state
pub struct App {
    pub active_tab: ActiveTab,
    pub mode: AppMode,

    // Data for System Junk Tab
    pub categories: Vec<CategoryInfo>,
    pub selected_category_idx: usize,
    pub selected_sub_item_idx: usize,

    // Data for Large Files Tab (>500MB)
    pub large_files: Vec<LargeFileInfo>,
    pub selected_large_file_idx: usize,
    pub large_files_total_size: u64,

    // Background scanner management (Background Thread)
    pub is_scanning: bool,
    pub scan_receiver: Option<Receiver<ScanMessage>>,
    pub spinner_frame: usize,

    // Dry-Run Preview mode data
    pub dry_run_items: Vec<DryRunItem>,
    pub dry_run_total_size: u64,
    pub dry_run_scroll: usize,

    // Post-deletion results
    pub deletion_report: Option<DeletionReport>,

    // Status bar notifications
    pub status_message: String,
    pub should_quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

impl App {
    pub fn new() -> Self {
        let mut app = Self {
            active_tab: ActiveTab::SystemJunk,
            mode: AppMode::Normal,

            categories: get_default_categories(),
            selected_category_idx: 0,
            selected_sub_item_idx: 0,

            large_files: Vec::new(),
            selected_large_file_idx: 0,
            large_files_total_size: 0,

            is_scanning: false,
            scan_receiver: None,
            spinner_frame: 0,

            dry_run_items: Vec::new(),
            dry_run_total_size: 0,
            dry_run_scroll: 0,

            deletion_report: None,

            status_message: "Starting application. Scanning system...".to_string(),
            should_quit: false,
        };

        app.trigger_rescan();
        app
    }

    /// Spawns a background thread to rescan all data
    pub fn trigger_rescan(&mut self) {
        if self.is_scanning {
            return;
        }

        self.is_scanning = true;
        self.large_files.clear();
        self.large_files_total_size = 0;
        self.selected_large_file_idx = 0;

        for cat in &mut self.categories {
            cat.total_size = 0;
            cat.item_count = 0;
            cat.sub_items.clear();
            cat.status = ScanStatus::Scanning;
        }

        let (tx, rx) = channel();
        self.scan_receiver = Some(rx);

        let home = crate::utils::get_user_home().unwrap_or_else(|| PathBuf::from("/"));
        let downloads_path = home.join("Downloads");

        start_full_scan(self.categories.clone(), downloads_path, tx);
        self.status_message = "Scanning junk folders and large files in Downloads...".to_string();
    }

    /// Listens for messages from the scanner thread on each event loop tick (non-blocking)
    pub fn handle_scan_messages(&mut self) {
        if !self.is_scanning {
            return;
        }

        self.spinner_frame = (self.spinner_frame + 1) % 10;

        let mut completed = false;
        if let Some(ref rx) = self.scan_receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    ScanMessage::CategoryStarted { category_id } => {
                        if let Some(cat) = self.categories.iter_mut().find(|c| c.id == category_id) {
                            cat.status = ScanStatus::Scanning;
                        }
                    }
                    ScanMessage::CategoryProgress {
                        category_id,
                        current_size,
                        items_count,
                    } => {
                        if let Some(cat) = self.categories.iter_mut().find(|c| c.id == category_id) {
                            cat.total_size = current_size;
                            cat.item_count = items_count;
                        }
                    }
                    ScanMessage::CategoryFinished {
                        category_id,
                        total_size,
                        item_count,
                        sub_items,
                        status,
                    } => {
                        if let Some(cat) = self.categories.iter_mut().find(|c| c.id == category_id) {
                            cat.total_size = total_size;
                            cat.item_count = item_count;
                            cat.sub_items = sub_items;
                            cat.status = status;
                        }
                    }
                    ScanMessage::LargeFileFound(file) => {
                        self.large_files.push(file);
                        // Sort large files in descending order
                        self.large_files.sort_by_key(|a| std::cmp::Reverse(a.size));
                    }
                    ScanMessage::LargeFilesCompleted {
                        total_found,
                        total_size,
                    } => {
                        self.large_files_total_size = total_size;
                        if total_found > 0 {
                            self.status_message = format!(
                                "Found {} large files (>500MB) in Downloads",
                                total_found
                            );
                        }
                    }
                    ScanMessage::AllScansCompleted => {
                        completed = true;
                    }
                }
            }
        }

        if completed {
            self.is_scanning = false;
            self.scan_receiver = None;
            let total_junk = self.total_selected_junk_size();
            self.status_message = format!(
                "Scan completed! Total selected junk: {}",
                crate::utils::format_bytes(total_junk)
            );
        }
    }

    /// Calculates total size of selected items across all categories and files
    pub fn total_selected_junk_size(&self) -> u64 {
        let cat_size: u64 = self
            .categories
            .iter()
            .filter(|c| c.selected)
            .map(|c| c.total_size)
            .sum();

        let large_size: u64 = self
            .large_files
            .iter()
            .filter(|f| f.selected)
            .map(|f| f.size)
            .sum();

        cat_size + large_size
    }

    /// Counts total number of selected items
    pub fn total_selected_items_count(&self) -> usize {
        let cat_items: usize = self
            .categories
            .iter()
            .filter(|c| c.selected)
            .map(|c| c.item_count)
            .sum();

        let large_items: usize = self.large_files.iter().filter(|f| f.selected).count();

        cat_items + large_items
    }

    /// Moves cursor up
    pub fn move_up(&mut self) {
        match self.mode {
            AppMode::Normal => match self.active_tab {
                ActiveTab::SystemJunk => {
                    if self.selected_category_idx > 0 {
                        self.selected_category_idx -= 1;
                        self.selected_sub_item_idx = 0;
                    }
                }
                ActiveTab::LargeFiles => {
                    if self.selected_large_file_idx > 0 {
                        self.selected_large_file_idx -= 1;
                    }
                }
            },
            AppMode::DryRunPreview => {
                if self.dry_run_scroll > 0 {
                    self.dry_run_scroll -= 1;
                }
            }
            _ => {}
        }
    }

    /// Moves cursor down
    pub fn move_down(&mut self) {
        match self.mode {
            AppMode::Normal => match self.active_tab {
                ActiveTab::SystemJunk => {
                    if !self.categories.is_empty()
                        && self.selected_category_idx < self.categories.len() - 1
                    {
                        self.selected_category_idx += 1;
                        self.selected_sub_item_idx = 0;
                    }
                }
                ActiveTab::LargeFiles => {
                    if !self.large_files.is_empty()
                        && self.selected_large_file_idx < self.large_files.len() - 1
                    {
                        self.selected_large_file_idx += 1;
                    }
                }
            },
            AppMode::DryRunPreview => {
                if !self.dry_run_items.is_empty()
                    && self.dry_run_scroll < self.dry_run_items.len().saturating_sub(1)
                {
                    self.dry_run_scroll += 1;
                }
            }
            _ => {}
        }
    }

    /// Toggle selection checkbox with Space key
    pub fn toggle_selection(&mut self) {
        if self.mode != AppMode::Normal {
            return;
        }

        match self.active_tab {
            ActiveTab::SystemJunk => {
                if let Some(cat) = self.categories.get_mut(self.selected_category_idx) {
                    cat.selected = !cat.selected;
                    let state = if cat.selected { "Selected" } else { "Deselected" };
                    self.status_message = format!("{} category: {}", state, cat.name);
                }
            }
            ActiveTab::LargeFiles => {
                if let Some(file) = self.large_files.get_mut(self.selected_large_file_idx) {
                    file.selected = !file.selected;
                    let state = if file.selected { "Selected" } else { "Deselected" };
                    self.status_message = format!("{} large file: {}", state, file.name);
                }
            }
        }
    }

    /// Toggle all items in current tab with `a` key
    pub fn toggle_all(&mut self) {
        if self.mode != AppMode::Normal {
            return;
        }

        match self.active_tab {
            ActiveTab::SystemJunk => {
                let all_selected = self.categories.iter().all(|c| c.selected);
                for c in &mut self.categories {
                    c.selected = !all_selected;
                }
                self.status_message = if !all_selected {
                    "Selected all system junk categories".to_string()
                } else {
                    "Deselected all categories".to_string()
                };
            }
            ActiveTab::LargeFiles => {
                let all_selected = self.large_files.iter().all(|f| f.selected);
                for f in &mut self.large_files {
                    f.selected = !all_selected;
                }
                self.status_message = if !all_selected {
                    "Selected all large files".to_string()
                } else {
                    "Deselected all large files".to_string()
                };
            }
        }
    }

    /// Switch active tab (System Junk <-> Large Files) with Tab key
    pub fn switch_tab(&mut self) {
        if self.mode != AppMode::Normal {
            return;
        }

        self.active_tab = match self.active_tab {
            ActiveTab::SystemJunk => ActiveTab::LargeFiles,
            ActiveTab::LargeFiles => ActiveTab::SystemJunk,
        };
    }

    /// Opens Dry-Run preview screen to inspect items before deletion on Enter key
    pub fn enter_dry_run_preview(&mut self) {
        if self.mode != AppMode::Normal {
            return;
        }

        let (items, total_size) = generate_dry_run_list(&self.categories, &self.large_files);

        if items.is_empty() {
            self.status_message = "No items selected for cleanup!".to_string();
            return;
        }

        self.dry_run_items = items;
        self.dry_run_total_size = total_size;
        self.dry_run_scroll = 0;
        self.mode = AppMode::DryRunPreview;
        self.status_message = "Dry-Run Preview: Press 'y' to confirm deletion, 'Esc' or 'n' to cancel".to_string();
    }

    /// Confirms and executes file cleanup (Safeguard Clean)
    pub fn confirm_and_execute_cleaning(&mut self) {
        if self.mode != AppMode::DryRunPreview {
            return;
        }

        self.mode = AppMode::Deleting;
        let report = execute_safe_cleanup(&self.dry_run_items);
        self.deletion_report = Some(report);
        self.mode = AppMode::ReportModal;
    }

    /// Closes report modal and triggers rescan
    pub fn close_report_and_rescan(&mut self) {
        self.mode = AppMode::Normal;
        self.deletion_report = None;
        self.trigger_rescan();
    }

    /// Cancels Dry-Run preview and returns to main screen
    pub fn cancel_dry_run(&mut self) {
        self.mode = AppMode::Normal;
        self.dry_run_items.clear();
        self.dry_run_total_size = 0;
        self.status_message = "Cleanup operation cancelled".to_string();
    }

    /// Spinner animation characters
    pub fn get_spinner_char(&self) -> &'static str {
        const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        SPINNER[self.spinner_frame % SPINNER.len()]
    }
}
