use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    widgets::Paragraph,
};

use crate::app::App;

pub fn draw(f: &mut Frame, _app: &App, area: Rect) {
    let controls = "q: Quit | \u{2190}/\u{2192}/Up/Down: Step | Home/End: First/Last | Enter: Details | ?: Docs";

    let p = Paragraph::new(controls)
        .alignment(Alignment::Right)
        .style(Style::default().bg(Color::Rgb(30, 30, 30)).fg(Color::Gray));

    f.render_widget(p, area);
}
