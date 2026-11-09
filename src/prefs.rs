//! Settings that outlive the window, kept in a small INI file.
//!
//! The original editor writes its choices to `~/.config/katerc`; KateCraft
//! keeps its own file next to it (`~/.config/katecraft/katecraft.conf`) so it
//! never rewrites the original's configuration.

use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    pub dark: bool,
    pub font_size: f32,
    pub tab_width: usize,
    pub line_numbers: bool,
    pub minimap: bool,
    pub toolbar: bool,
    pub tabbar: bool,
    pub breadcrumb: bool,
    pub status: bool,
    pub sidebar: bool,
    pub panel: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            dark: false,
            font_size: 12.5,
            tab_width: 4,
            line_numbers: true,
            minimap: true,
            toolbar: true,
            tabbar: true,
            breadcrumb: true,
            status: true,
            sidebar: false,
            panel: "documents".to_string(),
        }
    }
}

pub const PANELS: [&str; 4] = ["documents", "files", "symbols", "plugins"];

/// `~/.config/katecraft/katecraft.conf`, honouring `XDG_CONFIG_HOME`.
pub fn config_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("katecraft").join("katecraft.conf"))
}

impl Prefs {
    pub fn load() -> Self {
        match config_path() {
            Some(p) => Self::load_from(&p),
            None => Self::default(),
        }
    }

    pub fn load_from(path: &Path) -> Self {
        match std::fs::read_to_string(path) {
            Ok(text) => {
                let mut p = Self::default();
                p.parse(&text);
                p
            }
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> bool {
        match config_path() {
            Some(p) => self.save_to(&p),
            None => false,
        }
    }

    pub fn save_to(&self, path: &Path) -> bool {
        if let Some(dir) = path.parent() {
            if std::fs::create_dir_all(dir).is_err() {
                return false;
            }
        }
        std::fs::write(path, self.to_ini()).is_ok()
    }

    /// Reads the key/value pairs we write, ignoring anything we don't know
    /// (a hand-edited or newer file never stops the editor from starting).
    fn parse(&mut self, text: &str) {
        let mut section = String::new();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some(inner) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                section = inner.trim().to_ascii_lowercase();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_ascii_lowercase();
            let value = value.trim();
            match (section.as_str(), key.as_str()) {
                ("appearance", "colorscheme") => {
                    self.dark = value.eq_ignore_ascii_case("dark");
                }
                ("appearance", "fontsize") => {
                    if let Ok(v) = value.parse::<f32>() {
                        if v.is_finite() {
                            self.font_size = v.clamp(8.0, 28.0);
                        }
                    }
                }
                ("editor", "tabwidth") => {
                    if let Ok(v) = value.parse::<usize>() {
                        self.tab_width = v.clamp(1, 16);
                    }
                }
                ("editor", "linenumbers") => self.line_numbers = truthy(value),
                ("editor", "minimap") => self.minimap = truthy(value),
                ("chrome", "toolbar") => self.toolbar = truthy(value),
                ("chrome", "tabbar") => self.tabbar = truthy(value),
                ("chrome", "breadcrumb") => self.breadcrumb = truthy(value),
                ("chrome", "statusbar") => self.status = truthy(value),
                ("chrome", "sidebar") => self.sidebar = truthy(value),
                ("chrome", "sidebarpanel") => {
                    let v = value.to_ascii_lowercase();
                    if PANELS.contains(&v.as_str()) {
                        self.panel = v;
                    }
                }
                ("chrome", "lastfolder") => {
                    // Older files may carry this; it is no longer a setting.
                }
                _ => {}
            }
        }
        if !PANELS.contains(&self.panel.as_str()) {
            self.panel = "documents".to_string();
        }
    }

    pub fn to_ini(&self) -> String {
        let scheme = if self.dark { "dark" } else { "light" };
        let mut s = String::new();
        s.push_str("# KateCraft settings — written when you change them.\n");
        s.push_str("[Appearance]\n");
        s.push_str(&format!("ColorScheme={scheme}\n"));
        s.push_str(&format!("FontSize={:.1}\n\n", self.font_size));
        s.push_str("[Editor]\n");
        s.push_str(&format!("TabWidth={}\n", self.tab_width));
        s.push_str(&format!("LineNumbers={}\n", self.line_numbers));
        s.push_str(&format!("Minimap={}\n\n", self.minimap));
        s.push_str("[Chrome]\n");
        s.push_str(&format!("Toolbar={}\n", self.toolbar));
        s.push_str(&format!("TabBar={}\n", self.tabbar));
        s.push_str(&format!("Breadcrumb={}\n", self.breadcrumb));
        s.push_str(&format!("StatusBar={}\n", self.status));
        s.push_str(&format!("Sidebar={}\n", self.sidebar));
        s.push_str(&format!("SidebarPanel={}\n", self.panel));
        s
    }
}

fn truthy(v: &str) -> bool {
    matches!(
        v.to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_every_choice() {
        let p = Prefs {
            dark: true,
            font_size: 15.0,
            tab_width: 8,
            line_numbers: false,
            minimap: false,
            toolbar: false,
            tabbar: true,
            breadcrumb: false,
            status: true,
            sidebar: true,
            panel: "files".to_string(),
        };
        let text = p.to_ini();
        let mut back = Prefs::default();
        back.parse(&text);
        assert_eq!(p, back);
    }

    #[test]
    fn saves_and_reads_its_file() {
        let dir = std::env::temp_dir().join(format!("katecraft-prefs-{}", std::process::id()));
        let path = dir.join("nested").join("katecraft.conf");
        let p = Prefs {
            dark: true,
            font_size: 13.5,
            ..Prefs::default()
        };
        assert!(p.save_to(&path));
        assert_eq!(Prefs::load_from(&path), p);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn odd_values_do_not_break_the_file() {
        let mut p = Prefs::default();
        p.parse("[Appearance]\nColorScheme=Dark\nFontSize=999\n\n[Chrome]\nSidebarPanel=nonsense\nSidebar=yes\n[Unknown]\nKey=1\n");
        assert!(p.dark);
        assert_eq!(p.font_size, 28.0);
        assert_eq!(p.panel, "documents");
        assert!(p.sidebar);
    }
}
