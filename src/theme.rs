use gpui::{App, Global, Hsla, Window, px, rgb};
use gpui_component::{
    Theme, ThemeMode,
    highlighter::{HighlightTheme, HighlightThemeStyle, SyntaxColors},
};
use serde_json::json;
use std::sync::Arc;

pub const DISPLAY: &str = "Instrument Serif";
pub const SANS: &str = "Instrument Sans";
pub const MONO: &str = "Source Code Pro";

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum EditorFont {
    #[default]
    Code,
    Sans,
    Serif,
}

impl EditorFont {
    pub const ALL: [Self; 3] = [Self::Code, Self::Sans, Self::Serif];
    pub fn family(self) -> &'static str {
        match self {
            Self::Code => MONO,
            Self::Sans => SANS,
            Self::Serif => DISPLAY,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Code => "Mono",
            Self::Sans => "Sans",
            Self::Serif => "Serif",
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum LineSpacing {
    Compact,
    #[default]
    Comfortable,
    Relaxed,
}

impl LineSpacing {
    pub const ALL: [Self; 3] = [Self::Compact, Self::Comfortable, Self::Relaxed];
    pub fn ratio(self) -> f32 {
        match self {
            Self::Compact => 1.5,
            Self::Comfortable => 1.75,
            Self::Relaxed => 2.0,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Compact => "Compact",
            Self::Comfortable => "Comfortable",
            Self::Relaxed => "Relaxed",
        }
    }
}

pub const DEFAULT_FONT_SIZE: f32 = 12.;
pub const MIN_FONT_SIZE: f32 = 10.;
pub const MAX_FONT_SIZE: f32 = 32.;

/// User preferences, saved as JSON and applied to every window.
#[derive(Clone, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct Settings {
    pub appearance: Appearance,
    pub font: EditorFont,
    pub font_size: f32,
    pub line_spacing: LineSpacing,
    pub wrap_text: bool,
    pub wrap_code: bool,
    pub line_numbers: bool,
    pub beautify_json_on_save: bool,
    pub check_updates_on_launch: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            appearance: Appearance::System,
            font: EditorFont::Code,
            font_size: DEFAULT_FONT_SIZE,
            line_spacing: LineSpacing::Comfortable,
            wrap_text: true,
            wrap_code: false,
            line_numbers: true,
            beautify_json_on_save: false,
            check_updates_on_launch: true,
        }
    }
}

impl Settings {
    pub fn load(path: &std::path::Path) -> Self {
        let mut settings: Self = std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        settings.font_size = if settings.font_size.is_finite() {
            settings.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE)
        } else {
            DEFAULT_FONT_SIZE
        };
        settings
    }

    fn save(&self, path: &std::path::Path) -> anyhow::Result<()> {
        use std::io::Write as _;
        let dir = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("No settings folder."))?;
        std::fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        file.write_all(&serde_json::to_vec_pretty(self)?)?;
        file.persist(path)?;
        Ok(())
    }
}

impl Global for Settings {}

pub struct SettingsPath(pub Option<std::path::PathBuf>);
impl Global for SettingsPath {}

/// Changes a preference, saves it, and refreshes every window and the menu bar.
pub fn update_settings(cx: &mut App, change: impl FnOnce(&mut Settings)) {
    let before = cx.global::<Settings>().clone();
    let mut after = before.clone();
    change(&mut after);
    after.font_size = after.font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
    if after == before {
        return;
    }
    if let Some(path) = cx.try_global::<SettingsPath>().and_then(|p| p.0.clone())
        && let Err(error) = after.save(&path)
    {
        eprintln!("Couldn’t save settings: {error:#}");
    }
    let appearance = after.appearance;
    *cx.global_mut::<Settings>() = after;
    apply(appearance, None, cx);
    crate::menus_changed(cx);
}

pub struct Palette {
    pub paper: Hsla,
    pub chrome: Hsla,
    pub active_line: Hsla,
    pub ink: Hsla,
    pub muted: Hsla,
    pub rule: Hsla,
    pub caret: Hsla,
    pub selection: Hsla,
    syntax: [&'static str; 4],
}

impl Palette {
    pub fn current(cx: &App) -> Self {
        Self::new(Theme::global(cx).is_dark())
    }

    fn new(dark: bool) -> Self {
        let color = |hex| Hsla::from(rgb(hex));
        if dark {
            Self {
                paper: color(0x171717),
                chrome: color(0x242424),
                active_line: color(0x303030),
                ink: color(0xefefef),
                muted: color(0xbcbcbc),
                rule: color(0x3c3c3c),
                caret: color(0xe8e8e8),
                selection: color(0x434343),
                syntax: ["#e6b7e9", "#add19b", "#abcbe8", "#f6b282"],
            }
        } else {
            Self {
                paper: color(0xfffcf5),
                chrome: color(0xf7f1e6),
                active_line: color(0xf0e8d9),
                ink: color(0x24372e),
                muted: color(0x56635a),
                rule: color(0xdcd2bf),
                caret: color(0x9f402a),
                selection: color(0xe8d8b3),
                syntax: ["#734277", "#376040", "#365d78", "#8a461e"],
            }
        }
    }
}

pub fn apply(appearance: Appearance, window: Option<&mut Window>, cx: &mut App) {
    let mode = match appearance {
        Appearance::System => window
            .as_ref()
            .map_or_else(|| cx.window_appearance(), |w| w.appearance())
            .into(),
        Appearance::Light => ThemeMode::Light,
        Appearance::Dark => ThemeMode::Dark,
    };
    Theme::change(mode, window, cx);
    let p = Palette::new(mode.is_dark());
    let theme = Theme::global_mut(cx);
    theme.font_family = SANS.into();
    theme.font_size = px(13.);
    theme.mono_font_family = MONO.into();
    theme.mono_font_size = px(12.);
    theme.radius = px(8.);
    theme.background = p.paper;
    theme.foreground = p.ink;
    theme.border = p.rule;
    theme.caret = p.caret;
    theme.ring = p.caret;
    theme.selection = p.selection;
    theme.accent = p.active_line;
    theme.accent_foreground = p.ink;
    theme.muted = p.chrome;
    theme.muted_foreground = p.muted;
    theme.popover = p.paper;
    theme.popover_foreground = p.ink;
    theme.secondary_foreground = p.ink;
    theme.title_bar = p.chrome;
    theme.title_bar_border = p.rule;
    // One clear accent for anything switched on or selected; off states stay neutral.
    let (accent, accent_hover, accent_active, on_accent, off) = if mode.is_dark() {
        (0xe39a63, 0xedaa78, 0xd08a55, 0x1b1b1b, 0x555555)
    } else {
        (0xb4532f, 0xc45f39, 0xa24826, 0xfffaf0, 0xcdc4b0)
    };
    theme.primary = rgb(accent).into();
    theme.primary_hover = rgb(accent_hover).into();
    theme.primary_active = rgb(accent_active).into();
    theme.primary_foreground = rgb(on_accent).into();
    theme.switch = rgb(off).into();
    theme.switch_thumb = rgb(if mode.is_dark() { 0xf2f2f2 } else { 0xffffff }).into();

    let [keyword, string, function, number] = p.syntax;
    let syntax: SyntaxColors = serde_json::from_value(json!({
        "keyword": {"color": keyword},
        "title": {"color": keyword},
        "string": {"color": string},
        "function": {"color": function},
        "type": {"color": function},
        "link_text": {"color": function},
        "number": {"color": number},
        "constant": {"color": number},
        "boolean": {"color": number},
        "comment": {"color": p.muted},
        "comment_doc": {"color": p.muted}
    }))
    .expect("Larik Text syntax colors must be valid");
    theme.highlight_theme = Arc::new(HighlightTheme {
        name: "Larik Text".to_owned(),
        appearance: mode,
        style: HighlightThemeStyle {
            editor_background: Some(p.paper),
            editor_foreground: Some(p.ink),
            editor_active_line: Some(p.active_line),
            editor_line_number: Some(p.muted),
            editor_active_line_number: Some(p.ink),
            syntax,
            ..Default::default()
        },
    });
    cx.refresh_windows();
}
