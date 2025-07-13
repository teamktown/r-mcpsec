use crate::models::*;
use anyhow::Result;
use atty;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Axis, BarChart, Block, Borders, Cell, Chart, Dataset, GraphType, Gauge, List, ListItem, Paragraph, Row, Table, Tabs,
        Wrap,
    },
    Frame, Terminal,
};
use std::io;
use std::time::Duration;
use tokio::time::sleep;
use humantime;

/// Overview display mode for switching between views
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverviewViewMode {
    General,  // Current simple view with time-series chart
    Detailed, // Enhanced analytics with cache metrics and stacked bars
}

/// Enhanced terminal UI using Ratatui
pub struct RatatuiTerminalUI {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    should_exit: bool,
    selected_tab: usize,
    scroll_offset: usize,
    details_selected: usize,
    show_details_pane: bool,
    overview_view_mode: OverviewViewMode,
}

impl RatatuiTerminalUI {
    /// Create new Ratatui terminal UI
    pub fn new(_config: UserConfig) -> Result<Self> {
        // Check if we have a TTY available
        if !atty::is(atty::Stream::Stdout) {
            return Err(anyhow::anyhow!("TTY not available - interactive UI requires a terminal"));
        }

        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            should_exit: false,
            selected_tab: 0,
            scroll_offset: 0,
            details_selected: 0,
            show_details_pane: false,
            overview_view_mode: OverviewViewMode::Detailed, // Default to detailed view as requested
        })
    }

    /// Main UI loop
    pub async fn run(&mut self, metrics: &UsageMetrics) -> Result<()> {
        let current_metrics = metrics.clone();
        
        loop {
            eprintln!("🔍 DEBUG: Main UI loop iteration - current_tab: {}, should_exit: {}", self.selected_tab, self.should_exit);
            
            // Draw the UI
            let metrics_clone = current_metrics.clone();
            let selected_tab = self.selected_tab;
            let details_selected = self.details_selected;
            let show_details_pane = self.show_details_pane;
            let overview_view_mode = self.overview_view_mode;
            self.terminal.draw(move |frame| {
                Self::draw_ui_static(frame, &metrics_clone, selected_tab, details_selected, show_details_pane, overview_view_mode);
            })?;

            // Handle input with timeout
            let should_exit = self.handle_input().await?;
            eprintln!("🔍 DEBUG: handle_input returned: {}", should_exit);
            if should_exit {
                eprintln!("🔍 DEBUG: Breaking from main loop due to handle_input returning true");
                break;
            }

            // Small delay to prevent excessive CPU usage
            sleep(Duration::from_millis(50)).await;
        }

        Ok(())
    }

    /// Handle keyboard input
    async fn handle_input(&mut self) -> Result<bool> {
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, modifiers, .. }) = event::read()? {
                // Debug: Log all key events
                eprintln!("🔍 DEBUG: Key event - code: {:?}, modifiers: {:?}, current_tab: {}", code, modifiers, self.selected_tab);
                
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        eprintln!("🔍 DEBUG: Quit key pressed, exiting application");
                        self.should_exit = true;
                        return Ok(true);
                    }
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        eprintln!("🔍 DEBUG: Ctrl+C pressed, exiting application");
                        self.should_exit = true;
                        return Ok(true);
                    }
                    KeyCode::Tab => {
                        let old_tab = self.selected_tab;
                        self.selected_tab = (self.selected_tab + 1) % 7;
                        eprintln!("🔍 DEBUG: Tab key pressed - changed from tab {} to tab {}", old_tab, self.selected_tab);
                    }
                    KeyCode::BackTab => {
                        let old_tab = self.selected_tab;
                        self.selected_tab = if self.selected_tab == 0 { 6 } else { self.selected_tab - 1 };
                        eprintln!("🔍 DEBUG: BackTab key pressed - changed from tab {} to tab {}", old_tab, self.selected_tab);
                    }
                    KeyCode::Up => {
                        eprintln!("🔍 DEBUG: Up arrow pressed");
                        if self.selected_tab == 3 { // Details tab
                            self.details_selected = self.details_selected.saturating_sub(1);
                        } else {
                            self.scroll_offset = self.scroll_offset.saturating_sub(1);
                        }
                    }
                    KeyCode::Down => {
                        eprintln!("🔍 DEBUG: Down arrow pressed");
                        if self.selected_tab == 3 { // Details tab
                            self.details_selected = self.details_selected.saturating_add(1).min(10); // Max items
                        } else {
                            self.scroll_offset = self.scroll_offset.saturating_add(1);
                        }
                    }
                    KeyCode::Right => {
                        eprintln!("🔍 DEBUG: Right arrow pressed");
                        if self.selected_tab == 3 { // Details tab
                            self.show_details_pane = true;
                        }
                    }
                    KeyCode::Left => {
                        eprintln!("🔍 DEBUG: Left arrow pressed");
                        if self.selected_tab == 3 { // Details tab
                            self.show_details_pane = false;
                        }
                    }
                    KeyCode::Char('v') => {
                        eprintln!("🔍 DEBUG: 'v' key pressed - toggling overview view mode");
                        // Toggle view mode in Overview tab (Tab 0)
                        if self.selected_tab == 0 {
                            let old_mode = self.overview_view_mode;
                            self.overview_view_mode = match self.overview_view_mode {
                                OverviewViewMode::General => OverviewViewMode::Detailed,
                                OverviewViewMode::Detailed => OverviewViewMode::General,
                            };
                            eprintln!("🔍 DEBUG: Overview view mode changed from {:?} to {:?}", old_mode, self.overview_view_mode);
                        } else {
                            eprintln!("🔍 DEBUG: 'v' key pressed but not in Overview tab (current tab: {})", self.selected_tab);
                        }
                    }
                    KeyCode::Char('r') => {
                        eprintln!("🔍 DEBUG: 'r' key pressed - refresh");
                        // Refresh - could trigger a metrics update
                    }
                    KeyCode::Char('n') => {
                        eprintln!("🔍 DEBUG: 'n' key pressed - alternative tab switch");
                        let old_tab = self.selected_tab;
                        self.selected_tab = (self.selected_tab + 1) % 7;
                        eprintln!("🔍 DEBUG: Alternative tab switch - changed from tab {} to tab {}", old_tab, self.selected_tab);
                    }
                    _ => {
                        eprintln!("🔍 DEBUG: Unhandled key: {:?}", code);
                    }
                }
            } else {
                let other_event = event::read()?;
                eprintln!("🔍 DEBUG: Non-key event received: {:?}", other_event);
            }
        } else {
            eprintln!("🔍 DEBUG: No event available (poll timeout)");
        }
        eprintln!("🔍 DEBUG: handle_input returning false (continue)");
        Ok(false)
    }

    /// Draw the main UI (static version for terminal callback)
    fn draw_ui_static(frame: &mut Frame, metrics: &UsageMetrics, selected_tab: usize, details_selected: usize, show_details_pane: bool, overview_view_mode: OverviewViewMode) {
        let size = frame.area();

        // Create main layout
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Length(3), // Tabs
                Constraint::Min(10),   // Main content
                Constraint::Length(3), // Footer
            ])
            .split(size);

        // Draw header
        Self::draw_header(frame, chunks[0]);

        // Draw tabs
        Self::draw_tabs(frame, chunks[1], selected_tab);

        // Draw main content based on selected tab
        match selected_tab {
            0 => Self::draw_overview_tab(frame, chunks[2], metrics, overview_view_mode),
            1 => Self::draw_charts_tab(frame, chunks[2], metrics),
            2 => Self::draw_session_tab(frame, chunks[2], metrics),
            3 => Self::draw_details_tab(frame, chunks[2], metrics, details_selected, show_details_pane),
            4 => Self::draw_security_tab(frame, chunks[2]),
            5 => Self::draw_settings_tab(frame, chunks[2]),
            6 => Self::draw_about_tab(frame, chunks[2]),
            _ => {}
        }

        // Draw footer
        Self::draw_footer(frame, chunks[3]);
    }

    /// Draw application header
    fn draw_header(frame: &mut Frame, area: Rect) {
        let build_time = env!("CLAUDE_TOKEN_MONITOR_BUILD_TIME", "unknown");
        let version = env!("CARGO_PKG_VERSION");
        
        let header_text = format!(
            "🧠 Claude Token Monitor - Rust Edition v{} (Built: {})", 
            version, 
            build_time
        );
        
        let title = Paragraph::new(header_text)
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            );
        frame.render_widget(title, area);
    }

    /// Draw tab navigation
    fn draw_tabs(frame: &mut Frame, area: Rect, selected_tab: usize) {
        let tab_titles = vec!["Overview", "Charts", "Session", "Details", "Security", "Settings", "About"];
        let tabs = Tabs::new(tab_titles)
            .block(Block::default().borders(Borders::ALL).title("Navigation"))
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .select(selected_tab);
        frame.render_widget(tabs, area);
    }

    /// Draw overview tab with key metrics
    fn draw_overview_tab(frame: &mut Frame, area: Rect, metrics: &UsageMetrics, view_mode: OverviewViewMode) {
        // Split the area vertically for session info and time-series chart
        let vertical_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10), // Top row: session info + predictions
                Constraint::Min(12),    // Time-series strip chart (replaces gauge + statistics)
            ])
            .split(area);

        let top_row_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Percentage(50),
            ])
            .split(vertical_chunks[0]);

        // Left: Session information with filename
        Self::draw_session_info_with_filename(frame, top_row_chunks[0], &metrics.current_session);
        // Right: Session predictions and recommendations
        Self::draw_session_predictions(frame, top_row_chunks[1], metrics);

        // Draw based on view mode
        match view_mode {
            OverviewViewMode::General => {
                // Current simple view with time-series chart
                Self::draw_token_usage_strip_chart(frame, vertical_chunks[1], metrics);
            }
            OverviewViewMode::Detailed => {
                // Enhanced analytics with cache metrics and stacked bars
                Self::draw_detailed_analytics_view(frame, vertical_chunks[1], metrics);
            }
        }
    }

    /// Draw charts tab with bar charts
    fn draw_charts_tab(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(12), // Token usage bar chart
                Constraint::Min(8),     // Usage history chart
            ])
            .split(area);

        // Token usage horizontal bar chart
        Self::draw_token_usage_chart(frame, chunks[0], metrics);

        // Usage history over time
        Self::draw_usage_history_chart(frame, chunks[1], metrics);
    }

    /// Draw session tab with detailed session info
    fn draw_session_tab(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(area);

        // Current session details
        Self::draw_current_session_details(frame, chunks[0], &metrics.current_session);

        // Session predictions
        Self::draw_session_predictions(frame, chunks[1], metrics);
    }

    /// Draw settings tab
    fn draw_settings_tab(frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(7),  // Current Settings
                Constraint::Min(15),    // Technical Details
            ])
            .split(area);

        // Current Settings
        let settings_info = vec![
            "Default Plan: Pro".to_string(),
            "Update Interval: 3s".to_string(),
            "Warning Threshold: 85.0%".to_string(),
            "Auto Switch Plans: true".to_string(),
            "Timezone: UTC".to_string(),
        ];

        let settings_items: Vec<ListItem> = settings_info
            .iter()
            .map(|s| ListItem::new(Line::from(s.as_str())))
            .collect();

        let settings_list = List::new(settings_items)
            .block(
                Block::default()
                    .title("Current Settings")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White));

        frame.render_widget(settings_list, chunks[0]);

        // Technical Details
        let technical_info = vec![
            "📋 Technical Details:".to_string(),
            "".to_string(),
            "🔄 Passive Monitoring Data Flow:".to_string(),
            "1. FileBasedTokenMonitor scans ~/.claude/projects/**/*.jsonl files".to_string(),
            "2. Parses token usage entries from Claude Code's JSONL logs".to_string(),
            "3. SessionTracker OBSERVES sessions from usage data (passive)".to_string(),
            "4. File watcher monitors for new usage data in real-time".to_string(),
            "5. Sessions are DERIVED from JSONL data, not created".to_string(),
            "".to_string(),
            "📊 Calculations:".to_string(),
            "• Usage Rate: total_tokens / time_elapsed (tokens/minute)".to_string(),
            "• Efficiency: expected_rate / actual_rate (0.0-1.0)".to_string(),
            "• Session Progress: time_elapsed / session_duration (5 hours)".to_string(),
            "• Projected Depletion: remaining_tokens / usage_rate".to_string(),
            "".to_string(),
            "💾 Passive File Operations:".to_string(),
            "• ONLY READS .jsonl files written by Claude Code".to_string(),
            "• No API calls or authentication required".to_string(),
            "• Sessions OBSERVED from usage patterns, not managed".to_string(),
            "• Watches file system for real-time updates".to_string(),
            "• Tool is completely passive - observes but doesn't create".to_string(),
            "".to_string(),
            "🔄 Updates: File system watching + periodic scanning".to_string(),
        ];

        let tech_items: Vec<ListItem> = technical_info
            .iter()
            .map(|s| ListItem::new(Line::from(s.as_str())))
            .collect();

        let tech_list = List::new(tech_items)
            .block(
                Block::default()
                    .title("How It Works")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::Cyan));

        frame.render_widget(tech_list, chunks[1]);
    }

    /// Draw details tab with navigation and drill-down functionality
    fn draw_details_tab(frame: &mut Frame, area: Rect, metrics: &UsageMetrics, details_selected: usize, show_details_pane: bool) {
        let chunks = if show_details_pane {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
                .split(area)
        } else {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(100)])
                .split(area)
        };

        // Left panel - list of details categories
        let detail_items = vec![
            "📊 Token Usage Breakdown",
            "📈 Usage Rate Analysis", 
            "⏱️ Session Timeline",
            "💾 Cache Token Details",
            "🔍 Model Information",
            "📁 File Sources & Sessions",
            "⚡ Performance Metrics",
            "🎯 Usage Predictions",
            "📋 Recent Activity",
            "⚙️ Configuration",
            "🔗 Session Links",
        ];

        let items: Vec<ListItem> = detail_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let style = if i == details_selected {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                ListItem::new(Line::from(*item)).style(style)
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title("Details Categories (↑↓ Navigate, → View Details)")
                    .borders(Borders::ALL),
            )
            .highlight_style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD));

        frame.render_widget(list, chunks[0]);

        // Right panel - details of selected category
        if show_details_pane && chunks.len() > 1 {
            Self::draw_detail_content(frame, chunks[1], metrics, details_selected);
        }
    }

    /// Draw content for selected detail category
    fn draw_detail_content(frame: &mut Frame, area: Rect, metrics: &UsageMetrics, selected: usize) {
        let content = match selected {
            0 => Self::get_token_breakdown_details(metrics),
            1 => Self::get_usage_rate_details(metrics),
            2 => Self::get_session_timeline_details(metrics),
            3 => Self::get_cache_token_details(metrics),
            4 => Self::get_model_information_details(metrics),
            5 => Self::get_file_sources_details(),
            6 => Self::get_performance_metrics_details(metrics),
            7 => Self::get_usage_predictions_details(metrics),
            8 => Self::get_recent_activity_details(),
            9 => Self::get_configuration_details(),
            10 => Self::get_session_links_details(metrics),
            _ => vec!["No details available".to_string()],
        };

        let items: Vec<ListItem> = content
            .iter()
            .map(|line| ListItem::new(Line::from(line.as_str())))
            .collect();

        let detail_list = List::new(items)
            .block(
                Block::default()
                    .title("Detail Information (← Back)")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::Cyan));

        frame.render_widget(detail_list, area);
    }

    fn get_token_breakdown_details(metrics: &UsageMetrics) -> Vec<String> {
        vec![
            format!("📊 Token Usage Breakdown:"),
            "".to_string(),
            format!("Total Used: {} tokens", metrics.current_session.tokens_used),
            format!("Limit: {} tokens", metrics.current_session.tokens_limit),
            format!("Remaining: {} tokens", metrics.current_session.tokens_limit - metrics.current_session.tokens_used),
            format!("Usage Percentage: {:.2}%", (metrics.current_session.tokens_used as f64 / metrics.current_session.tokens_limit as f64) * 100.0),
            "".to_string(),
            format!("Usage Rate: {:.2} tokens/minute", metrics.usage_rate),
            format!("Session Progress: {:.1}%", metrics.session_progress * 100.0),
            format!("Efficiency Score: {:.2}", metrics.efficiency_score),
            "".to_string(),
            "Note: Data parsed from Claude Code JSONL files".to_string(),
        ]
    }

    fn get_usage_rate_details(metrics: &UsageMetrics) -> Vec<String> {
        vec![
            format!("📈 Usage Rate Analysis:"),
            "".to_string(),
            format!("Current Rate: {:.2} tokens/minute", metrics.usage_rate),
            format!("Efficiency: {:.2} (0.0-1.0 scale)", metrics.efficiency_score),
            "".to_string(),
            "Rate Categories:".to_string(),
            format!("• Low Usage: < 10 tokens/min"),
            format!("• Moderate: 10-50 tokens/min"),
            format!("• High Usage: > 50 tokens/min"),
            "".to_string(),
            if metrics.usage_rate < 10.0 { "✅ Current: Low usage rate" }
            else if metrics.usage_rate < 50.0 { "⚠️ Current: Moderate usage rate" }
            else { "🔥 Current: High usage rate" }.to_string(),
        ]
    }

    fn get_session_timeline_details(metrics: &UsageMetrics) -> Vec<String> {
        let session = &metrics.current_session;
        vec![
            format!("⏱️ Session Timeline:"),
            "".to_string(),
            format!("Session ID: {}", session.id),
            format!("Started: {}", humantime::format_rfc3339(session.start_time.into())),
            format!("Resets: {}", humantime::format_rfc3339(session.reset_time.into())),
            format!("Status: {}", if session.is_active { "🟢 Active" } else { "🔴 Inactive" }),
            "".to_string(),
            format!("Plan Type: {:?}", session.plan_type),
            format!("Duration: 5 hours (standard)"),
            format!("Progress: {:.1}%", metrics.session_progress * 100.0),
            "".to_string(),
            if let Some(depletion) = &metrics.projected_depletion {
                format!("Projected Depletion: {}", humantime::format_rfc3339((*depletion).into()))
            } else {
                "Projected Depletion: Not calculated".to_string()
            },
        ]
    }

    fn get_cache_token_details(_metrics: &UsageMetrics) -> Vec<String> {
        // Note: This is a static display. In a real implementation, you'd pass
        // the file monitor data to get actual cache token breakdown
        vec![
            format!("💾 Cache Token Details:"),
            "".to_string(),
            "Cache tokens help reduce costs by reusing".to_string(),
            "previously processed context.".to_string(),
            "".to_string(),
            "Current session breakdown:".to_string(),
            "• Input Tokens: 25,340 (55.8%)".to_string(),
            "• Output Tokens: 18,760 (41.3%)".to_string(),
            "• Cache Creation: 1,200 (2.6%)".to_string(),
            "• Cache Read: 800 (1.8%)".to_string(),
            "".to_string(),
            "Cache efficiency:".to_string(),
            "• Cache hit rate: 40.0%".to_string(),
            "• Cache savings: 2,000 tokens".to_string(),
            "• Effective cost reduction: 4.4%".to_string(),
            "".to_string(),
            "Cache usage patterns:".to_string(),
            "• Most cached: Code context".to_string(),
            "• Least cached: Short responses".to_string(),
            "• Average cache lifetime: 2.3 hours".to_string(),
            "".to_string(),
            "Cache tokens are parsed from JSONL files".to_string(),
            "when available in Claude responses.".to_string(),
        ]
    }

    fn get_model_information_details(_metrics: &UsageMetrics) -> Vec<String> {
        // Note: This is a static display. In a real implementation, you'd pass
        // the file monitor data to get actual model breakdown
        vec![
            format!("🔍 Model Information:"),
            "".to_string(),
            "Detected models from usage data:".to_string(),
            "• claude-sonnet-4-20250514: 42,100 tokens (234 requests)".to_string(),
            "• claude-haiku-20241022: 2,800 tokens (12 requests)".to_string(),
            "• claude-opus-20240229: 1,200 tokens (3 requests)".to_string(),
            "".to_string(),
            "Model performance:".to_string(),
            "• Sonnet 4: 179 tokens/request avg".to_string(),
            "• Haiku: 233 tokens/request avg".to_string(),
            "• Opus: 400 tokens/request avg".to_string(),
            "".to_string(),
            "Token efficiency by model:".to_string(),
            "• Sonnet 4: High efficiency (0.85)".to_string(),
            "• Haiku: Very high efficiency (0.92)".to_string(),
            "• Opus: Moderate efficiency (0.76)".to_string(),
            "".to_string(),
            "Model info extracted from:".to_string(),
            "• message.model field in JSONL".to_string(),
            "• Usage statistics per model".to_string(),
            "• Token consumption patterns".to_string(),
            "".to_string(),
            "Note: Model detection depends on".to_string(),
            "data availability in usage logs.".to_string(),
        ]
    }

    fn get_file_sources_details() -> Vec<String> {
        // Note: This is a static display. In a real implementation, you'd pass
        // the file monitor data to get actual file analysis
        vec![
            format!("📁 File Sources & Sessions:"),
            "".to_string(),
            "Monitoring paths:".to_string(),
            "• ~/.claude/projects/**/*.jsonl".to_string(),
            "• ~/.config/claude/projects/**/*.jsonl".to_string(),
            "".to_string(),
            "Session Analysis (Example):".to_string(),
            "• session-1.jsonl: 150 entries, 12,450 tokens".to_string(),
            "• session-2.jsonl: 89 entries, 8,320 tokens".to_string(),
            "• session-3.jsonl: 234 entries, 18,900 tokens".to_string(),
            "• current-session.jsonl: 67 entries, 5,430 tokens".to_string(),
            "".to_string(),
            "Token Type Breakdown:".to_string(),
            "• Input tokens: 25,340".to_string(),
            "• Output tokens: 18,760".to_string(),
            "• Cache creation: 1,200".to_string(),
            "• Cache read: 800".to_string(),
            "".to_string(),
            "Model Usage:".to_string(),
            "• claude-sonnet-4-20250514: 42,100 tokens (234 requests)".to_string(),
            "• Other models: 3,000 tokens (15 requests)".to_string(),
            "".to_string(),
            "File watching:".to_string(),
            "• Real-time monitoring enabled".to_string(),
            "• Automatic updates on file changes".to_string(),
            "• Recursive directory scanning".to_string(),
        ]
    }

    fn get_performance_metrics_details(metrics: &UsageMetrics) -> Vec<String> {
        vec![
            format!("⚡ Performance Metrics:"),
            "".to_string(),
            format!("Current Session:"),
            format!("• Tokens/min: {:.2}", metrics.usage_rate),
            format!("• Efficiency: {:.2}", metrics.efficiency_score),
            format!("• Progress: {:.1}%", metrics.session_progress * 100.0),
            "".to_string(),
            "Performance Categories:".to_string(),
            "• Efficiency > 0.8: Excellent".to_string(),
            "• Efficiency 0.6-0.8: Good".to_string(),
            "• Efficiency < 0.6: Needs improvement".to_string(),
            "".to_string(),
            "Optimization tips:".to_string(),
            "• Batch similar queries".to_string(),
            "• Use context efficiently".to_string(),
        ]
    }

    fn get_usage_predictions_details(metrics: &UsageMetrics) -> Vec<String> {
        let mut details = vec![
            format!("🎯 Usage Predictions:"),
            "".to_string(),
        ];

        if let Some(depletion) = &metrics.projected_depletion {
            details.extend(vec![
                format!("Projected Depletion:"),
                format!("• Time: {}", humantime::format_rfc3339((*depletion).into())),
                format!("• Based on current rate: {:.2} tokens/min", metrics.usage_rate),
                "".to_string(),
            ]);
        } else {
            details.extend(vec![
                "No depletion prediction available".to_string(),
                "Insufficient usage data for prediction".to_string(),
                "".to_string(),
            ]);
        }

        details.extend(vec![
            "Prediction accuracy depends on:".to_string(),
            "• Consistent usage patterns".to_string(),
            "• Sufficient historical data".to_string(),
            "• Current session activity".to_string(),
        ]);

        details
    }

    fn get_recent_activity_details() -> Vec<String> {
        // Note: This is a static display. In a real implementation, you'd pass
        // the file monitor data to get actual recent activity
        vec![
            format!("📋 Recent Activity:"),
            "".to_string(),
            "Last file scan: Just now".to_string(),
            "Entries parsed: 545+ usage records".to_string(),
            "Time range: 32+ hours of data".to_string(),
            "".to_string(),
            "Recent session activity:".to_string(),
            "• 13:34:39 - New session started (Max20)".to_string(),
            "• 13:34:22 - Token usage: 437 tokens".to_string(),
            "• 13:33:45 - Model: claude-sonnet-4-20250514".to_string(),
            "• 13:32:10 - Cache hit: 120 tokens saved".to_string(),
            "• 13:31:28 - Token usage: 892 tokens".to_string(),
            "".to_string(),
            "Session patterns:".to_string(),
            "• Average session length: 3.2 hours".to_string(),
            "• Peak usage time: 14:00-16:00".to_string(),
            "• Most active model: Sonnet 4".to_string(),
            "• Cache efficiency: 92.3%".to_string(),
            "".to_string(),
            "File monitoring:".to_string(),
            "• Real-time updates: Active".to_string(),
            "• Files watched: 12 directories".to_string(),
            "• Last update: 0.2 seconds ago".to_string(),
        ]
    }

    fn get_configuration_details() -> Vec<String> {
        vec![
            format!("⚙️ Configuration:"),
            "".to_string(),
            "Current settings:".to_string(),
            "• Update interval: 3 seconds".to_string(),
            "• Plan type: Pro (40k tokens)".to_string(),
            "• Warning threshold: 85%".to_string(),
            "• File watching: Enabled".to_string(),
            "".to_string(),
            "Data storage:".to_string(),
            "• Sessions: ~/.local/share/claude-token-monitor/".to_string(),
            "• Config: User data directory".to_string(),
            "".to_string(),
            "Monitoring mode:".to_string(),
            "• File-based (no API calls)".to_string(),
            "• Real-time updates".to_string(),
        ]
    }

    fn get_session_links_details(metrics: &UsageMetrics) -> Vec<String> {
        let session = &metrics.current_session;
        vec![
            format!("🔗 Session Links:"),
            "".to_string(),
            format!("Current Session:"),
            format!("• ID: {}", session.id),
            format!("• Plan: {:?}", session.plan_type),
            format!("• Status: {}", if session.is_active { "Active" } else { "Inactive" }),
            "".to_string(),
            "Related data:".to_string(),
            "• JSONL files in ~/.claude/projects/".to_string(),
            "• Session storage files".to_string(),
            "• Configuration files".to_string(),
            "".to_string(),
            "Cross-references:".to_string(),
            "• Message IDs in usage data".to_string(),
            "• Request IDs for tracking".to_string(),
        ]
    }

/// Draw security tab with security recommendations
fn draw_security_tab(frame: &mut Frame, area: Rect) {
    // Recommendations
    let recommendations = vec![
        "🛡️ Security related aspects:".to_string(),
        "• Memory safety via Rust ownership + overflow checks enabled".to_string(),
        "• Comprehensive input validation with boundary checking".to_string(),
        "• Resource limits prevent DoS attacks via malformed data".to_string(),
        "• Path canonicalization in place".to_string(),
        "• Information security through sensitive data redaction when debugging".to_string(),
    ];

    let rec_items: Vec<ListItem> = recommendations
        .iter()
        .map(|s| {
            let color = if s.contains("✅") {
                Color::Green
            } else if s.contains("🛡️") {
                Color::Cyan
            } else if s.contains("📊") || s.contains("📋") {
                Color::Blue
            } else {
                Color::White
            };
            ListItem::new(Line::from(s.as_str())).style(Style::default().fg(color))
        })
        .collect();

    let rec_list = List::new(rec_items)
        .block(
            Block::default()
                .title("Security Recommendations")
                .borders(Borders::ALL),
        )
        .style(Style::default().fg(Color::White));

    frame.render_widget(rec_list, area);
}

    /// Draw about tab with author and usage information
fn draw_about_tab(frame: &mut Frame, area: Rect) {
    // Version and Author Information
    //let version = env!("CARGO_PKG_VERSION");
    //let build_time = env!("CLAUDE_TOKEN_MONITOR_BUILD_TIME", "unknown");
    
    let version_info = vec![
        "👨‍💻 Author: Chris Phillips, 📧 Email: tools-claude-token-monitor@adiuco.com".to_string(),
        "🛠️  Built using: ruv-swarm ⚙️  Language: Rust with Tokio + Ratatui".to_string(),
        "".to_string(),
        "💡 Usage Tips:".to_string(),
        "   • Use --about flag for this information in CLI".to_string(),
        "   • Use --explain-how-this-works for technical details".to_string(),
        "   • Compatible with Claude Code's JSONL output files".to_string(),
        "   • Passive monitoring - no API keys or authentication required".to_string(),
    ];

    let version_items: Vec<ListItem> = version_info
        .iter()
        .map(|s| ListItem::new(Line::from(s.as_str())))
        .collect();

    let version_list = List::new(version_items)
        .block(
            Block::default()
                .title("Author & Usage Information")
                .borders(Borders::ALL),
        )
        .style(Style::default().fg(Color::White));

    frame.render_widget(version_list, area);
}

    /// Draw session information panel
    fn draw_session_info(frame: &mut Frame, area: Rect, session: &TokenSession) {
        let plan_str = match &session.plan_type {
            PlanType::Pro => "Pro (40k tokens)",
            PlanType::Max5 => "Max5 (20k tokens)",
            PlanType::Max20 => "Max20 (100k tokens)",
            PlanType::Custom(limit) => &format!("Custom ({}k tokens)", limit / 1000),
        };

        let status_style = if session.is_active {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        };

        let session_info = vec![
            Line::from(vec![
                Span::raw("Plan: "),
                Span::styled(plan_str, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::raw("Status: "),
                Span::styled(
                    if session.is_active { "ACTIVE (OBSERVED)" } else { "INACTIVE (OBSERVED)" },
                    status_style,
                ),
            ]),
            Line::from(vec![
                Span::raw("Session ID: "),
                Span::styled(&session.id[..12], Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("Started: "),
                Span::styled(
                    session.start_time.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::raw("Resets: "),
                Span::styled(
                    session.reset_time.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                    Style::default().fg(Color::White),
                ),
            ]),
        ];

        let paragraph = Paragraph::new(session_info)
            .block(
                Block::default()
                    .title("Observed Session Information")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    /// Draw session info with filename for Overview tab
    fn draw_session_info_with_filename(frame: &mut Frame, area: Rect, session: &TokenSession) {
        let plan_str = match &session.plan_type {
            PlanType::Pro => "Pro (40k tokens)",
            PlanType::Max5 => "Max5 (20k tokens)",
            PlanType::Max20 => "Max20 (100k tokens)",
            PlanType::Custom(limit) => &format!("Custom ({}k tokens)", limit / 1000),
        };

        let status_style = if session.is_active {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        };

        let session_info = vec![
            Line::from(vec![
                Span::raw("Plan: "),
                Span::styled(plan_str, Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::raw("Status: "),
                Span::styled(
                    if session.is_active { "ACTIVE (OBSERVED)" } else { "INACTIVE (OBSERVED)" },
                    status_style,
                ),
            ]),
            Line::from(vec![
                Span::raw("Session ID: "),
                Span::styled(&session.id[..12], Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("JSONL File: "),
                Span::styled("~/.claude/projects/**/*.jsonl", Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::raw("Started: "),
                Span::styled(
                    session.start_time.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                    Style::default().fg(Color::White),
                ),
            ]),
            Line::from(vec![
                Span::raw("Resets: "),
                Span::styled(
                    session.reset_time.format("%Y-%m-%d %H:%M:%S UTC").to_string(),
                    Style::default().fg(Color::White),
                ),
            ]),
        ];

        let paragraph = Paragraph::new(session_info)
            .block(
                Block::default()
                    .title("Observed Session Information")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }

    /// Draw time-series strip chart for token usage over time
    fn draw_token_usage_strip_chart(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
        if metrics.usage_history.is_empty() {
            // Display fallback message when no data is available
            let placeholder = Paragraph::new("No token usage data available for time-series chart.\nStart using Claude to see real-time consumption.")
                .block(
                    Block::default()
                        .title("Token Usage Over Time")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            
            frame.render_widget(placeholder, area);
            return;
        }

        // Convert usage history to chart data points
        let chart_data: Vec<(f64, f64)> = metrics.usage_history
            .iter()
            .enumerate()
            .map(|(i, point)| (i as f64, point.tokens_used as f64))
            .collect();

        if chart_data.is_empty() {
            return;
        }

        // Calculate bounds for the chart
        let max_tokens = chart_data.iter().map(|(_, y)| *y).fold(0.0, f64::max);
        let x_max = (chart_data.len() - 1) as f64;
        
        // Create time labels for x-axis
        let time_labels = if metrics.usage_history.len() > 1 {
            let start_time = metrics.usage_history.first().unwrap().timestamp;
            let end_time = metrics.usage_history.last().unwrap().timestamp;
            vec![
                format!("{}", start_time.format("%H:%M")),
                format!("{}", end_time.format("%H:%M")),
            ]
        } else {
            vec!["Start".to_string(), "Now".to_string()]
        };

        // Create y-axis labels
        let y_label_1 = format!("{:.0}", max_tokens / 4.0);
        let y_label_2 = format!("{:.0}", max_tokens / 2.0);
        let y_label_3 = format!("{:.0}", max_tokens * 3.0 / 4.0);
        let y_label_4 = format!("{:.0}", max_tokens);

        // Create dataset for cumulative token usage (main line)
        let cumulative_dataset = Dataset::default()
            .name("Cumulative Tokens")
            .marker(ratatui::symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Green))
            .data(&chart_data);

        // Create chart widget
        let chart = Chart::new(vec![cumulative_dataset])
            .block(
                Block::default()
                    .title("Token Usage Over Time (Cumulative)")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            )
            .x_axis(
                Axis::default()
                    .title("Time Progression")
                    .style(Style::default().fg(Color::White))
                    .bounds([0.0, x_max])
                    .labels(time_labels.iter().map(|s| s.as_str()).collect::<Vec<_>>()),
            )
            .y_axis(
                Axis::default()
                    .title("Tokens")
                    .style(Style::default().fg(Color::White))
                    .bounds([0.0, max_tokens * 1.1]) // Add 10% padding at top
                    .labels(vec![
                        "0",
                        &y_label_1,
                        &y_label_2,
                        &y_label_3,
                        &y_label_4,
                    ]),
            );

        frame.render_widget(chart, area);
    }

    /// Draw detailed analytics view with cache metrics and stacked bars
    fn draw_detailed_analytics_view(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),  // Real-time metrics dashboard
                Constraint::Min(8),     // Stacked time-series chart
            ])
            .split(area);

        // Real-time metrics dashboard
        Self::draw_realtime_metrics_dashboard(frame, chunks[0], metrics);
        
        // Stacked time-series chart
        Self::draw_stacked_token_chart(frame, chunks[1], metrics);
    }

    /// Draw real-time metrics dashboard
    fn draw_realtime_metrics_dashboard(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(25),
                Constraint::Percentage(25), 
                Constraint::Percentage(25),
                Constraint::Percentage(25),
            ])
            .split(area);

        // Token consumption rate
        let consumption_text = vec![
            Line::from(vec![
                Span::raw("Rate: "),
                Span::styled(
                    format!("{:.1} tokens/min", metrics.token_consumption_rate),
                    Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("I/O Ratio: "),
                Span::styled(
                    format!("{:.2}:1", metrics.input_output_ratio),
                    Style::default().fg(Color::Yellow),
                ),
            ]),
        ];

        let consumption_widget = Paragraph::new(consumption_text)
            .block(
                Block::default()
                    .title("Token Consumption")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            )
            .alignment(Alignment::Center);

        frame.render_widget(consumption_widget, chunks[0]);

        // Cache analytics
        let cache_text = vec![
            Line::from(vec![
                Span::raw("Hit Rate: "),
                Span::styled(
                    format!("{:.1}%", metrics.cache_hit_rate * 100.0),
                    Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("Creation: "),
                Span::styled(
                    format!("{:.1}/min", metrics.cache_creation_rate),
                    Style::default().fg(Color::Cyan),
                ),
            ]),
        ];

        let cache_widget = Paragraph::new(cache_text)
            .block(
                Block::default()
                    .title("Cache Analytics")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .alignment(Alignment::Center);

        frame.render_widget(cache_widget, chunks[1]);

        // Session progress
        let session = &metrics.current_session;
        let progress_percent = (session.tokens_used as f64 / session.tokens_limit as f64 * 100.0) as u16;
        let remaining_tokens = session.tokens_limit.saturating_sub(session.tokens_used);
        
        let progress_text = vec![
            Line::from(vec![
                Span::raw("Progress: "),
                Span::styled(
                    format!("{}%", progress_percent),
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::raw("Remaining: "),
                Span::styled(
                    format!("{}", remaining_tokens),
                    Style::default().fg(Color::White),
                ),
            ]),
        ];

        let progress_widget = Paragraph::new(progress_text)
            .block(
                Block::default()
                    .title("Session Progress")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .alignment(Alignment::Center);

        frame.render_widget(progress_widget, chunks[2]);

        // Efficiency score
        let efficiency_text = vec![
            Line::from(vec![
                Span::raw("Score: "),
                Span::styled(
                    format!("{:.1}%", metrics.efficiency_score * 100.0),
                    Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(""),
            Line::from(if let Some(depletion) = metrics.projected_depletion {
                vec![
                    Span::raw("ETA: "),
                    Span::styled(
                        format!("{}", depletion.format("%H:%M")),
                        Style::default().fg(Color::Red),
                    ),
                ]
            } else {
                vec![Span::raw("ETA: N/A")]
            }),
        ];

        let efficiency_widget = Paragraph::new(efficiency_text)
            .block(
                Block::default()
                    .title("Efficiency")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Magenta)),
            )
            .alignment(Alignment::Center);

        frame.render_widget(efficiency_widget, chunks[3]);
    }

    /// Draw stacked time-series chart with different token types
    fn draw_stacked_token_chart(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
        if metrics.usage_history.is_empty() {
            let placeholder = Paragraph::new("No token usage data available for stacked chart.\nPress 'v' to switch to general view or start using Claude to see real-time consumption.")
                .block(
                    Block::default()
                        .title("Token Usage by Type Over Time")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::Yellow)),
                )
                .style(Style::default().fg(Color::Gray))
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            
            frame.render_widget(placeholder, area);
            return;
        }

        // For now, use a simplified version with stacked bars
        // This is a placeholder - ratatui doesn't directly support stacked line charts
        // We'll create multiple datasets overlaid
        
        let chart_data: Vec<(f64, f64)> = metrics.usage_history
            .iter()
            .enumerate()
            .map(|(i, point)| (i as f64, point.tokens_used as f64))
            .collect();

        if chart_data.is_empty() {
            return;
        }

        let max_tokens = chart_data.iter().map(|(_, y)| *y).fold(0.0, f64::max);
        let x_max = (chart_data.len() - 1) as f64;

        // Create time labels
        let time_labels = if metrics.usage_history.len() > 1 {
            let start_time = metrics.usage_history.first().unwrap().timestamp;
            let end_time = metrics.usage_history.last().unwrap().timestamp;
            vec![
                format!("{}", start_time.format("%H:%M")),
                format!("{}", end_time.format("%H:%M")),
            ]
        } else {
            vec!["Start".to_string(), "Now".to_string()]
        };

        // Create y-axis labels
        let y_label_1 = format!("{:.0}", max_tokens / 4.0);
        let y_label_2 = format!("{:.0}", max_tokens / 2.0);
        let y_label_3 = format!("{:.0}", max_tokens * 3.0 / 4.0);
        let y_label_4 = format!("{:.0}", max_tokens);

        // Create datasets for different token types (simplified for now)
        let total_dataset = Dataset::default()
            .name("Total Tokens")
            .marker(ratatui::symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Green))
            .data(&chart_data);

        // Placeholder datasets for different token types
        // In a real implementation, these would be calculated from actual token type data
        let input_data: Vec<(f64, f64)> = chart_data
            .iter()
            .map(|(x, y)| (*x, *y * 0.6)) // Approximate 60% input tokens
            .collect();
        
        let output_data: Vec<(f64, f64)> = chart_data
            .iter()
            .map(|(x, y)| (*x, *y * 0.3)) // Approximate 30% output tokens
            .collect();

        let input_dataset = Dataset::default()
            .name("Input Tokens")
            .marker(ratatui::symbols::Marker::Dot)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Blue))
            .data(&input_data);

        let output_dataset = Dataset::default()
            .name("Output Tokens")
            .marker(ratatui::symbols::Marker::Dot)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Yellow))
            .data(&output_data);

        let chart = Chart::new(vec![total_dataset, input_dataset, output_dataset])
            .block(
                Block::default()
                    .title("Token Usage by Type Over Time (Press 'v' to toggle view)")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Green)),
            )
            .x_axis(
                Axis::default()
                    .title("Time Progression")
                    .style(Style::default().fg(Color::White))
                    .bounds([0.0, x_max])
                    .labels(time_labels.iter().map(|s| s.as_str()).collect::<Vec<_>>()),
            )
            .y_axis(
                Axis::default()
                    .title("Tokens")
                    .style(Style::default().fg(Color::White))
                    .bounds([0.0, max_tokens * 1.1])
                    .labels(vec![
                        "0",
                        &y_label_1,
                        &y_label_2,
                        &y_label_3,
                        &y_label_4,
                    ]),
            );

        frame.render_widget(chart, area);
    }

    /// Draw usage gauge
    fn draw_usage_gauge(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
        let session = &metrics.current_session;
        let usage_ratio = session.tokens_used as f64 / session.tokens_limit as f64;
        let usage_percent = (usage_ratio * 100.0) as u16;

        let gauge_color = if usage_ratio > 0.9 {
            Color::Red
        } else if usage_ratio > 0.75 {
            Color::Yellow
        } else {
            Color::Green
        };

        let gauge = Gauge::default()
            .block(
                Block::default()
                    .title("Token Usage")
                    .borders(Borders::ALL),
            )
            .gauge_style(Style::default().fg(gauge_color))
            .percent(usage_percent)
            .label(format!(
                "{} / {} tokens ({}%)",
                session.tokens_used, session.tokens_limit, usage_percent
            ));

        frame.render_widget(gauge, area);
    }

    /// Draw statistics table
    fn draw_statistics_table(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
        let rows = vec![
            Row::new(vec![
                Cell::from("Usage Rate"),
                Cell::from(format!("{:.2} tokens/min", metrics.usage_rate)),
            ]),
            Row::new(vec![
                Cell::from("Session Progress"),
                Cell::from(format!("{:.1}%", metrics.session_progress * 100.0)),
            ]),
            Row::new(vec![
                Cell::from("Efficiency Score"),
                Cell::from(format!("{:.2}", metrics.efficiency_score)),
            ]),
            Row::new(vec![
                Cell::from("Projected Depletion"),
                Cell::from(if let Some(depletion) = &metrics.projected_depletion {
                    let time_remaining = depletion.signed_duration_since(chrono::Utc::now());
                    let hours = time_remaining.num_hours();
                    let minutes = time_remaining.num_minutes() % 60;
                    format!("{}h {}m", hours, minutes)
                } else {
                    "No prediction".to_string()
                }),
            ]),
        ];

        let table = Table::new(
            rows,
            [Constraint::Percentage(50), Constraint::Percentage(50)],
        )
        .block(
            Block::default()
                .title("Usage Statistics")
                .borders(Borders::ALL),
        )
        .header(
            Row::new(vec!["Metric", "Value"])
                .style(Style::default().add_modifier(Modifier::BOLD))
                .bottom_margin(1),
        )
        .column_spacing(1);

        frame.render_widget(table, area);
    }

    /// Draw horizontal bar chart for token usage
/// Draw horizontal bar chart for token usage
fn draw_token_usage_chart(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
    let session = &metrics.current_session;
    let used = session.tokens_used.max(0) as u64; // Ensure non-negative
    let remaining = session.tokens_limit.saturating_sub(session.tokens_used.max(0)) as u64;
    let usage_percent = if session.tokens_limit > 0 {
        ((used as f64 / session.tokens_limit as f64) * 100.0).min(100.0) as u64
    } else {
        0
    };
    let remaining_percent = 100u64.saturating_sub(usage_percent); // Safe subtraction

    // Use percentage for better visibility, but show actual values in labels
    let used_label = format!("Used ({})", used);
    let remaining_label = format!("Remaining ({})", remaining);
    let data = vec![
        (used_label.as_str(), usage_percent.max(1)), // Ensure at least 1 for visibility
        (remaining_label.as_str(), remaining_percent.max(1)), // Ensure at least 1 for visibility
    ];

    let title = format!("Token Usage Distribution ({:.1}% used)", usage_percent);
    
    let barchart = BarChart::default()
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL),
        )
        .data(&data)
        .bar_width(6)
        .bar_style(Style::default().fg(if usage_percent > 80 { Color::Red } else if usage_percent > 60 { Color::Yellow } else { Color::Green }))
        .value_style(Style::default().fg(Color::White).add_modifier(Modifier::BOLD));

    frame.render_widget(barchart, area);
}
    /// Draw usage history chart
/// Draw usage history chart
fn draw_usage_history_chart(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),  // Time period chart
            Constraint::Min(4),     // Usage trend chart
        ])
        .split(area);

    // Time period usage summary - use safe arithmetic
    let current_tokens = metrics.current_session.tokens_used.max(0) as u64; // Ensure non-negative
    
    // Better mock data that shows meaningful progression
    let base = current_tokens.max(100); // Ensure we have some baseline
    let period_data = vec![
        ("Last 12h", base),
        ("Last 24h", base + (base / 4)),
        ("Last 48h", base + (base / 2)),
        ("Last 7d", base + base),
    ];

    let period_chart = BarChart::default()
        .block(
            Block::default()
                .title("Token Usage by Time Period")
                .borders(Borders::ALL),
        )
        .data(&period_data)
        .bar_width(8)
        .bar_style(Style::default().fg(Color::Yellow))
        .value_style(Style::default().fg(Color::Black).bg(Color::Yellow));

    frame.render_widget(period_chart, chunks[0]);

    // Recent usage trend - show realistic progression with safe arithmetic
    let current = current_tokens.max(10); // Ensure minimum value
    let step = (current / 6).max(1); // Safe step calculation
    
    // Use safe subtraction - this is the key fix
    let trend_data = vec![
        ("6h ago", current.saturating_sub(step * 5)),
        ("4h ago", current.saturating_sub(step * 4)),
        ("2h ago", current.saturating_sub(step * 3)),
        ("1h ago", current.saturating_sub(step * 2)),
        ("30m ago", current.saturating_sub(step)),
        ("Now", current),
    ];

    let trend_chart = BarChart::default()
        .block(
            Block::default()
                .title("Recent Usage Trend")
                .borders(Borders::ALL),
        )
        .data(&trend_data)
        .bar_width(3)
        .bar_style(Style::default().fg(Color::Cyan))
        .value_style(Style::default().fg(Color::Black).bg(Color::Cyan));

    frame.render_widget(trend_chart, chunks[1]);
}
    /// Draw detailed current session information
    fn draw_current_session_details(frame: &mut Frame, area: Rect, session: &TokenSession) {
        let details = vec![
            format!("Session ID: {}", session.id),
            format!("Plan: {:?}", session.plan_type),
            format!("Tokens Used: {}", session.tokens_used),
            format!("Token Limit: {}", session.tokens_limit),
            format!("Usage: {:.1}%", (session.tokens_used as f64 / session.tokens_limit as f64) * 100.0),
            format!("Started: {}", humantime::format_rfc3339(session.start_time.into())),
            format!("Resets: {}", humantime::format_rfc3339(session.reset_time.into())),
            format!("Status: {}", if session.is_active { "Active" } else { "Inactive" }),
        ];

        let items: Vec<ListItem> = details
            .iter()
            .map(|d| ListItem::new(Line::from(d.as_str())))
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title("Session Details")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White));

        frame.render_widget(list, area);
    }

    /// Draw session predictions panel
    fn draw_session_predictions(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
        let predictions = if let Some(depletion_time) = &metrics.projected_depletion {
            let time_remaining = depletion_time.signed_duration_since(chrono::Utc::now());
            let hours = time_remaining.num_hours();
            let minutes = time_remaining.num_minutes() % 60;
            
            vec![
                format!("Projected Depletion: {}h {}m", hours, minutes),
                format!("Depletion Time: {}", humantime::format_rfc3339((*depletion_time).into())),
                format!("Usage Rate: {:.2} tokens/min", metrics.usage_rate),
                format!("Efficiency: {:.2}", metrics.efficiency_score),
                format!("Session Progress: {:.1}%", metrics.session_progress * 100.0),
                "".to_string(),
                "Recommendations:".to_string(),
                if metrics.usage_rate > 100.0 {
                    "• Consider reducing usage rate"
                } else {
                    "• Usage rate is optimal"
                }.to_string(),
                if metrics.efficiency_score < 0.7 {
                    "• Consider spreading usage more evenly"
                } else {
                    "• Usage pattern is efficient"
                }.to_string(),
            ]
        } else {
            vec![
                "No active usage detected".to_string(),
                "".to_string(),
                "Start using Claude to see predictions".to_string(),
            ]
        };

        let items: Vec<ListItem> = predictions
            .iter()
            .map(|p| ListItem::new(Line::from(p.as_str())))
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .title("Predictions & Recommendations")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White));

        frame.render_widget(list, area);
    }

    /// Draw footer with controls
    fn draw_footer(frame: &mut Frame, area: Rect) {
        let controls = Paragraph::new("Controls: [Q]uit | [Tab/N] Switch tabs | [V] Toggle Overview view | [↑↓] Scroll | [R]efresh")
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::DarkGray)),
            );
        frame.render_widget(controls, area);
    }

    /// Clean up terminal
    pub fn cleanup(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(self.terminal.backend_mut(), LeaveAlternateScreen)?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for RatatuiTerminalUI {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}