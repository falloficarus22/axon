use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, Wrap},
    Frame,
};

use crate::types::{Message, MessageRole, Session};

/// Chat component for displaying messages
pub struct Chat {
    /// Scroll offset
    scroll: u16,
    /// Whether to auto-scroll to bottom
    auto_scroll: bool,
}

impl Chat {
    pub fn new() -> Self {
        Self {
            scroll: 0,
            auto_scroll: true,
        }
    }

    /// Add a message to the chat
    pub fn add_message(&mut self, message: Message) {
        // Message is stored in session, we just trigger a re-render
        if self.auto_scroll {
            self.scroll = u16::MAX;
        }
    }

    /// Clear the chat display
    pub fn clear(&mut self) {
        self.scroll = 0;
    }

    /// Scroll up
    pub fn scroll_up(&mut self, amount: u16) {
        self.auto_scroll = false;
        self.scroll = self.scroll.saturating_sub(amount);
    }

    /// Scroll down
    pub fn scroll_down(&mut self, amount: u16) {
        self.scroll = self.scroll.saturating_add(amount);
        // TODO: Check if at bottom to re-enable auto-scroll
    }

    /// Draw the chat component
    pub fn draw(&self, frame: &mut Frame, area: Rect, session: &Session) {
        let block = Block::default()
            .title(format!(" Chat - {} ", session.title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White));

        let inner_area = block.inner(area);

        // Render messages
        let mut text_lines: Vec<Line> = vec![];

        for message in &session.messages {
            let (prefix, style) = match message.role {
                MessageRole::User => (
                    "You",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ),
                MessageRole::Agent => {
                    let agent_name = message
                        .agent_id
                        .as_ref()
                        .map(|id| id.as_str())
                        .unwrap_or("Agent");
                    (
                        agent_name,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    )
                }
                MessageRole::System => (
                    "System",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            };

            // Timestamp
            let timestamp = message.timestamp.format("%H:%M:%S").to_string();
            let header = Line::from(vec![
                Span::styled(
                    format!("[{}] ", timestamp),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(format!("{}: ", prefix), style),
            ]);

            text_lines.push(header);

            // Message content
            for line in message.content.lines() {
                text_lines.push(Line::from(format!("  {}", line)));
            }

            // Empty line between messages
            text_lines.push(Line::from(""));
        }

        let paragraph = Paragraph::new(Text::from(text_lines))
            .block(block)
            .wrap(Wrap { trim: true })
            .scroll((self.scroll, 0));

        frame.render_widget(paragraph, area);

        // Scrollbar
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        frame.render_stateful_widget(
            scrollbar,
            area.inner(Margin {
                vertical: 1,
                horizontal: 0,
            }),
            &mut self.scroll.into(),
        );
    }
}

impl Default for Chat {
    fn default() -> Self {
        Self::new()
    }
}
