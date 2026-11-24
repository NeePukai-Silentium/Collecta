//! Look of the window from debug\Collecta\theme.ini (`--theme <file>`), re-read when the file changes.
//! Lines look like `dark.bg = 141416`; `;` and `#` start comments. Missing keys keep the defaults of ui/theme.slint.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use slint::{Color, ComponentHandle, Timer, TimerMode};

use crate::{MainWindow, Palette, Theme};

const CHECK_INTERVAL: Duration = Duration::from_millis(500);

/// Values of ui/theme.slint before the file is applied: what a key falls back to when it's removed.
#[derive(Clone)]
struct Defaults {
    dark: Palette,
    light: Palette,
    unit: f32,
    radius: f32,
    control_height: f32,
    icon_size: f32,
    font_size: f32,
    sidebar_width: f32,
    thumb_size: f32,
    anim: i64,
    easing: slint::SharedString,
}

pub struct ThemeFile {
    path: PathBuf,
    defaults: Defaults,
    modified: Option<SystemTime>,
    /// `theme` as last read: the toggle in the window wins until this field itself changes.
    theme: Option<String>,
}

impl ThemeFile {
    pub fn new(window: &MainWindow, path: PathBuf) -> Self {
        let theme = window.global::<Theme>();
        let defaults = Defaults {
            dark: theme.get_dark_palette(),
            light: theme.get_light_palette(),
            unit: theme.get_unit(),
            radius: theme.get_radius(),
            control_height: theme.get_control_height(),
            icon_size: theme.get_icon_size(),
            font_size: theme.get_font_size(),
            sidebar_width: theme.get_sidebar_width(),
            thumb_size: theme.get_thumb_size(),
            anim: theme.get_anim(),
            easing: theme.get_easing_name(),
        };
        Self { path, defaults, modified: None, theme: None }
    }

    /// Folder with test pictures (`images_dir`), relative paths count from the file's folder.
    pub fn images_dir(&self) -> Option<PathBuf> {
        let text = std::fs::read_to_string(&self.path).ok()?;
        let value = values(&text).find(|(key, _)| *key == "images_dir")?.1;
        if value.is_empty() {
            return None;
        }
        let dir = self.path.parent().unwrap_or(Path::new(".")).join(value);
        Some(dir)
    }

    /// Applies the file if it changed since the last call.
    pub fn apply_if_changed(&mut self, window: &MainWindow) {
        let modified = std::fs::metadata(&self.path).and_then(|meta| meta.modified()).ok();
        if modified.is_none() || modified == self.modified {
            return;
        }
        self.modified = modified;
        let Ok(text) = std::fs::read_to_string(&self.path) else {
            return;
        };
        self.apply(window, &text);
        crate::timing::write_log("theme.ini applied");
    }

    fn apply(&mut self, window: &MainWindow, text: &str) {
        let theme = window.global::<Theme>();
        let mut d = self.defaults.clone();
        for (key, value) in values(text) {
            if let Some(field) = key.strip_prefix("dark.") {
                set_color(&mut d.dark, field, value);
            } else if let Some(field) = key.strip_prefix("light.") {
                set_color(&mut d.light, field, value);
            }
            let number = value.parse::<f32>().ok();
            match (key, number) {
                ("unit", Some(n)) => d.unit = n.clamp(1.0, 16.0),
                ("radius", Some(n)) => d.radius = n.clamp(0.0, 40.0),
                ("control_height", Some(n)) => d.control_height = n.clamp(16.0, 64.0),
                ("icon_size", Some(n)) => d.icon_size = n.clamp(8.0, 48.0),
                ("font_size", Some(n)) => d.font_size = n.clamp(8.0, 32.0),
                ("sidebar_width", Some(n)) => d.sidebar_width = n.clamp(120.0, 480.0),
                ("thumb_size", Some(n)) => d.thumb_size = n.clamp(60.0, 600.0),
                ("anim_ms", Some(n)) => d.anim = n.clamp(0.0, 3000.0) as i64,
                ("easing", _) => d.easing = value.into(),
                ("theme", _) if self.theme.as_deref() != Some(value) => {
                    self.theme = Some(value.to_string());
                    theme.set_dark(value != "light");
                }
                _ => {}
            }
        }
        theme.set_dark_palette(d.dark);
        theme.set_light_palette(d.light);
        theme.set_unit(d.unit);
        theme.set_radius(d.radius);
        theme.set_control_height(d.control_height);
        theme.set_icon_size(d.icon_size);
        theme.set_font_size(d.font_size);
        theme.set_sidebar_width(d.sidebar_width);
        theme.set_thumb_size(d.thumb_size);
        theme.set_anim(d.anim);
        theme.set_easing_name(d.easing);
    }
}

/// Checks the file every half second for as long as the window lives.
pub fn watch(window: &MainWindow, mut file: ThemeFile) -> Timer {
    file.apply_if_changed(window);
    let timer = Timer::default();
    let weak = window.as_weak();
    timer.start(TimerMode::Repeated, CHECK_INTERVAL, move || {
        if let Some(window) = weak.upgrade() {
            file.apply_if_changed(&window);
        }
    });
    timer
}

fn values(text: &str) -> impl Iterator<Item = (&str, &str)> {
    text.lines().filter_map(|line| {
        let line = line.split([';', '#']).next().unwrap_or("");
        let (key, value) = line.split_once('=')?;
        Some((key.trim(), value.trim()))
    })
}

fn set_color(palette: &mut Palette, field: &str, value: &str) {
    let Some(color) = parse_color(value) else {
        return;
    };
    let slot = match field {
        "bg" => &mut palette.bg,
        "surface" => &mut palette.surface,
        "surface_hover" => &mut palette.surface_hover,
        "surface_active" => &mut palette.surface_active,
        "border" => &mut palette.border,
        "text" => &mut palette.text,
        "text_muted" => &mut palette.text_muted,
        "accent" => &mut palette.accent,
        "danger" => &mut palette.danger,
        "scrim" => &mut palette.scrim,
        _ => return,
    };
    *slot = color;
}

/// "RRGGBB" or "RRGGBBAA".
fn parse_color(value: &str) -> Option<Color> {
    if !matches!(value.len(), 6 | 8) {
        return None;
    }
    let number = u32::from_str_radix(value, 16).ok()?;
    let (rgb, alpha) = if value.len() == 6 { (number, 255) } else { (number >> 8, number & 0xff) };
    Some(Color::from_argb_u8(alpha as u8, (rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8))
}
