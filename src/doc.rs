//! Document model: text, cursor, undo history, language detection and a small
//! per-line syntax highlighter (Python, Rust, JS/TS, JSON, TOML, Markdown,
//! CSS, HTML, shell, plain text).

use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Python,
    Rust,
    JavaScript,
    Json,
    Toml,
    Markdown,
    Css,
    Html,
    Shell,
    C,
    Plain,
}

impl Lang {
    pub fn from_path(path: &Path) -> Lang {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        match ext.as_str() {
            "py" | "pyw" | "pyi" => Lang::Python,
            "rs" => Lang::Rust,
            "js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" => Lang::JavaScript,
            "json" | "jsonc" => Lang::Json,
            "toml" | "ini" | "cfg" | "conf" => Lang::Toml,
            "md" | "markdown" | "mdx" => Lang::Markdown,
            "css" | "scss" | "less" => Lang::Css,
            "html" | "htm" | "xml" | "svg" => Lang::Html,
            "sh" | "bash" | "zsh" | "fish" => Lang::Shell,
            "c" | "h" | "cpp" | "hpp" | "cc" => Lang::C,
            _ => Lang::Plain,
        }
    }

    pub fn from_name(name: &str) -> Lang {
        Lang::from_path(Path::new(name))
    }

    pub fn name(&self) -> &'static str {
        match self {
            Lang::Python => "Python",
            Lang::Rust => "Rust",
            Lang::JavaScript => "JavaScript",
            Lang::Json => "JSON",
            Lang::Toml => "TOML",
            Lang::Markdown => "Markdown",
            Lang::Css => "CSS",
            Lang::Html => "HTML",
            Lang::Shell => "Shell",
            Lang::C => "C++",
            Lang::Plain => "None",
        }
    }

    pub const ALL: [Lang; 11] = [
        Lang::Plain,
        Lang::Python,
        Lang::Rust,
        Lang::C,
        Lang::JavaScript,
        Lang::Json,
        Lang::Toml,
        Lang::Markdown,
        Lang::Css,
        Lang::Html,
        Lang::Shell,
    ];

    pub fn comment_prefix(&self) -> &'static str {
        match self {
            Lang::Python | Lang::Shell | Lang::Toml => "#",
            Lang::Markdown => "<!--",
            _ => "//",
        }
    }
}

// ------------------------------------------------------------- highlight ---

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tok {
    Keyword,
    Type,
    Str,
    Comment,
    Func,
    Num,
    Decorator,
    Heading,
}

#[derive(Clone)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    pub tok: Tok,
}

#[derive(Default, Clone)]
pub struct HiState {
    pub triple: Option<&'static str>,
    pub block_comment: bool,
}

fn is_word_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

const PY_KEYWORDS: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "False", "finally", "for", "from", "global", "if", "import", "in", "is",
    "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True", "try", "while",
    "with", "yield", "match", "case", "self",
];
const PY_BUILTINS: &[&str] = &[
    "str", "int", "float", "bool", "list", "dict", "set", "tuple", "len", "print", "range",
    "enumerate", "zip", "map", "filter", "sorted", "sum", "min", "max", "abs", "open", "type",
    "isinstance", "super", "object", "Exception", "ValueError", "KeyError",
];

const RS_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];
const RS_TYPES: &[&str] = &[
    "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64", "u128", "f32", "f64", "bool",
    "char", "str", "String", "Vec", "Option", "Result", "Box", "Rc", "Arc", "HashMap", "usize",
    "isize",
];

const JS_KEYWORDS: &[&str] = &[
    "async", "await", "break", "case", "catch", "class", "const", "continue", "default", "delete",
    "do", "else", "export", "extends", "false", "finally", "for", "from", "function", "if",
    "import", "in", "instanceof", "let", "new", "null", "of", "return", "static", "super",
    "switch", "this", "throw", "true", "try", "typeof", "undefined", "var", "void", "while",
    "yield",
];

/// Tokenise one line, continuing multi-line strings/comments through `st`.
pub fn highlight_line(lang: Lang, line: &str, st: &mut HiState) -> Vec<Span> {
    let mut out: Vec<Span> = Vec::new();
    let bytes = line.as_bytes();
    let n = line.len();

    if lang == Lang::Plain {
        return out;
    }

    // Continue a Python triple-quoted string.
    if let Some(q) = st.triple {
        if let Some(pos) = line.find(q) {
            out.push(Span {
                start: 0,
                end: pos + 3,
                tok: Tok::Str,
            });
            st.triple = None;
            let rest = &line[pos + 3..];
            let mut sub = HiState::default();
            out.extend(highlight_line(lang, rest, &mut sub).into_iter().map(|mut s| {
                s.start += pos + 3;
                s.end += pos + 3;
                s
            }));
        } else {
            out.push(Span {
                start: 0,
                end: n,
                tok: Tok::Str,
            });
        }
        return out;
    }

    if st.block_comment {
        if let Some(pos) = line.find("*/") {
            out.push(Span {
                start: 0,
                end: pos + 2,
                tok: Tok::Comment,
            });
            st.block_comment = false;
            let rest = &line[pos + 2..];
            let mut sub = HiState::default();
            out.extend(highlight_line(lang, rest, &mut sub).into_iter().map(|mut s| {
                s.start += pos + 2;
                s.end += pos + 2;
                s
            }));
        } else {
            out.push(Span {
                start: 0,
                end: n,
                tok: Tok::Comment,
            });
        }
        return out;
    }

    if lang == Lang::Markdown {
        markdown_line(line, &mut out);
        return out;
    }

    let mut i = 0usize;
    let mut prev_nonspace: Option<char> = None;
    while i < n {
        let c = line[i..].chars().next().unwrap_or(' ');
        let clen = c.len_utf8();

        // line comments
        let line_comment = match lang {
            Lang::Python | Lang::Toml | Lang::Shell => c == '#',
            _ => c == '/' && bytes.get(i + 1) == Some(&b'/'),
        };
        if line_comment {
            out.push(Span {
                start: i,
                end: n,
                tok: Tok::Comment,
            });
            break;
        }
        // block comments
        if c == '/' && bytes.get(i + 1) == Some(&b'*') {
            if let Some(pos) = line[i + 2..].find("*/") {
                out.push(Span {
                    start: i,
                    end: i + 2 + pos + 2,
                    tok: Tok::Comment,
                });
                i += 2 + pos + 2;
                prev_nonspace = Some('/');
                continue;
            } else {
                out.push(Span {
                    start: i,
                    end: n,
                    tok: Tok::Comment,
                });
                st.block_comment = true;
                break;
            }
        }
        // strings
        if c == '"' || c == '\'' || (c == '`' && lang == Lang::JavaScript) {
            if (lang == Lang::Python) && i + 3 <= n && line[i..].starts_with(&c.to_string().repeat(3))
            {
                let q: &'static str = if c == '"' { "\"\"\"" } else { "'''" };
                if let Some(pos) = line[i + 3..].find(q) {
                    out.push(Span {
                        start: i,
                        end: i + 3 + pos + 3,
                        tok: Tok::Str,
                    });
                    i += 3 + pos + 3;
                    prev_nonspace = Some(c);
                    continue;
                } else {
                    out.push(Span {
                        start: i,
                        end: n,
                        tok: Tok::Str,
                    });
                    st.triple = Some(q);
                    break;
                }
            }
            let mut j = i + clen;
            let mut closed = false;
            while j < n {
                let d = line[j..].chars().next().unwrap();
                if d == '\\' {
                    j += d.len_utf8();
                    if j < n {
                        j += line[j..].chars().next().unwrap().len_utf8();
                    }
                    continue;
                }
                j += d.len_utf8();
                if d == c {
                    closed = true;
                    break;
                }
            }
            out.push(Span {
                start: i,
                end: j.min(n),
                tok: Tok::Str,
            });
            i = j.min(n);
            if !closed {
                // unterminated single-line string: stop here
            }
            prev_nonspace = Some(c);
            continue;
        }
        // numbers
        if c.is_ascii_digit() {
            let mut j = i + clen;
            while j < n {
                let d = line[j..].chars().next().unwrap();
                if d.is_ascii_alphanumeric() || d == '.' || d == '_' {
                    j += d.len_utf8();
                } else {
                    break;
                }
            }
            out.push(Span {
                start: i,
                end: j,
                tok: Tok::Num,
            });
            i = j;
            prev_nonspace = Some('0');
            continue;
        }
        // decorators
        if c == '@' && matches!(lang, Lang::Python) {
            let mut j = i + 1;
            while j < n {
                let d = line[j..].chars().next().unwrap();
                if is_word(d) || d == '.' {
                    j += d.len_utf8();
                } else {
                    break;
                }
            }
            if j > i + 1 {
                out.push(Span {
                    start: i,
                    end: j,
                    tok: Tok::Decorator,
                });
            }
            i = j;
            continue;
        }
        // words
        if is_word_start(c) {
            let mut j = i + clen;
            while j < n {
                let d = line[j..].chars().next().unwrap();
                if is_word(d) {
                    j += d.len_utf8();
                } else {
                    break;
                }
            }
            let word = &line[i..j];
            let after_dot = prev_nonspace == Some('.');
            let mut tok: Option<Tok> = None;
            match lang {
                Lang::Python => {
                    if PY_KEYWORDS.contains(&word) {
                        tok = Some(Tok::Keyword);
                    } else if PY_BUILTINS.contains(&word) {
                        tok = Some(Tok::Type);
                    }
                }
                Lang::Rust => {
                    if RS_KEYWORDS.contains(&word) {
                        tok = Some(Tok::Keyword);
                    } else if RS_TYPES.contains(&word) {
                        tok = Some(Tok::Type);
                    } else if word.chars().next().map(|c| c.is_uppercase()).unwrap_or(false)
                        && !after_dot
                    {
                        tok = Some(Tok::Type);
                    }
                }
                Lang::JavaScript => {
                    if JS_KEYWORDS.contains(&word) {
                        tok = Some(Tok::Keyword);
                    }
                }
                Lang::Json => {
                    let rest = line[j..].trim_start();
                    if rest.starts_with(':') {
                        tok = Some(Tok::Func);
                    } else if matches!(word, "true" | "false" | "null") {
                        tok = Some(Tok::Keyword);
                    }
                }
                Lang::Toml => {
                    let rest = line[j..].trim_start();
                    if rest.starts_with('=') {
                        tok = Some(Tok::Func);
                    } else if matches!(word, "true" | "false") {
                        tok = Some(Tok::Keyword);
                    }
                }
                Lang::Css => {
                    let rest = line[j..].trim_start();
                    if rest.starts_with(':') {
                        tok = Some(Tok::Func);
                    }
                }
                Lang::Html => {
                    // handled crudely: tags are matched below
                }
                _ => {}
            }
            // call / definition name
            if tok.is_none() && !after_dot {
                let rest = line[j..].trim_start();
                if rest.starts_with('(') {
                    tok = Some(Tok::Func);
                }
            }
            if let Some(t) = tok {
                out.push(Span {
                    start: i,
                    end: j,
                    tok: t,
                });
            }
            i = j;
            prev_nonspace = Some(word.chars().last().unwrap_or(c));
            continue;
        }
        if !c.is_whitespace() {
            prev_nonspace = Some(c);
        }
        i += clen;
    }
    out
}

fn markdown_line(line: &str, out: &mut Vec<Span>) {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    if trimmed.starts_with('#') {
        out.push(Span {
            start: indent,
            end: line.len(),
            tok: Tok::Heading,
        });
        return;
    }
    if trimmed.starts_with('>') {
        out.push(Span {
            start: indent,
            end: line.len(),
            tok: Tok::Comment,
        });
        return;
    }
    if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
        out.push(Span {
            start: indent,
            end: indent + 1,
            tok: Tok::Keyword,
        });
    }
    // inline code and bold
    let b = line.as_bytes();
    let mut i = indent;
    while i < b.len() {
        if b[i] == b'`' {
            if let Some(off) = line[i + 1..].find('`') {
                out.push(Span {
                    start: i,
                    end: i + 1 + off + 1,
                    tok: Tok::Str,
                });
                i = i + 1 + off + 1;
                continue;
            }
        }
        if b[i] == b'*' && b.get(i + 1) == Some(&b'*') {
            if let Some(off) = line[i + 2..].find("**") {
                out.push(Span {
                    start: i,
                    end: i + 2 + off + 2,
                    tok: Tok::Func,
                });
                i = i + 2 + off + 2;
                continue;
            }
        }
        i += 1;
    }
}

// -------------------------------------------------------------- document ---

#[derive(Clone)]
pub(crate) struct Snapshot {
    text: String,
    cursor: usize,
    anchor: usize,
}

pub struct Document {
    pub path: Option<PathBuf>,
    pub title: String,
    pub text: String,
    pub lang: Lang,
    pub dirty: bool,
    pub cursor: usize,
    pub anchor: usize,
    pub scroll_y: f32,
    pub scroll_x: f32,
    pub undo: Vec<Snapshot>,
    pub redo: Vec<Snapshot>,
    pub last_edit: Option<Instant>,
    pub version: u64,
    pub wanted_col: Option<usize>,
    pub read_only: bool,
    pub spans: Vec<Vec<Span>>,
    pub spans_version: u64,
}

impl Document {
    /// Re-tokenise the whole buffer when it changed since the last pass.
    pub fn ensure_spans(&mut self) {
        if self.spans_version == self.version && !self.spans.is_empty() {
            return;
        }
        let lang = self.lang;
        let mut st = HiState::default();
        let mut spans = Vec::new();
        for line in self.text.split('\n') {
            spans.push(highlight_line(lang, line, &mut st));
        }
        self.spans = spans;
        self.spans_version = self.version;
    }

    pub fn new_untitled(index: u32) -> Self {
        let title = if index == 0 {
            "Untitled".to_string()
        } else {
            format!("Untitled {index}")
        };
        Self {
            path: None,
            title,
            text: String::new(),
            lang: Lang::Plain,
            dirty: false,
            cursor: 0,
            anchor: 0,
            scroll_y: 0.0,
            scroll_x: 0.0,
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
            version: 0,
            wanted_col: None,
            read_only: false,
            spans: Vec::new(),
            spans_version: u64::MAX,
        }
    }

    pub fn from_text(path: Option<PathBuf>, text: String) -> Self {
        let title = path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "Untitled".to_string());
        let lang = path
            .as_ref()
            .map(|p| Lang::from_path(p))
            .unwrap_or(Lang::Plain);
        Self {
            path,
            title,
            text,
            lang,
            dirty: false,
            cursor: 0,
            anchor: 0,
            scroll_y: 0.0,
            scroll_x: 0.0,
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
            version: 0,
            wanted_col: None,
            read_only: false,
            spans: Vec::new(),
            spans_version: u64::MAX,
        }
    }

    pub fn open(path: &Path) -> std::io::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Ok(Self::from_text(Some(path.to_path_buf()), text))
    }

    pub fn save_to(&mut self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        std::fs::write(path, self.text.as_bytes())?;
        self.path = Some(path.to_path_buf());
        self.title = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();
        self.lang = Lang::from_path(path);
        self.dirty = false;
        Ok(())
    }

    /// Byte offset of the start of `line` (0-based).
    pub fn line_start(&self, line: usize) -> usize {
        let mut cur = 0usize;
        for (i, l) in self.text.split('\n').enumerate() {
            if i == line {
                return cur;
            }
            cur += l.len() + 1;
        }
        self.text.len()
    }

    pub fn line_count(&self) -> usize {
        self.text.split('\n').count().max(1)
    }

    pub fn line_of(&self, pos: usize) -> usize {
        let pos = pos.min(self.text.len());
        self.text[..pos].bytes().filter(|b| *b == b'\n').count()
    }

    pub fn line_range(&self, line: usize) -> (usize, usize) {
        let start = self.line_start(line);
        let end = self.text[start..]
            .find('\n')
            .map(|o| start + o)
            .unwrap_or(self.text.len());
        (start, end)
    }

    pub fn line_text(&self, line: usize) -> &str {
        let (a, b) = self.line_range(line);
        &self.text[a..b]
    }

    pub fn col_of(&self, pos: usize) -> usize {
        let line = self.line_of(pos);
        let (a, _) = self.line_range(line);
        self.text[a..pos.min(self.text.len())].chars().count()
    }

    pub fn selection(&self) -> Option<(usize, usize)> {
        if self.cursor == self.anchor {
            None
        } else if self.cursor < self.anchor {
            Some((self.cursor, self.anchor))
        } else {
            Some((self.anchor, self.cursor))
        }
    }

    pub fn has_selection(&self) -> bool {
        self.cursor != self.anchor
    }

    fn snapshot(&self) -> Snapshot {
        Snapshot {
            text: self.text.clone(),
            cursor: self.cursor,
            anchor: self.anchor,
        }
    }

    fn push_undo(&mut self, coalesce: bool) {
        let now = Instant::now();
        let quick = self
            .last_edit
            .map(|t| now.duration_since(t).as_millis() < 700)
            .unwrap_or(false);
        if !(coalesce && quick) {
            let snap = self.snapshot();
            self.undo.push(snap);
            if self.undo.len() > 400 {
                self.undo.remove(0);
            }
        }
        self.redo.clear();
        self.last_edit = Some(now);
    }

    pub fn undo(&mut self) -> bool {
        if let Some(snap) = self.undo.pop() {
            let cur = self.snapshot();
            self.redo.push(cur);
            self.text = snap.text;
            self.cursor = snap.cursor.min(self.text.len());
            self.anchor = snap.anchor.min(self.text.len());
            self.dirty = true;
            self.version += 1;
            self.last_edit = None;
            true
        } else {
            false
        }
    }

    pub fn redo(&mut self) -> bool {
        if let Some(snap) = self.redo.pop() {
            let cur = self.snapshot();
            self.undo.push(cur);
            self.text = snap.text;
            self.cursor = snap.cursor.min(self.text.len());
            self.anchor = snap.anchor.min(self.text.len());
            self.dirty = true;
            self.version += 1;
            self.last_edit = None;
            true
        } else {
            false
        }
    }

    pub fn insert(&mut self, s: &str) {
        if self.read_only {
            return;
        }
        self.push_undo(true);
        let cut = self.selection();
        if let Some((a, b)) = cut {
            self.text.replace_range(a..b, "");
            self.cursor = a;
        }
        let at = self.cursor.min(self.text.len());
        self.text.insert_str(at, s);
        self.cursor = at + s.len();
        self.anchor = self.cursor;
        self.dirty = true;
        self.version += 1;
    }

    pub fn delete_range(&mut self, a: usize, b: usize) {
        if self.read_only || a >= b {
            return;
        }
        let a = a.min(self.text.len());
        let b = b.min(self.text.len());
        self.push_undo(false);
        self.text.replace_range(a..b, "");
        self.cursor = a;
        self.anchor = a;
        self.dirty = true;
        self.version += 1;
    }

    pub fn backspace(&mut self) {
        if let Some((a, b)) = self.selection() {
            self.delete_range(a, b);
            return;
        }
        if self.cursor == 0 {
            return;
        }
        let prev = self.text[..self.cursor]
            .char_indices()
            .last()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.delete_range(prev, self.cursor);
    }

    pub fn delete_forward(&mut self) {
        if let Some((a, b)) = self.selection() {
            self.delete_range(a, b);
            return;
        }
        if self.cursor >= self.text.len() {
            return;
        }
        let next = self.text[self.cursor..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| self.cursor + i)
            .unwrap_or(self.text.len());
        self.delete_range(self.cursor, next);
    }

    /// Insert a newline, carrying the current line's indentation over.
    pub fn newline(&mut self) {
        let line = self.line_of(self.cursor);
        let (ls, _) = self.line_range(line);
        let indent: String = self.text[ls..self.cursor]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();
        let text = format!("\n{indent}");
        self.insert(&text);
    }

    pub fn indent_selection(&mut self, width: usize) {
        let (a, b) = match self.selection() {
            Some(s) => s,
            None => {
                self.insert(&" ".repeat(width));
                return;
            }
        };
        let first = self.line_of(a);
        let last = self.line_of(b);
        let pad = " ".repeat(width);
        self.push_undo(false);
        for line in (first..=last).rev() {
            let (ls, _) = self.line_range(line);
            self.text.insert_str(ls, &pad);
        }
        self.cursor += width;
        self.anchor += width * (last - first + 1);
        self.dirty = true;
        self.version += 1;
    }

    pub fn outdent_selection(&mut self, width: usize) {
        let (a, b) = match self.selection() {
            Some(s) => s,
            None => (self.cursor, self.cursor),
        };
        let first = self.line_of(a);
        let last = self.line_of(b);
        self.push_undo(false);
        let mut removed_first = 0usize;
        for line in (first..=last).rev() {
            let (ls, le) = self.line_range(line);
            let take = self.text[ls..le]
                .chars()
                .take(width)
                .take_while(|c| *c == ' ')
                .count();
            if take > 0 {
                self.text.replace_range(ls..ls + take, "");
                if line == first {
                    removed_first = take;
                }
            }
        }
        self.cursor = self.cursor.saturating_sub(removed_first);
        if self.anchor != self.cursor {
            self.anchor = self.anchor.saturating_sub(width * (last - first + 1));
        }
        self.dirty = true;
        self.version += 1;
    }

    pub fn move_cursor(&mut self, to: usize, shift: bool) {
        let to = to.min(self.text.len());
        if shift {
            if self.cursor == self.anchor {
                self.anchor = self.cursor;
            }
            self.cursor = to;
        } else {
            self.cursor = to;
            self.anchor = to;
        }
    }

    pub fn move_left(&mut self, shift: bool) {
        let prev = if self.has_selection() && !shift {
            self.selection().unwrap().0
        } else {
            self.text[..self.cursor]
                .char_indices()
                .last()
                .map(|(i, _)| i)
                .unwrap_or(0)
        };
        self.move_cursor(prev, shift);
    }

    pub fn move_right(&mut self, shift: bool) {
        let next = if self.has_selection() && !shift {
            self.selection().unwrap().1
        } else {
            self.text[self.cursor..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| self.cursor + i)
                .unwrap_or(self.text.len())
        };
        self.move_cursor(next, shift);
    }

    pub fn move_word_left(&mut self) {
        let mut i = self.cursor;
        while i > 0 {
            let prev = self.text[..i].char_indices().last().map(|(i, _)| i).unwrap_or(0);
            let c = self.text[prev..i].chars().next().unwrap_or(' ');
            i = prev;
            if !c.is_whitespace() {
                break;
            }
        }
        while i > 0 {
            let prev = self.text[..i].char_indices().last().map(|(i, _)| i).unwrap_or(0);
            let c = self.text[prev..i].chars().next().unwrap_or(' ');
            if c.is_whitespace() || !is_word(c) {
                break;
            }
            i = prev;
        }
        self.move_cursor(i, false);
    }

    pub fn move_word_right(&mut self) {
        let mut i = self.cursor;
        while i < self.text.len() {
            let c = self.text[i..].chars().next().unwrap_or(' ');
            if c.is_whitespace() || !is_word(c) {
                break;
            }
            i += c.len_utf8();
        }
        while i < self.text.len() {
            let c = self.text[i..].chars().next().unwrap_or(' ');
            if !c.is_whitespace() {
                break;
            }
            i += c.len_utf8();
        }
        self.move_cursor(i, false);
    }

    pub fn move_line(&mut self, delta: isize, shift: bool) {
        let line = self.line_of(self.cursor);
        let col = self.wanted_col.unwrap_or_else(|| self.col_of(self.cursor));
        let total = self.line_count() as isize;
        let target = (line as isize + delta).clamp(0, total - 1) as usize;
        self.wanted_col = Some(col);
        let (ls, le) = self.line_range(target);
        let mut idx = le;
        for (n, (b, _)) in self.text[ls..le].char_indices().enumerate() {
            if n == col {
                idx = ls + b;
                break;
            }
        }
        self.move_cursor(idx, shift);
    }

    pub fn move_home(&mut self, shift: bool) {
        let line = self.line_of(self.cursor);
        let (ls, le) = self.line_range(line);
        let first_nonspace = self.text[ls..le]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .map(|c| c.len_utf8())
            .sum::<usize>();
        let target = if self.cursor == ls + first_nonspace {
            ls
        } else {
            ls + first_nonspace
        };
        self.move_cursor(target, shift);
    }

    pub fn move_end(&mut self, shift: bool) {
        let line = self.line_of(self.cursor);
        let (_, le) = self.line_range(line);
        self.move_cursor(le, shift);
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.cursor = self.text.len();
    }

    pub fn select_line(&mut self) {
        let line = self.line_of(self.cursor);
        let (ls, le) = self.line_range(line);
        self.anchor = ls;
        self.cursor = (le + 1).min(self.text.len());
    }

    pub fn selected_text(&self) -> String {
        match self.selection() {
            Some((a, b)) => self.text[a..b].to_string(),
            None => String::new(),
        }
    }

    /// Replace every match of `query` in the document (case-insensitive when
    /// `ci`) and return how many were replaced.
    pub fn replace_all(&mut self, query: &str, with: &str, ci: bool) -> usize {
        if query.is_empty() || self.read_only {
            return 0;
        }
        self.push_undo(false);
        let hay = self.text.clone();
        let mut out = String::with_capacity(hay.len());
        let mut count = 0;
        let hay_cmp = if ci {
            hay.to_lowercase()
        } else {
            hay.clone()
        };
        let q_cmp = if ci {
            query.to_lowercase()
        } else {
            query.to_string()
        };
        let mut i = 0usize;
        while i < hay.len() {
            if hay_cmp[i..].starts_with(&q_cmp) && hay.is_char_boundary(i) {
                out.push_str(with);
                i += q_cmp.len();
                count += 1;
            } else {
                let c = hay[i..].chars().next().unwrap();
                out.push(c);
                i += c.len_utf8();
            }
        }
        self.text = out;
        self.dirty = true;
        self.version += 1;
        count
    }

    pub fn trim_trailing_whitespace(&mut self) {
        self.push_undo(false);
        let mut out = String::with_capacity(self.text.len());
        for (i, line) in self.text.split('\n').enumerate() {
            if i > 0 {
                out.push('\n');
            }
            out.push_str(line.trim_end());
        }
        self.text = out;
        self.cursor = self.cursor.min(self.text.len());
        self.anchor = self.anchor.min(self.text.len());
        self.dirty = true;
        self.version += 1;
    }

    pub fn sort_lines(&mut self) {
        let (a, b) = self.selection().unwrap_or((0, self.text.len()));
        let first = self.line_of(a);
        let last = self.line_of(b);
        let (ls, _) = self.line_range(first);
        let (_, le) = self.line_range(last);
        let mut lines: Vec<String> = self.text[ls..le].split('\n').map(|s| s.to_string()).collect();
        lines.sort_by_key(|l| l.to_lowercase());
        let joined = lines.join("\n");
        self.push_undo(false);
        self.text.replace_range(ls..le, &joined);
        self.dirty = true;
        self.version += 1;
    }

    pub fn transform_selection(&mut self, f: impl Fn(&str) -> String) {
        let (a, b) = match self.selection() {
            Some(s) => s,
            None => return,
        };
        let piece = self.text[a..b].to_string();
        let new = f(&piece);
        self.push_undo(false);
        self.text.replace_range(a..b, &new);
        self.cursor = a + new.len();
        self.dirty = true;
        self.version += 1;
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_and_undo() {
        let mut d = Document::new_untitled(0);
        d.insert("hello");
        assert_eq!(d.text, "hello");
        d.insert(" world");
        assert_eq!(d.text, "hello world");
        // rapid keystrokes coalesce into one undo step, as in the original
        d.undo();
        assert_eq!(d.text, "");
        d.redo();
        assert_eq!(d.text, "hello world");
        // a separate edit session is its own undo step
        d.last_edit = None;
        d.insert("!");
        assert_eq!(d.text, "hello world!");
        d.undo();
        assert_eq!(d.text, "hello world");
    }

    #[test]
    fn backspace_and_newline_keep_cursor_valid() {
        let mut d = Document::new_untitled(0);
        d.insert("ab");
        d.insert("c");
        d.backspace();
        assert_eq!(d.text, "ab");
        d.newline();
        assert_eq!(d.text, "ab\n");
        assert_eq!(d.cursor, 3);
        assert_eq!(d.line_of(d.cursor), 1);
    }

    #[test]
    fn selection_replace_and_line_numbers() {
        let mut d = Document::from_text(None, "one\ntwo\nthree".to_string());
        assert_eq!(d.line_count(), 3);
        assert_eq!(d.line_text(1), "two");
        d.anchor = 0;
        d.cursor = 3;
        assert_eq!(d.selected_text(), "one");
        d.insert("1");
        assert_eq!(d.text, "1\ntwo\nthree");
    }

    #[test]
    fn save_then_reopen_roundtrip() {
        let dir = std::env::temp_dir().join("katecraft-test");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("roundtrip.txt");
        let mut d = Document::from_text(None, "alpha\nbeta\n".to_string());
        d.save_to(&path).expect("save");
        let again = Document::open(&path).expect("open");
        assert_eq!(again.text, "alpha\nbeta\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn highlighting_finds_python_keywords() {
        let mut st = HiState::default();
        let spans = highlight_line(Lang::Python, "def f(x): return 1", &mut st);
        assert!(spans.iter().any(|s| s.tok == Tok::Keyword));
        assert!(spans.iter().any(|s| s.tok == Tok::Num));
    }
}
