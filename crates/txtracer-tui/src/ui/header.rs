use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::Span,
    widgets::{Block, Paragraph},
};

use crate::app::App;
use crate::widgets::badge::Badge;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let bg_color = Color::Rgb(30, 30, 30);
    f.render_widget(Block::default().style(Style::default().bg(bg_color)), area);

    // Minimal header: Title - Badge - Exit Code
    // Layout:
    // [ Retracer (Bold) ] [ Testnet (Blue) ] [ Exit code: 37 (Red) ]

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Length(10), // Retracer
                Constraint::Length(10), // Badge
                Constraint::Min(0),     // Spacer
                Constraint::Length(15), // Exit code
            ]
            .as_ref(),
        )
        .split(area);

    // Title
    let title = Span::styled(
        "Retracer",
        Style::default()
            .add_modifier(Modifier::BOLD)
            .fg(Color::White),
    );
    f.render_widget(Paragraph::new(title), chunks[0]);

    // Network Badge
    let badge = Badge::new("Testnet").bg(Color::Blue).fg(Color::White);
    f.render_widget(badge, chunks[1]);

    // Exit Code
    let exit_code_val = app.transaction.as_ref().map(|tx| tx.exit_code).unwrap_or(0);
    let exit_color = if exit_code_val != 0 {
        Color::Red
    } else {
        Color::Green
    };
    let exit_code = Span::styled(
        format!("Exit code: {}", exit_code_val),
        Style::default().fg(exit_color).add_modifier(Modifier::BOLD),
    );
    f.render_widget(
        Paragraph::new(exit_code).alignment(ratatui::layout::Alignment::Right),
        chunks[3],
    );
}
