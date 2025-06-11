// Simplified watch mode for immediate functionality
use anyhow::{Result, Context};
use chrono::Utc;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Frame, Terminal,
};
use std::collections::VecDeque;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::time::interval;

#[derive(Debug, Clone)]
pub struct SimpleEvent {
    pub timestamp: Instant,
    pub message: String,
    pub cost: f64,
    pub project: String,
}

pub struct SimpleWatchMode {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    events: VecDeque<SimpleEvent>,
    projects_dir: PathBuf,
    project_filter: Option<String>,
    start_time: Instant,
    total_cost: f64,
    paused: bool,
    show_help: bool,
}

impl SimpleWatchMode {
    pub fn new(project_filter: Option<String>) -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let projects_dir = dirs::home_dir()
            .context("Could not find home directory")?
            .join(".claude")
            .join("projects");

        Ok(SimpleWatchMode {
            terminal,
            events: VecDeque::new(),
            projects_dir,
            project_filter,
            start_time: Instant::now(),
            total_cost: 0.0,
            paused: false,
            show_help: false,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        self.add_demo_events(); // Add some demo events to show the interface
        
        let mut tick_interval = interval(Duration::from_millis(200));

        loop {
            tokio::select! {
                _ = tick_interval.tick() => {
                    if !self.paused {
                        self.draw()?;
                        
                        // Simulate some live activity every few seconds
                        if self.start_time.elapsed().as_secs() % 5 == 0 && self.events.len() < 50 {
                            self.simulate_activity();
                        }
                    }
                }
                
                _ = self.handle_input() => {
                    // Input handled
                }
            }

            if crossterm::event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Char('p') | KeyCode::Char(' ') => {
                                self.paused = !self.paused;
                            }
                            KeyCode::Char('h') | KeyCode::F(1) => {
                                self.show_help = !self.show_help;
                            }
                            KeyCode::Char('r') => {
                                self.reset();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        self.cleanup()?;
        Ok(())
    }

    fn add_demo_events(&mut self) {
        let demo_events = vec![
            ("Welcome to ccost watch mode!", 0.0, "system"),
            ("Monitoring Claude usage in real-time...", 0.0, "system"),
            ("Press 'h' for help, 'q' to quit", 0.0, "system"),
            ("New message processed", 0.045, "my-project"),
            ("Cache hit detected", -0.012, "another-project"),
            ("Model switch: Sonnet → Opus", 0.0, "expensive-project"),
            ("Conversation cost: $0.123", 0.123, "expensive-project"),
        ];

        for (msg, cost, project) in demo_events {
            self.events.push_back(SimpleEvent {
                timestamp: Instant::now(),
                message: msg.to_string(),
                cost,
                project: project.to_string(),
            });
            self.total_cost += cost.max(0.0);
        }
    }

    fn simulate_activity(&mut self) {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        
        let messages = vec![
            "New conversation started",
            "Message processed with cache hit",
            "Long conversation detected",
            "Model efficiency warning",
            "Cost threshold exceeded",
        ];
        
        let projects = vec!["demo-project", "test-app", "analysis-work", "research"];
        
        let message = messages[rng.gen_range(0..messages.len())];
        let project = projects[rng.gen_range(0..projects.len())];
        let cost = rng.gen_range(0.001..0.200);
        
        self.events.push_back(SimpleEvent {
            timestamp: Instant::now(),
            message: message.to_string(),
            cost,
            project: project.to_string(),
        });
        
        self.total_cost += cost;
        
        // Keep only recent events
        if self.events.len() > 100 {
            self.events.pop_front();
        }
    }

    fn reset(&mut self) {
        self.events.clear();
        self.total_cost = 0.0;
        self.start_time = Instant::now();
        self.add_demo_events();
    }

    async fn handle_input(&self) -> Result<()> {
        // This is a placeholder for future file watching
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }

    fn draw(&mut self) -> Result<()> {
        let show_help = self.show_help;
        let events = self.events.clone();
        let projects_dir = self.projects_dir.clone();
        let project_filter = self.project_filter.clone();
        let start_time = self.start_time;
        let total_cost = self.total_cost;
        let paused = self.paused;
        
        self.terminal.draw(move |f| {
            if show_help {
                render_help_static(f);
            } else {
                render_main_static(f, &events, &projects_dir, &project_filter, start_time, total_cost, paused);
            }
        })?;
        Ok(())
    }

    fn render_main(&self, f: &mut Frame) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Main content
                Constraint::Length(1), // Status line
            ])
            .split(f.size());

        self.render_header(f, chunks[0]);
        self.render_events(f, chunks[1]);
        self.render_status(f, chunks[2]);
    }

    fn render_header(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let uptime = self.start_time.elapsed();
        let uptime_str = format!("{}:{:02}:{:02}", 
            uptime.as_secs() / 3600,
            (uptime.as_secs() % 3600) / 60,
            uptime.as_secs() % 60
        );
        
        let status = if self.paused { " [PAUSED]" } else { "" };
        
        let header_text = vec![
            Line::from(vec![
                Span::styled("ccost Watch Mode", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                Span::styled(status, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::raw("Uptime: "),
                Span::styled(uptime_str, Style::default().fg(Color::Green)),
                Span::raw(" | Events: "),
                Span::styled(self.events.len().to_string(), Style::default().fg(Color::Yellow)),
                Span::raw(" | Total Cost: "),
                Span::styled(format!("${:.4}", self.total_cost), Style::default().fg(Color::Green)),
            ]),
        ];

        let header = Paragraph::new(header_text)
            .block(Block::default().borders(Borders::ALL).title("Real-time Claude Usage Monitor"));
        f.render_widget(header, area);
    }

    fn render_events(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let event_items: Vec<ListItem> = self.events
            .iter()
            .rev()
            .take(50)
            .map(|event| {
                let elapsed = event.timestamp.elapsed().as_secs();
                let timestamp = if elapsed < 60 {
                    format!("{}s ago", elapsed)
                } else {
                    format!("{}m ago", elapsed / 60)
                };
                
                let (color, icon) = if event.cost > 0.1 {
                    (Color::Red, "⚠️")
                } else if event.cost > 0.0 {
                    (Color::Yellow, "💬")
                } else if event.cost < 0.0 {
                    (Color::Green, "⚡")
                } else {
                    (Color::Cyan, "ℹ️")
                };
                
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{} ", icon), Style::default().fg(color)),
                    Span::styled(format!("[{}] ", timestamp), Style::default().fg(Color::Gray)),
                    Span::styled(format!("{}: ", event.project), Style::default().fg(Color::Blue)),
                    Span::raw(&event.message),
                    if event.cost != 0.0 {
                        Span::styled(format!(" (${:.4})", event.cost.abs()), Style::default().fg(color))
                    } else {
                        Span::raw("")
                    }
                ]))
            })
            .collect();

        let events_list = List::new(event_items)
            .block(Block::default().borders(Borders::ALL).title("Live Activity Feed"));
        f.render_widget(events_list, area);
    }

    fn render_status(&self, f: &mut Frame, area: ratatui::layout::Rect) {
        let status_text = if self.paused {
            "[PAUSED] Press 'p' to resume | 'q' to quit | 'h' for help | 'r' to reset"
        } else {
            "Press 'p' to pause | 'q' to quit | 'h' for help | 'r' to reset"
        };

        let status = Paragraph::new(status_text)
            .style(Style::default().fg(Color::Gray))
            .alignment(Alignment::Center);
        f.render_widget(status, area);
    }

    fn render_help(&self, f: &mut Frame) {
        let area = centered_rect(60, 50, f.size());
        
        f.render_widget(ratatui::widgets::Clear, area);
        
        let help_text = vec![
            Line::from(Span::styled("ccost Watch Mode - Help", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::raw(""),
            Line::raw("This is a demonstration of the watch mode interface."),
            Line::raw("The full implementation will monitor real Claude usage."),
            Line::raw(""),
            Line::raw("Controls:"),
            Line::raw("  p / Space    - Pause/Resume"),
            Line::raw("  r            - Reset data"),
            Line::raw("  h / F1       - Show/Hide help"),
            Line::raw("  q / Esc      - Quit"),
            Line::raw(""),
            Line::raw("Features (in development):"),
            Line::raw("  • Real-time JSONL file monitoring"),
            Line::raw("  • Live cost and token tracking"),
            Line::raw("  • Session management"),
            Line::raw("  • Model efficiency analysis"),
            Line::raw("  • Multiple dashboard tabs"),
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

    fn cleanup(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}

impl Drop for SimpleWatchMode {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: ratatui::layout::Rect) -> ratatui::layout::Rect {
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
fn render_main_static(
    f: &mut Frame, 
    events: &VecDeque<SimpleEvent>, 
    _projects_dir: &PathBuf, 
    _project_filter: &Option<String>, 
    start_time: Instant, 
    total_cost: f64, 
    paused: bool
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content
            Constraint::Length(1), // Status line
        ])
        .split(f.size());

    render_header_static(f, chunks[0], start_time, total_cost, paused, events.len());
    render_events_static(f, chunks[1], events);
    render_status_static(f, chunks[2], paused);
}

fn render_header_static(f: &mut Frame, area: ratatui::layout::Rect, start_time: Instant, total_cost: f64, paused: bool, event_count: usize) {
    let uptime = start_time.elapsed();
    let uptime_str = format!("{}:{:02}:{:02}", 
        uptime.as_secs() / 3600,
        (uptime.as_secs() % 3600) / 60,
        uptime.as_secs() % 60
    );
    
    let status = if paused { " [PAUSED]" } else { "" };
    
    let header_text = vec![
        Line::from(vec![
            Span::styled("ccost Watch Mode", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::styled(status, Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("Uptime: "),
            Span::styled(uptime_str, Style::default().fg(Color::Green)),
            Span::raw(" | Events: "),
            Span::styled(event_count.to_string(), Style::default().fg(Color::Yellow)),
            Span::raw(" | Total Cost: "),
            Span::styled(format!("${:.4}", total_cost), Style::default().fg(Color::Green)),
        ]),
    ];

    let header = Paragraph::new(header_text)
        .block(Block::default().borders(Borders::ALL).title("Real-time Claude Usage Monitor"));
    f.render_widget(header, area);
}

fn render_events_static(f: &mut Frame, area: ratatui::layout::Rect, events: &VecDeque<SimpleEvent>) {
    let event_items: Vec<ListItem> = events
        .iter()
        .rev()
        .take(50)
        .map(|event| {
            let elapsed = event.timestamp.elapsed().as_secs();
            let timestamp = if elapsed < 60 {
                format!("{}s ago", elapsed)
            } else {
                format!("{}m ago", elapsed / 60)
            };
            
            let (color, icon) = if event.cost > 0.1 {
                (Color::Red, "⚠️")
            } else if event.cost > 0.0 {
                (Color::Yellow, "💬")
            } else if event.cost < 0.0 {
                (Color::Green, "⚡")
            } else {
                (Color::Cyan, "ℹ️")
            };
            
            ListItem::new(Line::from(vec![
                Span::styled(format!("{} ", icon), Style::default().fg(color)),
                Span::styled(format!("[{}] ", timestamp), Style::default().fg(Color::Gray)),
                Span::styled(format!("{}: ", event.project), Style::default().fg(Color::Blue)),
                Span::raw(&event.message),
                if event.cost != 0.0 {
                    Span::styled(format!(" (${:.4})", event.cost.abs()), Style::default().fg(color))
                } else {
                    Span::raw("")
                }
            ]))
        })
        .collect();

    let events_list = List::new(event_items)
        .block(Block::default().borders(Borders::ALL).title("Live Activity Feed"));
    f.render_widget(events_list, area);
}

fn render_status_static(f: &mut Frame, area: ratatui::layout::Rect, paused: bool) {
    let status_text = if paused {
        "[PAUSED] Press 'p' to resume | 'q' to quit | 'h' for help | 'r' to reset"
    } else {
        "Press 'p' to pause | 'q' to quit | 'h' for help | 'r' to reset"
    };

    let status = Paragraph::new(status_text)
        .style(Style::default().fg(Color::Gray))
        .alignment(Alignment::Center);
    f.render_widget(status, area);
}

fn render_help_static(f: &mut Frame) {
    let area = centered_rect(60, 50, f.size());
    
    f.render_widget(ratatui::widgets::Clear, area);
    
    let help_text = vec![
        Line::from(Span::styled("ccost Watch Mode - Help", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
        Line::raw(""),
        Line::raw("This is a demonstration of the watch mode interface."),
        Line::raw("The full implementation will monitor real Claude usage."),
        Line::raw(""),
        Line::raw("Controls:"),
        Line::raw("  p / Space    - Pause/Resume"),
        Line::raw("  r            - Reset data"),
        Line::raw("  h / F1       - Show/Hide help"),
        Line::raw("  q / Esc      - Quit"),
        Line::raw(""),
        Line::raw("Features (in development):"),
        Line::raw("  • Real-time JSONL file monitoring"),
        Line::raw("  • Live cost and token tracking"),
        Line::raw("  • Session management"),
        Line::raw("  • Model efficiency analysis"),
        Line::raw("  • Multiple dashboard tabs"),
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