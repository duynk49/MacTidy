# MacTidy - macOS System Cleaner TUI

**MacTidy** is an intuitive terminal user interface (TUI) application written in **Rust**, designed to safely analyze and clean system junk files and identify large files on macOS.

---

## 🌟 Key Features

1. **Target Categories Scanning:**
   - Automatically scans and computes total sizes for common macOS junk directories:
     - `~/Library/Caches`: Application and user caches.
     - `~/Library/Logs`: System and application log files.
     - `~/.Trash`: User Trash bin.
     - `~/Library/Application Support/MobileSync/Backup`: iOS device backups (iPhone/iPad). Unchecked by default to protect user data.
   - Shows a preview list of the largest sub-items within the selected category.

2. **Large Files Finder:**
   - Recursively scans `~/Downloads`.
   - Filters and displays all files larger than **500MB** (such as old `.dmg`, `.iso`, `.zip` files).
   - Automatically sorts files in descending order by size.

3. **Dry-Run Preview Mode:**
   - Pressing `Enter` opens the **Dry-Run Preview** screen.
   - Lists all files and directories slated for deletion along with the estimated space to be freed.
   - Requires explicit user confirmation by pressing `y` to permanently delete or `n`/`Esc` to cancel.

4. **Safeguard Logic:**
   - **Never deletes root system folders:** Cleans only the contents inside `Caches`, `Logs`, and `Trash` rather than removing the root directories themselves (preserving permissions and macOS system structure).
   - **Handles locked / in-use files:** Gracefully handles `PermissionDenied` or files locked by running applications (e.g., Chrome, Safari). Safely skips locked files and proceeds with remaining files without crashing.
   - **Post-cleanup report:** Displays a detailed report with actual space freed, number of deleted items, and a list of safely skipped items.

5. **Non-blocking Background Scanner:**
   - System scan runs on a background thread communicating with the UI thread via `mpsc::channel`.
   - UI stays responsive at high frame rates, complete with an animated spinner (`⠋⠙⠹...`) indicating loading/scanning state.

---

## 🏗 Project Architecture

```
MacTidy/
├── Cargo.toml            # Package configuration and dependencies (ratatui, crossterm, walkdir, dirs)
├── src/
│   ├── main.rs           # Entry point: Terminal raw mode, event loop (crossterm), panic hook
│   ├── lib.rs            # Module declarations and library exports for integration tests
│   ├── app.rs            # State Machine management (Tabs, Selection, Modes, Channels, Spinner)
│   ├── scanner.rs        # Background Scanner Engine (symlink handling, permission safety)
│   ├── file_ops.rs       # Safe file operations (Dry-run generator, Safeguard deletion, Report)
│   ├── ui.rs             # TUI rendering with Ratatui (Header, Tabs, Split View, Modals)
│   └── utils.rs          # Utilities (format_bytes, format_path_for_display, truncate_string)
└── tests/
    └── cleaner_tests.rs  # Automated unit and integration test suite
```

---

## ⌨️ Keybindings

| Key | Action |
| :--- | :--- |
| `↑` / `k` | Move cursor up |
| `↓` / `j` | Move cursor down |
| `Space` | Toggle category or file selection (`[x]` / `[ ]`) |
| `a` | Select all / Deselect all in current tab |
| `Tab` | Switch between tabs: **System Junk** and **Large Files** |
| `1` / `2` | Jump directly to corresponding tab |
| `Enter` | Open **Dry-Run Preview** screen |
| `y` | *(In Preview modal)* Confirm permanent deletion |
| `n` / `Esc` | *(In Preview modal)* Cancel and return to main screen |
| `r` | Rescan entire system |
| `q` | Quit application |

---

## 🚀 Getting Started

### 1. Requirements
- macOS operating system.
- [Rust and Cargo](https://rustup.rs/) installed.

### 2. Running Tests
Run all unit and integration tests:
```bash
cargo test
```

### 3. Running the Application
Run directly via Cargo:
```bash
cargo run
```

Or build an optimized release binary:
```bash
cargo build --release
./target/release/MacTidy
```
