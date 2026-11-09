//! Colours, fonts and small vector icons, matched to the reference screenshots
//! of the original editor (Breeze Light chrome, Hack for code).

use std::sync::Arc;

use eframe::egui::{
    self, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, Rect, Stroke, TextStyle, Vec2,
};

// ---------------------------------------------------------------- colours ---

pub const TITLE_BG: Color32 = Color32::from_rgb(0x29, 0x2c, 0x30);
pub const TITLE_FG: Color32 = Color32::from_rgb(0xf0, 0xf0, 0xf0);
pub const MENU_BG: Color32 = Color32::from_rgb(0xde, 0xe0, 0xe2);
pub const CHROME_BG: Color32 = Color32::from_rgb(0xef, 0xf0, 0xf1);
pub const CHROME_HOVER: Color32 = Color32::from_rgb(0xdc, 0xdd, 0xdf);
pub const CHROME_PRESSED: Color32 = Color32::from_rgb(0xc8, 0xca, 0xcc);
pub const BREADCRUMB_BG: Color32 = Color32::from_rgb(0xf8, 0xf7, 0xf6);
pub const RAIL_BG: Color32 = Color32::from_rgb(0xf5, 0xf5, 0xf6);
pub const TEXT: Color32 = Color32::from_rgb(0x23, 0x26, 0x29);
pub const TEXT_DIM: Color32 = Color32::from_rgb(0x6a, 0x6d, 0x70);
pub const TEXT_DISABLED: Color32 = Color32::from_rgb(0xa5, 0xa8, 0xab);
pub const BORDER: Color32 = Color32::from_rgb(0xc9, 0xcb, 0xcd);
pub const SEP: Color32 = Color32::from_rgb(0xd5, 0xd5, 0xd5);
pub const EDITOR_BG: Color32 = Color32::WHITE;
pub const GUTTER_BG: Color32 = Color32::from_rgb(0xf0, 0xf0, 0xf0);
pub const GUTTER_FG: Color32 = Color32::from_rgb(0x9d, 0x9d, 0x9d);
pub const CUR_LINE: Color32 = Color32::from_rgb(0xf6, 0xf6, 0xf6);
pub const SEL: Color32 = Color32::from_rgb(0xb5, 0xd9, 0xf2);
pub const ACCENT: Color32 = Color32::from_rgb(0x3d, 0xae, 0xe9);
pub const STATUS_BG: Color32 = Color32::from_rgb(0xf2, 0xf2, 0xf1);
pub const MATCH_BG: Color32 = Color32::from_rgb(0xff, 0xf2, 0xa8);

// Syntax colours sampled from the original's light colour scheme.
pub const SYN_KEYWORD: Color32 = Color32::from_rgb(0xff, 0x55, 0x00);
pub const SYN_TYPE: Color32 = Color32::from_rgb(0x64, 0x4a, 0x9b);
pub const SYN_STRING: Color32 = Color32::from_rgb(0xbf, 0x03, 0x03);
pub const SYN_COMMENT: Color32 = Color32::from_rgb(0x89, 0x88, 0x87);
pub const SYN_FUNC: Color32 = Color32::from_rgb(0x00, 0x57, 0xae);
pub const SYN_NUMBER: Color32 = Color32::from_rgb(0x64, 0x4a, 0x9b);

// --------------------------------------------------------------- palettes ---

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub dark: bool,
}

impl Palette {
    pub fn light() -> Self {
        Self { dark: false }
    }

    pub fn editor_bg(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0x23, 0x26, 0x29)
        } else {
            EDITOR_BG
        }
    }

    pub fn text(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0xe3, 0xe5, 0xe6)
        } else {
            Color32::from_rgb(0x1f, 0x1c, 0x1b)
        }
    }

    pub fn gutter_bg(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0x1b, 0x1e, 0x20)
        } else {
            GUTTER_BG
        }
    }

    pub fn gutter_fg(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0x6d, 0x72, 0x76)
        } else {
            GUTTER_FG
        }
    }

    pub fn cur_line(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0x2c, 0x30, 0x34)
        } else {
            CUR_LINE
        }
    }

    pub fn sel(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0x2b, 0x5c, 0x80)
        } else {
            SEL
        }
    }

    pub fn match_bg(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0x60, 0x55, 0x1b)
        } else {
            MATCH_BG
        }
    }

    pub fn keyword(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0xff, 0x8f, 0x4d)
        } else {
            SYN_KEYWORD
        }
    }

    pub fn type_(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0xb0, 0x9a, 0xe0)
        } else {
            SYN_TYPE
        }
    }

    pub fn string(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0xe6, 0x7e, 0x80)
        } else {
            SYN_STRING
        }
    }

    pub fn comment(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0x7d, 0x85, 0x8b)
        } else {
            SYN_COMMENT
        }
    }

    pub fn func(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0x6f, 0xb1, 0xf0)
        } else {
            SYN_FUNC
        }
    }

    // ---- window chrome (menus, toolbar, panels, status) ----
    pub fn title_bg(&self) -> Color32 {
        if self.dark { Color32::from_rgb(0x1b, 0x1e, 0x20) } else { TITLE_BG }
    }

    pub fn menu_bg(&self) -> Color32 {
        if self.dark { Color32::from_rgb(0x2a, 0x2d, 0x31) } else { MENU_BG }
    }

    pub fn chrome(&self) -> Color32 {
        if self.dark { Color32::from_rgb(0x23, 0x26, 0x29) } else { CHROME_BG }
    }

    pub fn chrome_hover(&self) -> Color32 {
        if self.dark { Color32::from_rgb(0x34, 0x38, 0x3d) } else { CHROME_HOVER }
    }

    pub fn rail_bg(&self) -> Color32 {
        if self.dark { Color32::from_rgb(0x1f, 0x22, 0x25) } else { RAIL_BG }
    }

    pub fn breadcrumb_bg(&self) -> Color32 {
        if self.dark { Color32::from_rgb(0x26, 0x29, 0x2d) } else { BREADCRUMB_BG }
    }

    pub fn status_bg(&self) -> Color32 {
        if self.dark { Color32::from_rgb(0x1f, 0x22, 0x25) } else { STATUS_BG }
    }

    pub fn border(&self) -> Color32 {
        if self.dark { Color32::from_rgb(0x3a, 0x3e, 0x42) } else { BORDER }
    }

    pub fn sep(&self) -> Color32 {
        if self.dark { Color32::from_rgb(0x33, 0x37, 0x3b) } else { SEP }
    }

    pub fn text_dim(&self) -> Color32 {
        if self.dark { Color32::from_rgb(0x9a, 0xa0, 0xa5) } else { TEXT_DIM }
    }

    pub fn text_disabled(&self) -> Color32 {
        if self.dark { Color32::from_rgb(0x64, 0x69, 0x6e) } else { TEXT_DISABLED }
    }

    pub fn text_main(&self) -> Color32 {
        self.text()
    }

    /// Background of text fields drawn by hand on the chrome.
    pub fn field_bg(&self) -> Color32 {
        if self.dark { Color32::from_rgb(0x2c, 0x30, 0x34) } else { Color32::WHITE }
    }

    pub fn number(&self) -> Color32 {
        if self.dark {
            Color32::from_rgb(0xc7, 0xa9, 0xff)
        } else {
            SYN_NUMBER
        }
    }
}

// ------------------------------------------------------------------ fonts ---

pub const MONO: &str = "mono";
pub const MONO_BOLD: &str = "mono-bold";
pub const MONO_ITALIC: &str = "mono-italic";
pub const MONO_BOLD_ITALIC: &str = "mono-bold-italic";

pub fn mono_family(bold: bool, italic: bool) -> FontFamily {
    match (bold, italic) {
        (true, true) => FontFamily::Name(MONO_BOLD_ITALIC.into()),
        (true, false) => FontFamily::Name(MONO_BOLD.into()),
        (false, true) => FontFamily::Name(MONO_ITALIC.into()),
        (false, false) => FontFamily::Monospace,
    }
}

pub fn mono_font(size: f32, bold: bool, italic: bool) -> FontId {
    FontId::new(size, mono_family(bold, italic))
}

fn try_read(path: &str) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

/// Register the system fonts that the original uses (Noto Sans for chrome,
/// Hack for code). Falls back to egui's built-in fonts when unavailable.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let wanted: [(&str, &[&str]); 6] = [
        (
            "noto",
            &[
                "/usr/share/fonts/noto/NotoSans-Regular.ttf",
                "/usr/share/fonts/TTF/NotoSans-Regular.ttf",
            ],
        ),
        (
            "noto-bold",
            &[
                "/usr/share/fonts/noto/NotoSans-Bold.ttf",
                "/usr/share/fonts/TTF/NotoSans-Bold.ttf",
            ],
        ),
        (
            MONO,
            &[
                "/usr/share/fonts/TTF/Hack-Regular.ttf",
                "/usr/share/fonts/truetype/Hack-Regular.ttf",
            ],
        ),
        (
            MONO_BOLD,
            &[
                "/usr/share/fonts/TTF/Hack-Bold.ttf",
                "/usr/share/fonts/truetype/Hack-Bold.ttf",
            ],
        ),
        (
            MONO_ITALIC,
            &[
                "/usr/share/fonts/TTF/Hack-Italic.ttf",
                "/usr/share/fonts/TTF/Hack-BoldItalic.ttf",
            ],
        ),
        (
            MONO_BOLD_ITALIC,
            &["/usr/share/fonts/TTF/Hack-BoldItalic.ttf"],
        ),
    ];

    for (name, paths) in wanted {
        for path in paths {
            if let Some(bytes) = try_read(path) {
                fonts
                    .font_data
                    .insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
                break;
            }
        }
    }

    let has = |fonts: &FontDefinitions, k: &str| fonts.font_data.contains_key(k);

    if has(&fonts, "noto") {
        fonts
            .families
            .entry(FontFamily::Proportional)
            .or_default()
            .insert(0, "noto".to_owned());
    }
    for (family, key) in [
        (MONO_BOLD, MONO_BOLD),
        (MONO_ITALIC, MONO_ITALIC),
        (MONO_BOLD_ITALIC, MONO_BOLD_ITALIC),
        (MONO, MONO),
    ] {
        if has(&fonts, key) {
            let mut list = egui::epaint::text::FontDefinitions::default()
                .families
                .get(&FontFamily::Monospace)
                .cloned()
                .unwrap_or_default();
            list.insert(0, key.to_owned());
            fonts
                .families
                .insert(FontFamily::Name(family.into()), list);
        }
    }
    if has(&fonts, "noto-bold") {
        let mut list = egui::epaint::text::FontDefinitions::default()
            .families
            .get(&FontFamily::Proportional)
            .cloned()
            .unwrap_or_default();
        list.insert(0, "noto-bold".to_owned());
        fonts
            .families
            .insert(FontFamily::Name("bold".into()), list);
    }

    ctx.set_fonts(fonts);
}

pub fn apply_style(ctx: &egui::Context, pal: Palette) {
    ctx.all_styles_mut(|style| {
    style.text_styles = [
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
        (
            TextStyle::Heading,
            FontId::new(16.0, FontFamily::Name("bold".into())),
        ),
        (TextStyle::Monospace, FontId::new(12.5, FontFamily::Monospace)),
    ]
    .into();

    let mut v = if pal.dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    v.override_text_color = Some(pal.text_main());
    v.panel_fill = pal.chrome();
    v.window_fill = if pal.dark {
        Color32::from_rgb(0x2c, 0x30, 0x34)
    } else {
        Color32::WHITE
    };
    v.window_stroke = Stroke::new(1.0_f32, pal.border());
    v.menu_corner_radius = egui::CornerRadius::same(3);
    v.window_corner_radius = egui::CornerRadius::same(4);
    v.selection.bg_fill = pal.sel();
    v.selection.stroke = Stroke::new(1.0_f32, Color32::from_rgb(0x2a, 0x76, 0xa8));
    v.widgets.noninteractive.bg_fill = pal.chrome();
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, pal.border());
    v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    v.widgets.inactive.bg_stroke = Stroke::NONE;
    v.widgets.hovered.bg_fill = pal.chrome_hover();
    v.widgets.hovered.weak_bg_fill = pal.chrome_hover();
    v.widgets.hovered.bg_stroke = Stroke::NONE;
    v.widgets.active.bg_fill = pal.chrome_hover();
    v.widgets.active.weak_bg_fill = pal.chrome_hover();
    v.widgets.active.bg_stroke = Stroke::NONE;
    v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, pal.text_main());
    v.widgets.hovered.fg_stroke = Stroke::new(1.0_f32, pal.text_main());
    v.widgets.active.fg_stroke = Stroke::new(1.0_f32, pal.text_main());
    v.button_frame = false;
    style.visuals = v;

    style.spacing.item_spacing = Vec2::new(6.0, 4.0);
    style.spacing.button_padding = Vec2::new(6.0, 3.0);
    style.spacing.menu_margin = egui::Margin::same(4);
    style.spacing.scroll.bar_width = 10.0;
    style.spacing.scroll.bar_inner_margin = 2.0;
    });
}

// ------------------------------------------------------------------ icons ---

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    NewFile,
    Folder,
    Save,
    SaveAs,
    Undo,
    Redo,
    Search,
    Close,
    ChevronLeft,
    ChevronRight,
    ChevronDown,
    Doc,
    List,
    Diamond,
    Plug,
    Warning,
    Terminal,
    Output,
    Grid,
    Menu,
    ArrowUp,
    ArrowDown,
    Sort,
    Split,
    Py,
    FolderOpen,
    File,
    Bold,
    Check,
}

fn pt(rect: Rect, x: f32, y: f32) -> Pos2 {
    rect.min + Vec2::new(rect.width() * x / 16.0, rect.height() * y / 16.0)
}

/// Draw a 16x16 design-space icon into `rect`.
pub fn paint_icon(p: &egui::Painter, icon: Icon, rect: Rect, color: Color32) {
    let s = (rect.width().min(rect.height()) / 16.0).max(0.6);
    let stroke = Stroke::new((1.25 * s).max(1.0), color);
    let thin = Stroke::new((1.0 * s).max(0.8), color);
    let r = |x: f32, y: f32, w: f32, h: f32| {
        Rect::from_min_max(pt(rect, x, y), pt(rect, x + w, y + h))
    };
    let line = |a: (f32, f32), b: (f32, f32)| {
        p.line_segment([pt(rect, a.0, a.1), pt(rect, b.0, b.1)], stroke);
    };

    match icon {
        Icon::NewFile => {
            p.rect_stroke(
                r(3.0, 1.2, 9.0, 13.6),
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Inside,
            );
            line((5.5, 5.0), (9.5, 5.0));
            line((5.5, 7.4), (8.0, 7.4));
            line((10.0, 9.6), (10.0, 13.4));
            line((8.2, 11.5), (11.8, 11.5));
        }
        Icon::File | Icon::Doc => {
            p.rect_stroke(
                r(3.0, 1.2, 9.0, 13.6),
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Inside,
            );
            line((5.4, 5.0), (9.6, 5.0));
            line((5.4, 7.4), (9.6, 7.4));
            line((5.4, 9.8), (8.2, 9.8));
        }
        Icon::Folder | Icon::FolderOpen => {
            p.rect_stroke(
                r(1.6, 3.2, 12.8, 9.6),
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Inside,
            );
            line((1.6, 5.2), (5.6, 5.2));
            line((3.0, 3.2), (3.0, 5.2));
            if icon == Icon::FolderOpen {
                line((1.6, 12.8), (4.6, 8.0));
            }
        }
        Icon::Save | Icon::SaveAs => {
            p.rect_stroke(
                r(1.8, 2.4, 12.4, 11.6),
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                r(4.6, 2.4, 6.8, 4.4),
                egui::CornerRadius::same(1),
                thin,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                r(4.4, 9.6, 7.2, 4.4),
                egui::CornerRadius::same(1),
                thin,
                egui::StrokeKind::Inside,
            );
            if icon == Icon::SaveAs {
                line((10.4, 13.6), (14.6, 9.4));
                p.circle_filled(pt(rect, 10.0, 14.0), 1.5 * s, color);
            }
        }
        Icon::Undo | Icon::Redo => {
            let flip = icon == Icon::Redo;
            let x = |v: f32| if flip { 16.0 - v } else { v };
            let mut pts = Vec::new();
            for i in 0..=10 {
                let t = i as f32 / 10.0;
                let ang = std::f32::consts::PI * (1.0 - t * 0.85);
                let cx = 8.6 + 4.4 * ang.cos();
                let cy = 9.4 - 4.0 * ang.sin();
                pts.push(pt(rect, x(cx), cy));
            }
            for w in pts.windows(2) {
                p.line_segment([w[0], w[1]], stroke);
            }
            let tip = pt(rect, x(4.2), 9.4);
            p.line_segment([tip, pt(rect, x(3.0), 5.6)], stroke);
            p.line_segment([tip, pt(rect, x(7.4), 6.4)], stroke);
        }
        Icon::Search => {
            p.circle_stroke(pt(rect, 6.8, 6.8), 4.4 * s, stroke);
            p.line_segment([pt(rect, 10.2, 10.2), pt(rect, 14.0, 14.0)], stroke);
        }
        Icon::Close => {
            line((4.2, 4.2), (11.8, 11.8));
            line((11.8, 4.2), (4.2, 11.8));
        }
        Icon::ChevronLeft => {
            line((10.0, 3.4), (5.6, 8.0));
            line((5.6, 8.0), (10.0, 12.6));
        }
        Icon::ChevronRight => {
            line((6.0, 3.4), (10.4, 8.0));
            line((10.4, 8.0), (6.0, 12.6));
        }
        Icon::ChevronDown => {
            line((3.6, 6.0), (8.0, 10.4));
            line((8.0, 10.4), (12.4, 6.0));
        }
        Icon::ArrowUp => {
            line((8.0, 13.0), (8.0, 3.6));
            line((4.0, 7.4), (8.0, 3.4));
            line((12.0, 7.4), (8.0, 3.4));
        }
        Icon::ArrowDown => {
            line((8.0, 3.0), (8.0, 12.4));
            line((4.0, 8.6), (8.0, 12.6));
            line((12.0, 8.6), (8.0, 12.6));
        }
        Icon::List => {
            p.rect_stroke(
                r(1.6, 2.6, 12.8, 10.8),
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Inside,
            );
            line((4.2, 5.6), (11.8, 5.6));
            line((4.2, 8.0), (11.8, 8.0));
            line((4.2, 10.4), (8.8, 10.4));
        }
        Icon::Diamond => {
            let c = pt(rect, 8.0, 8.0);
            let d = 6.0 * s;
            let pts = [
                c + Vec2::new(0.0, -d),
                c + Vec2::new(d, 0.0),
                c + Vec2::new(0.0, d),
                c + Vec2::new(-d, 0.0),
            ];
            p.add(egui::Shape::convex_polygon(
                pts.to_vec(),
                color,
                Stroke::NONE,
            ));
            p.line_segment(
                [c + Vec2::new(-1.6 * s, 0.0), c + Vec2::new(1.6 * s, 0.0)],
                Stroke::new((1.0 * s).max(0.8), Color32::from_rgb(0xf5, 0xf5, 0xf6)),
            );
        }
        Icon::Plug => {
            p.rect_stroke(
                r(3.4, 6.6, 9.2, 7.4),
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Inside,
            );
            line((5.6, 6.6), (5.6, 2.8));
            line((8.4, 6.6), (8.4, 2.8));
            line((6.4, 14.0), (9.6, 14.0));
        }
        Icon::Warning => {
            let pts = [
                pt(rect, 8.0, 1.6),
                pt(rect, 15.0, 14.2),
                pt(rect, 1.0, 14.2),
            ];
            p.add(egui::Shape::convex_polygon(
                pts.to_vec(),
                color,
                Stroke::NONE,
            ));
            p.line_segment([pt(rect, 8.0, 5.6), pt(rect, 8.0, 9.8)], Stroke::new(1.8 * s, Color32::WHITE));
            p.circle_filled(pt(rect, 8.0, 12.0), 0.9 * s, Color32::WHITE);
        }
        Icon::Terminal => {
            p.rect_stroke(
                r(1.4, 2.6, 13.2, 10.8),
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Inside,
            );
            line((4.0, 6.6), (6.4, 8.6));
            line((6.4, 8.6), (4.0, 10.6));
            line((8.4, 10.8), (11.6, 10.8));
        }
        Icon::Output => {
            p.rect_stroke(
                r(1.4, 2.4, 13.2, 9.6),
                egui::CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Inside,
            );
            line((4.4, 12.0), (3.0, 14.2));
            line((3.0, 14.2), (6.6, 12.0));
        }
        Icon::Grid => {
            p.rect_stroke(
                r(2.0, 2.0, 5.2, 5.2),
                egui::CornerRadius::same(1),
                thin,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                r(8.8, 2.0, 5.2, 5.2),
                egui::CornerRadius::same(1),
                thin,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                r(2.0, 8.8, 5.2, 5.2),
                egui::CornerRadius::same(1),
                thin,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                r(8.8, 8.8, 5.2, 5.2),
                egui::CornerRadius::same(1),
                thin,
                egui::StrokeKind::Inside,
            );
        }
        Icon::Menu => {
            line((2.6, 4.0), (13.4, 4.0));
            line((2.6, 8.0), (13.4, 8.0));
            line((2.6, 12.0), (13.4, 12.0));
        }
        Icon::Sort => {
            line((4.6, 2.6), (4.6, 12.6));
            line((2.4, 10.4), (4.6, 12.8));
            line((6.8, 10.4), (4.6, 12.8));
            line((9.8, 3.4), (13.8, 3.4));
            line((9.8, 7.0), (12.6, 7.0));
            line((9.8, 10.6), (13.8, 10.6));
        }
        Icon::Split => {
            p.rect_stroke(
                r(1.6, 2.4, 12.8, 11.2),
                egui::CornerRadius::same(1),
                stroke,
                egui::StrokeKind::Inside,
            );
            line((8.0, 2.4), (8.0, 13.6));
        }
        Icon::Py => {
            p.circle_filled(pt(rect, 8.0, 8.0), 6.0 * s, Color32::from_rgb(0x2f, 0x6f, 0x9e));
            p.circle_filled(pt(rect, 6.6, 6.6), 2.2 * s, Color32::from_rgb(0xf6, 0xc9, 0x45));
            p.circle_filled(pt(rect, 9.6, 9.6), 1.7 * s, Color32::from_rgb(0xf5, 0xf5, 0xf6));
        }
        Icon::Bold => {
            p.rect_filled(r(4.6, 3.0, 2.6, 10.0), egui::CornerRadius::same(1), color);
            p.rect_stroke(
                r(7.0, 3.0, 5.0, 4.6),
                egui::CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Inside,
            );
            p.rect_stroke(
                r(7.0, 8.2, 5.6, 4.8),
                egui::CornerRadius::same(2),
                stroke,
                egui::StrokeKind::Inside,
            );
        }
        Icon::Check => {
            line((3.0, 8.4), (6.6, 12.0));
            line((6.6, 12.0), (13.0, 4.4));
        }
    }
}
