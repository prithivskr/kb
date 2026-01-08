use ratatui::style::{Color, Style};

pub const BG: Color = Color::Black;
pub const FG: Color = Color::Gray;
pub const BORDER: Color = Color::DarkGray;
pub const ACTIVE_BORDER: Color = Color::Cyan;
pub const DUE_OVERDUE: Color = Color::Red;
pub const DUE_TODAY: Color = Color::Yellow;
pub const DUE_SOON: Color = Color::Cyan;

pub fn title_style() -> Style {
    Style::default().fg(Color::White)
}
