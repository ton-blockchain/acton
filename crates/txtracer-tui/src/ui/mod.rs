use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
};

use crate::app::App;

mod details;
mod doc_popup;
mod footer;
mod header;
mod step_list_panel;
mod step_panel;

pub fn draw(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Length(1), // Header (Minimal)
                Constraint::Min(0),    // Content
                if app.details_expanded {
                    Constraint::Length(12) // Details expanded
                } else {
                    Constraint::Length(1) // Details collapsed
                },
                Constraint::Length(1), // Footer
            ]
            .as_ref(),
        )
        .split(f.area());

    header::draw(f, app, chunks[0]);

    let content_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)].as_ref())
        .split(chunks[1]);

    step_list_panel::draw(f, app, content_chunks[0]);
    step_panel::draw(f, app, content_chunks[1]);

    details::draw(f, app, chunks[2]);
    footer::draw(f, app, chunks[3]);

    if app.show_docs {
        doc_popup::draw(f, app);
    }
}
