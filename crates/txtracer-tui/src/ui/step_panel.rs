use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
};

use crate::app::App;
use retrace::trace::TraceStep;
use tycho_types::cell::Load;
use tycho_types::models::IntAddr;
use vmlogs::parser::{CellLike, VmStackValue};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let bg_color = Color::Rgb(30, 30, 30);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Rgb(100, 100, 100)))
        .title(" Step Details ")
        .style(Style::default().bg(bg_color));
    f.render_widget(block.clone(), area);

    let inner_area = block.inner(area);

    // Add horizontal padding
    let inner_area = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(inner_area)[1];

    let step = if let Some(step) = app.get_current_step() {
        step
    } else {
        f.render_widget(Paragraph::new("No trace data"), inner_area);
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1), // Step info header
                Constraint::Length(1), // Spacer
                Constraint::Min(0),    // Stack
            ]
            .as_ref(),
        )
        .split(inner_area);

    let cumulative_gas = app.get_cumulative_gas(app.current_step);

    // Header: Step x/y | OP ARGS | total_gas consumed
    let header_line = match step {
        TraceStep::Execute { instr, .. } => {
            let (name, args) = if let Some((n, a)) = instr.split_once(' ') {
                (n, format!(" {}", a))
            } else {
                (instr.as_str(), "".to_string())
            };

            Line::from(vec![
                Span::styled(
                    format!("Step {}/{} ", app.current_step + 1, app.total_steps()),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled("| ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    name.to_string(),
                    Style::default()
                        .fg(Color::LightBlue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(args, Style::default().fg(Color::White)),
                Span::styled(
                    format!("  {} total gas consumed", cumulative_gas),
                    Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
                ),
            ])
        }
        TraceStep::Exception { errno, message, .. } => Line::from(vec![
            Span::styled(
                format!("Step {}/{} ", app.current_step + 1, app.total_steps()),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "| EXCEPTION ",
                Style::default().bg(Color::Red).fg(Color::White),
            ),
            Span::styled(
                format!(" {}: {}", errno, message),
                Style::default().fg(Color::White),
            ),
            Span::styled(
                format!("  {} total gas consumed", cumulative_gas),
                Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
            ),
        ]),
        TraceStep::FinalC5 { cell } => Line::from(vec![
            Span::styled(
                format!("Step {}/{} ", app.current_step + 1, app.total_steps()),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                "| FINAL C5 ",
                Style::default().bg(Color::Magenta).fg(Color::White),
            ),
            Span::styled(
                format!(" {}", cell),
                Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
            ),
            Span::styled(
                format!("  {} total gas consumed", cumulative_gas),
                Style::default().fg(Color::Gray).add_modifier(Modifier::DIM),
            ),
        ]),
    };

    f.render_widget(Paragraph::new(header_line), chunks[0]);

    // Stack
    let stack_items: Vec<ListItem> = if let Some(stack_values) = step.stack() {
        stack_values
            .iter()
            .rev() // Show in reverse order
            .enumerate()
            .flat_map(|(i, val)| {
                let bg = if i % 2 == 0 {
                    Color::Rgb(40, 40, 40)
                } else {
                    Color::Rgb(35, 35, 35)
                };

                let lines = format_vm_stack_value(val);
                lines
                    .into_iter()
                    .map(move |l| ListItem::new(l).style(Style::default().bg(bg)))
            })
            .collect()
    } else {
        vec![]
    };

    let stack_block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(Color::Rgb(100, 100, 100)))
        .title("Stack ");

    let stack_list = List::new(stack_items).block(stack_block);
    f.render_widget(stack_list, chunks[2]);
}

fn format_vm_stack_value(val: &VmStackValue) -> Vec<Line<'static>> {
    let dim_style = Style::default().fg(Color::Gray).add_modifier(Modifier::DIM);

    let get_counts = |h: &str| -> Option<(usize, usize)> {
        if let Ok(cell) = tycho_types::boc::Boc::decode_hex(h) {
            Some((cell.bit_len() as usize, cell.references().len()))
        } else {
            None
        }
    };

    match val {
        VmStackValue::Integer(s) => {
            let mut val_str = s.to_string();
            if val_str.len() > 32 {
                val_str = format!("{}...{}", &val_str[..16], &val_str[val_str.len() - 16..]);
            }
            let mut spans = vec![Span::styled(val_str, Style::default().fg(Color::White))];
            if let Ok(n) = s.parse::<i128>() {
                let hex = format!(" (0x{:x})", n);
                if hex.len() > 32 {
                    spans.push(Span::styled(
                        format!(
                            " (0x{:x}...{:x})",
                            n >> 64,
                            n & 0xFFFFFFFFFFFFFFFFu64 as i128
                        ),
                        dim_style,
                    ));
                } else {
                    spans.push(Span::styled(hex, dim_style));
                }
            }
            vec![Line::from(spans)]
        }
        VmStackValue::Cell(cell) => match cell {
            CellLike::Cell(h) => {
                let mut hex_display = h.to_string();
                let counts = get_counts(h);
                if hex_display.len() > 64 {
                    hex_display = format!(
                        "{}...{}",
                        &hex_display[..32],
                        &hex_display[hex_display.len() - 32..]
                    );
                }
                let mut lines = vec![Line::from(vec![
                    Span::styled("Cell ", Style::default().fg(Color::Cyan)),
                    Span::styled(hex_display, dim_style),
                ])];
                if let Some((bits, refs)) = counts {
                    lines.push(Line::from(vec![Span::styled(
                        format!("bits: {} refs: {}", bits, refs),
                        dim_style,
                    )]));
                }
                lines
            }
            CellLike::Builder(h) => {
                let mut hex_display = h.to_string();
                let counts = get_counts(h);
                if hex_display.len() > 64 {
                    hex_display = format!(
                        "{}...{}",
                        &hex_display[..32],
                        &hex_display[hex_display.len() - 32..]
                    );
                }
                let mut lines = vec![Line::from(vec![
                    Span::styled("Builder ", Style::default().fg(Color::Cyan)),
                    Span::styled(hex_display, dim_style),
                ])];
                if let Some((bits, refs)) = counts {
                    lines.push(Line::from(vec![Span::styled(
                        format!("bits: {} refs: {}", bits, refs),
                        dim_style,
                    )]));
                }
                lines
            }
        },
        VmStackValue::CellSlice(cs) => {
            let (bits, refs) = if let Some(c) = get_counts(cs.value) {
                (Some(c.0), Some(c.1))
            } else {
                (
                    cs.bits.as_ref().and_then(|(_, b)| b.parse::<usize>().ok()),
                    cs.refs.as_ref().and_then(|(_, r)| r.parse::<usize>().ok()),
                )
            };

            if let Some(bits) = bits {
                if bits == 267 {
                    if let Ok(cell) = tycho_types::boc::Boc::decode_hex(cs.value) {
                        if let Ok(mut slice) = cell.as_slice() {
                            if let Ok(addr) = IntAddr::load_from(&mut slice) {
                                let addr_str = match &addr {
                                    IntAddr::Std(std) => std.display_base64(true).to_string(),
                                    IntAddr::Var(var) => format!("{:?}", var),
                                };
                                return vec![Line::from(vec![Span::styled(
                                    addr_str,
                                    Style::default().fg(Color::Yellow),
                                )])];
                            }
                        }
                    }
                }
            }

            let mut hex_display = cs.value.to_string();
            if hex_display.len() > 64 {
                hex_display = format!(
                    "{}...{}",
                    &hex_display[..32],
                    &hex_display[hex_display.len() - 32..]
                );
            }
            let mut lines = vec![Line::from(vec![
                Span::styled("Slice ", Style::default().fg(Color::Green)),
                Span::styled(hex_display, dim_style),
            ])];

            if let (Some(bits), Some(refs)) = (bits, refs) {
                lines.push(Line::from(vec![Span::styled(
                    format!("bits: {} refs: {}", bits, refs),
                    dim_style,
                )]));
            }
            lines
        }
        VmStackValue::Tuple(items) => {
            let mut lines = vec![Line::from(vec![Span::styled(
                format!("Tuple({})", items.len()),
                Style::default().fg(Color::Yellow),
            )])];
            for (i, item) in items.iter().enumerate() {
                let mut item_lines = format_vm_stack_value(item);
                for line in &mut item_lines {
                    line.spans.insert(0, Span::raw(format!("  {}: ", i)));
                }
                lines.extend(item_lines);
            }
            lines
        }
        VmStackValue::Null => vec![Line::from(vec![Span::styled(
            "NULL",
            Style::default().fg(Color::DarkGray),
        )])],
        VmStackValue::NaN => vec![Line::from(vec![Span::styled(
            "NaN",
            Style::default().fg(Color::Red),
        )])],
        VmStackValue::String(s) => {
            let mut val_str = s.to_string();
            if val_str.len() > 64 {
                val_str = format!("{}...{}", &val_str[..32], &val_str[val_str.len() - 32..]);
            }
            vec![Line::from(vec![Span::styled(
                format!("\"{}\"", val_str),
                Style::default().fg(Color::Yellow),
            )])]
        }
        VmStackValue::Continuation(s) => {
            let mut val_str = s.to_string();
            if val_str.len() > 64 {
                val_str = format!("{}...{}", &val_str[..32], &val_str[val_str.len() - 32..]);
            }
            vec![Line::from(vec![
                Span::styled("Cont", Style::default().fg(Color::Magenta)),
                Span::styled(format!(" {}", val_str), dim_style),
            ])]
        }
        VmStackValue::Builder(s) => {
            let mut val_str = s.to_string();
            let counts = get_counts(s);
            if val_str.len() > 64 {
                val_str = format!("{}...{}", &val_str[..32], &val_str[val_str.len() - 32..]);
            }
            let mut lines = vec![Line::from(vec![
                Span::styled("Builder ", Style::default().fg(Color::Cyan)),
                Span::styled(val_str, dim_style),
            ])];
            if let Some((bits, refs)) = counts {
                lines.push(Line::from(vec![Span::styled(
                    format!(" bits:{} refs:{}", bits, refs),
                    dim_style,
                )]));
            }
            lines
        }
        VmStackValue::Unknown => vec![Line::from(vec![Span::styled(
            "???",
            Style::default().fg(Color::Red),
        )])],
    }
}
