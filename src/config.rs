use crossterm::style::Color;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(transparent)]
pub struct ColorDef(pub u8);

impl ColorDef {
    pub fn to_crossterm(self) -> Color {
        Color::AnsiValue(self.0)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub focused_border: ColorDef,
    pub unfocused_border: ColorDef,
    pub pinned_border: ColorDef,
    pub hint_text: ColorDef,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            focused_border: ColorDef(14),
            unfocused_border: ColorDef(8),
            pinned_border: ColorDef(11),
            hint_text: ColorDef(8),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct KeyConfig {
    pub new_window: char,
    pub focus_next: char,
    pub focus_prev: char,
    pub quit: char,
    pub close_window: char,
    pub move_left: char,
    pub move_down: char,
    pub move_up: char,
    pub move_right: char,
    pub resize_left: char,
    pub resize_down: char,
    pub resize_up: char,
    pub resize_right: char,
    pub pin_window: char,
}

impl Default for KeyConfig {
    fn default() -> Self {
        Self {
            new_window: 'c',
            focus_next: 'n',
            focus_prev: 'p',
            quit: 'q',
            close_window: 'x',
            move_left: 'h',
            move_down: 'j',
            move_up: 'k',
            move_right: 'l',
            resize_left: 'H',
            resize_down: 'J',
            resize_up: 'K',
            resize_right: 'L',
            pin_window: 'w',
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    pub cascade_offset_x: i32,
    pub cascade_offset_y: i32,
    pub new_window_width_ratio: f64,
    pub new_window_height_ratio: f64,
    pub min_window_cols: u16,
    pub min_window_rows: u16,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            cascade_offset_x: 2,
            cascade_offset_y: 1,
            new_window_width_ratio: 0.5,
            new_window_height_ratio: 0.5,
            min_window_cols: 6,
            min_window_rows: 5,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(default)]
    pub shell: String,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub keys: KeyConfig,
    #[serde(default)]
    pub layout: LayoutConfig,
    pub alt_timeout_ms: u64,
    pub poll_interval_ms: u64,
    #[serde(default)]
    pub disable_mouse: bool,
    /// Forward mouse events to a child program that has enabled xterm mouse
    /// reporting (mc, vim, htop, …) instead of using them for window management.
    pub mouse_passthrough: bool,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            shell: String::new(),
            theme: ThemeConfig::default(),
            keys: KeyConfig::default(),
            layout: LayoutConfig::default(),
            alt_timeout_ms: 200,
            poll_interval_ms: 16,
            disable_mouse: false,
            mouse_passthrough: true,
        }
    }
}

/// Load configuration from the standard path.
///
/// Looks for `$XDG_CONFIG_HOME/float/config.toml`, falling back to
/// `~/.config/float/config.toml`.  Missing file or parse errors produce a
/// warning on stderr and return the full default configuration.
pub fn load() -> Config {
    let path = config_path();

    let contents = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Config::default(),
        Err(e) => {
            eprintln!("float: warning: reading {}: {}", path.display(), e);
            return Config::default();
        }
    };

    match toml::from_str::<Config>(&contents) {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("float: warning: parsing {}: {}", path.display(), e);
            Config::default()
        }
    }
}

fn config_path() -> std::path::PathBuf {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        return std::path::PathBuf::from(dir)
            .join("float")
            .join("config.toml");
    }

    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home)
            .join(".config")
            .join("float")
            .join("config.toml");
    }

    std::path::PathBuf::from(".config")
        .join("float")
        .join("config.toml")
}
