KateCraft — a text and code editor in Rust and egui, following Kate's shape.

## What's new in 0.1.1

A visual refresh, in both the light and dark schemes.

* **Depth in the chrome** — the title bar, menu bar, toolbar, tab bar, breadcrumb, editor, gutter and
  status bar each have their own layer, with hairline seams instead of one flat grey.
* **Raised tabs** — rounded, bordered cards with an accent indicator; the active tab joins the
  editor, inactive tabs get a hover state, and the small × is easier to hit.
* **Refined palette** — cooler neutrals, softer borders, accent-tinted selection and rail states, and
  themed editor scrollbars (they used to stay light in the dark scheme).
* **Details** — larger, softened dialogs, an accent focus ring on search fields, an accent language
  label and hairline dividers in the status bar, and a clearer breadcrumb with the filename in bold.
* **Editor polish** — breathing room after the gutter, a current-line band that stays in the text
  area, rounded selection bands and a cleaner minimap viewport.

Everything from 0.1.0 is still here: tabs and sessions, one window per user, bookmarks, find and
replace with find in files, syntax colouring for the common languages, a file sidebar, settings kept
between runs, and *Help ▸ Check for updates…*.

## Install

**Linux** — unpack the tarball for your architecture and run the binary:

```sh
tar xzf katecraft-0.1.1-x86_64-linux.tar.gz
./katecraft-0.1.1-x86_64-linux/katecraft
```

The tarballs are built on Debian 12, so they ask for glibc 2.35 — Debian 12, Ubuntu 22.04+, Mint 21+,
Fedora 36+ and anything newer. Build it yourself with `cargo build --release`.
