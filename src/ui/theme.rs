use ratatui::style::{Color, Style};

pub const BG: Color = Color::Black;
pub const FG: Color = Color::Gray;
pub const BORDER: Color = Color::DarkGray;
pub const ACTIVE_BORDER: Color = Color::Cyan;

pub fn title_style() -> Style {
    Style::default().fg(Color::White)
}
