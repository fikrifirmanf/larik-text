---
name: "Foldrest"
description: "A macOS editor for quick changes to individual text and code files."
platforms: ["macOS"]
colors:
  background: "#F4EFE4"
  surface: "#FFFCF5"
  chrome: "#F7F1E6"
  active-line: "#F0E8D9"
  text-primary: "#24372E"
  text-secondary: "#56635A"
  accent: "#284939"
  on-accent: "#FFF4DC"
  border: "#DCD2BF"
  focus: "#9F402A"
  keyword: "#734277"
  string: "#376040"
  function: "#365D78"
  number: "#8A461E"
  dark:
    surface: "#222823"
    chrome: "#2A322B"
    active-line: "#333D34"
    text-primary: "#EEE8D8"
    text-secondary: "#B4BDAC"
    border: "#435045"
    focus: "#F6AF83"
    keyword: "#E6B7E9"
    string: "#ADD19B"
    function: "#ABCBE8"
    number: "#F6B282"
typography:
  display:
    fontFamily: "Instrument Serif"
    fontSize: "40px"
    fontWeight: 400
    lineHeight: "1.05"
    letterSpacing: "-0.025em"
  body:
    fontFamily: "Instrument Sans"
    fontSize: "16px"
    fontWeight: 400
    lineHeight: "1.5"
  control:
    fontFamily: "Instrument Sans"
    fontSize: "14px"
    fontWeight: 600
    lineHeight: "1.4"
  status:
    fontFamily: "Instrument Sans"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: "1.5"
  editor:
    fontFamily: "Source Code Pro"
    fontSize: "14px"
    fontWeight: 400
    lineHeight: "1.75"
spacing:
  base: "4px"
  sm: "8px"
  md: "16px"
  lg: "24px"
  xl: "32px"
rounded:
  control: "8px"
  panel: "12px"
components:
  button-primary:
    background: "{colors.accent}"
    text-color: "{colors.on-accent}"
    radius: "{rounded.control}"
    height: "44px"
  button-secondary:
    background: "{colors.chrome}"
    text-color: "{colors.text-primary}"
    radius: "{rounded.control}"
    height: "44px"
  button-ghost:
    background: "transparent"
    text-color: "{colors.text-secondary}"
    radius: "{rounded.control}"
    height: "44px"
  input-search:
    background: "{colors.surface}"
    text-color: "{colors.text-primary}"
    radius: "{rounded.control}"
  editor:
    background: "{colors.surface}"
    text-color: "{colors.text-primary}"
    activeLine: "{colors.active-line}"
  status-bar:
    background: "{colors.chrome}"
    text-color: "{colors.text-secondary}"
contrast:
  body-on-paper:
    foreground: text-primary
    background: surface
    usage: text
  body-on-chrome:
    foreground: text-primary
    background: chrome
    usage: text
  code-on-active-line:
    foreground: text-primary
    background: active-line
    usage: text
  secondary-on-paper:
    foreground: text-secondary
    background: surface
    usage: text
  secondary-on-chrome:
    foreground: text-secondary
    background: chrome
    usage: text
  secondary-on-active-line:
    foreground: text-secondary
    background: active-line
    usage: text
  action-label:
    foreground: on-accent
    background: accent
    usage: text
  carets-and-focus:
    foreground: focus
    background: surface
    usage: ui
  caret-on-active-line:
    foreground: focus
    background: active-line
    usage: ui
  unsaved-on-chrome:
    foreground: focus
    background: chrome
    usage: text
  keyword-on-paper:
    foreground: keyword
    background: surface
    usage: text
  keyword-on-active-line:
    foreground: keyword
    background: active-line
    usage: text
  string-on-paper:
    foreground: string
    background: surface
    usage: text
  string-on-active-line:
    foreground: string
    background: active-line
    usage: text
  function-on-paper:
    foreground: function
    background: surface
    usage: text
  function-on-active-line:
    foreground: function
    background: active-line
    usage: text
  number-on-paper:
    foreground: number
    background: surface
    usage: text
  number-on-active-line:
    foreground: number
    background: active-line
    usage: text
---

# Foldrest design proposal

[Interactive mockups](mockup.html) · [App icon SVG](assets/app-icon.svg) · [Wordmark SVG](assets/wordmark.svg)

## Overview

Foldrest is a macOS-first editor for the moment you need to open one file, make a small change, and move on. A quick fix to a Python script, an edit to a README, and a plain-text note all open in the same quiet window. Each window holds one document. Opening another file opens its own window. Opening the same file again focuses its existing window.

**Recommended name · Foldrest.** The fold points to an individual page and gives the app a recognizable icon. “Rest” fits a calm place for a quick edit. **Leafmargin** is a more literary alternative built around a page and its margin. **Quietbuffer** is the more developer-oriented option, named for the text being edited. These are working candidates pending formal name clearance. An existing macOS writing app already uses Stillpage. [Noventra Studio’s product list](https://noventrastudio.com/products)

## Composition

- **Welcome.** An icon, “Open a file,” one open action, and a drop target. Keep New File in the File menu and on ⌘N.
- **Code.** A single editor fills the window. The title bar carries the filename, Open, and Find. The bottom bar shows line and column, indentation, encoding, line endings, language, and save state.
- **Markdown.** Open as editable source. Offer a rendered preview from View. Show it beside source when the window is wide enough and switch between source and preview in narrow windows.
- **Plain text.** Turn wrapping on by default. Hide code-only line numbers and indentation controls. Keep the language selector available for a manual override.

A file opened through Finder goes directly to the editor with keyboard focus. The welcome screen appears when launching without a document. Finder’s Open With, the open dialog, and a file drop all use the same file-opening flow.

## Colors

Warm paper and green-black ink draw from a printed page. The active line uses a light wash. Sienna marks the caret, focus, and unsaved state. In dark mode, charcoal and ivory preserve the same hierarchy, with apricot for the caret and focus. Moss is the primary-action and brand color across both themes. Dividers are decorative. Save status uses a dot and a text label. Syntax uses violet for keywords, fern for strings, blue for functions, and umber for numbers. The plain-text mode uses the primary ink throughout.

## Typography

Use Instrument Serif for the wordmark, welcome title, and Markdown preview headings. Use Instrument Sans for interface text. Source Code Pro keeps indentation, punctuation, and the caret aligned. Disable coding ligatures by default and let users change editor type size. Use system fallbacks for glyphs and scripts outside the bundled fonts. The mockup includes Latin specimens and ships with its font licenses. [Instrument Serif](https://github.com/Instrument/instrument-serif), [Instrument Sans](https://github.com/Instrument/instrument-sans), [Source Code Pro](https://github.com/adobe-fonts/source-code-pro)

## Layout

Use a 4-point spacing rhythm with 16-point title and status insets and 24 points above the first editor line. Let the editor fill the native window. Keep the filename in the title bar and show its full path on hover or through the native document menu. At narrow widths, move secondary status values into an accessible file-info menu. The mockup stacks Markdown source and preview on phones to show both views. The proposed desktop app uses a source/preview switch when space is tight.

## Components

| Situation | Proposed behavior |
| --- | --- |
| Open or create | Finder Open With, drag and drop, ⌘O, or ⌘N for an untitled document. |
| Edit | Undo and redo, ⌘F for Find, find and replace, line wrapping, and a language picker in the status bar and View menu. |
| Save | ⌘S writes to the same file. Untitled documents get a native Save dialog. Keep the file’s detected encoding and line endings unless the user chooses to convert them. |
| Close with edits | “Save changes to clean_titles.py?” with Save, Don’t Save, and Cancel. Cancel returns focus to the editor. |
| Write fails | Show “Couldn’t save clean_titles.py.” Include the reason when known, retain the edits, and offer Try Again or Save As. |
| File changed on disk | Offer to reload a clean buffer. For an unsaved buffer, keep edits in memory and offer Reload or Save As with a clear explanation. |
| Unknown language | Open as Plain Text. Choosing a syntax mode changes highlighting only and leaves file contents untouched. |
| Binary or undecodable file | Explain why the file cannot open as text. Offer another encoding for undecodable text and leave the original file untouched. |

Use native menu shortcuts and keyboard focus rings. Keep toolbar icons labeled for assistive technology and give icon-only controls tooltips. Announce save failures through the platform accessibility API.

## Graphics & Effects

The app icon is a cream page on a moss tile. Its folded corner suggests one file, and the sienna insertion caret signals editing. Use the square mark for the Dock and the full wordmark on the welcome or About surface. The wordmark SVG contains outlines and needs no font installation. The mockup includes icon checks at 16, 32, and 64 pixels. Use simple line icons for Open and Find. Keep the editor canvas flat.

## Motion

Use native motion for dialogs and window changes. Switch syntax and line highlights immediately. Respect Reduce Motion.

## Guardrails

- The first release covers open and save, Finder integration, undo and redo, find and replace, syntax detection and override, Markdown preview, and system, light, and dark appearance.
- The file contract is language-agnostic. Any readable plain-text source can open and save, including custom extensions. Highlighting uses a grammar when available. Seed the grammar set with Python, Markdown, JavaScript, TypeScript, HTML, CSS, JSON, YAML, TOML, shell, SQL, Go, Rust, Java, and C/C++. Keep an extensible registry with a Plain Text fallback. Code execution, debugging, and project workspaces are outside the first release.
- Keep file content local. Test clean open, one edit, same-path save, and close with a Python file, a Markdown file, and a custom extension. Include a round-trip check for line endings and encoding.
- Before release, measure cold launch to an editable caret and memory usage while editing. Use the same small file and test Mac when comparing with a full IDE.
