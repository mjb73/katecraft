//! KateCraft — a text and code editor (Rust + egui).
//!
//! Usage:
//!   katecraft [FILES...]
//!   katecraft --screenshot out.png [FILES...]

mod app;
mod doc;
mod editor;
mod prefs;
mod single;
mod theme;
mod update;

use std::path::PathBuf;

fn main() -> eframe::Result<()> {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut screenshot: Option<PathBuf> = None;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--screenshot" => {
                if let Some(p) = args.get(i + 1) {
                    screenshot = Some(PathBuf::from(p));
                    i += 1;
                }
            }
            "--help" | "-h" => {
                println!("KateCraft — text and code editor");
                println!("usage: katecraft [FILE...]");
                println!("       katecraft --screenshot FILE.png   draw a few frames, save a PNG, exit");
                return Ok(());
            }
            "--version" | "-v" => {
                println!("katecraft 0.1.0");
                return Ok(());
            }
            other if other.starts_with("--screenshot=") => {
                screenshot = Some(PathBuf::from(&other["--screenshot=".len()..]));
            }
            other if other.starts_with("--") => {}
            other => files.push(PathBuf::from(other)),
        }
        i += 1;
    }

    // One window per user: a file opened while the editor is already running
    // lands in that window, and a window left over from an older build is
    // replaced instead of being shown again. Picture mode is a private,
    // short-lived window and never touches the running editor.
    let mut instance = None;
    if screenshot.is_none() {
        single::refresh_installed_copy();
        let outcome = single::forward(&files);
        if matches!(outcome, single::Outcome::Forwarded) {
            return Ok(());
        }
        instance = Some(single::claim(matches!(outcome, single::Outcome::Absent)));
    }

    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([720.0, 420.0])
        .with_decorations(false)
        .with_title(app::APP_TITLE);
    // In picture mode the window keeps drawing but never steals focus.
    if screenshot.is_some() {
        viewport = viewport.with_active(false);
    }

    let options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Glow,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        app::APP_TITLE,
        options,
        Box::new(move |cc| {
            Ok(Box::new(app::KateCraftApp::new(
                cc,
                files.clone(),
                screenshot.clone(),
                instance,
            )))
        }),
    )
}
