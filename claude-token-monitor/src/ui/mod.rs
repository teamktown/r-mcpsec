pub mod ratatui_ui;

use crate::models::*;
// use colored::*;
use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    style::{Color, Print, ResetColor, SetForegroundColor},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{self, Write};
use std::time::Duration;

pub use ratatui_ui::RatatuiTerminalUI;

/// Terminal UI for displaying token usage
pub struct TerminalUI {
    should_exit: bool,
}

impl TerminalUI {
    pub fn new(_config: UserConfig) -> Self {
        Self {
            should_exit: false,
        }
    }

    /// Initialize terminal for full-screen display
    pub fn init(&mut self) -> io::Result<()> {
        terminal::enable_raw_mode()?;
        execute!(io::stdout(), EnterAlternateScreen, Hide)?;
        Ok(())
    }

    /// Restore terminal to normal mode
    pub fn cleanup(&mut self) -> io::Result<()> {
        execute!(io::stdout(), LeaveAlternateScreen, Show)?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    /// Main display loop
    pub async fn run(&mut self, metrics: &UsageMetrics) -> io::Result<()> {
        loop {
            self.draw_screen(metrics)?;
            
            if self.handle_input().await? {
                break;
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }

    /// Handle keyboard input
    async fn handle_input(&mut self) -> io::Result<bool> {
        if event::poll(Duration::from_millis(50))? {
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
                    KeyCode::Char('r') => {
                        // Refresh display
                    }
                    _ => {}
                }
            }
        }
        Ok(false)
    }

    /// Draw the main screen
    fn draw_screen(&self, metrics: &UsageMetrics) -> io::Result<()> {
        let mut stdout = io::stdout();
        
        execute!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
        
        // Title
        self.draw_title(&mut stdout)?;
        
        // Session info
        self.draw_session_info(&mut stdout, &metrics.current_session)?;
        
        // Progress bar
        self.draw_progress_bar(&mut stdout, metrics)?;
        
        // Usage statistics
        self.draw_usage_stats(&mut stdout, metrics)?;
        
        // Predictions
        self.draw_predictions(&mut stdout, metrics)?;
        
        // Controls
        self.draw_controls(&mut stdout)?;
        
        stdout.flush()?;
        Ok(())
    }

    /// Draw title header
    fn draw_title(&self, stdout: &mut io::Stdout) -> io::Result<()> {
        execute!(
            stdout,
            SetForegroundColor(Color::Blue),
            Print("╔═══════════════════════════════════════════════════════════════════════════════╗\n"),
            Print("║                            Claude Token Monitor                               ║\n"),
            Print("║                        Rust Edition - Hive Mind Build                        ║\n"),
            Print("╚═══════════════════════════════════════════════════════════════════════════════╝\n"),
            ResetColor,
            Print("\n")
        )?;
        Ok(())
    }

    /// Draw session information
    fn draw_session_info(&self, stdout: &mut io::Stdout, session: &TokenSession) -> io::Result<()> {
        let plan_str = match &session.plan_type {
            PlanType::Pro => "Pro",
            PlanType::Max5 => "Max5",
            PlanType::Max20 => "Max20",
            PlanType::Custom(limit) => &format!("Custom({limit})"),
        };

        let status_color = if session.is_active {
            Color::Green
        } else {
            Color::Red
        };

        let status_text = if session.is_active { "ACTIVE" } else { "INACTIVE" };

        execute!(
            stdout,
            Print("Session Information:\n"),
            Print("  Plan Type: "), SetForegroundColor(Color::Cyan), Print(plan_str), ResetColor,
            Print("\n  Status: "), SetForegroundColor(status_color), Print(status_text), ResetColor,
            Print(&format!("\n  Session ID: {}\n", &session.id[..8])),
            Print(&format!("  Started: {}\n", session.start_time.format("%Y-%m-%d %H:%M:%S UTC"))),
            Print(&format!("  Resets: {}\n\n", session.reset_time.format("%Y-%m-%d %H:%M:%S UTC")))
        )?;
        Ok(())
    }

    /// Draw progress bar
    fn draw_progress_bar(&self, stdout: &mut io::Stdout, metrics: &UsageMetrics) -> io::Result<()> {
        let session = &metrics.current_session;
        let usage_percent = (session.tokens_used as f64 / session.tokens_limit as f64) * 100.0;
        let bar_width = 50;
        let filled_width = ((usage_percent / 100.0) * bar_width as f64) as usize;
        
        let bar_color = if usage_percent > 90.0 {
            Color::Red
        } else if usage_percent > 75.0 {
            Color::Yellow
        } else {
            Color::Green
        };

        execute!(
            stdout,
            Print("Token Usage Progress:\n"),
            Print("  "),
            SetForegroundColor(bar_color),
            Print("█".repeat(filled_width)),
            SetForegroundColor(Color::DarkGrey),
            Print("░".repeat(bar_width - filled_width)),
            ResetColor,
            Print(&format!(" {usage_percent:.1}%\n")),
            Print(&format!("  {} / {} tokens used\n\n", session.tokens_used, session.tokens_limit))
        )?;
        Ok(())
    }

    /// Draw usage statistics
    fn draw_usage_stats(&self, stdout: &mut io::Stdout, metrics: &UsageMetrics) -> io::Result<()> {
        execute!(
            stdout,
            Print("Usage Statistics:\n"),
            Print(&format!("  Usage Rate: {:.2} tokens/minute\n", metrics.usage_rate)),
            Print(&format!("  Session Progress: {:.1}%\n", metrics.session_progress * 100.0)),
            Print(&format!("  Efficiency Score: {:.2}\n\n", metrics.efficiency_score))
        )?;
        Ok(())
    }

    /// Draw predictions
    fn draw_predictions(&self, stdout: &mut io::Stdout, metrics: &UsageMetrics) -> io::Result<()> {
        execute!(stdout, Print("Predictions:\n"))?;
        
        if let Some(depletion_time) = &metrics.projected_depletion {
            let time_remaining = depletion_time.signed_duration_since(chrono::Utc::now());
            let hours = time_remaining.num_hours();
            let minutes = time_remaining.num_minutes() % 60;
            
            let warning_color = if hours < 1 {
                Color::Red
            } else if hours < 3 {
                Color::Yellow
            } else {
                Color::Green
            };
            
            execute!(
                stdout,
                Print("  Projected Depletion: "),
                SetForegroundColor(warning_color),
                Print(&format!("{hours}h {minutes}m")),
                ResetColor,
                Print(&format!(" ({})\n", depletion_time.format("%H:%M:%S UTC")))
            )?;
        } else {
            execute!(stdout, Print("  Projected Depletion: No active usage\n"))?;
        }
        
        execute!(stdout, Print("\n"))?;
        Ok(())
    }

    /// Draw control instructions
    fn draw_controls(&self, stdout: &mut io::Stdout) -> io::Result<()> {
        execute!(
            stdout,
            SetForegroundColor(Color::DarkGrey),
            Print("Controls: [Q]uit | [R]efresh | [Ctrl+C] Exit\n"),
            ResetColor
        )?;
        Ok(())
    }
}

/// Simple progress bar utility
pub fn create_progress_bar(current: u32, total: u32, width: usize) -> String {
    let percentage = (current as f64 / total as f64) * 100.0;
    let filled = ((percentage / 100.0) * width as f64) as usize;
    let empty = width - filled;
    
    format!("[{}{}] {:.1}%", 
        "█".repeat(filled), 
        "░".repeat(empty), 
        percentage
    )
}

/// Format time duration in human-readable format
pub fn format_duration(duration: chrono::Duration) -> String {
    let total_seconds = duration.num_seconds();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    
    if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}