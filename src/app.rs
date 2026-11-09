//! Window chrome and application logic: title bar, menu bar, toolbar, tab bar,
//! breadcrumb, sidebar, editor host, status bar, find/replace, dialogs.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Id, Margin, Pos2, Rect, Response, Sense, Stroke,
    StrokeKind, Vec2,
};
use egui::ViewportCommand;

use crate::doc::{Document, Lang};
use crate::editor::{self, EditorStyle};
use crate::theme::{self, paint_icon, Icon, Palette};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarPanel {
    Documents,
    Filesystem,
    Symbols,
    Plugins,
}

pub enum Act {
    New,
    Open,
    Save,
    SaveAs,
    Reload,
    CloseDoc(usize),
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    SelectLine,
    Find,
    Replace,
    GoToLine,
    NextTab,
    PrevTab,
    Goto(usize),
    ToggleSidebar,
    Panel(SidebarPanel),
    ToggleToolbar,
    ToggleTabBar,
    ToggleBreadcrumb,
    ToggleStatus,
    ToggleMinimap,
    ToggleLineNumbers,
    Theme(bool),
    ZoomIn,
    ZoomOut,
    ZoomReset,
    SortLines,
    TrimWhitespace,
    Upper,
    Lower,
    TabWidth(usize),
    Lang(Lang),
    About,
    Shortcuts,
    Settings,
    OpenFolder,
    CloseFolder,
    SaveSession,
    RestoreSession,
    ToolView(usize),
    FindNext,
    FindPrev,
    ReplaceOne,
    ReplaceAll,
    CloseFind,
    NewFolderToggle,
}

struct Find {
    open: bool,
    replace: bool,
    query: String,
    with: String,
    case: bool,
    matches: Vec<(usize, usize)>,
    current: usize,
    last_doc: usize,
    last_version: u64,
}

impl Default for Find {
    fn default() -> Self {
        Self {
            open: false,
            replace: false,
            query: String::new(),
            with: String::new(),
            case: false,
            matches: Vec::new(),
            current: 0,
            last_doc: usize::MAX,
            last_version: u64::MAX,
        }
    }
}

enum Dialog {
    About,
    Shortcuts,
    Settings,
    Browser(Browser),
    Message(String, String),
    GoToLine(String),
    Confirm(&'static str, String, Act2),
}

#[derive(Clone, Copy)]
enum Act2 {
    CloseDoc(usize),
    Quit,
}

struct Browser {
    dir: PathBuf,
    name: String,
    save: bool,
    dirs: Vec<PathBuf>,
    files: Vec<PathBuf>,
    error: Option<String>,
}

impl Browser {
    fn new(dir: PathBuf, save: bool) -> Self {
        let mut b = Browser {
            dir,
            name: String::new(),
            save,
            dirs: Vec::new(),
            files: Vec::new(),
            error: None,
        };
        b.refresh();
        b
    }

    fn refresh(&mut self) {
        self.dirs.clear();
        self.files.clear();
        self.error = None;
        match std::fs::read_dir(&self.dir) {
            Ok(rd) => {
                for e in rd.flatten() {
                    let p = e.path();
                    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                    if name.starts_with('.') {
                        continue;
                    }
                    if p.is_dir() {
                        self.dirs.push(p);
                    } else {
                        self.files.push(p);
                    }
                }
                self.dirs.sort_by_key(|p| p.file_name().map(|n| n.to_os_string()));
                self.files.sort_by_key(|p| p.file_name().map(|n| n.to_os_string()));
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
}

pub struct KateCraftApp {
    pub docs: Vec<Document>,
    pub active: usize,
    pub pal: Palette,
    pub ed: EditorStyle,
    show_menu: bool,
    show_toolbar: bool,
    show_tabbar: bool,
    show_breadcrumb: bool,
    show_status: bool,
    sidebar: bool,
    panel: SidebarPanel,
    fs_root: Option<PathBuf>,
    listings: HashMap<PathBuf, Vec<PathBuf>>,
    expanded: HashSet<PathBuf>,
    find: Find,
    dialog: Option<Dialog>,
    tool_view: Option<usize>,
    editor_focused: bool,
    next_untitled: u32,
    search_field: String,
    output_log: Vec<String>,
    session: Option<Vec<PathBuf>>,
    toast: Option<(String, f64)>,
    pub screenshot: Option<PathBuf>,
    shot_frames: u64,
    shot_requested: bool,
    shot_done: Arc<AtomicBool>,
    frame: u64,
    pub quitting: bool,
    dirty_close: Option<usize>,
}

const TOOL_VIEWS: [(&str, Icon); 5] = [
    ("Output", Icon::Output),
    ("Diagnostics", Icon::Warning),
    ("Search", Icon::Search),
    ("Project", Icon::Grid),
    ("Terminal", Icon::Terminal),
];

impl KateCraftApp {
    pub fn new(cc: &eframe::CreationContext<'_>, files: Vec<PathBuf>, screenshot: Option<PathBuf>) -> Self {
        theme::install_fonts(&cc.egui_ctx);
        let pal = Palette::light();
        theme::apply_style(&cc.egui_ctx, pal);

        let mut docs = Vec::new();
        for f in files {
            match Document::open(&f) {
                Ok(d) => docs.push(d),
                Err(e) => {
                    let mut d = Document::new_untitled(0);
                    d.title = f
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Untitled")
                        .to_string();
                    d.text = format!("could not open {}: {e}\n", f.display());
                    docs.push(d);
                }
            }
        }
        if docs.is_empty() {
            let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples/welcome.py");
            if sample.exists() {
                if let Ok(d) = Document::open(&sample) {
                    docs.push(d);
                }
            }
            if docs.is_empty() {
                let mut d = Document::new_untitled(0);
                d.text = "Welcome\n=======\n\nType here.\n".to_string();
                d.lang = Lang::Plain;
                docs.push(d);
            }
        }

        let fs_root = docs
            .get(0)
            .and_then(|d| d.path.as_ref())
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .or_else(|| std::env::current_dir().ok());

        let mut app = Self {
            docs,
            active: 0,
            pal,
            ed: EditorStyle::default(),
            show_menu: true,
            show_toolbar: true,
            show_tabbar: true,
            show_breadcrumb: true,
            show_status: true,
            sidebar: false,
            panel: SidebarPanel::Documents,
            fs_root,
            listings: HashMap::new(),
            expanded: HashSet::new(),
            find: Find::default(),
            dialog: None,
            tool_view: None,
            editor_focused: true,
            next_untitled: 1,
            search_field: String::new(),
            output_log: vec!["KateCraft started.".into()],
            session: None,
            toast: None,
            screenshot: screenshot.clone(),
            shot_frames: 0,
            shot_requested: false,
            shot_done: Arc::new(AtomicBool::new(false)),
            frame: 0,
            quitting: false,
            dirty_close: None,
        };
        if let Some(root) = app.fs_root.clone() {
            app.expanded.insert(root);
        }
        if let Some(d) = app.docs.get_mut(0) {
            let dir = d.path.as_ref().and_then(|p| p.parent()).map(|p| p.to_path_buf());
            d.cursor = 0;
            if let Some(dir) = dir {
                app.expanded.insert(dir);
            }
        }
        // Lets a reviewer open a specific view straight away (used when
        // capturing screenshots of the panels and dialogs).
        if std::env::var("KATECRAFT_THEME").as_deref() == Ok("dark") {
            app.pal = Palette { dark: true };
            theme::apply_style(&cc.egui_ctx, app.pal);
        }
        match std::env::var("KATECRAFT_UI").as_deref() {
            Ok("find") => {
                app.find.open = true;
                app.find.query = "tab".to_string();
                app.refresh_matches();
            }
            Ok("replace") => {
                app.find.open = true;
                app.find.replace = true;
                app.find.query = "tab".to_string();
                app.find.with = "panel".to_string();
                app.refresh_matches();
            }
            Ok("sidebar") => {
                app.sidebar = true;
                app.panel = SidebarPanel::Documents;
            }
            Ok("files") => {
                app.sidebar = true;
                app.panel = SidebarPanel::Filesystem;
            }
            Ok("open") => {
                let dir = app.fs_root.clone().unwrap_or_default();
                app.dialog = Some(Dialog::Browser(Browser::new(dir, false)));
            }
            Ok("about") => app.dialog = Some(Dialog::About),
            Ok("shortcuts") => app.dialog = Some(Dialog::Shortcuts),
            Ok("settings") => app.dialog = Some(Dialog::Settings),
            Ok("dark") => {
                app.pal = Palette { dark: true };
                theme::apply_style(&cc.egui_ctx, app.pal);
            }
            _ => {}
        }
        app
    }

    fn doc(&self) -> &Document {
        &self.docs[self.active.min(self.docs.len().saturating_sub(1))]
    }

    fn doc_mut(&mut self) -> &mut Document {
        let i = self.active.min(self.docs.len().saturating_sub(1));
        &mut self.docs[i]
    }

    fn log(&mut self, msg: impl Into<String>) {
        let m = msg.into();
        self.output_log.push(m);
        if self.output_log.len() > 200 {
            self.output_log.remove(0);
        }
    }

    fn toast(&mut self, msg: impl Into<String>, now: f64) {
        self.toast = Some((msg.into(), now));
    }

    // ------------------------------------------------------------ actions ---

    fn apply(&mut self, act: Act, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        match act {
            Act::New => {
                let mut d = Document::new_untitled(self.next_untitled);
                self.next_untitled += 1;
                d.cursor = 0;
                self.docs.push(d);
                self.active = self.docs.len() - 1;
                self.editor_focused = true;
            }
            Act::Open => {
                let dir = self
                    .doc()
                    .path
                    .as_ref()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                    .or_else(|| std::env::current_dir().ok())
                    .unwrap_or_else(|| PathBuf::from("/"));
                self.dialog = Some(Dialog::Browser(Browser::new(dir, false)));
            }
            Act::Save => {
                if self.doc().path.is_some() {
                    let path = self.doc().path.clone().unwrap();
                    match self.doc_mut().save_to(&path) {
                        Ok(_) => self.toast(format!("Saved {}", path.display()), now),
                        Err(e) => self.dialog = Some(Dialog::Message("Could not save".into(), e.to_string())),
                    }
                } else {
                    let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
                    let mut b = Browser::new(dir, true);
                    b.name = self.doc().title.clone();
                    self.dialog = Some(Dialog::Browser(b));
                }
            }
            Act::SaveAs => {
                let dir = self
                    .doc()
                    .path
                    .as_ref()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                    .or_else(|| std::env::current_dir().ok())
                    .unwrap_or_else(|| PathBuf::from("/"));
                let mut b = Browser::new(dir, true);
                b.name = self.doc().title.clone();
                self.dialog = Some(Dialog::Browser(b));
            }
            Act::Reload => {
                if let Some(p) = self.doc().path.clone() {
                    if let Ok(d) = Document::open(&p) {
                        let idx = self.active;
                        let scroll = self.docs[idx].scroll_y;
                        self.docs[idx] = d;
                        self.docs[idx].scroll_y = scroll;
                        self.toast(format!("Reloaded {}", p.display()), now);
                    }
                }
            }
            Act::CloseDoc(i) => {
                if i < self.docs.len() {
                    if self.docs[i].dirty {
                        self.dialog = Some(Dialog::Confirm(
                            "Close document",
                            format!("\"{}\" has unsaved changes. Close it anyway?", self.docs[i].title),
                            Act2::CloseDoc(i),
                        ));
                    } else {
                        self.close_doc(i);
                    }
                }
            }
            Act::Quit => {
                if self.docs.iter().any(|d| d.dirty) {
                    self.dialog = Some(Dialog::Confirm(
                        "Quit",
                        "Some documents have unsaved changes. Quit anyway?".into(),
                        Act2::Quit,
                    ));
                } else {
                    self.quitting = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
            Act::Undo => {
                self.doc_mut().undo();
            }
            Act::Redo => {
                self.doc_mut().redo();
            }
            Act::Copy => {
                let s = self.doc().selected_text();
                if !s.is_empty() {
                    ctx.copy_text(s);
                }
            }
            Act::Cut => {
                let s = self.doc().selected_text();
                if !s.is_empty() {
                    ctx.copy_text(s);
                    if let Some((a, b)) = self.doc().selection() {
                        self.doc_mut().delete_range(a, b);
                    }
                }
            }
            Act::Paste => {
                if let Ok(t) = arboard_read() {
                    self.doc_mut().insert(&t);
                }
            }
            Act::SelectAll => self.doc_mut().select_all(),
            Act::SelectLine => self.doc_mut().select_line(),
            Act::Find => {
                self.find.open = true;
                self.find.replace = false;
                self.refresh_matches();
            }
            Act::Replace => {
                self.find.open = true;
                self.find.replace = true;
                self.refresh_matches();
            }
            Act::CloseFind => {
                self.find.open = false;
                self.editor_focused = true;
            }
            Act::FindNext => self.step_match(1),
            Act::FindPrev => self.step_match(-1),
            Act::ReplaceOne => {
                let cur = self.find.current;
                if let Some((a, b)) = self.find.matches.get(cur).copied() {
                    let with = self.find.with.clone();
                    let d = self.doc_mut();
                    d.delete_range(a, b);
                    d.insert(&with);
                    self.refresh_matches();
                }
            }
            Act::ReplaceAll => {
                let q = self.find.query.clone();
                let w = self.find.with.clone();
                let ci = !self.find.case;
                let n = self.doc_mut().replace_all(&q, &w, ci);
                self.refresh_matches();
                self.toast(format!("Replaced {n} occurrence(s)"), now);
            }
            Act::GoToLine => {
                self.dialog = Some(Dialog::GoToLine(String::new()));
            }
            Act::NextTab => {
                if !self.docs.is_empty() {
                    self.active = (self.active + 1) % self.docs.len();
                }
            }
            Act::PrevTab => {
                if !self.docs.is_empty() {
                    self.active = (self.active + self.docs.len() - 1) % self.docs.len();
                }
            }
            Act::Goto(i) => {
                if i < self.docs.len() {
                    self.active = i;
                }
            }
            Act::ToggleSidebar => self.sidebar = !self.sidebar,
            Act::Panel(p) => {
                if self.sidebar && self.panel == p {
                    self.sidebar = false;
                } else {
                    self.sidebar = true;
                    self.panel = p;
                }
            }
            Act::ToggleToolbar => self.show_toolbar = !self.show_toolbar,
            Act::ToggleTabBar => self.show_tabbar = !self.show_tabbar,
            Act::ToggleBreadcrumb => self.show_breadcrumb = !self.show_breadcrumb,
            Act::ToggleStatus => self.show_status = !self.show_status,
            Act::ToggleMinimap => self.ed.show_minimap = !self.ed.show_minimap,
            Act::ToggleLineNumbers => self.ed.show_line_numbers = !self.ed.show_line_numbers,
            Act::Theme(dark) => {
                self.pal = Palette { dark };
                theme::apply_style(ctx, self.pal);
            }
            Act::ZoomIn => self.ed.font_size = (self.ed.font_size + 1.0).min(28.0),
            Act::ZoomOut => self.ed.font_size = (self.ed.font_size - 1.0).max(7.0),
            Act::ZoomReset => self.ed.font_size = 12.5,
            Act::SortLines => {
                self.doc_mut().sort_lines();
                self.toast("Lines sorted", now);
            }
            Act::TrimWhitespace => {
                self.doc_mut().trim_trailing_whitespace();
                self.toast("Trailing whitespace removed", now);
            }
            Act::Upper => self.doc_mut().transform_selection(|s| s.to_uppercase()),
            Act::Lower => self.doc_mut().transform_selection(|s| s.to_lowercase()),
            Act::TabWidth(n) => self.ed.tab_width = n,
            Act::Lang(l) => {
                self.doc_mut().lang = l;
                let v = self.doc().version;
                let _ = v;
                let d = self.doc_mut();
                d.version += 1;
            }
            Act::About => self.dialog = Some(Dialog::About),
            Act::Shortcuts => self.dialog = Some(Dialog::Shortcuts),
            Act::Settings => self.dialog = Some(Dialog::Settings),
            Act::OpenFolder => {
                let dir = self.fs_root.clone().unwrap_or_else(|| PathBuf::from("/"));
                self.dialog = Some(Dialog::Browser(Browser::new(dir, false)));
            }
            Act::CloseFolder => {
                self.fs_root = None;
                self.listings.clear();
                self.expanded.clear();
            }
            Act::SaveSession => {
                let paths: Vec<PathBuf> = self
                    .docs
                    .iter()
                    .filter_map(|d| d.path.clone())
                    .collect();
                if let Some(file) = session_path() {
                    if let Some(dir) = file.parent() {
                        let _ = std::fs::create_dir_all(dir);
                    }
                    let body = paths
                        .iter()
                        .map(|p| p.to_string_lossy().into_owned())
                        .collect::<Vec<_>>()
                        .join("\n");
                    let _ = std::fs::write(&file, body);
                }
                self.session = Some(paths);
                self.toast("Session saved", now);
            }
            Act::RestoreSession => {
                if self.session.is_none() {
                    if let Some(body) = session_path().and_then(|p| std::fs::read_to_string(p).ok()) {
                        self.session = Some(
                            body.lines()
                                .filter(|l| !l.trim().is_empty())
                                .map(PathBuf::from)
                                .collect(),
                        );
                    }
                }
                if let Some(paths) = self.session.clone() {
                    for p in paths {
                        if let Ok(d) = Document::open(&p) {
                            self.docs.push(d);
                        }
                    }
                    self.active = self.docs.len().saturating_sub(1);
                    self.toast("Session restored", now);
                } else {
                    self.toast("No saved session yet", now);
                }
            }
            Act::ToolView(i) => {
                if self.tool_view == Some(i) {
                    self.tool_view = None;
                } else {
                    self.tool_view = Some(i);
                }
            }
            Act::NewFolderToggle => {}
        }
    }

    fn close_doc(&mut self, i: usize) {
        if i >= self.docs.len() {
            return;
        }
        self.docs.remove(i);
        if self.docs.is_empty() {
            self.docs.push(Document::new_untitled(self.next_untitled));
            self.next_untitled += 1;
            self.active = 0;
        } else if self.active >= self.docs.len() {
            self.active = self.docs.len() - 1;
        } else if self.active > i {
            self.active -= 1;
        }
    }

    fn refresh_matches(&mut self) {
        let q = self.find.query.clone();
        let case = self.find.case;
        if let Some(d) = self.docs.get(self.active) {
            self.find.matches = editor::find_all(&d.text, &q, case);
            self.find.current = self.find.current.min(self.find.matches.len().saturating_sub(1));
        } else {
            self.find.matches.clear();
        }
    }

    fn step_match(&mut self, dir: isize) {
        if self.find.matches.is_empty() {
            self.refresh_matches();
        }
        if self.find.matches.is_empty() {
            return;
        }
        let n = self.find.matches.len() as isize;
        self.find.current = ((self.find.current as isize + dir).rem_euclid(n)) as usize;
        let (a, b) = self.find.matches[self.find.current];
        let d = self.doc_mut();
        d.anchor = a;
        d.cursor = b;
        let line = d.line_of(a);
        if line < d.scroll_y as usize || line >= d.scroll_y as usize + 30 {
            d.scroll_y = (line as f32 - 8.0).max(0.0);
        }
    }

    fn jump_to_line(&mut self, n: usize) {
        let d = self.doc_mut();
        let line = n.saturating_sub(1).min(d.line_count() - 1);
        let (ls, le) = d.line_range(line);
        d.cursor = ls.min(le);
        d.anchor = d.cursor;
        d.scroll_y = (line as f32 - 5.0).max(0.0);
    }

    fn open_path(&mut self, path: &Path) {
        if let Some(i) = self
            .docs
            .iter()
            .position(|d| d.path.as_deref() == Some(path))
        {
            self.active = i;
            return;
        }
        match Document::open(path) {
            Ok(d) => {
                self.docs.push(d);
                self.active = self.docs.len() - 1;
                self.log(format!("Opened {}", path.display()));
            }
            Err(e) => {
                self.dialog = Some(Dialog::Message("Could not open".into(), e.to_string()));
            }
        }
    }

    fn save_path(&mut self, path: PathBuf, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        match self.doc_mut().save_to(&path) {
            Ok(_) => {
                self.log(format!("Saved {}", path.display()));
                self.toast(format!("Saved {}", path.display()), now);
            }
            Err(e) => self.dialog = Some(Dialog::Message("Could not save".into(), e.to_string())),
        }
    }

    // ------------------------------------------------------------- chrome ---

    fn title_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.max_rect();
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::ZERO, self.pal.title_bg());

        let drag = ui.interact(rect, Id::new("title-drag"), Sense::click_and_drag());
        if drag.drag_started() {
            ctx.send_viewport_cmd(ViewportCommand::StartDrag);
        }
        if drag.double_clicked() {
            let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
            ctx.send_viewport_cmd(ViewportCommand::Maximized(!max));
        }

        // neutral app mark
        let logo = Rect::from_center_size(
            Pos2::new(rect.left() + 20.0, rect.center().y),
            Vec2::splat(15.0),
        );
        p.rect_filled(logo, CornerRadius::same(3), theme::ACCENT);
        p.text(
            logo.center(),
            Align2::CENTER_CENTER,
            "K",
            FontId::new(10.0, egui::FontFamily::Name("bold".into())),
            Color32::WHITE,
        );

        let title = format!("{} — KateCraft", self.doc().title);
        p.text(
            Pos2::new(rect.center().x, rect.center().y),
            Align2::CENTER_CENTER,
            title,
            FontId::new(12.5, egui::FontFamily::Proportional),
            theme::TITLE_FG,
        );

        // window buttons
        let bw = 34.0;
        let mut x = rect.right() - 3.0 * bw;
        for (i, label) in ["min", "max", "close"].iter().enumerate() {
            let br = Rect::from_min_size(Pos2::new(x, rect.top()), Vec2::new(bw, rect.height()));
            let resp = ui.interact(br, Id::new(("wb", i)), Sense::click());
            if resp.hovered() {
                let col = if *label == "close" {
                    Color32::from_rgb(0xc0, 0x3a, 0x32)
                } else {
                    Color32::from_rgb(0x3b, 0x3f, 0x44)
                };
                ui.painter().rect_filled(br.shrink(2.0), CornerRadius::same(2), col);
            }
            let c = if resp.hovered() && *label == "close" {
                Color32::WHITE
            } else {
                theme::TITLE_FG
            };
            let cp = br.center();
            let pp = ui.painter();
            match *label {
                "min" => {
                    pp.line_segment(
                        [Pos2::new(cp.x - 5.0, cp.y + 4.0), Pos2::new(cp.x + 5.0, cp.y + 4.0)],
                        Stroke::new(1.0, c),
                    );
                }
                "max" => {
                    pp.rect_stroke(
                        Rect::from_center_size(cp, Vec2::splat(9.0)),
                        CornerRadius::same(1),
                        Stroke::new(1.0, c),
                        StrokeKind::Inside,
                    );
                }
                _ => {
                    pp.line_segment(
                        [Pos2::new(cp.x - 4.5, cp.y - 4.5), Pos2::new(cp.x + 4.5, cp.y + 4.5)],
                        Stroke::new(1.2, c),
                    );
                    pp.line_segment(
                        [Pos2::new(cp.x + 4.5, cp.y - 4.5), Pos2::new(cp.x - 4.5, cp.y + 4.5)],
                        Stroke::new(1.2, c),
                    );
                }
            }
            if resp.clicked() {
                match *label {
                    "min" => ctx.send_viewport_cmd(ViewportCommand::Minimized(true)),
                    "max" => {
                        let max = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(ViewportCommand::Maximized(!max));
                    }
                    _ => self.apply(Act::Quit, ctx),
                }
            }
            x += bw;
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut acts: Vec<Act> = Vec::new();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            ui.spacing_mut().button_padding = Vec2::new(9.0, 2.0);
            ui.add_space(4.0);
            ui.menu_button("File", |ui| {
                ui.set_min_width(230.0);
                if ui.button("New                    Ctrl+N").clicked() {
                    acts.push(Act::New);
                    ui.close();
                }
                if ui.button("Open…                  Ctrl+O").clicked() {
                    acts.push(Act::Open);
                    ui.close();
                }
                ui.separator();
                if ui.button("Save                   Ctrl+S").clicked() {
                    acts.push(Act::Save);
                    ui.close();
                }
                if ui.button("Save As…         Ctrl+Shift+S").clicked() {
                    acts.push(Act::SaveAs);
                    ui.close();
                }
                if ui.button("Reload").clicked() {
                    acts.push(Act::Reload);
                    ui.close();
                }
                ui.separator();
                if ui.button("Close Document   Ctrl+W").clicked() {
                    acts.push(Act::CloseDoc(self.active));
                    ui.close();
                }
                if ui.button("Quit                     Ctrl+Q").clicked() {
                    acts.push(Act::Quit);
                    ui.close();
                }
            });
            ui.menu_button("Edit", |ui| {
                ui.set_min_width(230.0);
                if ui.button("Undo                   Ctrl+Z").clicked() {
                    acts.push(Act::Undo);
                    ui.close();
                }
                if ui.button("Redo             Ctrl+Shift+Z").clicked() {
                    acts.push(Act::Redo);
                    ui.close();
                }
                ui.separator();
                if ui.button("Cut                    Ctrl+X").clicked() {
                    acts.push(Act::Cut);
                    ui.close();
                }
                if ui.button("Copy                   Ctrl+C").clicked() {
                    acts.push(Act::Copy);
                    ui.close();
                }
                if ui.button("Paste                  Ctrl+V").clicked() {
                    acts.push(Act::Paste);
                    ui.close();
                }
                ui.separator();
                if ui.button("Find                    Ctrl+F").clicked() {
                    acts.push(Act::Find);
                    ui.close();
                }
                if ui.button("Replace                  Ctrl+R").clicked() {
                    acts.push(Act::Replace);
                    ui.close();
                }
                if ui.button("Go to Line…      Ctrl+G").clicked() {
                    acts.push(Act::GoToLine);
                    ui.close();
                }
            });
            ui.menu_button("Selection", |ui| {
                ui.set_min_width(220.0);
                if ui.button("Select All             Ctrl+A").clicked() {
                    acts.push(Act::SelectAll);
                    ui.close();
                }
                if ui.button("Select Line").clicked() {
                    acts.push(Act::SelectLine);
                    ui.close();
                }
                ui.separator();
                if ui.button("Uppercase").clicked() {
                    acts.push(Act::Upper);
                    ui.close();
                }
                if ui.button("Lowercase").clicked() {
                    acts.push(Act::Lower);
                    ui.close();
                }
            });
            ui.menu_button("View", |ui| {
                ui.set_min_width(240.0);
                let mut menu = self.show_menu;
                if ui.checkbox(&mut menu, "Show Menu Bar").changed() {
                    self.show_menu = menu;
                }
                let mut tb = self.show_toolbar;
                if ui.checkbox(&mut tb, "Show Toolbar").changed() {
                    self.show_toolbar = tb;
                }
                let mut tab = self.show_tabbar;
                if ui.checkbox(&mut tab, "Show Tab Bar").changed() {
                    self.show_tabbar = tab;
                }
                let mut bc = self.show_breadcrumb;
                if ui.checkbox(&mut bc, "Show Path Bar").changed() {
                    self.show_breadcrumb = bc;
                }
                let mut st = self.show_status;
                if ui.checkbox(&mut st, "Show Status Bar").changed() {
                    self.show_status = st;
                }
                let mut side = self.sidebar;
                if ui.checkbox(&mut side, "Show Sidebar").changed() {
                    self.sidebar = side;
                }
                ui.separator();
                let mut mm = self.ed.show_minimap;
                if ui.checkbox(&mut mm, "Show Minimap").changed() {
                    self.ed.show_minimap = mm;
                }
                let mut ln = self.ed.show_line_numbers;
                if ui.checkbox(&mut ln, "Show Line Numbers").changed() {
                    self.ed.show_line_numbers = ln;
                }
                ui.separator();
                if ui.button("Zoom In            Ctrl++").clicked() {
                    acts.push(Act::ZoomIn);
                    ui.close();
                }
                if ui.button("Zoom Out           Ctrl+-").clicked() {
                    acts.push(Act::ZoomOut);
                    ui.close();
                }
                if ui.button("Reset Zoom").clicked() {
                    acts.push(Act::ZoomReset);
                    ui.close();
                }
            });
            ui.menu_button("Go", |ui| {
                ui.set_min_width(210.0);
                if ui.button("Next Tab            Ctrl+PgDn").clicked() {
                    acts.push(Act::NextTab);
                    ui.close();
                }
                if ui.button("Previous Tab    Ctrl+PgUp").clicked() {
                    acts.push(Act::PrevTab);
                    ui.close();
                }
                ui.separator();
                if ui.button("Go to Line…").clicked() {
                    acts.push(Act::GoToLine);
                    ui.close();
                }
                if ui.button("Start of Document").clicked() {
                    self.doc_mut().move_cursor(0, false);
                    ui.close();
                }
                if ui.button("End of Document").clicked() {
                    let n = self.doc().text.len();
                    self.doc_mut().move_cursor(n, false);
                    ui.close();
                }
            });
            ui.menu_button("Projects", |ui| {
                ui.set_min_width(220.0);
                if ui.button("Open Folder…").clicked() {
                    acts.push(Act::OpenFolder);
                    ui.close();
                }
                if ui.button("Close Folder").clicked() {
                    acts.push(Act::CloseFolder);
                    ui.close();
                }
                ui.separator();
                ui.label("Project folders appear in the sidebar.");
            });
            ui.menu_button("LSP Client", |ui| {
                ui.set_min_width(230.0);
                ui.label("No language server connected");
                ui.separator();
                ui.label("KateCraft has no language-server (LSP) support.");
            });
            ui.menu_button("Sessions", |ui| {
                ui.set_min_width(220.0);
                if ui.button("Save Session").clicked() {
                    acts.push(Act::SaveSession);
                    ui.close();
                }
                if ui.button("Restore Session").clicked() {
                    acts.push(Act::RestoreSession);
                    ui.close();
                }
            });
            ui.menu_button("Tools", |ui| {
                ui.set_min_width(230.0);
                if ui.button("Find…").clicked() {
                    acts.push(Act::Find);
                    ui.close();
                }
                if ui.button("Replace…").clicked() {
                    acts.push(Act::Replace);
                    ui.close();
                }
                ui.separator();
                if ui.button("Sort Selected Lines").clicked() {
                    acts.push(Act::SortLines);
                    ui.close();
                }
                if ui.button("Remove Trailing Whitespace").clicked() {
                    acts.push(Act::TrimWhitespace);
                    ui.close();
                }
                if ui.button("Uppercase Selection").clicked() {
                    acts.push(Act::Upper);
                    ui.close();
                }
                if ui.button("Lowercase Selection").clicked() {
                    acts.push(Act::Lower);
                    ui.close();
                }
            });
            ui.menu_button("Settings", |ui| {
                ui.set_min_width(230.0);
                let light = !self.pal.dark;
                if ui.radio(light, "Light colour scheme").clicked() {
                    acts.push(Act::Theme(false));
                    ui.close();
                }
                if ui.radio(!light, "Dark colour scheme").clicked() {
                    acts.push(Act::Theme(true));
                    ui.close();
                }
                ui.separator();
                ui.menu_button("Tab width", |ui| {
                    for w in [2usize, 4, 8] {
                        if ui.button(format!("{w} spaces")).clicked() {
                            acts.push(Act::TabWidth(w));
                            ui.close();
                        }
                    }
                });
                ui.menu_button("Syntax highlighting", |ui| {
                    for l in Lang::ALL {
                        if ui.button(l.name()).clicked() {
                            acts.push(Act::Lang(l));
                            ui.close();
                        }
                    }
                });
                ui.separator();
                if ui.button("Configure…").clicked() {
                    acts.push(Act::Settings);
                    ui.close();
                }
                if ui.button("Configure Shortcuts…").clicked() {
                    acts.push(Act::Shortcuts);
                    ui.close();
                }
            });
            ui.menu_button("Help", |ui| {
                ui.set_min_width(200.0);
                if ui.button("Keyboard Shortcuts").clicked() {
                    acts.push(Act::Shortcuts);
                    ui.close();
                }
                if ui.button("About KateCraft").clicked() {
                    acts.push(Act::About);
                    ui.close();
                }
            });
        });
        for a in acts {
            self.apply(a, ctx);
        }
    }

    fn tb_button(ui: &mut egui::Ui, pal: Palette, icon: Icon, label: &str, enabled: bool) -> Response {
        let font = FontId::new(12.5, egui::FontFamily::Proportional);
        let galley = ui.painter().layout_no_wrap(label.to_owned(), font.clone(), pal.text_main());
        let w = 10.0 + 16.0 + if label.is_empty() { 0.0 } else { 5.0 + galley.size().x };
        let h = 24.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, h), if enabled { Sense::click() } else { Sense::hover() });
        let hovered = resp.hovered() && enabled;
        if hovered {
            ui.painter().rect_filled(rect, CornerRadius::same(3), pal.chrome_hover());
        }
        let col = if enabled { pal.text_main() } else { pal.text_disabled() };
        paint_icon(ui.painter(), icon, Rect::from_min_size(Pos2::new(rect.left() + 4.0, rect.center().y - 8.0), Vec2::splat(16.0)), col);
        if !label.is_empty() {
            ui.painter().galley(
                Pos2::new(rect.left() + 4.0 + 16.0 + 5.0, rect.center().y - galley.size().y / 2.0),
                galley,
                col,
            );
        }
        resp
    }

    fn toolbar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let mut acts: Vec<Act> = Vec::new();
        let can_undo = !self.doc().undo.is_empty();
        let can_redo = !self.doc().redo.is_empty();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 3.0;
            ui.add_space(2.0);
            if Self::tb_button(ui, self.pal, Icon::NewFile, "New", true).clicked() {
                acts.push(Act::New);
            }
            if Self::tb_button(ui, self.pal, Icon::Folder, "Open", true).clicked() {
                acts.push(Act::Open);
            }
            if ui.allocate_exact_size(Vec2::splat(14.0), Sense::click()).1.clicked() {
                acts.push(Act::Open);
            }
            if Self::tb_button(ui, self.pal, Icon::Save, "Save", true).clicked() {
                acts.push(Act::Save);
            }
            if Self::tb_button(ui, self.pal, Icon::SaveAs, "Save As", true).clicked() {
                acts.push(Act::SaveAs);
            }
            let sep = ui.allocate_exact_size(Vec2::new(9.0, 20.0), Sense::hover()).0;
            ui.painter().line_segment(
                [Pos2::new(sep.center().x, sep.top()), Pos2::new(sep.center().x, sep.bottom())],
                Stroke::new(1.0, self.pal.sep()),
            );
            if Self::tb_button(ui, self.pal, Icon::Undo, "Undo", can_undo).clicked() {
                acts.push(Act::Undo);
            }
            if Self::tb_button(ui, self.pal, Icon::Redo, "Redo", can_redo).clicked() {
                acts.push(Act::Redo);
            }

            // search field pinned right
            let right = ui.max_rect().right() - 30.0;
            let field = Rect::from_min_size(
                Pos2::new(right - 220.0, ui.max_rect().center().y - 11.0),
                Vec2::new(220.0, 22.0),
            );
            let p = ui.painter();
            p.rect_filled(field, CornerRadius::same(11), self.pal.field_bg());
            p.rect_stroke(field, CornerRadius::same(11), Stroke::new(1.0, self.pal.border()), StrokeKind::Inside);
            paint_icon(
                ui.painter(),
                Icon::Search,
                Rect::from_center_size(Pos2::new(field.left() + 13.0, field.center().y), Vec2::splat(13.0)),
                self.pal.text_dim(),
            );
            let inner = Rect::from_min_max(
                Pos2::new(field.left() + 24.0, field.top() + 2.0),
                Pos2::new(field.right() - 20.0, field.bottom() - 2.0),
            );
            let mut search = self.search_field.clone();
            ui.put(
                inner,
                egui::TextEdit::singleline(&mut search)
                    .frame(egui::Frame::NONE)
                    .hint_text("Search")
                    .font(FontId::new(12.0, egui::FontFamily::Proportional))
                    .text_color(self.pal.text_main()),
            );
            if search != self.search_field {
                self.search_field = search;
                self.find.query = self.search_field.clone();
                self.refresh_matches();
                if !self.find.query.is_empty() {
                    self.find.open = true;
                    self.find.replace = false;
                }
            }
            let clr = Rect::from_center_size(Pos2::new(field.right() - 11.0, field.center().y), Vec2::splat(12.0));
            let cresp = ui.interact(clr, Id::new("search-clear"), Sense::click());
            if cresp.hovered() {
                paint_icon(ui.painter(), Icon::Close, clr, self.pal.text_dim());
            } else if !self.search_field.is_empty() {
                paint_icon(ui.painter(), Icon::Close, clr, self.pal.text_disabled());
            }
            if cresp.clicked() {
                self.search_field.clear();
                self.find.query.clear();
                self.refresh_matches();
                self.find.open = false;
            }
        });
        for a in acts {
            self.apply(a, ctx);
        }
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, self.pal.chrome());
        ui.painter().line_segment(
            [Pos2::new(rect.left(), rect.bottom()), Pos2::new(rect.right(), rect.bottom())],
            Stroke::new(1.0, self.pal.sep()),
        );

        let mut x = rect.left() + 20.0;
        let arrow = 22.0;
        // back / forward like the original's document navigation
        for (i, icon) in [Icon::ChevronLeft, Icon::ChevronRight].iter().enumerate() {
            let r = Rect::from_center_size(Pos2::new(x + arrow / 2.0, rect.center().y), Vec2::new(arrow, 20.0));
            let resp = ui.interact(r, Id::new(("nav", i)), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(2), self.pal.chrome_hover());
            }
            paint_icon(ui.painter(), *icon, r.shrink(5.0), self.pal.text_dim());
            if resp.clicked() {
                if i == 0 {
                    self.apply(Act::PrevTab, ctx);
                } else {
                    self.apply(Act::NextTab, ctx);
                }
            }
            x += arrow + 2.0;
        }
        x += 6.0;

        let mut close: Option<usize> = None;
        let mut goto: Option<usize> = None;
        let mut i = 0;
        while i < self.docs.len() {
            let title = self.docs[i].title.clone();
            let dirty = self.docs[i].dirty;
            let is_py = self.docs[i].lang == Lang::Python;
            let font = FontId::new(12.5, egui::FontFamily::Proportional);
            let galley = ui.painter().layout_no_wrap(
                title.clone(),
                font,
                if i == self.active { self.pal.text_main() } else { self.pal.text_dim() },
            );
            let tw = galley.size().x;
            let w = (28.0 + tw + 24.0).max(96.0);
            let tab = Rect::from_min_size(Pos2::new(x, rect.top() + 2.0), Vec2::new(w, rect.height() - 2.0));
            if tab.right() > rect.right() - 60.0 {
                break;
            }
            let active = i == self.active;
            if active {
                ui.painter().rect_filled(
                    tab,
                    CornerRadius {
                        nw: 3,
                        ne: 3,
                        sw: 0,
                        se: 0,
                    },
                    self.pal.editor_bg(),
                );
                ui.painter().rect_filled(
                    Rect::from_min_size(tab.min, Vec2::new(tab.width(), 2.0)),
                    CornerRadius::ZERO,
                    theme::ACCENT,
                );
            }
            // language badge
            let badge = Rect::from_center_size(Pos2::new(tab.left() + 15.0, tab.center().y), Vec2::splat(13.0));
            if is_py {
                paint_icon(ui.painter(), Icon::Py, badge, theme::ACCENT);
            } else {
                paint_icon(ui.painter(), Icon::File, badge, self.pal.text_dim());
            }
            ui.painter().galley(
                Pos2::new(tab.left() + 26.0, tab.center().y - galley.size().y / 2.0),
                galley,
                if active { self.pal.text_main() } else { self.pal.text_dim() },
            );
            if dirty {
                ui.painter().circle_filled(
                    Pos2::new(tab.left() + 26.0 + tw + 6.0, tab.center().y),
                    2.2,
                    self.pal.text_dim(),
                );
            }
            let close_rect = Rect::from_center_size(
                Pos2::new(tab.right() - 13.0, tab.center().y),
                Vec2::splat(14.0),
            );
            let crest = ui.interact(close_rect, Id::new(("tab-close", i)), Sense::click());
            let ccol = if crest.hovered() {
                ui.painter().circle_filled(close_rect.center(), 7.0, self.pal.chrome_hover());
                self.pal.text_main()
            } else if active {
                self.pal.text_dim()
            } else {
                self.pal.text_disabled()
            };
            paint_icon(ui.painter(), Icon::Close, close_rect.shrink(3.5), ccol);
            if crest.clicked() {
                close = Some(i);
            }
            let trest = ui.interact(tab, Id::new(("tab", i)), Sense::click());
            if trest.clicked() {
                goto = Some(i);
            }
            if trest.middle_clicked() {
                close = Some(i);
            }
            if active {
                ui.painter().line_segment(
                    [
                        Pos2::new(tab.left(), tab.bottom() - 0.5),
                        Pos2::new(tab.right(), tab.bottom() - 0.5),
                    ],
                    Stroke::new(1.0, self.pal.editor_bg()),
                );
            }
            x = tab.right() + 1.0;
            i += 1;
        }

        // right-hand icons
        let mut rx = rect.right() - 14.0;
        for icon in [Icon::Menu, Icon::Split] {
            let r = Rect::from_center_size(Pos2::new(rx, rect.center().y), Vec2::splat(20.0));
            let resp = ui.interact(r, Id::new(("tabicon", rx as i32)), Sense::click());
            if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(2), self.pal.chrome_hover());
            }
            paint_icon(ui.painter(), icon, r.shrink(5.0), self.pal.text_dim());
            if resp.clicked() && icon == Icon::Menu {
                // toggle sidebar as the "view options" affordance
                self.sidebar = !self.sidebar;
            }
            rx -= 22.0;
        }

        if let Some(i) = goto {
            self.active = i;
        }
        if let Some(i) = close {
            self.apply(Act::CloseDoc(i), ctx);
        }
    }

    fn breadcrumb(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, self.pal.breadcrumb_bg());
        let font = FontId::new(11.5, egui::FontFamily::Proportional);
        let mut x = rect.left() + 24.0;
        let y = rect.center().y;

        let segments: Vec<String> = match &self.doc().path {
            Some(p) => {
                let mut v = vec![];
                for c in p.components() {
                    let s = c.as_os_str().to_string_lossy().to_string();
                    v.push(if s == "/" { "root".to_string() } else { s });
                }
                v
            }
            None => vec![self.doc().title.clone()],
        };

        for (i, seg) in segments.iter().enumerate() {
            let last = i + 1 == segments.len();
            let galley = ui.painter().layout_no_wrap(
                seg.clone(),
                font.clone(),
                if last { self.pal.text_main() } else { self.pal.text_dim() },
            );
            let w = galley.size().x;
            if x + w > rect.right() - 120.0 {
                break;
            }
            if last {
                let badge = Rect::from_center_size(Pos2::new(x + 6.0, y), Vec2::splat(12.0));
                if self.doc().lang == Lang::Python {
                    paint_icon(ui.painter(), Icon::Py, badge, theme::ACCENT);
                } else {
                    paint_icon(ui.painter(), Icon::File, badge, self.pal.text_dim());
                }
                x += 15.0;
            }
            ui.painter().galley(Pos2::new(x, y - galley.size().y / 2.0), galley, self.pal.text_dim());
            x += w + 6.0;
            if !last {
                ui.painter().text(
                    Pos2::new(x, y),
                    Align2::LEFT_CENTER,
                    "›",
                    FontId::new(12.0, egui::FontFamily::Proportional),
                    self.pal.text_disabled(),
                );
                x += 10.0;
            }
        }
    }

    fn rail(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, self.pal.rail_bg());
        ui.painter().line_segment(
            [Pos2::new(rect.right() - 0.5, rect.top()), Pos2::new(rect.right() - 0.5, rect.bottom())],
            Stroke::new(1.0, self.pal.sep()),
        );
        let items = [
            (Icon::Doc, SidebarPanel::Documents),
            (Icon::List, SidebarPanel::Filesystem),
            (Icon::Diamond, SidebarPanel::Symbols),
            (Icon::Plug, SidebarPanel::Plugins),
        ];
        let mut y = rect.top() + 6.0;
        for (icon, panel) in items {
            let r = Rect::from_min_size(Pos2::new(rect.left() + 4.0, y), Vec2::new(rect.width() - 8.0, 26.0));
            let resp = ui.interact(r, Id::new(("rail", r.top() as i32)), Sense::click());
            let sel = self.sidebar && self.panel == panel;
            if sel {
                ui.painter().rect_filled(r, CornerRadius::same(3), theme::ACCENT.gamma_multiply(0.18));
            } else if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(3), self.pal.chrome_hover());
            }
            paint_icon(
                ui.painter(),
                icon,
                Rect::from_center_size(r.center(), Vec2::splat(16.0)),
                if sel { theme::ACCENT } else { self.pal.text_main() },
            );
            if resp.clicked() {
                self.apply(Act::Panel(panel), ctx);
            }
            y += 30.0;
        }
    }

    fn sidebar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, self.pal.chrome());
        ui.painter().line_segment(
            [Pos2::new(rect.right() - 0.5, rect.top()), Pos2::new(rect.right() - 0.5, rect.bottom())],
            Stroke::new(1.0, self.pal.sep()),
        );
        let mut acts: Vec<Act> = Vec::new();
        let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 26.0));
        ui.painter().rect_filled(header, CornerRadius::ZERO, self.pal.chrome());
        let title = match self.panel {
            SidebarPanel::Documents => "Documents",
            SidebarPanel::Filesystem => "Filesystem",
            SidebarPanel::Symbols => "Symbols",
            SidebarPanel::Plugins => "Plugins",
        };
        ui.painter().text(
            Pos2::new(header.left() + 12.0, header.center().y),
            Align2::LEFT_CENTER,
            title,
            FontId::new(12.0, egui::FontFamily::Name("bold".into())),
            self.pal.text_main(),
        );
        let closer = Rect::from_center_size(Pos2::new(header.right() - 14.0, header.center().y), Vec2::splat(16.0));
        let cresp = ui.interact(closer, Id::new("sidebar-close"), Sense::click());
        if cresp.hovered() {
            ui.painter().rect_filled(closer, CornerRadius::same(2), self.pal.chrome_hover());
        }
        paint_icon(ui.painter(), Icon::Close, closer.shrink(4.0), self.pal.text_dim());
        if cresp.clicked() {
            self.sidebar = false;
        }

        let body = Rect::from_min_max(
            Pos2::new(rect.left() + 6.0, header.bottom() + 2.0),
            Pos2::new(rect.right() - 6.0, rect.bottom() - 4.0),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(body), |ui| {
            match self.panel {
                SidebarPanel::Documents => {
                    let mut y = body.top();
                    for (i, d) in self.docs.iter().enumerate() {
                        let r = Rect::from_min_size(Pos2::new(body.left(), y), Vec2::new(body.width(), 22.0));
                        let resp = ui.interact(r, Id::new(("docrow", i)), Sense::click());
                        if i == self.active {
                            ui.painter().rect_filled(r, CornerRadius::same(2), theme::ACCENT.gamma_multiply(0.18));
                        } else if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(2), self.pal.chrome_hover());
                        }
                        paint_icon(
                            ui.painter(),
                            if d.lang == Lang::Python { Icon::Py } else { Icon::File },
                            Rect::from_center_size(Pos2::new(r.left() + 12.0, r.center().y), Vec2::splat(12.0)),
                            if i == self.active { theme::ACCENT } else { self.pal.text_dim() },
                        );
                        let name = if d.dirty { format!("{} *", d.title) } else { d.title.clone() };
                        ui.painter().text(
                            Pos2::new(r.left() + 24.0, r.center().y),
                            Align2::LEFT_CENTER,
                            name,
                            FontId::new(12.0, egui::FontFamily::Proportional),
                            self.pal.text_main(),
                        );
                        if resp.clicked() {
                            acts.push(Act::Goto(i));
                        }
                        y += 23.0;
                    }
                }
                SidebarPanel::Filesystem => {
                    let root = self.fs_root.clone();
                    if let Some(root) = root {
                        let mut y = body.top();
                        self.fs_row(ui, &root, 0, body.left(), &mut y, body, &mut acts);
                    } else {
                        ui.painter().text(
                            Pos2::new(body.left() + 6.0, body.top() + 8.0),
                            Align2::LEFT_TOP,
                            "No folder open.\nUse Projects ▸ Open Folder.",
                            FontId::new(11.5, egui::FontFamily::Proportional),
                            self.pal.text_dim(),
                        );
                    }
                }
                SidebarPanel::Symbols => {
                    let mut y = body.top();
                    let mut items: Vec<(String, usize)> = Vec::new();
                    for (ln, text) in self.doc().text.split('\n').enumerate() {
                        let t = text.trim_start();
                        let name = if let Some(rest) = t.strip_prefix("def ") {
                            Some(rest.split('(').next().unwrap_or(rest).to_string())
                        } else if let Some(rest) = t.strip_prefix("class ") {
                            Some(format!("class {}", rest.split(['(', ':']).next().unwrap_or(rest)))
                        } else if let Some(rest) = t.strip_prefix("fn ") {
                            Some(rest.split('(').next().unwrap_or(rest).to_string())
                        } else if let Some(rest) = t.strip_prefix("pub fn ") {
                            Some(rest.split('(').next().unwrap_or(rest).to_string())
                        } else if let Some(rest) = t.strip_prefix("function ") {
                            Some(rest.split('(').next().unwrap_or(rest).to_string())
                        } else {
                            None
                        };
                        if let Some(n) = name {
                            items.push((n, ln));
                        }
                    }
                    if items.is_empty() {
                        ui.painter().text(
                            Pos2::new(body.left() + 6.0, body.top() + 8.0),
                            Align2::LEFT_TOP,
                            "No symbols found.",
                            FontId::new(11.5, egui::FontFamily::Proportional),
                            self.pal.text_dim(),
                        );
                    }
                    for (name, ln) in items {
                        if y + 20.0 > body.bottom() {
                            break;
                        }
                        let r = Rect::from_min_size(Pos2::new(body.left(), y), Vec2::new(body.width(), 20.0));
                        let resp = ui.interact(r, Id::new(("sym", ln)), Sense::click());
                        if resp.hovered() {
                            ui.painter().rect_filled(r, CornerRadius::same(2), self.pal.chrome_hover());
                        }
                        paint_icon(
                            ui.painter(),
                            Icon::Diamond,
                            Rect::from_center_size(Pos2::new(r.left() + 10.0, r.center().y), Vec2::splat(11.0)),
                            theme::SYN_TYPE,
                        );
                        ui.painter().text(
                            Pos2::new(r.left() + 22.0, r.center().y),
                            Align2::LEFT_CENTER,
                            name,
                            FontId::new(12.0, egui::FontFamily::Proportional),
                            self.pal.text_main(),
                        );
                        ui.painter().text(
                            Pos2::new(r.right() - 6.0, r.center().y),
                            Align2::RIGHT_CENTER,
                            format!("{}", ln + 1),
                            FontId::new(10.5, egui::FontFamily::Monospace),
                            self.pal.text_disabled(),
                        );
                        if resp.clicked() {
                            self.jump_to_line(ln + 1);
                        }
                        y += 21.0;
                    }
                }
                SidebarPanel::Plugins => {
                    let mut y = body.top();
                    let items = [
                        ("Syntax highlighting", self.doc().lang != Lang::Plain),
                        ("Minimap", self.ed.show_minimap),
                        ("Line numbers", self.ed.show_line_numbers),
                        ("Find and replace", true),
                        ("Documents panel", true),
                    ];
                    for (name, on) in items {
                        let r = Rect::from_min_size(Pos2::new(body.left(), y), Vec2::new(body.width(), 22.0));
                        let cb = Rect::from_center_size(Pos2::new(r.left() + 10.0, r.center().y), Vec2::splat(14.0));
                        ui.painter().rect_stroke(cb, CornerRadius::same(2), Stroke::new(1.0, self.pal.border()), StrokeKind::Inside);
                        if on {
                            paint_icon(ui.painter(), Icon::Check, cb.shrink(2.0), theme::ACCENT);
                        }
                        ui.painter().text(
                            Pos2::new(r.left() + 24.0, r.center().y),
                            Align2::LEFT_CENTER,
                            name,
                            FontId::new(12.0, egui::FontFamily::Proportional),
                            self.pal.text_main(),
                        );
                        y += 23.0;
                    }
                }
            }
        });
        for a in acts {
            self.apply(a, ctx);
        }
    }

    fn fs_row(
        &mut self,
        ui: &mut egui::Ui,
        path: &Path,
        depth: usize,
        left: f32,
        y: &mut f32,
        bounds: Rect,
        acts: &mut Vec<Act>,
    ) {
        if *y + 20.0 > bounds.bottom() || depth > 6 {
            return;
        }
        let is_dir = path.is_dir();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("/")
            .to_string();
        let r = Rect::from_min_size(
            Pos2::new(left + depth as f32 * 12.0, *y),
            Vec2::new(bounds.right() - (left + depth as f32 * 12.0), 20.0),
        );
        let resp = ui.interact(r, Id::new(("fs", path.to_string_lossy().to_string())), Sense::click());
        if resp.hovered() {
            ui.painter().rect_filled(r, CornerRadius::same(2), self.pal.chrome_hover());
        }
        let open = self.expanded.contains(path);
        if is_dir {
            paint_icon(
                ui.painter(),
                if open { Icon::ChevronDown } else { Icon::ChevronRight },
                Rect::from_center_size(Pos2::new(r.left() + 6.0, r.center().y), Vec2::splat(11.0)),
                self.pal.text_dim(),
            );
            paint_icon(
                ui.painter(),
                if open { Icon::FolderOpen } else { Icon::Folder },
                Rect::from_center_size(Pos2::new(r.left() + 20.0, r.center().y), Vec2::splat(12.0)),
                theme::ACCENT,
            );
        } else {
            paint_icon(
                ui.painter(),
                Icon::File,
                Rect::from_center_size(Pos2::new(r.left() + 20.0, r.center().y), Vec2::splat(12.0)),
                self.pal.text_dim(),
            );
        }
        ui.painter().text(
            Pos2::new(r.left() + 30.0, r.center().y),
            Align2::LEFT_CENTER,
            name,
            FontId::new(12.0, egui::FontFamily::Proportional),
            self.pal.text_main(),
        );
        if resp.clicked() {
            if is_dir {
                if open {
                    self.expanded.remove(path);
                } else {
                    self.expanded.insert(path.to_path_buf());
                    if !self.listings.contains_key(path) {
                        let mut kids: Vec<PathBuf> = std::fs::read_dir(path)
                            .map(|rd| {
                                rd.flatten()
                                    .map(|e| e.path())
                                    .filter(|p| {
                                        p.file_name()
                                            .and_then(|n| n.to_str())
                                            .map(|n| !n.starts_with('.'))
                                            .unwrap_or(false)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        kids.sort_by_key(|p| (!p.is_dir(), p.file_name().map(|n| n.to_os_string())));
                        self.listings.insert(path.to_path_buf(), kids);
                    }
                }
            } else {
                let p = path.to_path_buf();
                if let Some(i) = self.docs.iter().position(|d| d.path.as_deref() == Some(path)) {
                    acts.push(Act::Goto(i));
                } else {
                    self.open_path(&p);
                }
            }
        }
        *y += 21.0;
        if is_dir && open {
            let kids = self.listings.get(path).cloned().unwrap_or_default();
            for k in kids {
                self.fs_row(ui, &k, depth + 1, left, y, bounds, acts);
            }
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, self.pal.status_bg());
        ui.painter().line_segment(
            [Pos2::new(rect.left(), rect.top() + 0.5), Pos2::new(rect.right(), rect.top() + 0.5)],
            Stroke::new(1.0, self.pal.sep()),
        );
        let mut x = rect.left() + 8.0;
        let mut acts: Vec<Act> = Vec::new();
        for (i, (name, icon)) in TOOL_VIEWS.iter().enumerate() {
            let font = FontId::new(12.0, egui::FontFamily::Proportional);
            let galley = ui.painter().layout_no_wrap(name.to_string(), font, self.pal.text_main());
            let w = 8.0 + 14.0 + 5.0 + galley.size().x + 6.0;
            let r = Rect::from_min_size(
                Pos2::new(x, rect.top() + 2.0),
                Vec2::new(w, rect.height() - 4.0),
            );
            let resp = ui.interact(r, Id::new(("status", i)), Sense::click());
            let active = self.tool_view == Some(i);
            if active {
                ui.painter().rect_filled(r, CornerRadius::same(2), theme::ACCENT.gamma_multiply(0.16));
            } else if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(2), self.pal.chrome_hover());
            }
            let col = if active { theme::ACCENT } else { self.pal.text_dim() };
            paint_icon(
                ui.painter(),
                *icon,
                Rect::from_min_size(Pos2::new(r.left() + 6.0, r.center().y - 7.0), Vec2::splat(14.0)),
                col,
            );
            ui.painter().galley(
                Pos2::new(r.left() + 8.0 + 14.0 + 5.0, r.center().y - galley.size().y / 2.0),
                galley,
                if active { self.pal.text_main() } else { self.pal.text_dim() },
            );
            if resp.clicked() {
                acts.push(Act::ToolView(i));
            }
            x += w + 4.0;
        }

        // right side tokens
        let d = self.doc();
        let line = d.line_of(d.cursor) + 1;
        let col = d.col_of(d.cursor) + 1;
        let tokens = [
            format!("{line}:{col}"),
            "INSERT".to_string(),
            format!("Soft Tabs: {}", self.ed.tab_width),
            "UTF-8".to_string(),
            d.lang.name().to_string(),
        ];
        let font = FontId::new(12.0, egui::FontFamily::Proportional);
        let mut widths = vec![];
        for t in &tokens {
            widths.push(ui.painter().layout_no_wrap(t.clone(), font.clone(), self.pal.text_main()).size().x);
        }
        let mut rx = rect.right() - 12.0;
        for i in (0..tokens.len()).rev() {
            let w = widths[i];
            rx -= w;
            ui.painter().text(
                Pos2::new(rx, rect.center().y),
                Align2::LEFT_CENTER,
                &tokens[i],
                font.clone(),
                self.pal.text_dim(),
            );
            rx -= 22.0;
        }

        // clickable encoding / syntax chips
        let lang_rect = Rect::from_min_max(
            Pos2::new(rect.right() - 12.0 - widths[4] - 4.0, rect.top() + 2.0),
            Pos2::new(rect.right() - 10.0, rect.bottom() - 2.0),
        );
        let lresp = ui.interact(lang_rect, Id::new("status-lang"), Sense::click());
        if lresp.hovered() {
            ui.painter().rect_filled(lang_rect, CornerRadius::same(2), self.pal.chrome_hover());
        }
        for a in acts {
            self.apply(a, ctx);
        }
    }

    fn find_bar(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, self.pal.chrome());
        ui.painter().line_segment(
            [Pos2::new(rect.left(), rect.top() + 0.5), Pos2::new(rect.right(), rect.top() + 0.5)],
            Stroke::new(1.0, self.pal.sep()),
        );
        let mut acts: Vec<Act> = Vec::new();
        let row_h = 30.0;
        let mut y = rect.top() + 3.0;

        let pal = self.pal;
        let mut draw_field = move |ui: &mut egui::Ui, y: f32, value: &mut String, hint: &str, id: &str| {
            let r = Rect::from_min_size(Pos2::new(rect.left() + 36.0, y), Vec2::new(300.0, row_h - 6.0));
            ui.painter().rect_filled(r, CornerRadius::same(3), pal.field_bg());
            ui.painter().rect_stroke(r, CornerRadius::same(3), Stroke::new(1.0_f32, pal.border()), StrokeKind::Inside);
            ui.put(
                r.shrink2(Vec2::new(6.0, 2.0)),
                egui::TextEdit::singleline(value)
                    .frame(egui::Frame::NONE)
                    .hint_text(hint)
                    .id(Id::new(id))
                    .font(FontId::new(12.0, egui::FontFamily::Proportional)),
            );
            r
        };

        paint_icon(
            ui.painter(),
            Icon::Search,
            Rect::from_center_size(Pos2::new(rect.left() + 20.0, y + row_h / 2.0 - 3.0), Vec2::splat(14.0)),
            self.pal.text_dim(),
        );
        let mut q = self.find.query.clone();
        draw_field(ui, y, &mut q, "Find", "find-field");
        if q != self.find.query {
            self.find.query = q;
            self.find.current = 0;
            self.refresh_matches();
            self.step_match(0);
        }

        // count
        let txt = if self.find.matches.is_empty() {
            "No matches".to_string()
        } else {
            format!("{} of {}", self.find.current + 1, self.find.matches.len())
        };
        ui.painter().text(
            Pos2::new(rect.left() + 346.0, y + row_h / 2.0 - 3.0),
            Align2::LEFT_CENTER,
            txt,
            FontId::new(11.5, egui::FontFamily::Proportional),
            self.pal.text_dim(),
        );

        // buttons
        let mut bx = rect.left() + 430.0;
        for (icon, act, tip) in [
            (Icon::ArrowUp, Act::FindPrev, "Previous match"),
            (Icon::ArrowDown, Act::FindNext, "Next match"),
        ] {
            let r = Rect::from_center_size(Pos2::new(bx + 11.0, y + row_h / 2.0 - 3.0), Vec2::splat(22.0));
            let resp = ui.interact(r, Id::new(("findbtn", bx as i32)), Sense::click()).on_hover_text(tip);
            if resp.hovered() {
                ui.painter().rect_filled(r, CornerRadius::same(3), self.pal.chrome_hover());
            }
            paint_icon(ui.painter(), icon, r.shrink(5.0), self.pal.text_main());
            if resp.clicked() {
                acts.push(act);
            }
            bx += 24.0;
        }
        // case toggle
        let cr = Rect::from_center_size(Pos2::new(bx + 14.0, y + row_h / 2.0 - 3.0), Vec2::new(30.0, 22.0));
        let cresp = ui.interact(cr, Id::new("find-case"), Sense::click()).on_hover_text("Match case");
        if self.find.case || cresp.hovered() {
            ui.painter().rect_filled(cr, CornerRadius::same(3), if self.find.case {
                theme::ACCENT.gamma_multiply(0.22)
            } else {
                self.pal.chrome_hover()
            });
        }
        ui.painter().text(
            cr.center(),
            Align2::CENTER_CENTER,
            "Aa",
            FontId::new(12.0, egui::FontFamily::Name("bold".into())),
            if self.find.case { theme::ACCENT } else { self.pal.text_dim() },
        );
        if cresp.clicked() {
            self.find.case = !self.find.case;
            self.refresh_matches();
        }
        bx += 40.0;

        let tr = Rect::from_center_size(Pos2::new(bx + 30.0, y + row_h / 2.0 - 3.0), Vec2::new(64.0, 22.0));
        let tresp = ui.interact(tr, Id::new("find-replace-toggle"), Sense::click());
        if tresp.hovered() || self.find.replace {
            ui.painter().rect_filled(
                tr,
                CornerRadius::same(3),
                if self.find.replace { theme::ACCENT.gamma_multiply(0.18) } else { self.pal.chrome_hover() },
            );
        }
        ui.painter().text(
            tr.center(),
            Align2::CENTER_CENTER,
            "Replace",
            FontId::new(12.0, egui::FontFamily::Proportional),
            self.pal.text_main(),
        );
        if tresp.clicked() {
            self.find.replace = !self.find.replace;
        }

        let close_r = Rect::from_center_size(Pos2::new(rect.right() - 18.0, y + row_h / 2.0 - 3.0), Vec2::splat(22.0));
        let closeresp = ui.interact(close_r, Id::new("find-close"), Sense::click());
        if closeresp.hovered() {
            ui.painter().rect_filled(close_r, CornerRadius::same(3), self.pal.chrome_hover());
        }
        paint_icon(ui.painter(), Icon::Close, close_r.shrink(5.0), self.pal.text_dim());
        if closeresp.clicked() {
            acts.push(Act::CloseFind);
        }

        if self.find.replace {
            y += row_h;
            let mut w = self.find.with.clone();
            draw_field(ui, y, &mut w, "Replace with", "replace-field");
            self.find.with = w;
            let mut rx = rect.left() + 350.0;
            for (label, act) in [("Replace", Act::ReplaceOne), ("Replace All", Act::ReplaceAll)] {
                let font = FontId::new(12.0, egui::FontFamily::Proportional);
                let galley = ui.painter().layout_no_wrap(label.to_string(), font, self.pal.text_main());
                let r = Rect::from_min_size(
                    Pos2::new(rx, y + 1.0),
                    Vec2::new(galley.size().x + 18.0, row_h - 8.0),
                );
                let resp = ui.interact(r, Id::new(("repbtn", label)), Sense::click());
                if resp.hovered() {
                    ui.painter().rect_filled(r, CornerRadius::same(3), self.pal.chrome_hover());
                }
                ui.painter().galley(
                    Pos2::new(r.left() + 9.0, r.center().y - galley.size().y / 2.0),
                    galley,
                    self.pal.text_main(),
                );
                if resp.clicked() {
                    acts.push(act);
                }
                rx += r.width() + 8.0;
            }
        }

        for a in acts {
            self.apply(a, ctx);
        }
    }

    fn tool_panel(&mut self, ui: &mut egui::Ui) {
        let rect = ui.max_rect();
        ui.painter().rect_filled(rect, CornerRadius::ZERO, self.pal.editor_bg());
        ui.painter().line_segment(
            [Pos2::new(rect.left(), rect.top() + 0.5), Pos2::new(rect.right(), rect.top() + 0.5)],
            Stroke::new(1.0, self.pal.sep()),
        );
        let idx = self.tool_view.unwrap_or(0);
        let font = FontId::new(12.0, egui::FontFamily::Monospace);
        let mut y = rect.top() + 6.0;
        let lines: Vec<String> = match idx {
            0 => self.output_log.iter().rev().take(40).rev().cloned().collect(),
            1 => vec!["No problems found in this document.".into()],
            2 => {
                if self.find.query.is_empty() {
                    vec!["Type in the search field to list matches.".into()]
                } else {
                    self.find
                        .matches
                        .iter()
                        .take(60)
                        .map(|(a, _)| {
                            let line = self.doc().line_of(*a) + 1;
                            let col = self.doc().col_of(*a) + 1;
                            format!("{}:{}  {}", line, col, self.doc().line_text(line - 1).trim())
                        })
                        .collect()
                }
            }
            3 => {
                let mut v = vec![];
                if let Some(root) = &self.fs_root {
                    v.push(root.display().to_string());
                    if let Ok(rd) = std::fs::read_dir(root) {
                        for e in rd.flatten().take(40) {
                            let p = e.path();
                            v.push(format!(
                                "  {}{}",
                                p.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                                if p.is_dir() { "/" } else { "" }
                            ));
                        }
                    }
                }
                v
            }
            _ => vec![
                "The terminal tool view is not part of KateCraft.".into(),
                "Use your system terminal alongside the editor.".into(),
            ],
        };
        for l in lines {
            if y > rect.bottom() - 4.0 {
                break;
            }
            ui.painter().text(
                Pos2::new(rect.left() + 12.0, y),
                Align2::LEFT_TOP,
                l,
                font.clone(),
                self.pal.text_main(),
            );
            y += 17.0;
        }
    }

    // ------------------------------------------------------------ dialogs ---

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.dialog.is_none() {
            return;
        }
        let mut close = false;
        let mut acts: Vec<Act> = Vec::new();
        let mut new_dialog: Option<Dialog> = None;
        let mut confirmed: Option<Act2> = None;
        let pal = self.pal;

        match self.dialog.as_mut().unwrap() {
            Dialog::About => {
                dialog_shell(ctx, "About", 410.0, pal, &mut close, |ui, pal, close| {
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(Vec2::splat(48.0), Sense::hover());
                        paint_mark(ui.painter(), rect, true);
                        ui.add_space(14.0);
                        ui.vertical(|ui| {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("KateCraft")
                                    .size(22.0)
                                    .strong()
                                    .color(pal.text_main()),
                            );
                            ui.label(
                                egui::RichText::new(format!("Version {}", env!("CARGO_PKG_VERSION")))
                                    .size(11.5)
                                    .color(pal.text_dim()),
                            );
                        });
                    });
                    ui.add_space(16.0);
                    ui.label(
                        egui::RichText::new(
                            "A text and code editor with tabs, syntax colours, find and replace, \
                             and a file browser.",
                        )
                        .color(pal.text_main()),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(
                            "Written in Rust with egui. Built from a description of the original editor.",
                        )
                        .size(12.0)
                        .color(pal.text_dim()),
                    );
                    ui.add_space(18.0);
                    button_row(ui, |ui| {
                        if dialog_button(ui, pal, "Close", true) {
                            *close = true;
                        }
                    });
                });
            }
            Dialog::Shortcuts => {
                dialog_shell(ctx, "Configure Shortcuts", 430.0, pal, &mut close, |ui, pal, close| {
                    let rows = [
                        ("New document", "Ctrl+N"),
                        ("Open", "Ctrl+O"),
                        ("Save", "Ctrl+S"),
                        ("Save as", "Ctrl+Shift+S"),
                        ("Close document", "Ctrl+W"),
                        ("Quit", "Ctrl+Q"),
                        ("Find", "Ctrl+F"),
                        ("Replace", "Ctrl+R"),
                        ("Go to line", "Ctrl+G"),
                        ("Undo / Redo", "Ctrl+Z / Ctrl+Shift+Z"),
                        ("Select all", "Ctrl+A"),
                        ("Next / previous tab", "Ctrl+PgDn / Ctrl+PgUp"),
                        ("Zoom in / out", "Ctrl++ / Ctrl+-"),
                    ];
                    egui::Grid::new("shortcut-grid")
                        .num_columns(2)
                        .spacing([26.0, 7.0])
                        .show(ui, |ui| {
                            for (a, b) in rows {
                                ui.label(egui::RichText::new(a).color(pal.text_main()));
                                ui.label(
                                    egui::RichText::new(b)
                                        .monospace()
                                        .size(12.0)
                                        .color(pal.text_dim()),
                                );
                                ui.end_row();
                            }
                        });
                    ui.add_space(18.0);
                    button_row(ui, |ui| {
                        if dialog_button(ui, pal, "Close", true) {
                            *close = true;
                        }
                    });
                });
            }
            Dialog::Settings => {
                dialog_shell(ctx, "Configure KateCraft", 430.0, pal, &mut close, |ui, pal, close| {
                    section_label(ui, pal, "Appearance");
                    let mut dark = pal.dark;
                    if check_row(ui, pal, "Dark colour scheme", &mut dark) {
                        acts.push(Act::Theme(dark));
                    }
                    ui.add_space(10.0);
                    section_label(ui, pal, "Editor");
                    let mut ln = self.ed.show_line_numbers;
                    if check_row(ui, pal, "Show line numbers", &mut ln) {
                        self.ed.show_line_numbers = ln;
                    }
                    let mut mm = self.ed.show_minimap;
                    if check_row(ui, pal, "Show minimap", &mut mm) {
                        self.ed.show_minimap = mm;
                    }
                    ui.add_space(10.0);
                    egui::Grid::new("settings-grid")
                        .num_columns(2)
                        .spacing([18.0, 10.0])
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new("Tab width").color(pal.text_main()));
                            ui.horizontal(|ui| {
                                for w in [2usize, 4, 8] {
                                    if chip(ui, pal, &format!("{w}"), self.ed.tab_width == w) {
                                        self.ed.tab_width = w;
                                    }
                                }
                                ui.label(
                                    egui::RichText::new("spaces").size(11.5).color(pal.text_dim()),
                                );
                            });
                            ui.end_row();
                            ui.label(egui::RichText::new("Font size").color(pal.text_main()));
                            ui.horizontal(|ui| {
                                if chip(ui, pal, "\u{2212}", false) {
                                    self.ed.font_size = (self.ed.font_size - 1.0).max(8.0);
                                }
                                ui.add(
                                    egui::DragValue::new(&mut self.ed.font_size)
                                        .speed(0.25)
                                        .range(8.0..=24.0)
                                        .fixed_decimals(1)
                                        .suffix(" px"),
                                );
                                if chip(ui, pal, "+", false) {
                                    self.ed.font_size = (self.ed.font_size + 1.0).min(24.0);
                                }
                            });
                            ui.end_row();
                        });
                    ui.add_space(18.0);
                    button_row(ui, |ui| {
                        if dialog_button(ui, pal, "Done", true) {
                            *close = true;
                        }
                    });
                });
            }
            Dialog::Message(title, body) => {
                let t = title.clone();
                let b = body.clone();
                dialog_shell(ctx, &t, 420.0, pal, &mut close, |ui, pal, close| {
                    ui.label(egui::RichText::new(b).color(pal.text_main()));
                    ui.add_space(16.0);
                    button_row(ui, |ui| {
                        if dialog_button(ui, pal, "OK", true) {
                            *close = true;
                        }
                    });
                });
            }
            Dialog::GoToLine(text) => {
                dialog_shell(ctx, "Go to Line", 320.0, pal, &mut close, |ui, pal, close| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Line number").color(pal.text_main()));
                        let r = ui.add(
                            egui::TextEdit::singleline(text)
                                .desired_width(96.0)
                                .frame(field_frame(pal)),
                        );
                        r.request_focus();
                    });
                    ui.add_space(14.0);
                    button_row(ui, |ui| {
                        let go = dialog_button(ui, pal, "Go", true);
                        if dialog_button(ui, pal, "Cancel", false) {
                            *close = true;
                        }
                        if go || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            if let Ok(n) = text.trim().parse::<usize>() {
                                new_dialog = Some(Dialog::Message("__goto__".into(), n.to_string()));
                            }
                            *close = true;
                        }
                    });
                });
            }
            Dialog::Confirm(title, body, act) => {
                let t = title.to_string();
                let b = body.clone();
                let act = *act;
                dialog_shell(ctx, &t, 440.0, pal, &mut close, |ui, pal, close| {
                    ui.label(egui::RichText::new(b).color(pal.text_main()));
                    ui.add_space(16.0);
                    button_row(ui, |ui| {
                        let go = dialog_button(ui, pal, "Continue", true);
                        if dialog_button(ui, pal, "Cancel", false) {
                            *close = true;
                        }
                        if go {
                            confirmed = Some(act);
                            *close = true;
                        }
                    });
                });
            }
            Dialog::Browser(b) => {
                let mut chosen: Option<PathBuf> = None;
                let mut cancelled = false;
                let save = b.save;
                let title = if save { "Save As" } else { "Open File" };
                egui::Window::new(title)
                    .title_bar(false)
                    .collapsible(false)
                    .resizable(true)
                    .default_size([640.0, 470.0])
                    .frame(dialog_frame(pal))
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .show(ctx, |ui| {
                        ui.set_min_width(640.0);
                        let w = ui.available_width();
                        dialog_header(ui, title, w, pal, &mut close);
                        egui::Frame::NONE
                            .inner_margin(Margin {
                                left: 16,
                                right: 16,
                                top: 12,
                                bottom: 14,
                            })
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    if dialog_button(ui, pal, "Up", false) {
                                        if let Some(p) = b.dir.parent() {
                                            b.dir = p.to_path_buf();
                                            b.refresh();
                                        }
                                    }
                                    if dialog_button(ui, pal, "Home", false) {
                                        if let Some(h) = std::env::var_os("HOME") {
                                            b.dir = PathBuf::from(h);
                                            b.refresh();
                                        }
                                    }
                                    ui.add_space(10.0);
                                    ui.add(
                                        egui::Label::new(
                                            egui::RichText::new(b.dir.display().to_string())
                                                .size(11.5)
                                                .color(pal.text_dim()),
                                        )
                                        .truncate(),
                                    );
                                });
                                ui.add_space(10.0);
                                egui::Frame::NONE
                                    .fill(pal.editor_bg())
                                    .stroke(Stroke::new(1.0, pal.border()))
                                    .corner_radius(CornerRadius::same(4))
                                    .inner_margin(Margin::same(4))
                                    .show(ui, |ui| {
                                        ui.set_min_width(ui.available_width());
                                        egui::ScrollArea::vertical()
                                            .max_height(288.0)
                                            .auto_shrink([false, false])
                                            .show(ui, |ui| {
                                                let dirs = b.dirs.clone();
                                                for d in dirs {
                                                    let name = d
                                                        .file_name()
                                                        .and_then(|n| n.to_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    if dialog_row(ui, pal, Icon::Folder, &name) {
                                                        b.dir = d.clone();
                                                        b.refresh();
                                                    }
                                                }
                                                let files = b.files.clone();
                                                for f in files {
                                                    let name = f
                                                        .file_name()
                                                        .and_then(|n| n.to_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    if dialog_row(ui, pal, Icon::File, &name) {
                                                        if save {
                                                            b.name = name;
                                                        } else {
                                                            chosen = Some(f.clone());
                                                        }
                                                    }
                                                }
                                                if let Some(e) = &b.error {
                                                    ui.add_space(4.0);
                                                    ui.colored_label(
                                                        Color32::from_rgb(0xc0, 0x3a, 0x32),
                                                        e,
                                                    );
                                                }
                                            });
                                    });
                                ui.add_space(12.0);
                                ui.horizontal(|ui| {
                                    ui.label(egui::RichText::new("Name").color(pal.text_main()));
                                    let field = (ui.available_width() - 8.0).max(120.0);
                                    ui.add(
                                        egui::TextEdit::singleline(&mut b.name)
                                            .desired_width(field)
                                            .frame(field_frame(pal)),
                                    );
                                });
                                ui.add_space(14.0);
                                button_row(ui, |ui| {
                                        let go = dialog_button(
                                            ui,
                                            pal,
                                            if save { "Save" } else { "Open" },
                                            true,
                                        );
                                        if dialog_button(ui, pal, "Cancel", false) {
                                            cancelled = true;
                                        }
                                    if go {
                                        let target = b.dir.join(&b.name);
                                        if save {
                                            chosen = Some(target);
                                        } else if target.is_file() {
                                            chosen = Some(target);
                                        }
                                    }
                                });
                            });
                    });
                if let Some(p) = chosen {
                    if save {
                        self.save_path(p, ctx);
                    } else {
                        self.open_path(&p);
                    }
                    close = true;
                }
                if cancelled {
                    close = true;
                }
            }
        }

        for a in acts {
            self.apply(a, ctx);
        }
        if let Some(a) = confirmed {
            match a {
                Act2::CloseDoc(i) => {
                    self.docs[i].dirty = false;
                    self.close_doc(i);
                }
                Act2::Quit => {
                    self.quitting = true;
                    ctx.send_viewport_cmd(ViewportCommand::Close);
                }
            }
            close = true;
        }
        if let Some(d) = new_dialog {
            if let Dialog::Message(t, n) = &d {
                if t == "__goto__" {
                    if let Ok(n) = n.parse::<usize>() {
                        self.jump_to_line(n);
                    }
                    self.dialog = None;
                    return;
                }
            }
            self.dialog = Some(d);
            return;
        }
        if close {
            self.dialog = None;
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let (mods, keys) = ctx.input(|i| {
            let mut pressed: Vec<egui::Key> = Vec::new();
            for k in [
                egui::Key::N,
                egui::Key::O,
                egui::Key::S,
                egui::Key::W,
                egui::Key::Q,
                egui::Key::F,
                egui::Key::R,
                egui::Key::G,
                egui::Key::PageDown,
                egui::Key::PageUp,
                egui::Key::Plus,
                egui::Key::Minus,
                egui::Key::Equals,
                egui::Key::Tab,
                egui::Key::Escape,
            ] {
                if i.key_pressed(k) {
                    pressed.push(k);
                }
            }
            (i.modifiers, pressed)
        });
        if self.dialog.is_some() {
            return;
        }
        let ctrl = mods.command;
        if !ctrl {
            if keys.contains(&egui::Key::Escape) && self.find.open {
                self.apply(Act::CloseFind, ctx);
            }
            return;
        }
        for k in keys {
            let act = match k {
                egui::Key::N => Some(Act::New),
                egui::Key::O => Some(Act::Open),
                egui::Key::S => Some(if mods.shift { Act::SaveAs } else { Act::Save }),
                egui::Key::W => Some(Act::CloseDoc(self.active)),
                egui::Key::Q => Some(Act::Quit),
                egui::Key::F => Some(Act::Find),
                egui::Key::R => Some(Act::Replace),
                egui::Key::G => Some(Act::GoToLine),
                egui::Key::PageDown => Some(Act::NextTab),
                egui::Key::PageUp => Some(Act::PrevTab),
                egui::Key::Plus | egui::Key::Equals => Some(Act::ZoomIn),
                egui::Key::Minus => Some(Act::ZoomOut),
                _ => None,
            };
            if let Some(a) = act {
                self.apply(a, ctx);
            }
        }
    }

    /// The window has no system decorations, so the edges are drag handles.
    fn resize_grips(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let r = ctx.viewport_rect();
        let t = 6.0;
        let edges: [(Rect, egui::ResizeDirection, &str); 4] = [
            (
                Rect::from_min_size(Pos2::new(r.left() + t, r.top()), Vec2::new((r.width() - 2.0 * t).max(0.0), t)),
                egui::ResizeDirection::North,
                "rz-n",
            ),
            (
                Rect::from_min_size(Pos2::new(r.left() + t, r.bottom() - t), Vec2::new((r.width() - 2.0 * t).max(0.0), t)),
                egui::ResizeDirection::South,
                "rz-s",
            ),
            (
                Rect::from_min_size(Pos2::new(r.left(), r.top() + t), Vec2::new(t, (r.height() - 2.0 * t).max(0.0))),
                egui::ResizeDirection::West,
                "rz-w",
            ),
            (
                Rect::from_min_size(Pos2::new(r.right() - t, r.top() + t), Vec2::new(t, (r.height() - 2.0 * t).max(0.0))),
                egui::ResizeDirection::East,
                "rz-e",
            ),
        ];
        let corners: [(Rect, egui::ResizeDirection, &str); 4] = [
            (Rect::from_min_size(r.left_top(), Vec2::splat(t)), egui::ResizeDirection::NorthWest, "rz-nw"),
            (Rect::from_min_size(Pos2::new(r.right() - t, r.top()), Vec2::splat(t)), egui::ResizeDirection::NorthEast, "rz-ne"),
            (Rect::from_min_size(Pos2::new(r.left(), r.bottom() - t), Vec2::splat(t)), egui::ResizeDirection::SouthWest, "rz-sw"),
            (Rect::from_min_size(Pos2::new(r.right() - t, r.bottom() - t), Vec2::splat(t)), egui::ResizeDirection::SouthEast, "rz-se"),
        ];
        for (rect, dir, id) in edges.into_iter().chain(corners) {
            let resp = ui.interact(rect, Id::new(id), Sense::drag());
            if resp.drag_started() {
                ctx.send_viewport_cmd(ViewportCommand::BeginResize(dir));
            }
        }
    }

    fn screenshot_tick(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else {
            return;
        };
        self.shot_frames += 1;
        ctx.request_repaint();
        if self.shot_frames >= 120 && !self.shot_requested {
            self.shot_requested = true;
            let done = self.shot_done.clone();
            ctx.request_screenshot(move |image: std::sync::Arc<egui::ColorImage>| {
                save_png(&path, &image);
                done.store(true, Ordering::SeqCst);
            });
        }
        if self.shot_done.load(Ordering::SeqCst) {
            self.shot_frames += 1;
            if self.shot_frames > 126 {
                ctx.send_viewport_cmd(ViewportCommand::Close);
            }
        }
    }
}

/// Where the open-file list is kept between runs: KateCraft's own folder, so it
/// never collides with the original editor's configuration.
fn session_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))?;
    Some(base.join("katecraft").join("session.txt"))
}

/// Read the system clipboard without pulling in a GUI clipboard dependency.
fn arboard_read() -> Result<String, ()> {
    let mut cmds: [(&str, &[&str]); 3] = [
        ("wl-paste", &["--no-newline"]),
        ("xclip", &["-selection", "clipboard", "-o"]),
        ("xsel", &["--clipboard", "--output"]),
    ];
    for (cmd, args) in cmds.iter_mut() {
        if let Ok(out) = std::process::Command::new(cmd).args(args.iter()).output() {
            if out.status.success() {
                return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
            }
        }
    }
    Err(())
}

pub fn save_png(path: &Path, image: &egui::ColorImage) {
    let [w, h] = image.size;
    let mut buf: Vec<u8> = Vec::with_capacity(w * h * 4);
    for p in &image.pixels {
        buf.extend_from_slice(&[p.r(), p.g(), p.b(), p.a()]);
    }
    if let Some(img) = image::RgbaImage::from_raw(w as u32, h as u32, buf) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = img.save(path);
    }
}

// -------------------------------------------------------------- dialog look ---

/// The KateCraft mark: an accent tile with a white monogram.
fn paint_mark(p: &egui::Painter, rect: Rect, large: bool) {
    p.rect_filled(
        rect,
        CornerRadius::same(if large { 11 } else { 3 }),
        theme::ACCENT,
    );
    p.text(
        rect.center(),
        Align2::CENTER_CENTER,
        "K",
        FontId::new(
            if large { 27.0 } else { 10.0 },
            egui::FontFamily::Name("bold".into()),
        ),
        Color32::WHITE,
    );
}

fn dialog_frame(pal: Palette) -> egui::Frame {
    let bg = if pal.dark {
        Color32::from_rgb(0x2c, 0x30, 0x34)
    } else {
        Color32::WHITE
    };
    egui::Frame::NONE
        .fill(bg)
        .stroke(Stroke::new(1.0, pal.border()))
        .corner_radius(CornerRadius::same(6))
        .inner_margin(Margin::ZERO)
        .shadow(egui::epaint::Shadow {
            offset: [0, 8],
            blur: 24,
            spread: 0,
            color: Color32::from_black_alpha(56),
        })
}

fn dialog_header(ui: &mut egui::Ui, title: &str, width: f32, pal: Palette, close: &mut bool) {
    let _ = pal;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 32.0), Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius {
            nw: 6,
            ne: 6,
            sw: 0,
            se: 0,
        },
        theme::TITLE_BG,
    );
    ui.painter().text(
        Pos2::new(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        title,
        FontId::new(12.0, egui::FontFamily::Name("bold".into())),
        theme::TITLE_FG,
    );

    let xr = Rect::from_center_size(
        Pos2::new(rect.right() - 17.0, rect.center().y),
        Vec2::splat(22.0),
    );
    let resp = ui.interact(xr, Id::new(("dialog-close", title)), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(xr, CornerRadius::same(3), Color32::from_rgb(0xc0, 0x3a, 0x32));
    }
    let c = if resp.hovered() {
        Color32::WHITE
    } else {
        theme::TITLE_FG
    };
    let cc = xr.center();
    ui.painter().line_segment(
        [
            Pos2::new(cc.x - 4.5, cc.y - 4.5),
            Pos2::new(cc.x + 4.5, cc.y + 4.5),
        ],
        Stroke::new(1.2, c),
    );
    ui.painter().line_segment(
        [
            Pos2::new(cc.x + 4.5, cc.y - 4.5),
            Pos2::new(cc.x - 4.5, cc.y + 4.5),
        ],
        Stroke::new(1.2, c),
    );
    if resp.clicked() {
        *close = true;
    }
}

/// A dialog window with the app's dark title strip and padded body.
fn dialog_shell(
    ctx: &egui::Context,
    title: &str,
    width: f32,
    pal: Palette,
    close: &mut bool,
    add: impl FnOnce(&mut egui::Ui, Palette, &mut bool),
) {
    egui::Window::new(title)
        .title_bar(false)
        .collapsible(false)
        .resizable(false)
        .frame(dialog_frame(pal))
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.set_min_width(width);
            dialog_header(ui, title, width, pal, close);
            egui::Frame::NONE
                .inner_margin(Margin {
                    left: 18,
                    right: 18,
                    top: 14,
                    bottom: 16,
                })
                .show(ui, |ui| {
                    ui.set_min_width(width - 36.0);
                    add(ui, pal, close);
                });
        });
}

/// A row of dialog buttons, right-aligned, one fixed height (keeps dialogs compact).
fn button_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let w = ui.available_width();
    ui.allocate_ui_with_layout(
        Vec2::new(w, 28.0),
        egui::Layout::right_to_left(egui::Align::Center),
        |ui| add(ui),
    );
}

fn section_label(ui: &mut egui::Ui, pal: Palette, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase())
            .size(10.5)
            .strong()
            .color(pal.text_dim()),
    );
    ui.add_space(6.0);
}

fn dialog_button(ui: &mut egui::Ui, pal: Palette, label: &str, primary: bool) -> bool {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::new(13.0, egui::FontFamily::Proportional),
        Color32::WHITE,
    );
    let size = Vec2::new((galley.size().x + 34.0).max(84.0), 28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if primary {
        let bg = if resp.hovered() {
            Color32::from_rgb(0x35, 0x9f, 0xd8)
        } else {
            theme::ACCENT
        };
        ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::new(13.0, egui::FontFamily::Proportional),
            Color32::WHITE,
        );
    } else {
        let bg = if resp.hovered() {
            pal.chrome_hover()
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0, pal.border()),
            StrokeKind::Inside,
        );
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::new(13.0, egui::FontFamily::Proportional),
            pal.text_main(),
        );
    }
    resp.clicked()
}

fn chip(ui: &mut egui::Ui, pal: Palette, label: &str, active: bool) -> bool {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::new(12.5, egui::FontFamily::Proportional),
        Color32::WHITE,
    );
    let size = Vec2::new((galley.size().x + 22.0).max(34.0), 24.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if active {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), theme::ACCENT);
    } else {
        let bg = if resp.hovered() {
            pal.chrome_hover()
        } else {
            Color32::TRANSPARENT
        };
        ui.painter().rect_filled(rect, CornerRadius::same(4), bg);
        ui.painter().rect_stroke(
            rect,
            CornerRadius::same(4),
            Stroke::new(1.0, pal.border()),
            StrokeKind::Inside,
        );
    }
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::new(12.5, egui::FontFamily::Proportional),
        if active { Color32::WHITE } else { pal.text_main() },
    );
    resp.clicked()
}

fn field_frame(pal: Palette) -> egui::Frame {
    egui::Frame::NONE
        .fill(pal.editor_bg())
        .stroke(Stroke::new(1.0, pal.border()))
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin {
            left: 8,
            right: 8,
            top: 5,
            bottom: 5,
        })
}

fn check_row(ui: &mut egui::Ui, pal: Palette, label: &str, value: &mut bool) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
    let box_r = Rect::from_min_size(
        Pos2::new(rect.left() + 1.0, rect.center().y - 7.0),
        Vec2::splat(14.0),
    );
    if *value {
        ui.painter()
            .rect_filled(box_r, CornerRadius::same(3), theme::ACCENT);
        paint_icon(
            ui.painter(),
            Icon::Check,
            box_r.shrink(2.0),
            Color32::WHITE,
        );
    } else {
        let bg = if resp.hovered() {
            pal.chrome_hover()
        } else {
            pal.editor_bg()
        };
        ui.painter().rect_filled(box_r, CornerRadius::same(3), bg);
        ui.painter().rect_stroke(
            box_r,
            CornerRadius::same(3),
            Stroke::new(1.0, pal.border()),
            StrokeKind::Inside,
        );
    }
    ui.painter().text(
        Pos2::new(box_r.right() + 9.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(13.0, egui::FontFamily::Proportional),
        pal.text_main(),
    );
    if resp.clicked() {
        *value = !*value;
        true
    } else {
        false
    }
}

fn dialog_row(ui: &mut egui::Ui, pal: Palette, icon: Icon, label: &str) -> bool {
    let w = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(w, 24.0), Sense::click());
    if resp.hovered() {
        ui.painter()
            .rect_filled(rect, CornerRadius::same(3), pal.chrome_hover());
    }
    let p = ui.painter().with_clip_rect(rect);
    paint_icon(
        &p,
        icon,
        Rect::from_min_size(
            Pos2::new(rect.left() + 6.0, rect.center().y - 8.0),
            Vec2::splat(16.0),
        ),
        pal.text_dim(),
    );
    p.text(
        Pos2::new(rect.left() + 28.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::new(13.0, egui::FontFamily::Proportional),
        pal.text_main(),
    );
    resp.clicked()
}

fn frame_fill(color: Color32, margin: Margin) -> egui::Frame {
    egui::Frame::NONE
        .fill(color)
        .inner_margin(margin)
        .stroke(Stroke::NONE)
}

impl eframe::App for KateCraftApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        let c = self.pal.chrome();
        [c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0, 1.0]
    }

    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx_owned = root.ctx().clone();
        let ctx = &ctx_owned;
        self.frame += 1;
        self.screenshot_tick(ctx);
        self.shortcuts(ctx);

        if self.show_menu {
            egui::Panel::top("titlebar")
                .exact_size(28.0)
                .frame(frame_fill(self.pal.title_bg(), Margin::ZERO))
                .show_separator_line(false)
                .show(root, |ui| self.title_bar(ui, ctx));
            egui::Panel::top("menubar")
                .exact_size(27.0)
                .frame(frame_fill(self.pal.menu_bg(), Margin::symmetric(3, 0)))
                .show_separator_line(false)
                .show(root, |ui| self.menu_bar(ui, ctx));
        }
        if self.show_toolbar {
            egui::Panel::top("toolbar")
                .exact_size(32.0)
                .frame(frame_fill(self.pal.chrome(), Margin::symmetric(4, 3)))
                .show_separator_line(false)
                .show(root, |ui| self.toolbar(ui, ctx));
        }
        if self.show_tabbar {
            egui::Panel::top("tabbar")
                .exact_size(30.0)
                .frame(frame_fill(self.pal.chrome(), Margin::ZERO))
                .show_separator_line(false)
                .show(root, |ui| self.tab_bar(ui, ctx));
        }
        if self.show_breadcrumb {
            egui::Panel::top("breadcrumb")
                .exact_size(22.0)
                .frame(frame_fill(self.pal.breadcrumb_bg(), Margin::ZERO))
                .show_separator_line(false)
                .show(root, |ui| self.breadcrumb(ui));
        }

        if self.show_status {
            egui::Panel::bottom("statusbar")
                .exact_size(24.0)
                .frame(frame_fill(self.pal.status_bg(), Margin::ZERO))
                .show_separator_line(false)
                .show(root, |ui| self.status_bar(ui, ctx));
        }
        if self.find.open {
            let h = if self.find.replace { 64.0 } else { 34.0 };
            egui::Panel::bottom("findbar")
                .exact_size(h)
                .frame(frame_fill(self.pal.chrome(), Margin::ZERO))
                .show_separator_line(false)
                .show(root, |ui| self.find_bar(ui, ctx));
        }
        if self.tool_view.is_some() {
            egui::Panel::bottom("toolpanel")
                .exact_size(170.0)
                .frame(frame_fill(self.pal.editor_bg(), Margin::ZERO))
                .show_separator_line(false)
                .show(root, |ui| self.tool_panel(ui));
        }

        egui::Panel::left("rail")
            .exact_size(28.0)
            .resizable(false)
            .frame(frame_fill(self.pal.rail_bg(), Margin::ZERO))
            .show_separator_line(false)
            .show(root, |ui| self.rail(ui, ctx));
        if self.sidebar {
            egui::Panel::left("sidebar")
                .exact_size(226.0)
                .resizable(false)
                .frame(frame_fill(self.pal.chrome(), Margin::ZERO))
                .show_separator_line(false)
                .show(root, |ui| self.sidebar(ui, ctx));
        }

        egui::CentralPanel::default()
            .frame(frame_fill(self.pal.editor_bg(), Margin::ZERO))
            .show(root, |ui| {
                let any_focus = ctx.memory(|m| m.focused()).is_some();
                let allow_keys = self.editor_focused && !any_focus;
                let doc_idx = self.active.min(self.docs.len().saturating_sub(1));
                let matches: Vec<(usize, usize)> = if self.find.query.is_empty() {
                    Vec::new()
                } else {
                    self.find.matches.clone()
                };
                if doc_idx < self.docs.len() {
                    let out = {
                        let doc = &mut self.docs[doc_idx];
                        editor::show(ui, doc, &self.ed, self.pal, allow_keys, &matches)
                    };
                    if out.changed {
                        self.refresh_matches();
                    }
                    if out.clicked {
                        self.editor_focused = true;
                        if let Some(id) = ctx.memory(|m| m.focused()) {
                            ctx.memory_mut(|m| m.surrender_focus(id));
                        }
                    }
                }
            });

        self.dialogs(ctx);
        self.resize_grips(root);

        if self.docs.is_empty() {
            self.docs.push(Document::new_untitled(self.next_untitled));
            self.active = 0;
        }
        self.active = self.active.min(self.docs.len() - 1);

        if let Some((msg, t)) = &self.toast {
            let now = ctx.input(|i| i.time);
            if now - t < 2.5 {
                let screen = ctx.viewport_rect();
                let text_pos = Pos2::new(screen.center().x, screen.bottom() - 70.0);
                let galley = ctx.fonts_mut(|f| {
                    f.layout_no_wrap(
                        msg.clone(),
                        FontId::new(12.0, egui::FontFamily::Proportional),
                        Color32::WHITE,
                    )
                });
                let r = Rect::from_center_size(
                    text_pos,
                    Vec2::new(galley.size().x + 24.0, 26.0),
                );
                let p = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    Id::new("toast"),
                ));
                p.rect_filled(r, CornerRadius::same(6), Color32::from_rgb(0x2c, 0x30, 0x34));
                p.galley(
                    Pos2::new(
                        r.center().x - galley.size().x / 2.0,
                        r.center().y - galley.size().y / 2.0,
                    ),
                    galley,
                    Color32::WHITE,
                );
                ctx.request_repaint();
            }
        }
    }
}
