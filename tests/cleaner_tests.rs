#[cfg(test)]
mod tests {
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;

    use MacTidy::file_ops::{execute_safe_cleanup, generate_dry_run_list, DryRunItem};
    use MacTidy::scanner::{get_default_categories, LargeFileInfo};
    use MacTidy::utils::{format_bytes, truncate_string};

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.00 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.00 GB");
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world and universe", 10), "hello w...");
        assert_eq!(truncate_string("hi", 2), "hi");
    }

    #[test]
    fn test_default_categories() {
        let cats = get_default_categories();
        assert_eq!(cats.len(), 4);
        assert_eq!(cats[0].name, "User Caches");
        assert_eq!(cats[1].name, "System & App Logs");
        assert_eq!(cats[2].name, "Trash Bin");
        assert_eq!(cats[3].name, "iOS Device Backups");

        // Ensure iOS Backups is unselected by default for safety
        assert!(!cats[3].selected);
    }

    #[test]
    fn test_dry_run_generation() {
        let mut cats = get_default_categories();
        cats[0].selected = true;
        cats[0].sub_items = vec![MacTidy::scanner::SubItem {
            name: "test_cache".to_string(),
            path: PathBuf::from("/tmp/test_cache"),
            size: 2048,
            is_dir: true,
        }];

        let large_files = vec![LargeFileInfo {
            name: "big_installer.dmg".to_string(),
            path: PathBuf::from("/tmp/big_installer.dmg"),
            size: 1_000_000_000,
            selected: true,
        }];

        let (items, total_size) = generate_dry_run_list(&cats, &large_files);
        assert_eq!(items.len(), 1 + 1); // 1 sub_item + 1 large file
        assert_eq!(total_size, 2048 + 1_000_000_000);
    }

    #[test]
    fn test_safe_cleanup_safeguard() {
        // Create temporary directory to test safe cleanup
        let test_dir = std::env::temp_dir().join("mactidy_test_cleanup");
        let _ = fs::remove_dir_all(&test_dir);
        fs::create_dir_all(&test_dir).expect("Failed to create test dir");

        let test_file = test_dir.join("test_file.txt");
        let mut file = File::create(&test_file).expect("Failed to create test file");
        file.write_all(b"Hello MacTidy Safeguard").unwrap();
        drop(file);

        let dry_run_items = vec![
            DryRunItem {
                path: test_file.clone(),
                name: "test_file.txt".to_string(),
                size: 23,
                is_dir: false,
                category_name: "Test".to_string(),
            },
            // Add a non-existent file to ensure the app handles missing files without crashing
            DryRunItem {
                path: test_dir.join("non_existent_file.log"),
                name: "non_existent_file.log".to_string(),
                size: 100,
                is_dir: false,
                category_name: "Test".to_string(),
            },
        ];

        let report = execute_safe_cleanup(&dry_run_items);
        assert_eq!(report.targeted_items_count, 2);
        assert_eq!(report.deleted_items_count, 1);
        assert_eq!(report.skipped_items_count, 1);
        assert_eq!(report.freed_bytes, 23);
        assert!(!test_file.exists());

        // Clean up test directory
        let _ = fs::remove_dir_all(&test_dir);
    }
}
