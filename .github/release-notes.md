KateCraft — a text and code editor in Rust and egui, following Kate's shape.

## What's new in 0.1.0

The first release.

* **Tabs** — new, open, save, save as, reload, close (middle-click closes), switch; the last ten
  documents under *File ▸ Open Recent*, kept between sessions.
* **One window** — opening a file while the editor is running switches that window to the file,
  the way Kate does, and a window left by an older build is replaced when a newer one starts.
* **Editing** — typing, undo/redo, cut/copy/paste, word and line movement, soft-tab indentation,
  auto-indent on Enter, trailing-whitespace removal, line sorting and case changes.
* **Bookmarks** — Ctrl+B marks a line (an accent bar in the gutter), F2/Shift+F2 step between them
  with wrap-around, and the marks follow the text while it is edited and undone.
* **Syntax colouring** for Python, Rust, C/C++, JavaScript, JSON, TOML, Markdown, CSS, HTML and
  shell scripts, with line numbers, the current-line band, a minimap and a caret with selection.
* **Find and replace** — match count, next/previous, match case, replace one, replace all, hits
  highlighted in the text; **find in files** (Ctrl+Shift+F) searches the open folder, skips hidden
  folders, build output and binary or large files, groups hits by file and opens one at the chosen
  line with the word selected.
* **Chrome** — menu bar, toolbar, tab bar, path breadcrumb, a sidebar with documents, search, a file
  browser, symbols and plugin switches, and a status bar with line/column, a live word count, insert
  mode, tab width, encoding and syntax. The title bar carries the document name, the app name and its
  version.
* **A frameless window** that resizes from its own edge, with a fine outline so the edge stays
  visible, and light and dark colour schemes.
* **Settings** — colour scheme, font size, tab width, line numbers, minimap and the visible chrome
  are written to `~/.config/katecraft/katecraft.conf` the moment they change.
* **Check for updates…** in the *Help* menu: asks GitHub for the newest release and offers it with
  its notes and an *Install and restart* button. Nothing checks on its own; the download is verified
  against the release's sha256 before the running binary is replaced, and the window comes back on
  the new build with the open documents saved (asking first if any are unsaved). This is the only
  request the program makes.

**Linux** — unpack the tarball for your architecture and run the binary:

```sh
tar xzf katecraft-0.1.0-x86_64-linux.tar.gz
./katecraft-0.1.0-x86_64-linux/katecraft
```

The tarballs are built on Debian 12, so they ask for glibc 2.35 — Debian 12, Ubuntu 22.04+, Mint 21+,
Fedora 36+ and anything newer. Build it yourself with `cargo build --release`.
