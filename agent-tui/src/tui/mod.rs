pub mod components;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame, Terminal,
};
use std::{
    io,
    time::{Duration, Instant},
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::{
    config::Config,
    llm::LlmClient,
    types::{AppEvent, Id, Message, MessageRole, Session, SessionMode, AgentState},
};

use self::components::{Chat, Input, Sidebar};

/// Main application state
pub struct App {
    /// Application configuration
    config: Config,
    /// Current session
    session: Session,
    /// Chat component
    chat: Chat,
    /// Input component
    input: Input,
    /// Sidebar component
    sidebar: Sidebar,
    /// LLM client
    llm_client: Option<LlmClient>,
    /// Whether the app should quit
    should_quit: bool,
    /// Show sidebar
    show_sidebar: bool,
    /// Current mode
    mode: AppMode,
    /// Event receiver
    event_rx: mpsc::Receiver<AppEvent>,
    /// Event sender
    event_tx: mpsc::Sender<AppEvent>,
    /// Last tick time
    last_tick: Instant,
    /// Tick rate
    tick_rate: Duration,
}

/// Application modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppMode {
    /// Normal mode - chat and input
    Normal,
    /// Command mode - entering a slash command
    Command,
    /// Agent selection mode
    AgentSelect,
    /// Memory manager mode
    MemoryManager,
    /// Confirmation dialog
    Confirm,
}

impl App {
    /// Create a new application instance
    pub fn new(config: Config) -> Result<Self> {
        let (event_tx, event_rx) = mpsc::channel(100);
        
        let session = Session::new("New Session");
        
        // Initialize LLM client if API key is available
        let llm_client = if config.llm.api_key.starts_with("$") {
            // Try to get from environment variable
            let env_var = &config.llm.api_key[1..];
            match std::env::var(env_var) {
                Ok(api_key) => {
                    info!("Initializing LLM client with API key from environment");
                    Some(LlmClient::new(
                        &api_key,
                        &config.llm.model,
                        config.llm.max_tokens,
                        config.llm.temperature,
                    ))
                }
                Err(_) => {
                    warn!("LLM API key environment variable '{}' not set", env_var);
                    None
                }
            }
        } else {
            info!("Initializing LLM client with configured API key");
            Some(LlmClient::new(
                &config.llm.api_key,
                &config.llm.model,
                config.llm.max_tokens,
                config.llm.temperature,
            ))
        };
        
        Ok(Self {
            config: config.clone(),
            session: session.clone(),
            chat: Chat::new(),
            input: Input::new(),
            sidebar: Sidebar::new(),
            llm_client,
            should_quit: false,
            show_sidebar: true,
            mode: AppMode::Normal,
            event_rx,
            event_tx,
            last_tick: Instant::now(),
            tick_rate: Duration::from_millis(250),
        })
    }

    /// Run the main application loop
    pub async fn run(&mut self) -> Result<()> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Main loop
        let result = self.run_loop(&mut terminal).await;

        // Restore terminal
        disable_raw_mode()?;
        execute!(
            terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        terminal.show_cursor()?;

        result
    }

    /// Main application loop
    async fn run_loop<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        let mut last_tick = Instant::now();

        while !self.should_quit {
            // Draw UI
            terminal.draw(|f| self.draw(f))?;

            // Handle timing
            let timeout = self.tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            // Handle events
            if crossterm::event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key_event(key.code).await?;
                    }
                }
            }

            // Handle tick
            if last_tick.elapsed() >= self.tick_rate {
                self.on_tick().await?;
                last_tick = Instant::now();
            }

            // Handle app events
            while let Ok(event) = self.event_rx.try_recv() {
                self.handle_app_event(event).await?;
            }
        }

        Ok(())
    }

    /// Handle key events
    async fn handle_key_event(&mut self, key: KeyCode) -> Result<()> {
        match self.mode {
            AppMode::Normal => match key {
                KeyCode::Char('c') if self.input.is_ctrl_pressed() => {
                    self.should_quit = true;
                }
                KeyCode::Char('b') if self.input.is_ctrl_pressed() => {
                    self.show_sidebar = !self.show_sidebar;
                }
                KeyCode::Char('m') if self.input.is_ctrl_pressed() => {
                    self.mode = AppMode::MemoryManager;
                }
                KeyCode::Char('a') if self.input.is_ctrl_pressed() => {
                    self.mode = AppMode::AgentSelect;
                }
                KeyCode::Enter => {
                    self.submit_input().await?;
                }
                KeyCode::Up => {
                    self.input.previous_history();
                }
                KeyCode::Down => {
                    self.input.next_history();
                }
                KeyCode::Tab => {
                    self.input.autocomplete();
                }
                KeyCode::Char('/') => {
                    self.input.insert_char('/');
                    self.mode = AppMode::Command;
                }
                KeyCode::Char(c) => {
                    self.input.insert_char(c);
                }
                KeyCode::Backspace => {
                    self.input.delete_char();
                    if self.input.is_empty() {
                        self.mode = AppMode::Normal;
                    }
                }
                KeyCode::Left => {
                    self.input.move_cursor_left();
                }
                KeyCode::Right => {
                    self.input.move_cursor_right();
                }
                _ => {}
            },
            AppMode::Command => match key {
                KeyCode::Enter => {
                    self.execute_command().await?;
                    self.mode = AppMode::Normal;
                }
                KeyCode::Esc => {
                    self.mode = AppMode::Normal;
                    self.input.clear();
                }
                KeyCode::Char(c) => {
                    self.input.insert_char(c);
                }
                KeyCode::Backspace => {
                    self.input.delete_char();
                    if self.input.is_empty() {
                        self.mode = AppMode::Normal;
                    }
                }
                _ => {}
            },
            AppMode::AgentSelect => match key {
                KeyCode::Esc => {
                    self.mode = AppMode::Normal;
                }
                KeyCode::Char(c) if c.is_ascii_digit() => {
                    let idx = c.to_digit(10).unwrap() as usize;
                    // TODO: Select agent by index
                    self.mode = AppMode::Normal;
                }
                _ => {}
            },
            AppMode::MemoryManager => match key {
                KeyCode::Esc | KeyCode::Char('q') => {
                    self.mode = AppMode::Normal;
                }
                _ => {}
            },
            AppMode::Confirm => match key {
                KeyCode::Char('y') | KeyCode::Enter => {
                    // TODO: Confirm action
                    self.mode = AppMode::Normal;
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    // TODO: Cancel action
                    self.mode = AppMode::Normal;
                }
                _ => {}
            },
        }
        Ok(())
    }

    /// Handle application events
    async fn handle_app_event(&mut self, event: AppEvent) -> Result<()> {
        match event {
            AppEvent::MessageReceived(msg) => {
                self.session.add_message(msg.clone());
                self.chat.add_message(msg);
            }
            AppEvent::AgentStateChanged(agent_id, state) => {
                debug!("Agent {} state changed to {:?}", agent_id, state);
                // TODO: Update sidebar
            }
            AppEvent::TaskStatusChanged(task_id, status) => {
                debug!("Task {} status changed to {:?}", task_id, status);
                // TODO: Update task tracking
            }
            AppEvent::RoutingDecision(decision) => {
                info!(
                    "Routing decision: {:?} with confidence {}",
                    decision.selected_agents, decision.confidence
                );
                // TODO: Handle routing decision
            }
            AppEvent::Error(msg) => {
                error!("Application error: {}", msg);
                self.chat.add_message(Message::system(&format!("Error: {}", msg)));
            }
            AppEvent::Status(msg) => {
                info!("Status: {}", msg);
            }
        }
        Ok(())
    }

    /// Handle tick event
    async fn on_tick(&mut self) -> Result<()> {
        // TODO: Periodic tasks (auto-save, health checks, etc.)
        Ok(())
    }

    /// Submit the current input
    async fn submit_input(&mut self) -> Result<()> {
        let content = self.input.get_content();
        if content.trim().is_empty() {
            return Ok(());
        }

        // Add user message
        let msg = Message::user(&content);
        self.session.add_message(msg.clone());
        self.chat.add_message(msg);

        // Clear input
        self.input.clear();

        // Check if we have an LLM client
        if let Some(client) = &self.llm_client {
            // Create system message for context
            let system_msg = Message::system("You are a helpful AI assistant.");
            
            // Build message history (last 10 messages for context)
            let history: Vec<Message> = self
                .session
                .messages
                .iter()
                .rev()
                .take(10)
                .rev()
                .cloned()
                .collect();
            
            // Add system message at the beginning
            let mut messages = vec![system_msg];
            messages.extend(history);

            // Send to LLM
            match client.send_message(&messages).await {
                Ok(response) => {
                    let response_msg = Message::agent(&response, "assistant");
                    self.session.add_message(response_msg.clone());
                    self.chat.add_message(response_msg);
                }
                Err(e) => {
                    let error_msg = Message::system(&format!("Error from LLM: {}", e));
                    self.session.add_message(error_msg.clone());
                    self.chat.add_message(error_msg);
                }
            }
        } else {
            let response = Message::system(
                "No LLM client configured. Please set OPENAI_API_KEY environment variable or configure api_key in ~/.config/agent-tui/config.toml"
            );
            self.session.add_message(response.clone());
            self.chat.add_message(response);
        }

        Ok(())
    }

    /// Execute a slash command
    async fn execute_command(&mut self) -> Result<()> {
        let content = self.input.get_content();
        let parts: Vec<&str> = content.split_whitespace().collect();
        
        if parts.is_empty() {
            return Ok(());
        }

        let command = parts[0];
        let args = &parts[1..];

        match command {
            "/help" => {
                let help_text = r#"Available commands:
/help - Show this help message
/mode auto - Enable automatic agent routing
/mode manual - Enable manual agent selection
/agent <name> - Select specific agent (manual mode)
/clear - Clear current session
/new - Start new session
/sessions - List saved sessions
/memory - Open memory manager
/quit - Exit application"#;
                let msg = Message::system(help_text);
                self.session.add_message(msg.clone());
                self.chat.add_message(msg);
            }
            "/mode" => {
                if let Some(mode) = args.first() {
                    match *mode {
                        "auto" => {
                            self.session.set_mode(SessionMode::Auto);
                            let msg = Message::system("Switched to AUTO mode. Agents will be selected automatically.");
                            self.session.add_message(msg.clone());
                            self.chat.add_message(msg);
                        }
                        "manual" => {
                            self.session.set_mode(SessionMode::Manual);
                            let msg = Message::system("Switched to MANUAL mode. Use /agent <name> to select an agent.");
                            self.session.add_message(msg.clone());
                            self.chat.add_message(msg);
                        }
                        _ => {
                            let msg = Message::system(&format!("Unknown mode: {}. Use 'auto' or 'manual'.", mode));
                            self.session.add_message(msg.clone());
                            self.chat.add_message(msg);
                        }
                    }
                }
            }
            "/clear" => {
                self.session.messages.clear();
                self.chat.clear();
                let msg = Message::system("Session cleared.");
                self.session.add_message(msg.clone());
                self.chat.add_message(msg);
            }
            "/new" => {
                self.session = Session::new("New Session");
                self.chat.clear();
                let msg = Message::system("Started new session.");
                self.session.add_message(msg.clone());
                self.chat.add_message(msg);
            }
            "/quit" | "/exit" => {
                self.should_quit = true;
            }
            _ => {
                let msg = Message::system(&format!("Unknown command: {}. Type /help for available commands.", command));
                self.session.add_message(msg.clone());
                self.chat.add_message(msg);
            }
        }

        self.input.clear();
        Ok(())
    }

    /// Draw the UI
    fn draw(&mut self, frame: &mut Frame) {
        let main_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(if self.show_sidebar {
                vec![Constraint::Percentage(20), Constraint::Percentage(80)]
            } else {
                vec![Constraint::Percentage(0), Constraint::Percentage(100)]
            })
            .split(frame.area());

        // Sidebar
        if self.show_sidebar {
            self.sidebar.draw(frame, main_layout[0], &self.session);
        }

        // Main content area
        let content_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(main_layout[1]);

        // Chat area
        self.chat.draw(frame, content_layout[0], &self.session);

        // Input area
        self.input.draw(frame, content_layout[1], self.mode);

        // Draw overlays based on mode
        match self.mode {
            AppMode::AgentSelect => {
                self.draw_agent_selector(frame);
            }
            AppMode::MemoryManager => {
                self.draw_memory_manager(frame);
            }
            _ => {}
        }

        // Status bar
        self.draw_status_bar(frame);
    }

    /// Draw agent selector popup
    fn draw_agent_selector(&self, frame: &mut Frame) {
        let area = Self::centered_rect(60, 60, frame.area());
        
        let block = Block::default()
            .title("Select Agent")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let text = vec![
            Line::from("Available agents:"),
            Line::from(""),
            Line::from("1. planner - Plans and orchestrates workflows"),
            Line::from("2. coder - Writes and modifies code"),
            Line::from("3. reviewer - Reviews code for quality"),
            Line::from("4. tester - Generates and runs tests"),
            Line::from("5. explorer - Explores codebase structure"),
            Line::from("6. integrator - Synthesizes results"),
            Line::from(""),
            Line::from("Press number to select, ESC to cancel"),
        ];

        let paragraph = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: true });

        frame.render_widget(Clear, area);
        frame.render_widget(paragraph, area);
    }

    /// Draw memory manager popup
    fn draw_memory_manager(&self, frame: &mut Frame) {
        let area = Self::centered_rect(80, 80, frame.area());
        
        let block = Block::default()
            .title("Memory Manager")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let text = vec![
            Line::from("Memory management not yet implemented."),
            Line::from(""),
            Line::from("Press ESC or 'q' to close"),
        ];

        let paragraph = Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: true });

        frame.render_widget(Clear, area);
        frame.render_widget(paragraph, area);
    }

    /// Draw status bar
    fn draw_status_bar(&self, frame: &mut Frame) {
        let status_area = Rect {
            x: frame.area().x,
            y: frame.area().height - 1,
            width: frame.area().width,
            height: 1,
        };

        let mode_text = match self.session.mode {
            SessionMode::Auto => "AUTO",
            SessionMode::Manual => "MANUAL",
        };

        let status = format!(
            " [{}] | Messages: {} | Press Ctrl+C to quit | Ctrl+B: toggle sidebar | Ctrl+H: help ",
            mode_text,
            self.session.messages.len()
        );

        let status_bar = Paragraph::new(status)
            .style(Style::default().bg(Color::Blue).fg(Color::White));

        frame.render_widget(status_bar, status_area);
    }

    /// Calculate centered rectangle
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
}
