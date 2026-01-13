use ratatui::style::{Color, Style};

use crate::config;

pub fn bg() -> Color {
    config::get().colors.bg
}

pub fn fg() -> Color {
    config::get().colors.fg
}

pub fn border() -> Color {
    config::get().colors.border
}

pub fn active_border() -> Color {
    config::get().colors.active_border
}

pub fn due_overdue() -> Color {
    config::get().colors.due_overdue
}

pub fn due_today() -> Color {
    config::get().colors.due_today
}

pub fn due_soon() -> Color {
    config::get().colors.due_soon
}

pub fn title_style() -> Style {
    Style::default().fg(config::get().colors.title)
}
