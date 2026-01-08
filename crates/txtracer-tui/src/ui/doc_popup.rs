use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::app::App;
use crate::docs;

use retrace::trace::TraceStep;

pub fn draw(f: &mut Frame, app: &App) {
    let (opcode, doc_text) = if let Some(step) = app.get_current_step() {
        match step {
            TraceStep::Execute { instr, .. } => {
                let name = instr.split_whitespace().next().unwrap_or(instr);
                (name.to_string(), docs::get_instruction_doc(name))
            }
            TraceStep::Exception { message, .. } => ("EXCEPTION".to_string(), message.clone()),
            TraceStep::FinalC5 { .. } => (
                "FINAL C5".to_string(),
                "Final state of the c5 control register".to_string(),
            ),
        }
    } else {
        (
            "N/A".to_string(),
            "No instruction data available".to_string(),
        )
    };

    let area = centered_rect(60, 20, f.area());

    let bg_color = Color::Black;
    let text_color = Color::White;

    let block = Block::default()
        .title(format!(" Instruction: {} ", opcode))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(150, 150, 150)))
        .style(Style::default().bg(bg_color));

    let content = Paragraph::new(vec![
        Line::from(""),
        Line::from(vec![Span::styled(
            doc_text,
            Style::default().fg(text_color).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Press '?' or 'Esc' to close",
            Style::default().fg(Color::Gray),
        )]),
    ])
    .alignment(Alignment::Center)
    .block(block);

    f.render_widget(Clear, area); // Clear the area below popup
    f.render_widget(content, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ]
            .as_ref(),
        )
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints(
            [
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ]
            .as_ref(),
        )
        .split(popup_layout[1])[1]
}
