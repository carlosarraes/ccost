// Real-time dashboard for watch mode using ratatui
use anyhow::Result;
use chrono::{DateTime, Utc};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span, Text},
    widgets::{
        block::Title, BarChart, Block, Borders, Clear, Gauge, List, ListItem, Paragraph, Sparkline,
        Tabs, Widget,
    },
    Frame, Terminal,
};
use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};

use crate::watch::events::{WatchEvent, EfficiencyLevel};
use crate::watch::session::{SessionTracker, SessionStatistics};

#[derive(Debug, Clone)]
pub struct DashboardState {
    pub events: VecDeque<WatchEvent>,
    pub session_tracker: SessionTracker,
    pub start_time: Instant,
    pub last_update: Instant,
    pub paused: bool,
    pub show_help: bool,
    pub message_costs: VecDeque<f64>,
    pub message_tokens: VecDeque<u64>,
    pub refresh_rate: Duration,
    pub expensive_threshold: f64,
    pub max_events: usize,
    pub max_history: usize,
}

impl Default for DashboardState {
    fn default() -> Self {
        DashboardState {
            events: VecDeque::new(),
            session_tracker: SessionTracker::new(30, 0.10), // 30 min idle, $0.10 expensive threshold
            start_time: Instant::now(),
            last_update: Instant::now(),
            paused: false,
            show_help: false,
            message_costs: VecDeque::new(),
            message_tokens: VecDeque::new(),
            refresh_rate: Duration::from_millis(200),
            expensive_threshold: 0.10,
            max_events: 50, // Reduced to prevent log overflow
            max_history: 60, // Keep 60 data points for sparklines
        }
    }
}

impl DashboardState {
    pub fn new(expensive_threshold: f64, refresh_rate_ms: u64) -> Self {
        DashboardState {
            session_tracker: SessionTracker::new(30, expensive_threshold),
            refresh_rate: Duration::from_millis(refresh_rate_ms),
            expensive_threshold,
            ..Default::default()
        }
    }

    pub fn add_event(&mut self, event: WatchEvent) {
        // Update session tracker and track individual message costs
        if let WatchEvent::NewMessage { tokens, cost, model, project, .. } = &event {
            self.session_tracker.update_activity(project, *tokens, *cost, model);
            
            // Track individual message costs for sparklines (not cumulative)
            self.message_costs.push_back(*cost);
            self.message_tokens.push_back(*tokens as u64);
            
            if self.message_costs.len() > self.max_history {
                self.message_costs.pop_front();
            }
            if self.message_tokens.len() > self.max_history {
                self.message_tokens.pop_front();
            }
        }

        // Add to events list (keep only recent events)
        self.events.push_back(event);
        if self.events.len() > self.max_events {
            self.events.pop_front();
        }

        self.last_update = Instant::now();
    }

    pub fn toggle_pause(&mut self) {
        self.paused = !self.paused;
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }


    pub fn reset(&mut self) {
        self.events.clear();
        self.session_tracker.reset_sessions();
        self.message_costs.clear();
        self.message_tokens.clear();
        self.start_time = Instant::now();
        self.last_update = Instant::now();
    }

    pub fn uptime(&self) -> Duration {
        self.start_time.elapsed()
    }

    pub fn time_since_last_update(&self) -> Duration {
        self.last_update.elapsed()
    }
}

pub struct Dashboard {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    state: DashboardState,
}

impl Dashboard {
    pub fn new(expensive_threshold: f64, refresh_rate_ms: u64) -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Dashboard {
            terminal,
            state: DashboardState::new(expensive_threshold, refresh_rate_ms),
        })
    }

    pub fn run(&mut self) -> Result<()> {
        loop {
            if !self.state.paused {
                self.draw()?;
            }

            if crossterm::event::poll(self.state.refresh_rate)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Char('p') | KeyCode::Char(' ') => self.state.toggle_pause(),
                            KeyCode::Char('h') | KeyCode::F(1) => self.state.toggle_help(),
                            KeyCode::Char('r') => self.state.reset(),
                            _ => {}
                        }
                    }
                }
            }

            // Check for idle sessions
            let _ended_sessions = self.state.session_tracker.check_idle_sessions();
        }

        Ok(())
    }

    pub async fn run_with_events(&mut self, event_receiver: tokio::sync::mpsc::UnboundedReceiver<WatchEvent>) -> Result<()> {
        let (reset_sender, _reset_receiver) = tokio::sync::mpsc::unbounded_channel();
        self.run_with_events_and_reset(event_receiver, reset_sender).await
    }

    pub async fn run_with_events_and_reset(&mut self, mut event_receiver: tokio::sync::mpsc::UnboundedReceiver<WatchEvent>, reset_sender: tokio::sync::mpsc::UnboundedSender<()>) -> Result<()> {
        use tokio::time::{interval, Duration};
        
        let mut refresh_timer = interval(Duration::from_millis(self.state.refresh_rate.as_millis() as u64));
        let mut last_terminal_size = self.terminal.size()?;
        
        loop {
            tokio::select! {
                // Handle incoming watch events
                Some(watch_event) = event_receiver.recv() => {
                    self.state.add_event(watch_event);
                }
                
                // Handle keyboard input and drawing
                _ = refresh_timer.tick() => {
                    // Check if terminal size changed and force redraw if needed
                    let current_size = self.terminal.size()?;
                    let size_changed = current_size != last_terminal_size;
                    if size_changed {
                        last_terminal_size = current_size;
                        // Force a full redraw on terminal resize
                        self.terminal.clear()?;
                    }
                    
                    if !self.state.paused || size_changed {
                        self.draw()?;
                    }

                    // Check for keyboard input with a short timeout
                    if crossterm::event::poll(Duration::from_millis(10))? {
                        if let Event::Key(key) = event::read()? {
                            if key.kind == KeyEventKind::Press {
                                match key.code {
                                    KeyCode::Char('q') | KeyCode::Esc => break,
                                    KeyCode::Char('c') | KeyCode::Char('d') if key.modifiers.contains(event::KeyModifiers::CONTROL) => break,
                                    KeyCode::Char('p') | KeyCode::Char(' ') => self.state.toggle_pause(),
                                    KeyCode::Char('h') | KeyCode::F(1) => self.state.toggle_help(),
                                    KeyCode::Char('r') => {
                                        self.state.reset();
                                        let _ = reset_sender.send(()); // Notify watch mode to reset file tracking
                                    },
                                    _ => {}
                                }
                            }
                        }
                    }

                    // Check for idle sessions
                    let _ended_sessions = self.state.session_tracker.check_idle_sessions();
                }
            }
        }

        Ok(())
    }

    pub fn add_event(&mut self, event: WatchEvent) {
        self.state.add_event(event);
    }

    pub fn cleanup(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        let show_help = self.state.show_help;
        self.terminal.draw(|f| {
            if show_help {
                render_help_popup_static(f);
            } else {
                render_main_layout_static(f, &self.state);
            }
        })?;
        Ok(())
    }

    fn render_main_layout(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Main content
                Constraint::Length(1), // Status line
            ])
            .split(f.size());

        self.render_header(f, chunks[0]);
        self.render_overview_content(f, chunks[1]);
        self.render_status_line(f, chunks[2]);
    }

    fn render_header(&self, f: &mut Frame, area: Rect) {
        let stats = self.state.session_tracker.get_session_statistics();
        let uptime = self.state.uptime();
        let uptime_str = format_duration(uptime);
        
        let status = if self.state.paused { " [PAUSED]" } else { "" };
        
        let header_text = vec![
            Line::from(vec![
                Span::styled("ccost Watch Mode", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(status, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("Uptime: "),
                Span::styled(uptime_str, Style::default().fg(Color::Green)),
                Span::raw(" | Active Sessions: "),
                Span::styled(stats.active_sessions.to_string(), Style::default().fg(Color::Yellow)),
                Span::raw(" | Total Cost: "),
                Span::styled(format!("${:.4}", stats.total_cost), Style::default().fg(Color::Green)),
            ]),
        ];

        let header = Paragraph::new(header_text)
            .block(Block::default().borders(Borders::ALL).title("Real-time Claude Usage Monitor"));
        f.render_widget(header, area);
    }



    fn render_overview_content(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(8), Constraint::Min(0)])
            .split(area);

        let top_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(chunks[0]);

        // Real-time metrics
        self.render_metrics_panel(f, top_chunks[0]);
        
        // Active sessions (replaces cost sparkline)
        self.render_sessions_panel(f, top_chunks[1]);

        // Recent activity
        self.render_recent_activity(f, chunks[1]);
    }

    fn render_metrics_panel(&self, f: &mut Frame, area: Rect) {
        let stats = self.state.session_tracker.get_session_statistics();
        
        let metrics_text = vec![
            Line::from(vec![
                Span::raw("Active Sessions: "),
                Span::styled(stats.active_sessions.to_string(), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("Total Messages: "),
                Span::styled(stats.total_messages.to_string(), Style::default().fg(Color::Cyan)),
            ]),
            Line::from(vec![
                Span::raw("Total Tokens: "),
                Span::styled(format!("{}", stats.total_tokens), Style::default().fg(Color::Blue)),
            ]),
            Line::from(vec![
                Span::raw("Total Cost: "),
                Span::styled(format!("${:.4}", stats.total_cost), Style::default().fg(Color::Green)),
            ]),
            Line::from(vec![
                Span::raw("Avg Cost/Session: "),
                Span::styled(format!("${:.4}", stats.average_cost_per_session), Style::default().fg(Color::Magenta)),
            ]),
        ];

        let metrics = Paragraph::new(metrics_text)
            .block(Block::default().borders(Borders::ALL).title("Live Metrics"));
        f.render_widget(metrics, area);
    }

    fn render_sessions_panel(&self, f: &mut Frame, area: Rect) {
        let active_sessions = self.state.session_tracker.get_active_sessions();
        
        if active_sessions.is_empty() {
            let placeholder = Paragraph::new("No active sessions")
                .block(Block::default().borders(Borders::ALL).title("Active Sessions"))
                .alignment(Alignment::Center);
            f.render_widget(placeholder, area);
            return;
        }

        // Take only the most recent/active sessions that fit
        let available_height = area.height.saturating_sub(2); // Account for borders
        let max_sessions = (available_height as usize / 2).max(1); // 2 lines per session
        
        let session_items: Vec<ListItem> = active_sessions
            .iter()
            .take(max_sessions)
            .map(|session| {
                let duration = format_duration(session.duration());
                let cost_color = if session.total_cost > self.state.expensive_threshold {
                    Color::Red
                } else {
                    Color::Green
                };
                
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(format!("📁 {}", session.project), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                    ]),
                    Line::from(vec![
                        Span::raw("   "),
                        Span::styled(format!("${:.4}", session.total_cost), Style::default().fg(cost_color)),
                        Span::raw(" | "),
                        Span::styled(format!("{} msg", session.message_count), Style::default().fg(Color::Blue)),
                        Span::raw(" | "),
                        Span::styled(duration, Style::default().fg(Color::Yellow)),
                    ]),
                ])
            })
            .collect();

        let sessions_list = List::new(session_items)
            .block(Block::default().borders(Borders::ALL).title("Active Sessions"));
        f.render_widget(sessions_list, area);
    }

    fn render_recent_activity(&self, f: &mut Frame, area: Rect) {
        // Calculate how many events we can display based on available height
        let available_height = area.height.saturating_sub(2); // Account for borders
        let max_events = (available_height as usize).saturating_sub(1).max(1); // At least show 1
        
        let recent_events: Vec<ListItem> = self.state.events
            .iter()
            .rev()
            .take(max_events)
            .map(|event| {
                let (icon, color, text) = format_event_for_list(event);
                let timestamp = event.get_timestamp().format("%H:%M:%S");
                
                // Truncate long messages to fit in the available width
                let available_width = area.width.saturating_sub(20); // Account for timestamp and icon
                let truncated_text = if text.len() > available_width as usize {
                    format!("{}...", &text[..available_width.saturating_sub(3) as usize])
                } else {
                    text
                };
                
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", icon), Style::default().fg(color)),
                    Span::styled(format!("[{}] ", timestamp), Style::default().fg(Color::Gray)),
                    Span::raw(truncated_text),
                ]))
            })
            .collect();

        let events_list = List::new(recent_events)
            .block(Block::default().borders(Borders::ALL).title("Recent Activity"));
        f.render_widget(events_list, area);
    }


    fn render_status_line(&self, f: &mut Frame, area: Rect) {
        let status_text = if self.state.paused {
            "[PAUSED] Press 'p' to resume | 'q' to quit | 'h' for help | 'r' to reset"
        } else {
            "Press 'p' to pause | 'q' to quit | 'h' for help | 'r' to reset"
        };

        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(status, area);
    }

    fn render_help_popup(&self, f: &mut Frame) {
        let area = centered_rect(60, 70, f.size());
        
        f.render_widget(Clear, area);
        
        let help_text = vec![
            Line::from(Span::styled("ccost Watch Mode - Help", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::raw(""),
            Line::raw("Controls:"),
            Line::raw("  p / Space    - Pause/Resume"),
            Line::raw("  r            - Reset all data"),
            Line::raw("  h / F1       - Show/Hide help"),
            Line::raw("  q / Esc      - Quit"),
            Line::raw(""),
            Line::raw("Display:"),
            Line::raw("  Live Metrics     - Real-time cost and token counts"),
            Line::raw("  Active Sessions  - Current conversation sessions"),
            Line::raw("  Recent Activity  - Latest messages and events"),
            Line::raw(""),
            Line::raw("Efficiency Symbols:"),
            Line::from(vec![
                Span::raw("  "),
                Span::styled("★", Style::default().fg(Color::Green)),
                Span::raw(" Excellent (cache hits)  "),
                Span::styled("✓", Style::default().fg(Color::Green)),
                Span::raw(" Good  "),
                Span::styled("⚠", Style::default().fg(Color::Yellow)),
                Span::raw(" Warning  "),
                Span::styled("⚡", Style::default().fg(Color::Red)),
                Span::raw(" Expensive"),
            ]),
            Line::raw(""),
            Line::raw("Press 'h' again to close this help."),
        ];

        let help = Paragraph::new(help_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Help")
                    .title_alignment(Alignment::Center),
            )
            .wrap(ratatui::widgets::Wrap { trim: true });
        f.render_widget(help, area);
    }
}

impl Drop for Dashboard {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn format_event_for_list(event: &WatchEvent) -> (&'static str, Color, String) {
    match event {
        WatchEvent::NewMessage { tokens, cost, model, project, .. } => {
            let icon = "💬";
            let color = if *cost > 0.05 { Color::Red } else if *cost > 0.01 { Color::Yellow } else { Color::Green };
            let text = format!("{}: {} tokens, ${:.4} ({})", project, tokens, cost, model);
            (icon, color, text)
        }
        WatchEvent::ModelSwitch { from, to, project, .. } => {
            ("🔄", Color::Blue, format!("{}: Model switch {} → {}", project, from, to))
        }
        WatchEvent::CacheHit { saved_tokens, saved_cost, project, .. } => {
            ("⚡", Color::Green, format!("{}: Cache hit! Saved {} tokens (${:.4})", project, saved_tokens, saved_cost))
        }
        WatchEvent::ExpensiveConversation { cost, threshold, project, .. } => {
            ("⚠️", Color::Red, format!("{}: Expensive conversation ${:.4} (>${:.2})", project, cost, threshold))
        }
        WatchEvent::SessionStart { project, .. } => {
            ("🚀", Color::Cyan, format!("{}: Session started", project))
        }
        WatchEvent::SessionEnd { project, duration, total_cost, .. } => {
            ("🏁", Color::Gray, format!("{}: Session ended after {} (${:.4})", project, format_duration(*duration), total_cost))
        }
        WatchEvent::ProjectActivity { project, message_count, cost, .. } => {
            ("📊", Color::Magenta, format!("{}: {} messages, ${:.4}", project, message_count, cost))
        }
    }
}

fn format_duration(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

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

// Static helper functions to avoid borrow checker issues
fn render_main_layout_static(f: &mut Frame, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status line
        ])
        .split(f.size());

    render_header_static(f, chunks[0], state);
    render_overview_content_static(f, chunks[1], state);
    render_status_line_static(f, chunks[2], state);
}

fn render_header_static(f: &mut Frame, area: Rect, state: &DashboardState) {
    let stats = state.session_tracker.get_session_statistics();
    let uptime = state.uptime();
    let uptime_str = format_duration(uptime);
    
    let status = if state.paused { " [PAUSED]" } else { "" };
    
    let header_text = vec![
        Line::from(vec![
            Span::styled("ccost Watch Mode", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(status, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("Uptime: "),
            Span::styled(uptime_str, Style::default().fg(Color::Green)),
            Span::raw(" | Active Sessions: "),
            Span::styled(stats.active_sessions.to_string(), Style::default().fg(Color::Yellow)),
            Span::raw(" | Total Cost: "),
            Span::styled(format!("${:.4}", stats.total_cost), Style::default().fg(Color::Green)),
        ]),
    ];

    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).title("Real-time Claude Usage Monitor"));
    f.render_widget(header, area);
}



fn render_overview_content_static(f: &mut Frame, area: Rect, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(area);

    let top_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[0]);

    // Real-time metrics
    render_metrics_panel_static(f, top_chunks[0], state);
    
    // Active sessions (replaces cost sparkline)
    render_sessions_panel_static(f, top_chunks[1], state);

    // Recent activity
    render_recent_activity_static(f, chunks[1], state);
}

fn render_metrics_panel_static(f: &mut Frame, area: Rect, state: &DashboardState) {
    let stats = state.session_tracker.get_session_statistics();
    
    let metrics_text = vec![
        Line::from(vec![
            Span::raw("Active Sessions: "),
            Span::styled(stats.active_sessions.to_string(), Style::default().fg(Color::Yellow)),
        ]),
        Line::from(vec![
            Span::raw("Total Messages: "),
            Span::styled(stats.total_messages.to_string(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::raw("Total Tokens: "),
            Span::styled(format!("{}", stats.total_tokens), Style::default().fg(Color::Blue)),
        ]),
        Line::from(vec![
            Span::raw("Total Cost: "),
            Span::styled(format!("${:.4}", stats.total_cost), Style::default().fg(Color::Green)),
        ]),
        Line::from(vec![
            Span::raw("Avg Cost/Session: "),
            Span::styled(format!("${:.4}", stats.average_cost_per_session), Style::default().fg(Color::Magenta)),
        ]),
    ];

    let metrics = Paragraph::new(metrics_text)
        .block(Block::default().borders(Borders::ALL).title("Live Metrics"));
    f.render_widget(metrics, area);
}

fn render_sessions_panel_static(f: &mut Frame, area: Rect, state: &DashboardState) {
    let active_sessions = state.session_tracker.get_active_sessions();
    
    if active_sessions.is_empty() {
        let placeholder = Paragraph::new("No active sessions")
            .block(Block::default().borders(Borders::ALL).title("Active Sessions"))
            .alignment(Alignment::Center);
        f.render_widget(placeholder, area);
        return;
    }

    // Take only the most recent/active sessions that fit
    let available_height = area.height.saturating_sub(2); // Account for borders
    let max_sessions = (available_height as usize / 2).max(1); // 2 lines per session
    
    let session_items: Vec<ListItem> = active_sessions
        .iter()
        .take(max_sessions)
        .map(|session| {
            let duration = format_duration(session.duration());
            let cost_color = if session.total_cost > state.expensive_threshold {
                Color::Red
            } else {
                Color::Green
            };
            
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(format!("📁 {}", session.project), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::raw("   "),
                    Span::styled(format!("${:.4}", session.total_cost), Style::default().fg(cost_color)),
                    Span::raw(" | "),
                    Span::styled(format!("{} msg", session.message_count), Style::default().fg(Color::Blue)),
                    Span::raw(" | "),
                    Span::styled(duration, Style::default().fg(Color::Yellow)),
                ]),
            ])
        })
        .collect();

    let sessions_list = List::new(session_items)
        .block(Block::default().borders(Borders::ALL).title("Active Sessions"));
    f.render_widget(sessions_list, area);
}

fn render_recent_activity_static(f: &mut Frame, area: Rect, state: &DashboardState) {
    // Calculate how many events we can display based on available height
    let available_height = area.height.saturating_sub(2); // Account for borders
    let max_events = (available_height as usize).saturating_sub(1).max(1); // At least show 1
    
    let recent_events: Vec<ListItem> = state.events
        .iter()
        .rev()
        .take(max_events)
        .map(|event| {
            let (icon, color, text) = format_event_for_list(event);
            let timestamp = event.get_timestamp().format("%H:%M:%S");
            
            // Truncate long messages to fit in the available width
            let available_width = area.width.saturating_sub(20); // Account for timestamp and icon
            let truncated_text = if text.len() > available_width as usize {
                format!("{}...", &text[..available_width.saturating_sub(3) as usize])
            } else {
                text
            };
            
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", icon), Style::default().fg(color)),
                Span::styled(format!("[{}] ", timestamp), Style::default().fg(Color::Gray)),
                Span::raw(truncated_text),
            ]))
        })
        .collect();

    let events_list = List::new(recent_events)
        .block(Block::default().borders(Borders::ALL).title("Recent Activity"));
    f.render_widget(events_list, area);
}



fn render_status_line_static(f: &mut Frame, area: Rect, state: &DashboardState) {
    let status_text = if state.paused {
        "[PAUSED] Press 'p' to resume | 'q' to quit | 'h' for help | 'r' to reset"
    } else {
        "Press 'p' to pause | 'q' to quit | 'h' for help | 'r' to reset"
    };

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    f.render_widget(status, area);
}

fn render_help_popup_static(f: &mut Frame) {
    let area = centered_rect(60, 70, f.size());
    
    f.render_widget(Clear, area);
    
    let help_text = vec![
        Line::from(Span::styled("ccost Watch Mode - Help", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::raw(""),
        Line::raw("Controls:"),
        Line::raw("  p / Space    - Pause/Resume"),
        Line::raw("  r            - Reset all data"),
        Line::raw("  h / F1       - Show/Hide help"),
        Line::raw("  q / Esc      - Quit"),
        Line::raw(""),
        Line::raw("Display:"),
        Line::raw("  Live Metrics     - Real-time cost and token counts"),
        Line::raw("  Active Sessions  - Current conversation sessions"),
        Line::raw("  Recent Activity  - Latest messages and events"),
        Line::raw(""),
        Line::raw("Efficiency Symbols:"),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("★", Style::default().fg(Color::Green)),
            Span::raw(" Excellent (cache hits)  "),
            Span::styled("✓", Style::default().fg(Color::Green)),
            Span::raw(" Good  "),
            Span::styled("⚠", Style::default().fg(Color::Yellow)),
            Span::raw(" Warning  "),
            Span::styled("⚡", Style::default().fg(Color::Red)),
            Span::raw(" Expensive"),
        ]),
        Line::raw(""),
        Line::raw("Press 'h' again to close this help."),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Help")
                .title_alignment(Alignment::Center),
        )
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(help, area);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_state_creation() {
        let state = DashboardState::new(0.20, 500);
        assert_eq!(state.expensive_threshold, 0.20);
        assert_eq!(state.refresh_rate, Duration::from_millis(500));
        assert!(!state.paused);
    }

    #[test]
    fn test_dashboard_state_events() {
        let mut state = DashboardState::default();
        let event = WatchEvent::NewMessage {
            tokens: 1000,
            cost: 0.05,
            model: "claude-3-sonnet".to_string(),
            project: "test-project".to_string(),
            timestamp: Utc::now(),
        };

        state.add_event(event);
        assert_eq!(state.events.len(), 1);
        assert_eq!(state.message_costs.len(), 1);
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30s");
        assert_eq!(format_duration(Duration::from_secs(90)), "1m 30s");
        assert_eq!(format_duration(Duration::from_secs(3665)), "1h 1m 5s");
    }
}