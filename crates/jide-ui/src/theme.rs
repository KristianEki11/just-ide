use anyhow::{bail, Context};
use egui::{Color32, CornerRadius, Stroke, Visuals};
use serde::Deserialize;
use std::collections::HashMap;

const JUST_DARK_TOML: &str = include_str!("../../../assets/themes/just-dark.toml");
const JUST_LIGHT_TOML: &str = include_str!("../../../assets/themes/just-light.toml");
const HIGH_CONTRAST_TOML: &str = include_str!("../../../assets/themes/high-contrast.toml");

/// Identifier of a theme.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ThemeId {
    /// Built-in Just Dark theme (default).
    JustDark,
    /// Built-in Just Light theme.
    JustLight,
    /// Built-in High Contrast dark theme.
    HighContrast,
    /// Custom user theme loaded from disk.
    Custom(String),
}

/// Visual polarity / accessibility category of a theme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeKind {
    /// Standard dark UI.
    Dark,
    /// Standard light UI.
    Light,
    /// High-contrast dark UI.
    HighContrast,
}

/// All 35 UI and editor color tokens defined in `docs/03-DESIGN-SYSTEM.md` §2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeColors {
    /// Editor background (`bg.base`).
    pub bg_base: Color32,
    /// Sidebar, tab bar, panels, status bar (`bg.surface`).
    pub bg_surface: Color32,
    /// Command palette, menus, tooltips, dialogs (`bg.elevated`).
    pub bg_elevated: Color32,
    /// Hovered row/button (`bg.hover`).
    pub bg_hover: Color32,
    /// Pressed or selected row when unfocused (`bg.active`).
    pub bg_active: Color32,
    /// Text inputs (`bg.input`).
    pub bg_input: Color32,
    /// Subtle dividers (`border.subtle`).
    pub border_subtle: Color32,
    /// Input borders and dialog outlines (`border.strong`).
    pub border_strong: Color32,
    /// Focus ring (`border.focus`).
    pub border_focus: Color32,
    /// Primary foreground text (`text.primary`).
    pub text_primary: Color32,
    /// Secondary labels and inactive tabs (`text.secondary`).
    pub text_secondary: Color32,
    /// Muted hints and line numbers (`text.muted`).
    pub text_muted: Color32,
    /// Disabled text (`text.disabled`).
    pub text_disabled: Color32,
    /// Text on accent-filled buttons (`text.on-accent`).
    pub text_on_accent: Color32,
    /// Primary accent color (`accent`).
    pub accent: Color32,
    /// Hovered accent color (`accent.hover`).
    pub accent_hover: Color32,
    /// Subtle accent fill (`accent.subtle`).
    pub accent_subtle: Color32,
    /// Success status (`success`).
    pub success: Color32,
    /// Warning status (`warning`).
    pub warning: Color32,
    /// Danger/error status (`danger`).
    pub danger: Color32,
    /// Info status (`info`).
    pub info: Color32,
    /// Active line band (`editor.current-line`).
    pub editor_current_line: Color32,
    /// Focused selection fill (`editor.selection`).
    pub editor_selection: Color32,
    /// Unfocused selection fill (`editor.selection.inactive`).
    pub editor_selection_inactive: Color32,
    /// Primary caret (`editor.cursor`).
    pub editor_cursor: Color32,
    /// Secondary multi-cursors (`editor.cursor.secondary`).
    pub editor_cursor_secondary: Color32,
    /// Find match highlight (`editor.match`).
    pub editor_match: Color32,
    /// Current find match highlight (`editor.match.current`).
    pub editor_match_current: Color32,
    /// Matching bracket highlight (`editor.bracket-match`).
    pub editor_bracket_match: Color32,
    /// Gutter line numbers (`editor.line-number`).
    pub editor_line_number: Color32,
    /// Active line gutter number (`editor.line-number.active`).
    pub editor_line_number_active: Color32,
    /// Indentation guide (`editor.indent-guide`).
    pub editor_indent_guide: Color32,
    /// Active indentation guide (`editor.indent-guide.active`).
    pub editor_indent_guide_active: Color32,
    /// Visible whitespace glyphs (`editor.whitespace`).
    pub editor_whitespace: Color32,
    /// Optional column ruler (`editor.ruler`).
    pub editor_ruler: Color32,
}

/// Foreground color and font styling for a syntax scope token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SyntaxStyle {
    /// Foreground text color.
    pub fg: Color32,
    /// Render in bold weight.
    pub bold: bool,
    /// Render in italic style.
    pub italic: bool,
    /// Render with underline.
    pub underline: bool,
}

/// All 21 syntax color tokens defined in `docs/03-DESIGN-SYSTEM.md` §2.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeSyntax {
    /// Language keywords.
    pub keyword: SyntaxStyle,
    /// String literals.
    pub string: SyntaxStyle,
    /// Numeric literals.
    pub number: SyntaxStyle,
    /// Constants.
    pub constant: SyntaxStyle,
    /// Comments.
    pub comment: SyntaxStyle,
    /// Function and method identifiers.
    pub function: SyntaxStyle,
    /// Type names.
    pub type_name: SyntaxStyle,
    /// Class/struct names.
    pub class: SyntaxStyle,
    /// Variables.
    pub variable: SyntaxStyle,
    /// Parameters.
    pub parameter: SyntaxStyle,
    /// Operators.
    pub operator: SyntaxStyle,
    /// Punctuation.
    pub punctuation: SyntaxStyle,
    /// HTML/XML tags.
    pub tag: SyntaxStyle,
    /// HTML/XML attributes.
    pub attribute: SyntaxStyle,
    /// Regular expressions.
    pub regex: SyntaxStyle,
    /// Escape sequences.
    pub escape: SyntaxStyle,
    /// Markdown headings.
    pub heading: SyntaxStyle,
    /// Markdown/doc links.
    pub link: SyntaxStyle,
    /// Invalid syntax tokens.
    pub invalid: SyntaxStyle,
    /// Diff added lines.
    pub diff_added: SyntaxStyle,
    /// Diff removed lines.
    pub diff_removed: SyntaxStyle,
}

/// Resolved theme ready for rendering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    /// Identifier of this theme.
    pub id: ThemeId,
    /// Human-readable theme name.
    pub name: String,
    /// Theme polarity / contrast mode.
    pub kind: ThemeKind,
    /// UI and editor surface colors.
    pub colors: ThemeColors,
    /// Syntax token styles.
    pub syntax: ThemeSyntax,
}

#[derive(Deserialize)]
struct RawTheme {
    name: String,
    kind: ThemeKind,
    #[serde(default)]
    colors: HashMap<String, String>,
    #[serde(default)]
    syntax: HashMap<String, RawSyntaxStyle>,
}

#[derive(Deserialize)]
struct RawSyntaxStyle {
    fg: String,
    #[serde(default)]
    bold: bool,
    #[serde(default)]
    italic: bool,
    #[serde(default)]
    underline: bool,
}

/// Canonical list of required `[colors]` token keys.
pub const REQUIRED_COLOR_KEYS: &[&str] = &[
    "bg.base",
    "bg.surface",
    "bg.elevated",
    "bg.hover",
    "bg.active",
    "bg.input",
    "border.subtle",
    "border.strong",
    "border.focus",
    "text.primary",
    "text.secondary",
    "text.muted",
    "text.disabled",
    "text.on-accent",
    "accent",
    "accent.hover",
    "accent.subtle",
    "success",
    "warning",
    "danger",
    "info",
    "editor.current-line",
    "editor.selection",
    "editor.selection.inactive",
    "editor.cursor",
    "editor.cursor.secondary",
    "editor.match",
    "editor.match.current",
    "editor.bracket-match",
    "editor.line-number",
    "editor.line-number.active",
    "editor.indent-guide",
    "editor.indent-guide.active",
    "editor.whitespace",
    "editor.ruler",
];

/// Canonical list of required `[syntax]` token keys.
pub const REQUIRED_SYNTAX_KEYS: &[&str] = &[
    "keyword",
    "string",
    "number",
    "constant",
    "comment",
    "function",
    "type",
    "class",
    "variable",
    "parameter",
    "operator",
    "punctuation",
    "tag",
    "attribute",
    "regex",
    "escape",
    "heading",
    "link",
    "invalid",
    "diff.added",
    "diff.removed",
];

impl Theme {
    /// Returns the built-in Just Dark theme (default).
    #[must_use]
    pub fn builtin_dark() -> Self {
        Self::from_embedded(&ThemeId::JustDark, JUST_DARK_TOML)
    }

    /// Returns the built-in Just Light theme.
    #[must_use]
    pub fn builtin_light() -> Self {
        Self::from_embedded(&ThemeId::JustLight, JUST_LIGHT_TOML)
    }

    /// Returns the built-in High Contrast dark theme.
    #[must_use]
    pub fn builtin_high_contrast() -> Self {
        Self::from_embedded(&ThemeId::HighContrast, HIGH_CONTRAST_TOML)
    }

    fn from_embedded(id: &ThemeId, raw: &str) -> Self {
        match Self::from_toml_strict(id.clone(), raw) {
            Ok(theme) => theme,
            Err(err) => unreachable!("built-in theme {id:?} must always be valid: {err}"),
        }
    }

    /// Parses a TOML theme requiring every token in `docs/03-DESIGN-SYSTEM.md` §2 to be present.
    pub fn from_toml_strict(id: ThemeId, raw_toml: &str) -> anyhow::Result<Self> {
        let raw: RawTheme = toml::from_str(raw_toml).context("parsing theme TOML")?;
        for key in REQUIRED_COLOR_KEYS {
            if !raw.colors.contains_key(*key) {
                bail!("missing required color token `{key}`");
            }
        }
        for key in REQUIRED_SYNTAX_KEYS {
            if !raw.syntax.contains_key(*key) {
                bail!("missing required syntax token `{key}`");
            }
        }

        let colors = ThemeColors {
            bg_base: color_req(&raw.colors, "bg.base")?,
            bg_surface: color_req(&raw.colors, "bg.surface")?,
            bg_elevated: color_req(&raw.colors, "bg.elevated")?,
            bg_hover: color_req(&raw.colors, "bg.hover")?,
            bg_active: color_req(&raw.colors, "bg.active")?,
            bg_input: color_req(&raw.colors, "bg.input")?,
            border_subtle: color_req(&raw.colors, "border.subtle")?,
            border_strong: color_req(&raw.colors, "border.strong")?,
            border_focus: color_req(&raw.colors, "border.focus")?,
            text_primary: color_req(&raw.colors, "text.primary")?,
            text_secondary: color_req(&raw.colors, "text.secondary")?,
            text_muted: color_req(&raw.colors, "text.muted")?,
            text_disabled: color_req(&raw.colors, "text.disabled")?,
            text_on_accent: color_req(&raw.colors, "text.on-accent")?,
            accent: color_req(&raw.colors, "accent")?,
            accent_hover: color_req(&raw.colors, "accent.hover")?,
            accent_subtle: color_req(&raw.colors, "accent.subtle")?,
            success: color_req(&raw.colors, "success")?,
            warning: color_req(&raw.colors, "warning")?,
            danger: color_req(&raw.colors, "danger")?,
            info: color_req(&raw.colors, "info")?,
            editor_current_line: color_req(&raw.colors, "editor.current-line")?,
            editor_selection: color_req(&raw.colors, "editor.selection")?,
            editor_selection_inactive: color_req(&raw.colors, "editor.selection.inactive")?,
            editor_cursor: color_req(&raw.colors, "editor.cursor")?,
            editor_cursor_secondary: color_req(&raw.colors, "editor.cursor.secondary")?,
            editor_match: color_req(&raw.colors, "editor.match")?,
            editor_match_current: color_req(&raw.colors, "editor.match.current")?,
            editor_bracket_match: color_req(&raw.colors, "editor.bracket-match")?,
            editor_line_number: color_req(&raw.colors, "editor.line-number")?,
            editor_line_number_active: color_req(&raw.colors, "editor.line-number.active")?,
            editor_indent_guide: color_req(&raw.colors, "editor.indent-guide")?,
            editor_indent_guide_active: color_req(&raw.colors, "editor.indent-guide.active")?,
            editor_whitespace: color_req(&raw.colors, "editor.whitespace")?,
            editor_ruler: color_req(&raw.colors, "editor.ruler")?,
        };

        let syntax = ThemeSyntax {
            keyword: syntax_req(&raw.syntax, "keyword")?,
            string: syntax_req(&raw.syntax, "string")?,
            number: syntax_req(&raw.syntax, "number")?,
            constant: syntax_req(&raw.syntax, "constant")?,
            comment: syntax_req(&raw.syntax, "comment")?,
            function: syntax_req(&raw.syntax, "function")?,
            type_name: syntax_req(&raw.syntax, "type")?,
            class: syntax_req(&raw.syntax, "class")?,
            variable: syntax_req(&raw.syntax, "variable")?,
            parameter: syntax_req(&raw.syntax, "parameter")?,
            operator: syntax_req(&raw.syntax, "operator")?,
            punctuation: syntax_req(&raw.syntax, "punctuation")?,
            tag: syntax_req(&raw.syntax, "tag")?,
            attribute: syntax_req(&raw.syntax, "attribute")?,
            regex: syntax_req(&raw.syntax, "regex")?,
            escape: syntax_req(&raw.syntax, "escape")?,
            heading: syntax_req(&raw.syntax, "heading")?,
            link: syntax_req(&raw.syntax, "link")?,
            invalid: syntax_req(&raw.syntax, "invalid")?,
            diff_added: syntax_req(&raw.syntax, "diff.added")?,
            diff_removed: syntax_req(&raw.syntax, "diff.removed")?,
        };

        Ok(Self {
            id,
            name: raw.name,
            kind: raw.kind,
            colors,
            syntax,
        })
    }

    /// Parses a user theme TOML string, falling back per token to the built-in theme of the same `kind`.
    pub fn from_toml_str(id: ThemeId, raw_toml: &str) -> anyhow::Result<Self> {
        let raw: RawTheme = toml::from_str(raw_toml).context("parsing custom theme TOML")?;
        let base = match raw.kind {
            ThemeKind::Dark => Self::builtin_dark(),
            ThemeKind::Light => Self::builtin_light(),
            ThemeKind::HighContrast => Self::builtin_high_contrast(),
        };

        let mut colors = base.colors;
        apply_theme_colors(&raw.colors, &mut colors)?;

        let mut syntax = base.syntax;
        apply_theme_syntax(&raw.syntax, &mut syntax)?;

        Ok(Self {
            id,
            name: raw.name,
            kind: raw.kind,
            colors,
            syntax,
        })
    }

    /// Applies this theme's palette and widget styling to the `egui` context.
    pub fn apply_to_egui(&self, ctx: &egui::Context) {
        let is_light = self.kind == ThemeKind::Light;
        let mut visuals = if is_light {
            Visuals::light()
        } else {
            Visuals::dark()
        };

        visuals.dark_mode = !is_light;
        visuals.panel_fill = self.colors.bg_surface;
        visuals.window_fill = self.colors.bg_elevated;
        visuals.window_stroke = Stroke::new(1.0, self.colors.border_strong);
        visuals.window_corner_radius = CornerRadius::same(8);
        visuals.menu_corner_radius = CornerRadius::same(6);
        visuals.extreme_bg_color = self.colors.bg_input;
        visuals.faint_bg_color = self.colors.bg_hover;
        visuals.code_bg_color = self.colors.bg_elevated;
        visuals.hyperlink_color = self.colors.accent;
        visuals.warn_fg_color = self.colors.warning;
        visuals.error_fg_color = self.colors.danger;
        visuals.override_text_color = Some(self.colors.text_primary);
        visuals.selection.bg_fill = self.colors.editor_selection;
        visuals.selection.stroke = Stroke::new(1.0, self.colors.border_focus);

        visuals.widgets.noninteractive.bg_fill = self.colors.bg_surface;
        visuals.widgets.noninteractive.weak_bg_fill = self.colors.bg_surface;
        visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, self.colors.border_subtle);
        visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, self.colors.text_primary);
        visuals.widgets.noninteractive.corner_radius = CornerRadius::same(4);

        visuals.widgets.inactive.bg_fill = self.colors.bg_hover;
        visuals.widgets.inactive.weak_bg_fill = self.colors.bg_hover;
        visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, self.colors.border_subtle);
        visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, self.colors.text_primary);
        visuals.widgets.inactive.corner_radius = CornerRadius::same(4);

        visuals.widgets.hovered.bg_fill = self.colors.bg_active;
        visuals.widgets.hovered.weak_bg_fill = self.colors.bg_hover;
        visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, self.colors.border_strong);
        visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, self.colors.text_primary);
        visuals.widgets.hovered.corner_radius = CornerRadius::same(4);

        visuals.widgets.active.bg_fill = self.colors.accent;
        visuals.widgets.active.weak_bg_fill = self.colors.bg_active;
        visuals.widgets.active.bg_stroke = Stroke::new(1.5, self.colors.border_focus);
        visuals.widgets.active.fg_stroke = Stroke::new(1.0, self.colors.text_on_accent);
        visuals.widgets.active.corner_radius = CornerRadius::same(4);

        visuals.widgets.open.bg_fill = self.colors.bg_elevated;
        visuals.widgets.open.weak_bg_fill = self.colors.bg_hover;
        visuals.widgets.open.bg_stroke = Stroke::new(1.0, self.colors.border_focus);
        visuals.widgets.open.fg_stroke = Stroke::new(1.0, self.colors.text_primary);
        visuals.widgets.open.corner_radius = CornerRadius::same(4);

        let theme_target = if is_light {
            egui::Theme::Light
        } else {
            egui::Theme::Dark
        };
        ctx.set_theme(theme_target);
        ctx.set_visuals_of(theme_target, visuals);
    }
}

fn color_req(map: &HashMap<String, String>, key: &str) -> anyhow::Result<Color32> {
    let hex = map
        .get(key)
        .with_context(|| format!("missing color token `{key}`"))?;
    parse_hex_color(hex).with_context(|| format!("invalid hex color for `{key}`: {hex}"))
}

fn syntax_req(map: &HashMap<String, RawSyntaxStyle>, key: &str) -> anyhow::Result<SyntaxStyle> {
    let raw = map
        .get(key)
        .with_context(|| format!("missing syntax token `{key}`"))?;
    let fg = parse_hex_color(&raw.fg)
        .with_context(|| format!("invalid hex color for syntax `{key}`: {}", raw.fg))?;
    Ok(SyntaxStyle {
        fg,
        bold: raw.bold,
        italic: raw.italic,
        underline: raw.underline,
    })
}

fn apply_theme_colors(
    raw: &HashMap<String, String>,
    colors: &mut ThemeColors,
) -> anyhow::Result<()> {
    let fields: [(&str, &mut Color32); 35] = [
        ("bg.base", &mut colors.bg_base),
        ("bg.surface", &mut colors.bg_surface),
        ("bg.elevated", &mut colors.bg_elevated),
        ("bg.hover", &mut colors.bg_hover),
        ("bg.active", &mut colors.bg_active),
        ("bg.input", &mut colors.bg_input),
        ("border.subtle", &mut colors.border_subtle),
        ("border.strong", &mut colors.border_strong),
        ("border.focus", &mut colors.border_focus),
        ("text.primary", &mut colors.text_primary),
        ("text.secondary", &mut colors.text_secondary),
        ("text.muted", &mut colors.text_muted),
        ("text.disabled", &mut colors.text_disabled),
        ("text.on-accent", &mut colors.text_on_accent),
        ("accent", &mut colors.accent),
        ("accent.hover", &mut colors.accent_hover),
        ("accent.subtle", &mut colors.accent_subtle),
        ("success", &mut colors.success),
        ("warning", &mut colors.warning),
        ("danger", &mut colors.danger),
        ("info", &mut colors.info),
        ("editor.current-line", &mut colors.editor_current_line),
        ("editor.selection", &mut colors.editor_selection),
        (
            "editor.selection.inactive",
            &mut colors.editor_selection_inactive,
        ),
        ("editor.cursor", &mut colors.editor_cursor),
        (
            "editor.cursor.secondary",
            &mut colors.editor_cursor_secondary,
        ),
        ("editor.match", &mut colors.editor_match),
        ("editor.match.current", &mut colors.editor_match_current),
        ("editor.bracket-match", &mut colors.editor_bracket_match),
        ("editor.line-number", &mut colors.editor_line_number),
        (
            "editor.line-number.active",
            &mut colors.editor_line_number_active,
        ),
        ("editor.indent-guide", &mut colors.editor_indent_guide),
        (
            "editor.indent-guide.active",
            &mut colors.editor_indent_guide_active,
        ),
        ("editor.whitespace", &mut colors.editor_whitespace),
        ("editor.ruler", &mut colors.editor_ruler),
    ];
    for (key, dst) in fields {
        apply_color(raw, key, dst)?;
    }
    Ok(())
}

fn apply_theme_syntax(
    raw: &HashMap<String, RawSyntaxStyle>,
    syntax: &mut ThemeSyntax,
) -> anyhow::Result<()> {
    let fields: [(&str, &mut SyntaxStyle); 21] = [
        ("keyword", &mut syntax.keyword),
        ("string", &mut syntax.string),
        ("number", &mut syntax.number),
        ("constant", &mut syntax.constant),
        ("comment", &mut syntax.comment),
        ("function", &mut syntax.function),
        ("type", &mut syntax.type_name),
        ("class", &mut syntax.class),
        ("variable", &mut syntax.variable),
        ("parameter", &mut syntax.parameter),
        ("operator", &mut syntax.operator),
        ("punctuation", &mut syntax.punctuation),
        ("tag", &mut syntax.tag),
        ("attribute", &mut syntax.attribute),
        ("regex", &mut syntax.regex),
        ("escape", &mut syntax.escape),
        ("heading", &mut syntax.heading),
        ("link", &mut syntax.link),
        ("invalid", &mut syntax.invalid),
        ("diff.added", &mut syntax.diff_added),
        ("diff.removed", &mut syntax.diff_removed),
    ];
    for (key, dst) in fields {
        apply_syntax(raw, key, dst)?;
    }
    Ok(())
}

fn apply_color(map: &HashMap<String, String>, key: &str, dst: &mut Color32) -> anyhow::Result<()> {
    if let Some(hex) = map.get(key) {
        *dst = parse_hex_color(hex)
            .with_context(|| format!("invalid hex color for `{key}`: {hex}"))?;
    }
    Ok(())
}

fn apply_syntax(
    map: &HashMap<String, RawSyntaxStyle>,
    key: &str,
    dst: &mut SyntaxStyle,
) -> anyhow::Result<()> {
    if let Some(raw) = map.get(key) {
        let fg = parse_hex_color(&raw.fg)
            .with_context(|| format!("invalid hex color for syntax `{key}`: {}", raw.fg))?;
        *dst = SyntaxStyle {
            fg,
            bold: raw.bold,
            italic: raw.italic,
            underline: raw.underline,
        };
    }
    Ok(())
}

/// Parses `#RRGGBB` or `#RRGGBBAA` into an unmultiplied `Color32`.
pub fn parse_hex_color(hex: &str) -> anyhow::Result<Color32> {
    let trimmed = hex.trim();
    let digits = trimmed
        .strip_prefix('#')
        .with_context(|| format!("hex color `{trimmed}` must start with `#`"))?;
    match digits.len() {
        6 => {
            let r = u8::from_str_radix(&digits[0..2], 16)?;
            let g = u8::from_str_radix(&digits[2..4], 16)?;
            let b = u8::from_str_radix(&digits[4..6], 16)?;
            Ok(Color32::from_rgb(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&digits[0..2], 16)?;
            let g = u8::from_str_radix(&digits[2..4], 16)?;
            let b = u8::from_str_radix(&digits[4..6], 16)?;
            let a = u8::from_str_radix(&digits[6..8], 16)?;
            Ok(Color32::from_rgba_unmultiplied(r, g, b, a))
        }
        _ => bail!("expected #RRGGBB or #RRGGBBAA, got `{trimmed}`"),
    }
}

/// Computes the WCAG 2.1 relative luminance of an opaque sRGB `Color32`.
#[must_use]
pub fn relative_luminance(color: Color32) -> f64 {
    fn channel(c: u8) -> f64 {
        let s = f64::from(c) / 255.0;
        if s <= 0.03928 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

/// Computes the WCAG 2.1 contrast ratio between two opaque colors (range `1.0..=21.0`).
#[must_use]
pub fn contrast_ratio(fg: Color32, bg: Color32) -> f64 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (bright, dark) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
    (bright + 0.05) / (dark + 0.05)
}
