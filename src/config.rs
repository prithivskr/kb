use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use ratatui::style::Color;
use serde::Deserialize;

const APP_NAME: &str = "kb";
const CONFIG_DIR_NAME: &str = ".kb";
const DB_FILENAME: &str = "kanban-v2.db";

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_path: PathBuf,
    pub limits: Limits,
    pub colors: Colors,
}

#[derive(Debug, Clone)]
pub struct Limits {
    pub today_hard_limit: usize,
    pub this_week_soft_limit: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Colors {
    pub bg: Color,
    pub fg: Color,
    pub border: Color,
    pub active_border: Color,
    pub due_overdue: Color,
    pub due_today: Color,
    pub due_soon: Color,
    pub title: Color,
}

#[derive(Debug, Default, Deserialize)]
struct FileConfig {
    #[serde(default)]
    database: DatabaseFileConfig,
    #[serde(default)]
    limits: LimitsFileConfig,
    #[serde(default)]
    colors: ColorsFileConfig,
}

#[derive(Debug, Default, Deserialize)]
struct DatabaseFileConfig {
    path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct LimitsFileConfig {
    today_hard_limit: Option<usize>,
    this_week_soft_limit: Option<usize>,
}

#[derive(Debug, Default, Deserialize)]
struct ColorsFileConfig {
    bg: Option<String>,
    fg: Option<String>,
    border: Option<String>,
    active_border: Option<String>,
    due_overdue: Option<String>,
    due_today: Option<String>,
    due_soon: Option<String>,
    title: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            database_path: default_database_path(),
            limits: Limits {
                today_hard_limit: 4,
                this_week_soft_limit: 10,
            },
            colors: Colors {
                bg: Color::Reset,
                fg: Color::Gray,
                border: Color::DarkGray,
                active_border: Color::Cyan,
                due_overdue: Color::Red,
                due_today: Color::Yellow,
                due_soon: Color::Cyan,
                title: Color::White,
            },
        }
    }
}

pub fn get() -> &'static AppConfig {
    static CONFIG: OnceLock<AppConfig> = OnceLock::new();
    CONFIG.get_or_init(load)
}

fn load() -> AppConfig {
    let defaults = AppConfig::default();
    let path = default_config_path();
    let file = match fs::read_to_string(path) {
        Ok(raw) => toml::from_str::<FileConfig>(&raw).unwrap_or_default(),
        Err(_) => FileConfig::default(),
    };

    let database_path = file
        .database
        .path
        .as_deref()
        .and_then(expand_path)
        .unwrap_or_else(|| defaults.database_path.clone());

    let limits = Limits {
        today_hard_limit: file
            .limits
            .today_hard_limit
            .unwrap_or(defaults.limits.today_hard_limit),
        this_week_soft_limit: file
            .limits
            .this_week_soft_limit
            .unwrap_or(defaults.limits.this_week_soft_limit),
    };

    let colors = Colors {
        bg: parse_color_or_default(file.colors.bg.as_deref(), defaults.colors.bg),
        fg: parse_color_or_default(file.colors.fg.as_deref(), defaults.colors.fg),
        border: parse_color_or_default(file.colors.border.as_deref(), defaults.colors.border),
        active_border: parse_color_or_default(
            file.colors.active_border.as_deref(),
            defaults.colors.active_border,
        ),
        due_overdue: parse_color_or_default(
            file.colors.due_overdue.as_deref(),
            defaults.colors.due_overdue,
        ),
        due_today: parse_color_or_default(
            file.colors.due_today.as_deref(),
            defaults.colors.due_today,
        ),
        due_soon: parse_color_or_default(file.colors.due_soon.as_deref(), defaults.colors.due_soon),
        title: parse_color_or_default(file.colors.title.as_deref(), defaults.colors.title),
    };

    AppConfig {
        database_path,
        limits,
        colors,
    }
}

fn default_config_path() -> PathBuf {
    if let Some(home) = dirs::home_dir() {
        return home.join(CONFIG_DIR_NAME).join("config.toml");
    }
    PathBuf::from(CONFIG_DIR_NAME).join("config.toml")
}

fn default_database_path() -> PathBuf {
    if let Some(base) = dirs::data_local_dir() {
        return base.join(APP_NAME).join(DB_FILENAME);
    }
    if let Some(home) = dirs::home_dir() {
        return home
            .join(".local")
            .join("share")
            .join(APP_NAME)
            .join(DB_FILENAME);
    }
    PathBuf::from(DB_FILENAME)
}

fn expand_path(raw: &str) -> Option<PathBuf> {
    if let Some(rest) = raw.strip_prefix("~/") {
        return dirs::home_dir().map(|home| home.join(rest));
    }
    Some(PathBuf::from(raw))
}

fn parse_color_or_default(raw: Option<&str>, fallback: Color) -> Color {
    raw.and_then(parse_color).unwrap_or(fallback)
}

fn parse_color(raw: &str) -> Option<Color> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "reset" => Some(Color::Reset),
        "black" => Some(Color::Black),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "yellow" => Some(Color::Yellow),
        "blue" => Some(Color::Blue),
        "magenta" => Some(Color::Magenta),
        "cyan" => Some(Color::Cyan),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "dark_gray" | "dark-grey" | "darkgrey" => Some(Color::DarkGray),
        "white" => Some(Color::White),
        _ => parse_hex_color(&normalized),
    }
}

fn parse_hex_color(raw: &str) -> Option<Color> {
    let hex = raw.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(Color::Rgb(r, g, b))
}
