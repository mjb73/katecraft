//! The text view: gutter with line numbers, syntax-coloured text, caret,
//! selection, current-line band, find highlights, scrollbars and a minimap.

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use egui::text::{CCursor, LayoutJob, TextFormat};

use crate::doc::{Document, Lang, Span, Tok};
use crate::theme::{self, mono_font, Palette};

pub struct EditorStyle {
    pub font_size: f32,
    pub show_line_numbers: bool,
    pub show_minimap: bool,
    pub tab_width: usize,
}

impl Default for EditorStyle {
    fn default() -> Self {
        Self {
            font_size: 12.5,
            show_line_numbers: true,
            show_minimap: true,
            tab_width: 4,
        }
    }
}

pub struct Outcome {
    pub changed: bool,
    pub clicked: bool,
    /// A clipboard/selection command requested by the editor (keyboard
    /// shortcut or right-click menu) that the app should apply.
    pub edit: Option<EditAction>,
}

/// Clipboard and selection commands the editor can request but cannot carry
/// out itself (the app owns the system clipboard and the menu actions).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EditAction {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

fn token_style(pal: Palette, t: Tok) -> (Color32, bool, bool) {
    match t {
        Tok::Keyword => (pal.keyword(), true, false),
        Tok::Type => (pal.type_(), true, false),
        Tok::Str => (pal.string(), false, false),
        Tok::Comment => (pal.comment(), false, true),
        Tok::Func => (pal.func(), false, false),
        Tok::Num => (pal.number(), false, false),
        Tok::Decorator => (pal.func(), false, true),
        Tok::Heading => (pal.func(), true, false),
    }
}

fn expand_tabs(line: &str, tab: usize) -> String {
    let mut out = String::with_capacity(line.len() + 8);
    let mut col = 0usize;
    for c in line.chars() {
        if c == '\t' {
            let n = tab - (col % tab);
            for _ in 0..n {
                out.push(' ');
            }
            col += n;
        } else {
            out.push(c);
            col += 1;
        }
    }
    out
}

/// Display (expanded) character index for a byte offset inside `line`.
fn disp_index(line: &str, byte: usize, tab: usize) -> usize {
    let mut col = 0usize;
    for (i, c) in line.char_indices() {
        if i >= byte {
            break;
        }
        if c == '\t' {
            col += tab - (col % tab);
        } else {
            col += 1;
        }
    }
    col
}

/// Byte offset inside `line` for a display character index.
fn src_byte(line: &str, disp: usize, tab: usize) -> usize {
    let mut col = 0usize;
    for (i, c) in line.char_indices() {
        if col >= disp {
            return i;
        }
        if c == '\t' {
            col += tab - (col % tab);
        } else {
            col += 1;
        }
    }
    line.len()
}

fn line_job(
    ui: &egui::Ui,
    text: &str,
    spans: &[Span],
    size: f32,
    row_h: f32,
    pal: Palette,
) -> std::sync::Arc<egui::Galley> {
    let mut job = LayoutJob::default();
    job.wrap.max_width = f32::INFINITY;
    job.break_on_newline = false;

    let default = TextFormat {
        font_id: mono_font(size, false, false),
        color: pal.text(),
        line_height: Some(row_h),
        ..Default::default()
    };

    if spans.is_empty() {
        job.append(text, 0.0, default);
        if text.is_empty() {
            job.append(" ", 0.0, TextFormat {
                font_id: mono_font(size, false, false),
                color: Color32::TRANSPARENT,
                line_height: Some(row_h),
                ..Default::default()
            });
        }
    } else {
        let mut at = 0usize;
        for sp in spans {
            if sp.start > at {
                job.append(&text[at..sp.start], 0.0, default.clone());
            }
            let (color, bold, italic) = token_style(pal, sp.tok);
            job.append(
                &text[sp.start..sp.end.min(text.len())],
                0.0,
                TextFormat {
                    font_id: mono_font(size, bold, italic),
                    color,
                    line_height: Some(row_h),
                    ..Default::default()
                },
            );
            at = sp.end;
        }
        if at < text.len() {
            job.append(&text[at..], 0.0, default.clone());
        }
    }
    ui.ctx().fonts_mut(|f| f.layout_job(job))
}

fn word_range(text: &str, pos: usize) -> (usize, usize) {
    let is_w = |c: char| c.is_alphanumeric() || c == '_';
    let mut a = pos.min(text.len());
    while a > 0 {
        let prev = text[..a].char_indices().last().map(|(i, _)| i).unwrap_or(0);
        let c = text[prev..a].chars().next().unwrap_or(' ');
        if !is_w(c) {
            break;
        }
        a = prev;
    }
    let mut b = pos.min(text.len());
    while b < text.len() {
        let c = text[b..].chars().next().unwrap_or(' ');
        if !is_w(c) {
            break;
        }
        b += c.len_utf8();
    }
    (a, b)
}

/// Map a screen position inside the text area to a (line index, line start,
/// byte offset in the line). Tabs are expanded before measuring, exactly like
/// the painting pass, so clicking after a tab lands on the right character.
#[allow(clippy::too_many_arguments)]
fn hit_test(
    ui: &egui::Ui,
    doc: &Document,
    st: &EditorStyle,
    pal: Palette,
    text_area: Rect,
    row_h: f32,
    total_lines: usize,
    p: Pos2,
) -> (usize, usize, usize) {
    let line = (((p.y - text_area.top()) / row_h).floor() as isize
        + doc.scroll_y.floor() as isize)
        .clamp(0, total_lines as isize - 1) as usize;
    let (ls, le) = doc.line_range(line);
    let line_text = &doc.text[ls..le];
    let disp_line = expand_tabs(line_text, st.tab_width);
    let rel_x = (p.x - text_area.left() + doc.scroll_x).max(0.0);
    let galley = line_job(
        ui,
        &disp_line,
        &doc.spans.get(line).cloned().unwrap_or_default(),
        st.font_size,
        row_h,
        pal,
    );
    let cc = galley.cursor_from_pos(Vec2::new(rel_x, row_h / 2.0));
    let byte = src_byte(line_text, cc.index.0, st.tab_width);
    (line, ls, byte)
}

pub fn show(
    ui: &mut egui::Ui,
    doc: &mut Document,
    st: &EditorStyle,
    pal: Palette,
    allow_keys: bool,
    matches: &[(usize, usize)],
) -> Outcome {
    let mut changed = false;
    let full = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(full, Sense::click_and_drag());

    let size = st.font_size;
    let row_h = (size * 1.42).round().max(10.0);
    let char_w = ui
        .ctx()
        .fonts_mut(|f| f.layout_no_wrap("0".to_owned(), mono_font(size, false, false), Color32::WHITE).size().x)
        .max(3.0);

    let total_lines = doc.line_count();
    let gutter_w = if st.show_line_numbers {
        let digits = total_lines.to_string().len() as f32;
        (digits * char_w + 20.0).max(38.0)
    } else {
        0.0
    };
    let mini_w = if st.show_minimap { 64.0 } else { 0.0 };

    let text_left = full.left() + gutter_w;
    // A little breathing room between the gutter rule and the first character.
    let text_pad = 8.0;
    let text_right = full.right() - mini_w;
    let text_area = Rect::from_min_max(
        Pos2::new(text_left + text_pad, full.top()),
        Pos2::new(text_right.max(text_left + text_pad + 20.0), full.bottom()),
    );
    let visible_rows = ((text_area.height() / row_h).floor() as usize).max(1);

    // ---------------------------------------------------------- input ----
    let ptr = ui.input(|i| i.pointer.interact_pos());
    let hovered = response.hovered() || response.dragged();

    // Remember where the caret was before this frame's input so the view can
    // tell "the user moved the caret" from "the wheel scrolled the view".
    let cursor_before = doc.cursor;

    let mut edit: Option<EditAction> = None;

    // A left click places the caret (Shift extends the selection).
    if response.clicked() {
        if let Some(p) = ptr {
            if p.x >= text_area.left() - gutter_w {
                let (_line, ls, byte) = hit_test(ui, doc, st, pal, text_area, row_h, total_lines, p);
                doc.move_cursor(ls + byte, ui.input(|i| i.modifiers.shift));
                doc.wanted_col = None;
            }
        }
    }
    // A right click places the caret too, unless it lands inside the current
    // selection: there it belongs to the selection's context menu.
    if response.secondary_clicked() {
        if let Some(p) = ptr {
            if p.x >= text_area.left() - gutter_w {
                let (_line, ls, byte) = hit_test(ui, doc, st, pal, text_area, row_h, total_lines, p);
                let at = ls + byte;
                let inside = doc.selection().is_some_and(|(a, b)| at >= a && at <= b);
                if !inside {
                    doc.move_cursor(at, false);
                    doc.wanted_col = None;
                }
            }
        }
    }
    // Double click selects the word (the first click already placed the caret);
    // triple click selects the whole line.
    if response.double_clicked() {
        let at = doc.cursor;
        let (a, b) = word_range(&doc.text, at);
        doc.anchor = a;
        doc.cursor = b;
    }
    if response.triple_clicked() {
        doc.select_line();
    }
    if response.dragged_by(egui::PointerButton::Primary) {
        if let Some(p) = ptr {
            let (_line, ls, byte) = hit_test(ui, doc, st, pal, text_area, row_h, total_lines, p);
            doc.cursor = (ls + byte).min(doc.text.len());
            doc.wanted_col = None;
        }
    }

    // Wheel: scrolls the view. Consume the delta while over the editor so no
    // outer container reacts to the same gesture.
    if hovered {
        let (scroll, hscroll) = ui.input(|i| (i.smooth_scroll_delta.y, i.smooth_scroll_delta.x));
        if scroll != 0.0 {
            doc.scroll_y = (doc.scroll_y - scroll / row_h * 1.2).max(0.0);
            ui.ctx().input_mut(|i| i.smooth_scroll_delta.y = 0.0);
        }
        if hscroll != 0.0 {
            doc.scroll_x = (doc.scroll_x - hscroll).max(0.0);
            ui.ctx().input_mut(|i| i.smooth_scroll_delta.x = 0.0);
        }
    }

    // keys
    if allow_keys {
        let events = ui.input(|i| i.raw.events.clone());
        // egui-winit normally turns Ctrl+C/X/V into Copy/Cut/Paste events. If a
        // platform only delivers the plain key, handle it too — but never both,
        // or a paste would be inserted twice.
        let has_clip_event = events
            .iter()
            .any(|e| matches!(e, egui::Event::Copy | egui::Event::Cut | egui::Event::Paste(_)));
        for ev in events {
            match ev {
                egui::Event::Text(t) => {
                    if !t.is_empty() && !t.chars().any(|c| c.is_control()) {
                        doc.insert(&t);
                        changed = true;
                    }
                }
                egui::Event::Paste(t) => {
                    // Normalise Windows/old-Mac line endings and NULs so a
                    // paste from any application lands as clean text.
                    let t = t.replace("\r\n", "\n").replace(['\r', '\0'], "");
                    if !t.is_empty() {
                        doc.insert(&t);
                        changed = true;
                    }
                }
                egui::Event::Copy => edit = Some(EditAction::Copy),
                egui::Event::Cut => edit = Some(EditAction::Cut),
                egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    let shift = modifiers.shift;
                    let ctrl = modifiers.command;
                    match key {
                        egui::Key::Enter => {
                            doc.newline();
                            changed = true;
                        }
                        egui::Key::Backspace => {
                            if ctrl {
                                let line = doc.line_of(doc.cursor);
                                let (ls, _) = doc.line_range(line);
                                if doc.cursor > ls {
                                    let start = doc.text[ls..doc.cursor]
                                        .trim_end()
                                        .len()
                                        .max(0)
                                        + ls;
                                    doc.delete_range(start, doc.cursor);
                                } else if doc.cursor > 0 {
                                    doc.backspace();
                                }
                            } else {
                                doc.backspace();
                            }
                            changed = true;
                        }
                        egui::Key::Delete => {
                            doc.delete_forward();
                            changed = true;
                        }
                        egui::Key::Tab => {
                            if doc.has_selection() {
                                doc.indent_selection(st.tab_width);
                            } else {
                                doc.insert(&" ".repeat(st.tab_width));
                            }
                            changed = true;
                        }
                        egui::Key::ArrowLeft => {
                            if ctrl {
                                doc.move_word_left();
                            } else {
                                doc.move_left(shift);
                            }
                            doc.wanted_col = None;
                        }
                        egui::Key::ArrowRight => {
                            if ctrl {
                                doc.move_word_right();
                            } else {
                                doc.move_right(shift);
                            }
                            doc.wanted_col = None;
                        }
                        egui::Key::ArrowUp => {
                            doc.move_line(-1, shift);
                        }
                        egui::Key::ArrowDown => {
                            doc.move_line(1, shift);
                        }
                        egui::Key::Home => {
                            if ctrl {
                                doc.move_cursor(0, shift);
                            } else {
                                doc.move_home(shift);
                            }
                            doc.wanted_col = None;
                        }
                        egui::Key::End => {
                            if ctrl {
                                doc.move_cursor(doc.text.len(), shift);
                            } else {
                                doc.move_end(shift);
                            }
                            doc.wanted_col = None;
                        }
                        egui::Key::PageUp => {
                            doc.move_line(-(visible_rows as isize - 2), shift);
                        }
                        egui::Key::PageDown => {
                            doc.move_line(visible_rows as isize - 2, shift);
                        }
                        egui::Key::A if ctrl => doc.select_all(),
                        egui::Key::C if ctrl && !has_clip_event => edit = Some(EditAction::Copy),
                        egui::Key::X if ctrl && !has_clip_event => edit = Some(EditAction::Cut),
                        egui::Key::V if ctrl && !has_clip_event => edit = Some(EditAction::Paste),
                        egui::Key::Insert if ctrl && !has_clip_event => edit = Some(EditAction::Copy),
                        egui::Key::Z if ctrl => {
                            if shift {
                                doc.redo();
                            } else {
                                doc.undo();
                            }
                            changed = true;
                        }
                        egui::Key::Y if ctrl => {
                            doc.redo();
                            changed = true;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    doc.ensure_spans();

    // Keep the caret on screen after the caret itself moved (typing, clicking,
    // arrow keys, undo, …). This must NOT run after a wheel scroll: doing so
    // snapped the view straight back to a stationary caret, which made
    // scrolling away from the cursor impossible in either direction.
    let caret_line = doc.line_of(doc.cursor);
    if doc.cursor != cursor_before || changed {
        if caret_line < doc.scroll_y as usize {
            doc.scroll_y = caret_line as f32;
        } else if caret_line >= doc.scroll_y as usize + visible_rows {
            doc.scroll_y = (caret_line as f32 - visible_rows as f32 + 1.5).max(0.0);
        }
    }
    let max_scroll = (total_lines as f32 - visible_rows as f32 + 1.0).max(0.0);
    doc.scroll_y = doc.scroll_y.clamp(0.0, max_scroll);

    // Right-click menu for the usual editing commands.
    response.context_menu(|ui| {
        ui.set_min_width(190.0);
        if ui.button("Cut\t\tCtrl+X").clicked() {
            edit = Some(EditAction::Cut);
            ui.close();
        }
        if ui.button("Copy\t\tCtrl+C").clicked() {
            edit = Some(EditAction::Copy);
            ui.close();
        }
        if ui.button("Paste\t\tCtrl+V").clicked() {
            edit = Some(EditAction::Paste);
            ui.close();
        }
        ui.separator();
        if ui.button("Select All\t\tCtrl+A").clicked() {
            edit = Some(EditAction::SelectAll);
            ui.close();
        }
    });

    // -------------------------------------------------------- painting ---
    let painter = ui.painter_at(full);
    painter.rect_filled(full, CornerRadius::ZERO, pal.editor_bg());

    // gutter
    if st.show_line_numbers {
        let g = Rect::from_min_max(full.min, Pos2::new(text_left, full.bottom()));
        painter.rect_filled(g, CornerRadius::ZERO, pal.gutter_bg());
        painter.line_segment(
            [Pos2::new(text_left, full.top()), Pos2::new(text_left, full.bottom())],
            Stroke::new(1.0, theme::SEP),
        );
    }

    let first = doc.scroll_y.floor() as usize;
    let last = (first + visible_rows + 1).min(total_lines);

    // Do not let horizontal scrolling run past the end of the widest visible
    // line: otherwise the text slides off the left and the view becomes blank.
    let widest = (first..last)
        .map(|l| {
            let (a, b) = doc.line_range(l);
            doc.text[a..b].chars().count()
        })
        .max()
        .unwrap_or(0) as f32;
    let content_w = widest * char_w;
    if content_w > text_area.width() {
        doc.scroll_x = doc.scroll_x.min(content_w - text_area.width());
    } else {
        doc.scroll_x = 0.0;
    }

        let caret_pos = doc.cursor;
    let sel = doc.selection();
    let bookmarked: std::collections::HashSet<usize> = doc
        .bookmarks
        .iter()
        .map(|b| doc.line_of((*b).min(doc.text.len())))
        .collect();

    for line in first..last {
        let (ls, le) = doc.line_range(line);
        let y = text_area.top() + (line as f32 - doc.scroll_y) * row_h;
        let row = Rect::from_min_max(
            Pos2::new(full.left(), y),
            Pos2::new(text_area.right(), y + row_h),
        );

        if line == caret_line {
            painter.rect_filled(
                Rect::from_min_max(Pos2::new(text_left, y), Pos2::new(text_area.right(), y + row_h)),
                CornerRadius::ZERO,
                pal.cur_line(),
            );
        }

        // A bookmark shows as a short accent bar at the left of its line.
        if st.show_line_numbers && bookmarked.contains(&line) {
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(full.left() + 4.0, y + 2.5),
                    Vec2::new(3.0, row_h - 5.0),
                ),
                CornerRadius::same(1),
                theme::ACCENT,
            );
        }

        let src_line = &doc.text[ls..le];
        let disp = expand_tabs(src_line, st.tab_width);
        let spans = doc.spans.get(line).cloned().unwrap_or_default();
        let galley = line_job(ui, &disp, &spans, size, row_h, pal);
        let origin = Pos2::new(text_area.left() - doc.scroll_x, y);

        // selection band
        if let Some((s, e)) = sel {
            if e > ls && s < le || (s == e) {
                let a_byte = s.max(ls).min(le) - ls;
                let b_byte = e.max(ls).min(le) - ls;
                let x1 = origin.x + galley.pos_from_cursor(CCursor::new(disp_index(src_line, a_byte, st.tab_width))).min.x;
                let mut x2 = origin.x + galley.pos_from_cursor(CCursor::new(disp_index(src_line, b_byte, st.tab_width))).min.x;
                if e > le {
                    x2 += char_w * 0.5;
                }
                if x2 <= x1 {
                    x2 = x1 + char_w * 0.6;
                }
                let band = Rect::from_min_max(
                    Pos2::new(x1.max(text_left), y + 1.0),
                    Pos2::new(x2.min(text_area.right()), y + row_h - 1.0),
                );
                if band.width() > 0.5 {
                    painter.rect_filled(band, CornerRadius::same(2), pal.sel());
                }
            }
        }

        // find matches
        for (m, (ms, me)) in matches.iter().enumerate() {
            if *me > ls && *ms < le {
                let a = (*ms).max(ls).min(le) - ls;
                let b = (*me).max(ls).min(le) - ls;
                let x1 = origin.x + galley.pos_from_cursor(CCursor::new(disp_index(src_line, a, st.tab_width))).min.x;
                let x2 = origin.x + galley.pos_from_cursor(CCursor::new(disp_index(src_line, b, st.tab_width))).min.x;
                let col = if m == 0 {
                    Color32::from_rgb(0xff, 0xd0, 0x60)
                } else {
                    pal.match_bg()
                };
                painter.rect_filled(
                    Rect::from_min_max(
                        Pos2::new(x1, y + 1.0),
                        Pos2::new(x2.max(x1 + 1.0), y + row_h - 1.0),
                    ),
                    CornerRadius::same(1),
                    col,
                );
            }
        }

        painter.galley(origin, galley.clone(), pal.text());

        if st.show_line_numbers {
            let is_cur = line == caret_line;
            let color = if is_cur {
                if pal.dark {
                    Color32::from_rgb(0xd5, 0xd8, 0xda)
                } else {
                    Color32::from_rgb(0x2b, 0x2e, 0x31)
                }
            } else {
                pal.gutter_fg()
            };
            painter.text(
                Pos2::new(text_left - 9.0, y + row_h / 2.0),
                Align2::RIGHT_CENTER,
                (line + 1).to_string(),
                if is_cur {
                    mono_font(size, true, false)
                } else {
                    mono_font(size, false, false)
                },
                color,
            );
        }
        let _ = (row, ls, le, caret_pos);
    }

    // caret
    {
        let _ = &caret_pos;
        let line = caret_line;
        let (ls, le) = doc.line_range(line);
        let src_line = &doc.text[ls..le];
        let disp = expand_tabs(src_line, st.tab_width);
        let spans = doc.spans.get(line).cloned().unwrap_or_default();
        let galley = line_job(ui, &disp, &spans, size, row_h, pal);
        let y = text_area.top() + (line as f32 - doc.scroll_y) * row_h;
        let x = text_area.left() - doc.scroll_x
            + galley
                .pos_from_cursor(CCursor::new(disp_index(
                    src_line,
                    doc.cursor - ls,
                    st.tab_width,
                )))
                .min
                .x;
        painter.line_segment(
            [Pos2::new(x, y + 2.0), Pos2::new(x, y + row_h - 2.0)],
            Stroke::new(1.6, pal.text()),
        );
    }

    // horizontal scrollbar
    if content_w > text_area.width() {
        let frac = (text_area.width() / content_w).clamp(0.1, 1.0);
        let track = Rect::from_min_max(
            Pos2::new(text_area.left(), text_area.bottom() - 9.0),
            Pos2::new(text_area.right() - 2.0, text_area.bottom() - 2.0),
        );
        painter.rect_filled(track, CornerRadius::same(4), pal.scroll_track());
        let thumb_w = (track.width() * frac).max(24.0);
        let span = (content_w - text_area.width()).max(1.0);
        let t = (doc.scroll_x / span).clamp(0.0, 1.0);
        let x = track.left() + (track.width() - thumb_w) * t;
        painter.rect_filled(
            Rect::from_min_size(Pos2::new(x, track.top()), Vec2::new(thumb_w, track.height())),
            CornerRadius::same(4),
            pal.scroll_thumb(),
        );
    }

    // vertical scrollbar
    if total_lines as f32 > visible_rows as f32 {
        let track = Rect::from_min_max(
            Pos2::new(text_area.right() - 9.0, text_area.top() + 2.0),
            Pos2::new(text_area.right() - 2.0, text_area.bottom() - 2.0),
        );
        let frac = (visible_rows as f32 / total_lines as f32).clamp(0.08, 1.0);
        let thumb_h = (track.height() * frac).max(28.0);
        let span = (total_lines as f32 - visible_rows as f32).max(1.0);
        let t = (doc.scroll_y / span).clamp(0.0, 1.0);
        let y = track.top() + (track.height() - thumb_h) * t;
        let thumb = Rect::from_min_size(Pos2::new(track.left(), y), Vec2::new(track.width(), thumb_h));
        painter.rect_filled(thumb, CornerRadius::same(4), pal.scroll_thumb());
        let drag = ui.interact(thumb, ui.id().with("vscroll"), Sense::drag());
        if drag.dragged() {
            let d = ui.input(|i| i.pointer.delta().y);
            doc.scroll_y = (doc.scroll_y + d / track.height() * span).clamp(0.0, max_scroll);
        }
    }

    // minimap
    if st.show_minimap {
        let mm = Rect::from_min_max(Pos2::new(text_right, full.top()), full.max);
        painter.rect_filled(mm, CornerRadius::ZERO, if pal.dark {
            Color32::from_rgb(0x16, 0x1a, 0x1e)
        } else {
            Color32::from_rgb(0xf8, 0xf9, 0xfb)
        });
        painter.line_segment(
            [Pos2::new(mm.left(), mm.top()), Pos2::new(mm.left(), mm.bottom())],
            Stroke::new(1.0, theme::SEP),
        );
        let inner = mm.shrink2(Vec2::new(5.0, 6.0));
        // Fixed row pitch, like the original's minimap: lines stay 3px tall
        // however long the file is, so a short file fills only the top.
        let per = 3.0_f32;
        let shown = (inner.height() / per).floor() as usize;
        let cols = 96.0_f32;
        for i in 0..total_lines.min(shown) {
            let (a, b) = doc.line_range(i);
            let line = &doc.text[a..b];
            if line.trim().is_empty() {
                continue;
            }
            let chars = line.chars().count() as f32;
            let indent = line.chars().take_while(|c| *c == ' ').count() as f32;
            let x0 = inner.left() + (indent / cols).clamp(0.0, 1.0) * inner.width();
            let w = ((chars - indent) / cols).clamp(0.02, 1.0) * inner.width();
            let col = match doc.spans.get(i).and_then(|s| s.first()) {
                Some(sp) => token_style(pal, sp.tok)
                    .0
                    .gamma_multiply(if pal.dark { 0.55 } else { 0.40 }),
                None => pal.text_dim().gamma_multiply(if pal.dark { 0.85 } else { 0.55 }),
            };
            painter.rect_filled(
                Rect::from_min_size(
                    Pos2::new(x0, inner.top() + i as f32 * per),
                    Vec2::new(w.max(1.5), per - 1.4),
                ),
                CornerRadius::ZERO,
                col,
            );
        }
        // Only box the visible slice when the file is taller than the strip.
        if total_lines as f32 * per > inner.height() {
            let view_h = (visible_rows as f32 * per).min(inner.height());
            let view_y = inner.top() + (doc.scroll_y * per).min(inner.height() - view_h);
            let view = Rect::from_min_size(
                Pos2::new(inner.left(), view_y),
                Vec2::new(inner.width(), view_h.max(10.0)),
            );
            painter.rect_filled(
                view,
                CornerRadius::same(3),
                pal.accent_soft(),
            );
            painter.rect_stroke(
                view,
                CornerRadius::same(3),
                Stroke::new(1.0, theme::ACCENT.gamma_multiply(0.50)),
                StrokeKind::Inside,
            );
        }
    }

    // Language badge in the bottom-left of the gutter, like the original's
    // syntax icon in the status bar only — nothing extra here.

    Outcome {
        changed,
        clicked: response.clicked(),
        edit,
    }
}

/// Small helper used by the app to measure how wide one character is.
pub fn char_width(ui: &egui::Ui, size: f32) -> f32 {
    ui.ctx()
        .fonts_mut(|f| {
            f.layout_no_wrap("0".to_owned(), FontId::new(size, egui::FontFamily::Monospace), Color32::WHITE)
                .size()
                .x
        })
        .max(3.0)
}

/// Find every occurrence of `query` in `text` (byte ranges).
pub fn find_all(text: &str, query: &str, case_sensitive: bool) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if query.is_empty() {
        return out;
    }
    if case_sensitive {
        let mut start = 0usize;
        while let Some(p) = text[start..].find(query) {
            let a = start + p;
            out.push((a, a + query.len()));
            start = a + query.len().max(1);
            if out.len() > 5000 {
                break;
            }
        }
    } else {
        let hay = text.to_lowercase();
        let q = query.to_lowercase();
        let mut start = 0usize;
        while let Some(p) = hay[start..].find(&q) {
            let a = start + p;
            out.push((a, a + q.len()));
            start = a + q.len().max(1);
            if out.len() > 5000 {
                break;
            }
        }
    }
    out
}

/// A tiny language tag used by the tab icon.
pub fn lang_is_python(lang: Lang) -> bool {
    lang == Lang::Python
}
