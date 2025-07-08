use crate::models::*;
use anyhow::Result;
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
        BarChart, Block, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table, Tabs,
        Wrap,
    },
    Frame, Terminal,
};
use std::io;
use std::time::Duration;
use tokio::time::sleep;
use humantime;

/// Enhanced terminal UI using Ratatui
pub struct RatatuiTerminalUI {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    should_exit: bool,
    selected_tab: usize,
    scroll_offset: usize,
}

impl RatatuiTerminalUI {
    /// Create new Ratatui terminal UI
    pub fn new(_config: UserConfig) -> Result<Self> {
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
        })
    }

    /// Main UI loop
    pub async fn run(&mut self, metrics: &UsageMetrics) -> Result<()> {
        let current_metrics = metrics.clone();
        
        loop {
            // Draw the UI
            let metrics_clone = current_metrics.clone();
            let selected_tab = self.selected_tab;
            self.terminal.draw(move |frame| {
                Self::draw_ui_static(frame, &metrics_clone, selected_tab);
            })?;

            // Handle input with timeout
            if self.handle_input().await? {
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
                match code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        self.should_exit = true;
                        return Ok(true);
                    }
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        self.should_exit = true;
                        return Ok(true);
                    }
                    KeyCode::Tab => {
                        self.selected_tab = (self.selected_tab + 1) % 5;
                    }
                    KeyCode::BackTab => {
                        self.selected_tab = if self.selected_tab == 0 { 4 } else { self.selected_tab - 1 };
                    }
                    KeyCode::Up => {
                        self.scroll_offset = self.scroll_offset.saturating_sub(1);
                    }
                    KeyCode::Down => {
                        self.scroll_offset = self.scroll_offset.saturating_add(1);
                    }
                    KeyCode::Char('r') => {
                        // Refresh - could trigger a metrics update
                    }
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    /// Draw the main UI (static version for terminal callback)
    fn draw_ui_static(frame: &mut Frame, metrics: &UsageMetrics, selected_tab: usize) {
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
            0 => Self::draw_overview_tab(frame, chunks[2], metrics),
            1 => Self::draw_charts_tab(frame, chunks[2], metrics),
            2 => Self::draw_session_tab(frame, chunks[2], metrics),
            3 => Self::draw_settings_tab(frame, chunks[2]),
            4 => Self::draw_about_tab(frame, chunks[2]),
            _ => {}
        }

        // Draw footer
        Self::draw_footer(frame, chunks[3]);
    }

    /// Draw application header
    fn draw_header(frame: &mut Frame, area: Rect) {
        let title = Paragraph::new("🧠 Claude Token Monitor - Rust Edition")
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
        let tab_titles = vec!["Overview", "Charts", "Session", "Settings", "About"];
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
    fn draw_overview_tab(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8), // Session info
                Constraint::Length(6), // Usage gauge
                Constraint::Min(6),    // Statistics
            ])
            .split(area);

        // Session information
        Self::draw_session_info(frame, chunks[0], &metrics.current_session);

        // Usage gauge
        Self::draw_usage_gauge(frame, chunks[1], metrics);

        // Statistics table
        Self::draw_statistics_table(frame, chunks[2], metrics);
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
            "🔄 Data Flow:".to_string(),
            "1. FileBasedTokenMonitor scans ~/.claude/projects/**/*.jsonl files".to_string(),
            "2. Parses token usage entries from Claude Code's JSONL logs".to_string(),
            "3. SessionTracker manages sessions in ~/.local/share/claude-token-monitor/".to_string(),
            "4. File watcher monitors for new usage data in real-time".to_string(),
            "".to_string(),
            "📊 Calculations:".to_string(),
            "• Usage Rate: total_tokens / time_elapsed (tokens/minute)".to_string(),
            "• Efficiency: expected_rate / actual_rate (0.0-1.0)".to_string(),
            "• Session Progress: time_elapsed / session_duration (5 hours)".to_string(),
            "• Projected Depletion: remaining_tokens / usage_rate".to_string(),
            "".to_string(),
            "💾 File Operations:".to_string(),
            "• Reads .jsonl files written by Claude Code during conversations".to_string(),
            "• No API calls or authentication required".to_string(),
            "• Sessions persisted locally for history tracking".to_string(),
            "• Watches file system for real-time updates".to_string(),
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

    /// Draw about tab with version, author, and attribution information
    fn draw_about_tab(frame: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),  // Version & Author
                Constraint::Min(12),    // Attribution & Contributors
            ])
            .split(area);

        // Version and Author Information
        let version_info = vec![
            "📱 Claude Token Monitor v0.2.2".to_string(),
            "".to_string(),
            "👨‍💻 Author: Chris Phillips".to_string(),
            "📧 Email: chris@adiuco.com".to_string(),
            "🛠️  Built using: ruv-swarm".to_string(),
            "⚙️  Language: Rust with Tokio + Ratatui".to_string(),
        ];

        let version_items: Vec<ListItem> = version_info
            .iter()
            .map(|s| ListItem::new(Line::from(s.as_str())))
            .collect();

        let version_list = List::new(version_items)
            .block(
                Block::default()
                    .title("Version & Author")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::White));

        frame.render_widget(version_list, chunks[0]);

        // Attribution and Contributors
        let attribution_info = vec![
            "🙏 Attribution & Contributors:".to_string(),
            "".to_string(),
            "📚 Original Concept:".to_string(),
            "   • Created by: @Maciek-roboblog".to_string(),
            "   • Project: Claude-Code-Usage-Monitor".to_string(),
            "   • Repository: github.com/Maciek-roboblog/Claude-Code-Usage-Monitor".to_string(),
            "".to_string(),
            "🌟 Contributors to Original Project:".to_string(),
            "   See: github.com/Maciek-roboblog/Claude-Code-Usage-Monitor".to_string(),
            "        #-contributors section for full list".to_string(),
            "".to_string(),
            "🦀 This Rust Implementation:".to_string(),
            "   • Repository: github.com/teamktown/r-mcpsec/claude-token-monitor".to_string(),
            "   • License: MIT".to_string(),
            "   • Refactored for file-based monitoring approach".to_string(),
            "   • Enhanced with real-time file watching capabilities".to_string(),
            "".to_string(),
            "💡 Usage Tips:".to_string(),
            "   • Use --about flag for this information in CLI".to_string(),
            "   • Use --explain-how-this-works for technical details".to_string(),
            "   • Compatible with Claude Code's JSONL output files".to_string(),
        ];

        let attribution_items: Vec<ListItem> = attribution_info
            .iter()
            .map(|s| ListItem::new(Line::from(s.as_str())))
            .collect();

        let attribution_list = List::new(attribution_items)
            .block(
                Block::default()
                    .title("Attribution & Contributors")
                    .borders(Borders::ALL),
            )
            .style(Style::default().fg(Color::Cyan));

        frame.render_widget(attribution_list, chunks[1]);
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
                    if session.is_active { "ACTIVE" } else { "INACTIVE" },
                    status_style,
                ),
            ]),
            Line::from(vec![
                Span::raw("Session ID: "),
                Span::styled(&session.id[..8], Style::default().fg(Color::Yellow)),
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
                    .title("Session Information")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Blue)),
            )
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
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
    fn draw_token_usage_chart(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
        let session = &metrics.current_session;
        let used = session.tokens_used as u64;
        let remaining = session.tokens_limit.saturating_sub(session.tokens_used) as u64;

        let data = vec![
            ("Used", used),
            ("Remaining", remaining),
        ];

        let barchart = BarChart::default()
            .block(
                Block::default()
                    .title("Token Usage Distribution")
                    .borders(Borders::ALL),
            )
            .data(&data)
            .bar_width(4)
            .bar_style(Style::default().fg(Color::Green))
            .value_style(Style::default().fg(Color::Black).bg(Color::Green));

        frame.render_widget(barchart, area);
    }

    /// Draw usage history chart
    fn draw_usage_history_chart(frame: &mut Frame, area: Rect, metrics: &UsageMetrics) {
        // Create sample historical data for demonstration
        let history_data = vec![
            ("1h ago", metrics.current_session.tokens_used.saturating_sub(200) as u64),
            ("45m ago", metrics.current_session.tokens_used.saturating_sub(150) as u64),
            ("30m ago", metrics.current_session.tokens_used.saturating_sub(100) as u64),
            ("15m ago", metrics.current_session.tokens_used.saturating_sub(50) as u64),
            ("Now", metrics.current_session.tokens_used as u64),
        ];

        let barchart = BarChart::default()
            .block(
                Block::default()
                    .title("Token Usage History")
                    .borders(Borders::ALL),
            )
            .data(&history_data)
            .bar_width(3)
            .bar_style(Style::default().fg(Color::Cyan))
            .value_style(Style::default().fg(Color::Black).bg(Color::Cyan));

        frame.render_widget(barchart, area);
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
        let controls = Paragraph::new("Controls: [Q]uit | [Tab] Switch tabs | [↑↓] Scroll | [R]efresh")
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