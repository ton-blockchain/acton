use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, Paragraph},
};

use crate::app::App;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    if !app.details_expanded {
        // Collapsed view
        let block = Block::default()
            .title(" Transaction Details (Press Enter to expand) ")
            .borders(Borders::TOP)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(100, 100, 100)))
            .style(Style::default().bg(Color::Rgb(30, 30, 30)));
        f.render_widget(block, area);
        return;
    }

    let block = Block::default()
        .title(" Transaction Details ")
        .borders(Borders::TOP)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(100, 100, 100)))
        .style(Style::default().bg(Color::Rgb(25, 25, 25)));
    f.render_widget(block.clone(), area);

    let inner = block.inner(area);

    // Add horizontal padding
    let inner = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner)[1];

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1), // Status
                Constraint::Length(1), // Account
                Constraint::Length(1), // Sender
                Constraint::Length(1), // LT
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Compute Phase
            ]
            .as_ref(),
        )
        .split(inner);

    let tx = if let Some(tx) = &app.transaction {
        tx
    } else {
        f.render_widget(Paragraph::new("No transaction data"), inner);
        return;
    };

    // Helper to render label-value pairs
    let draw_row = |f: &mut Frame, row_area: Rect, label: &str, value: &str, value_color: Color| {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(20), Constraint::Min(0)].as_ref())
            .split(row_area);

        f.render_widget(
            Paragraph::new(label).style(Style::default().fg(Color::Gray)),
            chunks[0],
        );
        f.render_widget(
            Paragraph::new(value).style(Style::default().fg(value_color)),
            chunks[1],
        );
    };

    draw_row(f, rows[0], "Status", &tx.status, Color::Green);
    draw_row(f, rows[1], "Account", &tx.account, Color::Cyan);
    draw_row(f, rows[2], "Sender", &tx.sender, Color::Cyan);
    draw_row(f, rows[3], "LT", &tx.lt.to_string(), Color::White);

    draw_row(
        f,
        rows[5],
        "Compute Phase",
        &format!(
            "Exit {}  Steps: {}  Gas: {}",
            tx.exit_code, tx.vm_steps, tx.gas_used_total
        ),
        Color::White,
    );
}
