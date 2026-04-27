use eframe::egui::{
    self, vec2, Button, Color32, Context, CornerRadius, Frame, Margin, RichText, Stroke, Visuals,
};

pub const WINDOW_BG: Color32 = Color32::from_rgb(7, 8, 10);
pub const SURFACE_BG: Color32 = Color32::from_rgb(18, 20, 24);
pub const SURFACE_BG_HOVERED: Color32 = Color32::from_rgb(30, 34, 42);
pub const SURFACE_BG_ACTIVE: Color32 = Color32::from_rgb(42, 52, 70);
pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(105, 116, 132);
pub const BORDER_STRONG: Color32 = Color32::from_rgb(220, 228, 246);
pub const ACCENT: Color32 = Color32::from_rgb(255, 99, 99);
pub const ACCENT_SOFT: Color32 = Color32::from_rgb(255, 142, 142);
pub const INFO: Color32 = Color32::from_rgb(85, 179, 255);
pub const TEXT: Color32 = Color32::from_rgb(249, 249, 249);
pub const TEXT_MUTED: Color32 = Color32::from_rgb(206, 206, 206);
pub const TEXT_SUBTLE: Color32 = Color32::from_rgb(156, 156, 157);
pub const SUCCESS: Color32 = Color32::from_rgb(95, 201, 127);
pub const WARNING: Color32 = Color32::from_rgb(239, 191, 76);
pub const DANGER: Color32 = Color32::from_rgb(255, 145, 145);

pub const PANEL_PADDING: Margin = Margin::same(16);
pub const CARD_PADDING: Margin = Margin::same(14);
pub const BADGE_PADDING: Margin = Margin::symmetric(8, 3);

pub const BUTTON_HEIGHT: f32 = 30.0;
pub const TAB_HEIGHT: f32 = 28.0;

pub const WINDOW_RADIUS: u8 = 26;
pub const PANEL_RADIUS: u8 = 18;
pub const CARD_RADIUS: u8 = 14;
pub const BUTTON_RADIUS: u8 = 14;
pub const TAB_RADIUS: u8 = 14;
pub const BADGE_RADIUS: u8 = 12;

pub const SECTION_TITLE_SIZE: f32 = 16.0;
pub const BODY_SIZE: f32 = 14.5;
pub const META_SIZE: f32 = 12.5;
pub const BADGE_TEXT_SIZE: f32 = 11.0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BadgeTone {
    Neutral,
    Accent,
    Success,
    Warning,
    Danger,
}

impl BadgeTone {
    pub fn fill(self) -> Color32 {
        match self {
            BadgeTone::Neutral => SURFACE_BG,
            BadgeTone::Accent => ACCENT,
            BadgeTone::Success => SUCCESS,
            BadgeTone::Warning => WARNING,
            BadgeTone::Danger => DANGER,
        }
    }

    pub fn stroke(self) -> Color32 {
        match self {
            BadgeTone::Neutral => BORDER_SUBTLE,
            BadgeTone::Accent => ACCENT_SOFT,
            BadgeTone::Success => SUCCESS,
            BadgeTone::Warning => WARNING,
            BadgeTone::Danger => DANGER,
        }
    }

    pub fn text(self) -> Color32 {
        match self {
            BadgeTone::Neutral => TEXT_MUTED,
            BadgeTone::Accent => TEXT,
            BadgeTone::Success => WINDOW_BG,
            BadgeTone::Warning => WINDOW_BG,
            BadgeTone::Danger => WINDOW_BG,
        }
    }
}

pub fn apply_launcher_theme(ctx: &Context) {
    ctx.set_visuals(launcher_visuals());
}

pub fn launcher_visuals() -> Visuals {
    let mut visuals = Visuals::dark();
    visuals.panel_fill = Color32::TRANSPARENT;
    visuals.extreme_bg_color = Color32::TRANSPARENT;
    visuals.widgets.noninteractive.bg_fill = WINDOW_BG;
    visuals.widgets.noninteractive.weak_bg_fill = SURFACE_BG;
    visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    visuals.widgets.noninteractive.corner_radius = CornerRadius::same(WINDOW_RADIUS);
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, TEXT);

    visuals.widgets.inactive.bg_fill = SURFACE_BG;
    visuals.widgets.inactive.weak_bg_fill = SURFACE_BG;
    visuals.widgets.inactive.bg_stroke = Stroke::new(1.0, BORDER_SUBTLE);
    visuals.widgets.inactive.corner_radius = CornerRadius::same(BUTTON_RADIUS);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0, TEXT);

    visuals.widgets.hovered.bg_fill = SURFACE_BG_HOVERED;
    visuals.widgets.hovered.weak_bg_fill = SURFACE_BG_HOVERED;
    visuals.widgets.hovered.bg_stroke = Stroke::new(1.0, BORDER_STRONG);
    visuals.widgets.hovered.corner_radius = CornerRadius::same(BUTTON_RADIUS);
    visuals.widgets.hovered.fg_stroke = Stroke::new(1.0, TEXT);

    visuals.widgets.active.bg_fill = SURFACE_BG_ACTIVE;
    visuals.widgets.active.weak_bg_fill = SURFACE_BG_ACTIVE;
    visuals.widgets.active.bg_stroke = Stroke::new(1.0, ACCENT_SOFT);
    visuals.widgets.active.corner_radius = CornerRadius::same(BUTTON_RADIUS);
    visuals.widgets.active.fg_stroke = Stroke::new(1.0, TEXT);

    visuals.selection.bg_fill = Color32::from_rgba_unmultiplied(255, 99, 99, 80);
    visuals.selection.stroke = Stroke::new(1.0, INFO);
    visuals.override_text_color = None;
    visuals
}

pub fn launcher_shell_frame() -> Frame {
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(7, 8, 10, 188))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 28),
        ))
        .corner_radius(CornerRadius::same(WINDOW_RADIUS))
        .inner_margin(Margin::same(16))
}

pub fn search_shell_frame() -> Frame {
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(12, 14, 18, 214))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 44),
        ))
        .corner_radius(CornerRadius::same(26))
        .inner_margin(Margin::symmetric(20, 13))
}

pub fn panel_frame() -> Frame {
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(16, 17, 19, 150))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 22),
        ))
        .corner_radius(CornerRadius::same(PANEL_RADIUS))
        .inner_margin(PANEL_PADDING)
}

pub fn card_frame(selected: bool) -> Frame {
    // Translucent cards (mac-like): let the backdrop material show through a bit,
    // but keep text readable.
    let fill = if selected {
        Color32::from_rgba_unmultiplied(255, 99, 99, 38)
    } else {
        Color32::from_rgba_unmultiplied(24, 26, 31, 92)
    };

    let stroke = if selected {
        Color32::from_rgba_unmultiplied(255, 130, 130, 92)
    } else {
        Color32::from_rgba_unmultiplied(255, 255, 255, 24)
    };

    Frame::new()
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
        .corner_radius(CornerRadius::same(CARD_RADIUS))
        .inner_margin(CARD_PADDING)
}

pub fn icon_badge_frame() -> Frame {
    Frame::new()
        .fill(Color32::from_rgba_unmultiplied(255, 255, 255, 28))
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 58),
        ))
        .corner_radius(CornerRadius::same(15))
        .inner_margin(Margin::symmetric(10, 8))
}

pub fn search_icon_text() -> RichText {
    RichText::new("⌘")
        .size(20.0)
        .strong()
        .color(Color32::from_rgba_unmultiplied(255, 255, 255, 238))
}

pub fn badge_frame(tone: BadgeTone) -> Frame {
    Frame::new()
        .fill(tone.fill().gamma_multiply(0.85))
        .stroke(Stroke::new(1.0, tone.stroke().gamma_multiply(0.8)))
        .corner_radius(CornerRadius::same(BADGE_RADIUS))
        .inner_margin(BADGE_PADDING)
}

pub fn ghost_button<'a, T>(label: T) -> Button<'a>
where
    T: egui::IntoAtoms<'a>,
{
    Button::new(label)
        .min_size(vec2(0.0, BUTTON_HEIGHT))
        .corner_radius(CornerRadius::same(BUTTON_RADIUS))
        .frame(true)
}

pub fn action_button<'a, T>(label: T) -> Button<'a>
where
    T: egui::IntoAtoms<'a>,
{
    Button::new(label)
        .min_size(vec2(92.0, BUTTON_HEIGHT))
        .corner_radius(CornerRadius::same(BUTTON_RADIUS))
        .fill(ACCENT)
        .stroke(Stroke::new(1.0, ACCENT_SOFT))
}

pub fn tab_button<'a, T>(label: T, selected: bool) -> Button<'a>
where
    T: egui::IntoAtoms<'a>,
{
    let fill = if selected { ACCENT } else { SURFACE_BG };
    let stroke = if selected { ACCENT_SOFT } else { BORDER_SUBTLE };

    Button::new(label)
        .selected(selected)
        .min_size(vec2(0.0, TAB_HEIGHT))
        .corner_radius(CornerRadius::same(TAB_RADIUS))
        .fill(fill)
        .stroke(Stroke::new(1.0, stroke))
}

pub fn section_text(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(SECTION_TITLE_SIZE)
        .strong()
        .color(TEXT)
}

pub fn body_text(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).size(BODY_SIZE).color(TEXT)
}

pub fn muted_text(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(META_SIZE)
        .color(TEXT_SUBTLE)
}

#[allow(dead_code)]
pub fn shortcut_chip(ui: &mut egui::Ui, text: &str) {
    badge_frame(BadgeTone::Neutral).show(ui, |ui| {
        ui.label(muted_text(text));
    });
}

pub fn badge_text(text: impl Into<String>, tone: BadgeTone) -> RichText {
    RichText::new(text.into())
        .size(BADGE_TEXT_SIZE)
        .strong()
        .color(tone.text())
}

pub fn status_text(text: impl Into<String>, tone: BadgeTone) -> RichText {
    RichText::new(text.into())
        .size(META_SIZE)
        .color(tone.text())
}

#[allow(dead_code)]
pub fn placeholder_text(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(21.0)
        .color(Color32::from_rgba_unmultiplied(249, 249, 249, 170))
}

pub fn premium_hint_text(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(12.0)
        .strong()
        .color(Color32::from_rgba_unmultiplied(206, 206, 206, 150))
}

#[allow(dead_code)]
pub fn shortcut_key_text(text: impl Into<String>) -> RichText {
    RichText::new(text.into())
        .size(11.5)
        .strong()
        .color(Color32::from_rgba_unmultiplied(249, 249, 249, 210))
}
