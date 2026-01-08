use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Span,
    widgets::Widget,
};

pub struct Badge<'a> {
    label: &'a str,
    bg: Color,
    fg: Color,
}

impl<'a> Badge<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            bg: Color::Gray,
            fg: Color::Black,
        }
    }

    pub fn bg(mut self, bg: Color) -> Self {
        self.bg = bg;
        self
    }

    pub fn fg(mut self, fg: Color) -> Self {
        self.fg = fg;
        self
    }
}

impl<'a> Widget for Badge<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let style = Style::default()
            .bg(self.bg)
            .fg(self.fg)
            .add_modifier(Modifier::BOLD);

        // Simple rendering: just text with background, centered if possible
        let span = Span::styled(format!(" {} ", self.label), style);

        let x = area.x;
        let y = area.y + area.height / 2;

        if y < area.y + area.height {
            buf.set_string(x, y, span.content, style);
        }
    }
}
