use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph, Tabs},
    Frame,
};

use crate::app::{ActiveTab, App, AppMode};
use crate::scanner::ScanStatus;
use crate::utils::{format_bytes, format_path_for_display, truncate_string};

/// Main UI rendering function
pub fn draw(frame: &mut Frame, app: &mut App) {
    let size = frame.area();

    // Main layout: Header -> Tab Bar -> Main Content -> Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Length(3), // Tabs
            Constraint::Min(8),    // Body
            Constraint::Length(3), // Footer / Status Bar
        ])
        .split(size);

    render_header(frame, app, chunks[0]);
    render_tabs(frame, app, chunks[1]);
    render_body(frame, app, chunks[2]);
    render_footer(frame, app, chunks[3]);

    // Show modal popups in Dry-Run, Deleting, or Report mode
    match app.mode {
        AppMode::DryRunPreview => render_dry_run_modal(frame, app, size),
        AppMode::Deleting => render_deleting_modal(frame, size),
        AppMode::ReportModal => render_report_modal(frame, app, size),
        AppMode::Normal => {}
    }
}

/// Renders top Header bar
fn render_header(frame: &mut Frame, app: &App, area: Rect) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    let title_line = Line::from(vec![
        Span::styled(
            " 🧹 MacTidy ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " macOS System Cleaner TUI v0.1.0",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let title_p = Paragraph::new(title_line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(title_p, header_chunks[0]);

    // Background scan status
    let status_line = if app.is_scanning {
        Line::from(vec![
            Span::styled(
                format!(" {} ", app.get_spinner_char()),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Scanning system in background...",
                Style::default().fg(Color::Yellow),
            ),
        ])
    } else {
        Line::from(vec![
            Span::styled(" ● ", Style::default().fg(Color::Green)),
            Span::styled("System ready", Style::default().fg(Color::Green)),
        ])
    };

    let status_p = Paragraph::new(status_line)
        .alignment(Alignment::Right)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(status_p, header_chunks[1]);
}

/// Renders navigation Tabs
fn render_tabs(frame: &mut Frame, app: &App, area: Rect) {
    let titles = vec![
        " [1] System Junk ",
        " [2] Large Files (>500MB Downloads) ",
    ];

    let selected_index = match app.active_tab {
        ActiveTab::SystemJunk => 0,
        ActiveTab::LargeFiles => 1,
    };

    let tabs = Tabs::new(titles)
        .select(selected_index)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title(" Cleanup Categories [Press Tab to switch] "),
        )
        .style(Style::default().fg(Color::DarkGray))
        .highlight_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
        );

    frame.render_widget(tabs, area);
}

/// Renders main content based on active Tab
fn render_body(frame: &mut Frame, app: &App, area: Rect) {
    match app.active_tab {
        ActiveTab::SystemJunk => render_system_junk_tab(frame, app, area),
        ActiveTab::LargeFiles => render_large_files_tab(frame, app, area),
    }
}

/// System Junk Tab: Two columns (Category list & Sub-item preview)
fn render_system_junk_tab(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    // Left column: Category list
    let items: Vec<ListItem> = app
        .categories
        .iter()
        .enumerate()
        .map(|(idx, cat)| {
            let is_cursor = idx == app.selected_category_idx;
            let check_mark = if cat.selected { "[x]" } else { "[ ]" };

            let cursor_indicator = if is_cursor { "▶ " } else { "  " };

            let status_badge = match &cat.status {
                ScanStatus::Idle => Span::styled("Pending", Style::default().fg(Color::DarkGray)),
                ScanStatus::Scanning => {
                    Span::styled("Scanning...", Style::default().fg(Color::Yellow))
                }
                ScanStatus::Completed => {
                    Span::styled(format_bytes(cat.total_size), Style::default().fg(Color::Green))
                }
                ScanStatus::PermissionDenied => {
                    Span::styled("Permission Denied", Style::default().fg(Color::Red))
                }
                ScanStatus::NotFound => {
                    Span::styled("Not Found", Style::default().fg(Color::DarkGray))
                }
                ScanStatus::Error(_) => Span::styled("Error", Style::default().fg(Color::Red)),
            };

            let item_count_str = if cat.status == ScanStatus::Completed {
                format!(" ({} items)", cat.item_count)
            } else {
                "".to_string()
            };

            let line1 = Line::from(vec![
                Span::styled(
                    cursor_indicator,
                    if is_cursor {
                        Style::default().fg(Color::Cyan)
                    } else {
                        Style::default()
                    },
                ),
                Span::styled(
                    format!("{} ", check_mark),
                    if cat.selected {
                        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::DarkGray)
                    },
                ),
                Span::styled(
                    format!("{:<22} ", cat.name),
                    if is_cursor {
                        Style::default()
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(Color::White)
                    },
                ),
                status_badge,
                Span::styled(item_count_str, Style::default().fg(Color::DarkGray)),
            ]);

            let line2 = Line::from(vec![
                Span::raw("      "),
                Span::styled(
                    format!("{} ", cat.description),
                    Style::default().fg(Color::DarkGray),
                ),
            ]);

            let item_style = if is_cursor {
                Style::default().bg(Color::Rgb(30, 35, 45))
            } else {
                Style::default()
            };

            ListItem::new(vec![line1, line2, Line::raw("")]).style(item_style)
        })
        .collect();

    let list_title = " System Junk Categories (Space to toggle) ";
    let list_widget = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(list_title),
    );
    frame.render_widget(list_widget, chunks[0]);

    // Right column: Preview sub-items of highlighted category
    if let Some(selected_cat) = app.categories.get(app.selected_category_idx) {
        let right_title = format!(" Details: {} ", selected_cat.name);

        let sub_items_render: Vec<ListItem> = if selected_cat.status == ScanStatus::PermissionDenied {
            vec![
                ListItem::new(Line::from(vec![Span::styled(
                    " ⚠️  Permission Denied (macOS TCC)",
                    Style::default().fg(Color::LightRed).add_modifier(Modifier::BOLD),
                )])),
                ListItem::new(Line::raw("")),
                ListItem::new(Line::from(vec![Span::styled(
                    " macOS requires Full Disk Access (FDA) to",
                    Style::default().fg(Color::Yellow),
                )])),
                ListItem::new(Line::from(vec![Span::styled(
                    " read this protected system directory.",
                    Style::default().fg(Color::Yellow),
                )])),
                ListItem::new(Line::raw("")),
                ListItem::new(Line::from(vec![Span::styled(
                    " 💡 How to grant permission:",
                    Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
                )])),
                ListItem::new(Line::from(vec![Span::styled(
                    "  1. Open System Settings -> Privacy & Security",
                    Style::default().fg(Color::Cyan),
                )])),
                ListItem::new(Line::from(vec![Span::styled(
                    "  2. Click 'Full Disk Access'",
                    Style::default().fg(Color::Cyan),
                )])),
                ListItem::new(Line::from(vec![Span::styled(
                    "  3. Toggle ON for your Terminal / IDE app",
                    Style::default().fg(Color::Cyan),
                )])),
                ListItem::new(Line::from(vec![Span::styled(
                    "  4. Restart Terminal and press 'r' to rescan",
                    Style::default().fg(Color::Green),
                )])),
            ]
        } else if let ScanStatus::Error(ref err) = selected_cat.status {
            vec![
                ListItem::new(Line::from(vec![Span::styled(
                    " ⚠️  Scan Error",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                )])),
                ListItem::new(Line::raw("")),
                ListItem::new(Line::from(vec![Span::styled(
                    format!(" {}", err),
                    Style::default().fg(Color::DarkGray),
                )])),
            ]
        } else if selected_cat.status == ScanStatus::NotFound {
            vec![ListItem::new(Line::from(vec![Span::styled(
                " (Directory does not exist)",
                Style::default().fg(Color::DarkGray),
            )]))]
        } else if selected_cat.sub_items.is_empty() {
            vec![ListItem::new(Line::from(vec![Span::styled(
                " (No items found or folder is empty)",
                Style::default().fg(Color::DarkGray),
            )]))]
        } else {
            selected_cat
                .sub_items
                .iter()
                .take(15) // Limit display to top 15 items
                .map(|sub| {
                    let icon = if sub.is_dir { "📁 " } else { "📄 " };
                    Line::from(vec![
                        Span::styled(icon, Style::default().fg(Color::Yellow)),
                        Span::styled(
                            truncate_string(&sub.name, 28),
                            Style::default().fg(Color::White),
                        ),
                        Span::raw("  "),
                        Span::styled(
                            format!("{:>10}", format_bytes(sub.size)),
                            Style::default().fg(Color::LightCyan),
                        ),
                    ])
                })
                .map(ListItem::new)
                .collect()
        };

        let right_widget = List::new(sub_items_render).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray))
                .title(right_title),
        );
        frame.render_widget(right_widget, chunks[1]);
    }
}

/// Large Files Tab: Large files >500MB in Downloads
fn render_large_files_tab(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = if app.large_files.is_empty() {
        if app.is_scanning {
            vec![ListItem::new(Line::from(vec![Span::styled(
                " ⏳ Scanning ~/Downloads for files > 500MB...",
                Style::default().fg(Color::Yellow),
            )]))]
        } else {
            vec![ListItem::new(Line::from(vec![Span::styled(
                " ✅ No files larger than 500MB found in ~/Downloads!",
                Style::default().fg(Color::Green),
            )]))]
        }
    } else {
        app.large_files
            .iter()
            .enumerate()
            .map(|(idx, file)| {
                let is_cursor = idx == app.selected_large_file_idx;
                let check_mark = if file.selected { "[x]" } else { "[ ]" };
                let cursor_indicator = if is_cursor { "▶ " } else { "  " };

                let line = Line::from(vec![
                    Span::styled(
                        cursor_indicator,
                        if is_cursor {
                            Style::default().fg(Color::Cyan)
                        } else {
                            Style::default()
                        },
                    ),
                    Span::styled(
                        format!("{} ", check_mark),
                        if file.selected {
                            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::DarkGray)
                        },
                    ),
                    Span::styled(
                        format!("{:<35} ", truncate_string(&file.name, 34)),
                        if is_cursor {
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(Color::White)
                        },
                    ),
                    Span::styled(
                        format!("{:>10}   ", format_bytes(file.size)),
                        Style::default()
                            .fg(Color::Red)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format_path_for_display(&file.path),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);

                let item_style = if is_cursor {
                    Style::default().bg(Color::Rgb(30, 35, 45))
                } else {
                    Style::default()
                };

                ListItem::new(line).style(item_style)
            })
            .collect()
    };

    let title = format!(
        " Large Files in ~/Downloads (> 500MB) - Total: {} ",
        format_bytes(app.large_files_total_size)
    );

    let list_widget = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan))
            .title(title),
    );

    frame.render_widget(list_widget, area);
}

/// Renders bottom Footer bar: Status and Keybindings
fn render_footer(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(area);

    // Selected size summary
    let total_selected = app.total_selected_junk_size();
    let selected_items = app.total_selected_items_count();

    let summary_line = Line::from(vec![
        Span::raw(" Selected: "),
        Span::styled(
            format_bytes(total_selected),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(" ({} items) | ", selected_items),
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            truncate_string(&app.status_message, 45),
            Style::default().fg(Color::LightYellow),
        ),
    ]);

    let summary_p = Paragraph::new(summary_line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    frame.render_widget(summary_p, chunks[0]);

    // Keybinding hints
    let help_line = Line::from(vec![
        Span::styled(" [↑/↓]", Style::default().fg(Color::Cyan)),
        Span::raw(" Navigate "),
        Span::styled("[Space]", Style::default().fg(Color::Cyan)),
        Span::raw(" Toggle "),
        Span::styled("[a]", Style::default().fg(Color::Cyan)),
        Span::raw(" All "),
        Span::styled("[Enter]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" Clean "),
        Span::styled("[r]", Style::default().fg(Color::Yellow)),
        Span::raw(" Rescan "),
        Span::styled("[q]", Style::default().fg(Color::Red)),
        Span::raw(" Quit"),
    ]);

    let help_p = Paragraph::new(help_line)
        .alignment(Alignment::Right)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::DarkGray)),
        );
    frame.render_widget(help_p, chunks[1]);
}

/// Dry-Run Preview and confirmation modal
fn render_dry_run_modal(frame: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(80, 75, area);

    // Clear background behind modal
    frame.render_widget(Clear, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Title and summary
            Constraint::Min(6),    // List of files to delete
            Constraint::Length(4), // Safeguard notice & actions
        ])
        .split(popup_area);

    // Modal border frame
    let modal_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" ⚠️ CLEANUP PREVIEW (DRY-RUN) ");
    frame.render_widget(modal_block, popup_area);

    // Section 1: Summary
    let summary_text = vec![Line::from(vec![
        Span::raw(" Estimated space to free: "),
        Span::styled(
            format_bytes(app.dry_run_total_size),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(" across {} selected items.", app.dry_run_items.len())),
    ])];
    let summary_p = Paragraph::new(summary_text);
    frame.render_widget(summary_p, chunks[0]);

    // Section 2: Detailed list of files to delete (Scrollable)
    let visible_items: Vec<ListItem> = app
        .dry_run_items
        .iter()
        .skip(app.dry_run_scroll)
        .take(12)
        .map(|item| {
            let icon = if item.is_dir { "📁" } else { "📄" };
            let line = Line::from(vec![
                Span::raw(format!(" {} ", icon)),
                Span::styled(
                    format!("[{}] ", item.category_name),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(
                    truncate_string(&item.name, 35),
                    Style::default().fg(Color::White),
                ),
                Span::raw(" - "),
                Span::styled(format_bytes(item.size), Style::default().fg(Color::Yellow)),
                Span::raw(" ("),
                Span::styled(
                    format_path_for_display(&item.path),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::raw(")"),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list_title = format!(
        " Files to be deleted ({}/{}) - Scroll [↑/↓] ",
        app.dry_run_scroll + 1,
        app.dry_run_items.len()
    );
    let items_list = List::new(visible_items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(list_title),
    );
    frame.render_widget(items_list, chunks[1]);

    // Section 3: Safeguard Notice & Action Confirmation
    let safeguard_notice = vec![
        Line::from(vec![
            Span::styled(" 🛡️ SAFEGUARD: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled("Locked or permission-restricted files will be safely skipped automatically.", Style::default().fg(Color::LightGreen)),
        ]),
        Line::from(vec![
            Span::styled(" ▶ Are you sure you want to delete? Press ", Style::default().fg(Color::White)),
            Span::styled(" [y] Confirm permanent deletion ", Style::default().bg(Color::Red).fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw(" or "),
            Span::styled(" [n / Esc] Cancel & Back ", Style::default().bg(Color::DarkGray).fg(Color::White)),
        ]),
    ];
    let safeguard_p = Paragraph::new(safeguard_notice);
    frame.render_widget(safeguard_p, chunks[2]);
}

/// Modal indicating deletion in progress
fn render_deleting_modal(frame: &mut Frame, area: Rect) {
    let popup_area = centered_rect(50, 20, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Red))
        .title(" ⏳ CLEANING IN PROGRESS ");

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled("  Safely deleting selected files...", Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::styled("  Please do not close the application during this process.", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let p = Paragraph::new(text).block(block).alignment(Alignment::Center);
    frame.render_widget(p, popup_area);
}

/// Modal displaying post-cleanup report (Report Modal)
fn render_report_modal(frame: &mut Frame, app: &App, area: Rect) {
    let popup_area = centered_rect(70, 60, area);
    frame.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Double)
        .border_style(Style::default().fg(Color::Green))
        .title(" 🎉 CLEANUP REPORT ");

    let report = match &app.deletion_report {
        Some(r) => r,
        None => return,
    };

    let mut text_lines = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled(" ✅ Space freed: ", Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::styled(format_bytes(report.freed_bytes), Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(format!(" (out of {} targeted)", format_bytes(report.total_targeted_bytes)), Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled(" 🗑️ Items successfully deleted: ", Style::default().fg(Color::White)),
            Span::styled(format!("{} items", report.deleted_items_count), Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::styled(" 🛡️ Items safely skipped: ", Style::default().fg(Color::White)),
            Span::styled(format!("{} items (In-use, locked, or permission denied)", report.skipped_items_count), Style::default().fg(Color::Yellow)),
        ]),
        Line::raw(""),
    ];

    if !report.failed_items.is_empty() {
        text_lines.push(Line::from(vec![
            Span::styled(" Skipped items details (Safeguard Skipped):", Style::default().fg(Color::Yellow).add_modifier(Modifier::UNDERLINED)),
        ]));

        for (path, err) in report.failed_items.iter().take(5) {
            text_lines.push(Line::from(vec![
                Span::styled(format!("  - {}: ", truncate_string(&path.file_name().unwrap_or_default().to_string_lossy(), 25)), Style::default().fg(Color::White)),
                Span::styled(truncate_string(err, 40), Style::default().fg(Color::DarkGray)),
            ]));
        }

        if report.failed_items.len() > 5 {
            text_lines.push(Line::from(vec![
                Span::styled(format!("  ... and {} other items.", report.failed_items.len() - 5), Style::default().fg(Color::DarkGray)),
            ]));
        }
    }

    text_lines.push(Line::raw(""));
    text_lines.push(Line::from(vec![
        Span::styled(" 👉 Press [Enter] or [Esc] to return to main screen and rescan ", Style::default().bg(Color::Cyan).fg(Color::Black).add_modifier(Modifier::BOLD)),
    ]));

    let p = Paragraph::new(text_lines).block(block);
    frame.render_widget(p, popup_area);
}

/// Helper function to calculate centered popup area on screen
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
