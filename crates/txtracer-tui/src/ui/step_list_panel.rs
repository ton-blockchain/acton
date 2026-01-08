use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem},
};

use crate::app::App;
use retrace::trace::TraceStep;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(100, 100, 100)))
        .title(" Steps ")
        .style(Style::default().bg(Color::Rgb(30, 30, 30)));
    f.render_widget(block.clone(), area);

    let inner_area = block.inner(area);

    // Add horizontal padding
    let inner_area = ratatui::layout::Layout::default()
        .direction(ratatui::layout::Direction::Horizontal)
        .constraints([
            ratatui::layout::Constraint::Length(1),
            ratatui::layout::Constraint::Min(0),
            ratatui::layout::Constraint::Length(1),
        ])
        .split(inner_area)[1];

    let steps: &[TraceStep] = if let Some(trace) = &app.trace {
        &trace.steps
    } else {
        &[]
    };

    let mut cumulative_gas = 0;

    let items: Vec<ListItem> = steps
        .iter()
        .enumerate()
        .map(|(i, step)| {
            let opcode_color = Color::LightBlue;

            let content = match step {
                TraceStep::Execute { instr, gas, .. } => {
                    cumulative_gas += gas;

                    // Split instruction into name and args
                    let (name, args) = if let Some((n, a)) = instr.split_once(' ') {
                        (n, format!(" {}", a))
                    } else {
                        (instr.as_str(), "".to_string())
                    };

                    let is_current = i == app.current_step;
                    let gas_text = if is_current {
                        format!("  {}  {}", gas, cumulative_gas)
                    } else {
                        "".to_string()
                    };

                    Line::from(vec![
                        Span::styled(
                            format!("{:>3}: ", i + 1),
                            Style::default().fg(Color::DarkGray),
                        ),
                        Span::styled(name.to_string(), Style::default().fg(opcode_color)),
                        Span::styled(args, Style::default().fg(Color::White)),
                        Span::styled(
                            gas_text,
                            Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                        ),
                    ])
                }
                TraceStep::Exception { errno, message, .. } => Line::from(vec![
                    Span::styled(
                        format!("{:>3}: ", i + 1),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(
                        format!("Exception {}", errno),
                        Style::default().fg(Color::Red),
                    ),
                    Span::styled(
                        format!(" ({})", message),
                        Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                    ),
                ]),
                TraceStep::FinalC5 { .. } => Line::from(vec![
                    Span::styled(
                        format!("{:>3}: ", i + 1),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled("Final C5", Style::default().fg(Color::Magenta)),
                ]),
            };

            ListItem::new(content)
        })
        .collect();

    let list = List::new(items).highlight_style(Style::default().bg(Color::Rgb(50, 50, 80)));

    let mut state = app.step_list_state.borrow_mut();
    f.render_stateful_widget(list, inner_area, &mut state);
}
