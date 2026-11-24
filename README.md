<p align="center">
  <img src="assets/app-icon.svg" alt="Larik Text app icon" width="112" height="112">
</p>

<h1 align="center">Larik Text</h1>

<p align="center">
  A small, native editor for opening one file fast, with tabs, code highlighting, and Markdown preview.<br>
  <a href="https://github.com/fikrifirmanf/larik-text/releases/latest">Download</a> ·
  <a href="CONTRIBUTING.md">Contribute</a> ·
  <a href="LICENSE">MIT license</a>
</p>

Larik Text is a native desktop editor with tabs for text and code files. It uses Rust and GPUI. The app opens text files with any extension and adds Tree-sitter highlighting when it can identify the language.

## Open and edit a file

On macOS, open `dist/Larik Text.app` or choose Larik Text from Finder’s **Open With** menu after moving the app to Applications. On any platform, drop a file into the window, use **Open file**, or pass the path to the executable.

| Action | macOS | Windows and Linux |
| --- | --- | --- |
| New tab | ⌘T | Ctrl+T |
| Open file | ⌘O | Ctrl+O |
| Save | ⌘S | Ctrl+S |
| Save as | ⇧⌘S | Ctrl+Shift+S |
| Find and replace | ⌘F | Ctrl+F |
| Markdown preview | ⌥⌘P | Ctrl+Alt+P |
| Close tab | ⌘W | Ctrl+W |
| Next tab | Ctrl+Tab | Ctrl+Tab |

Closing the app window or quitting saves the tab list, active tab, and unsaved drafts locally. Tabs disappear from that session when you close them. A dot marks an untitled tab or a file with edits to save.

**Preview** in the status bar (or **View → Markdown preview**) renders Markdown beside its source in a wide window. **Edit → Beautify JSON** reformats a JSON file on demand. Resizing only changes the layout.

**Settings** (⌘, on macOS, or the Settings item in the app menu) holds the theme, editor font, text size, line spacing, line wrapping for text and code, line numbers, beautify JSON on save, and the update check on launch. Changes apply at once and are saved to `settings.json` in the app's config folder.

Larik Text opens a blank tab when there are no saved tabs to restore.

The status bar shows the language, encoding, line endings, and save state. Click the language to change syntax mode. Use **View** to choose System, Light, or Dark appearance, toggle line wrapping, or change the editor text size. Markdown preview shows a split view in a wide window and takes the full editor area in a narrow one.

Larik Text preserves UTF-8, UTF-8 with a BOM, UTF-16 with a BOM, and Windows-1252 when selected to open a legacy file. For mixed line endings, unchanged lines keep their original endings. Inserted or edited lines use the dominant ending. If another app changes the file before you save, Larik Text keeps your edits and offers Save As or Reload from Disk. Markdown previews use local text rendering and display image alt text.

## Help and updates

**Help → About Larik Text** shows the installed version. **Check for updates** compares it with the latest version from the configured release feed, and Larik Text also checks quietly a few seconds after launch. A local build can use an HTTPS URL supplied through the `LARIK_TEXT_UPDATE_FEED` environment variable or compiled in with the same variable.

The feed is JSON with a semantic `version` and an HTTPS `url` for the release page. It can also list per-platform packages under `downloads`, keyed `macos-aarch64`, `windows-x86_64` or `linux-aarch64`, each with an HTTPS `url` and a `sha256`:

```json
{"version": "0.2.0", "url": "https://example.com/releases/0.2.0",
 "downloads": {"macos-aarch64": {"url": "https://example.com/Larik-Text-macos.zip", "sha256": "…64 hex characters…"}}}
```

When a newer version exists, a dialog offers **Download and install** or **Later**. The download is checked against its checksum before anything is replaced. macOS and Linux restart into the new version with tabs and drafts restored. Windows opens the downloaded package. Without a usable package for the platform, the dialog offers the release page instead.

[Larik Text icon](assets/app-icon.svg) · [Larik Text wordmark](assets/wordmark.svg)

## Opening on macOS

Release builds are ad-hoc signed unless the maintainer has set up Apple notarization, so macOS may say it can’t verify Larik Text the first time you open a downloaded copy.

1. Move **Larik Text** to Applications and try to open it once.
2. Open **System Settings → Privacy & Security**, scroll to Security, and click **Open Anyway** next to Larik Text.
3. Confirm with your password. macOS remembers the choice.

Prefer Terminal? Run `xattr -dr com.apple.quarantine "/Applications/Larik Text.app"` once instead. On macOS 15 and later, right-click → Open no longer skips the warning.

Updates installed from inside Larik Text (**Help → Check for updates**) are downloaded by the app itself, so they should not show this warning again.

## Open with, default app, and file icons

- **macOS.** Larik Text appears under **Open With** for text, Markdown, JSON, YAML, and common source and config files, with a paper-and-caret document icon. To make it the default for one type, select a file, press ⌘I, choose Larik Text under **Open with**, and click **Change All…**.
- **Windows.** The first launch adds Larik Text to **Open with** for common text and code extensions, with the document icon, for your user only. Windows only lets you pick a default yourself: right-click a file, choose **Open with → Choose another app**, select Larik Text, then **Always**. If the executable moves, the next launch registers the new location. `Register-LarikText.ps1 -Unregister` removes the entries.
- **Linux.** The first launch adds Larik Text to the application menu and **Open With** for your user. To make it the default for plain text, Markdown, Python, JSON, and shell scripts, run `./install.sh --default` from the extracted folder (or `xdg-mime default larik-text.desktop text/plain`). `./install.sh --uninstall` removes it. File-type icons come from your icon theme on Linux, so only the app icon changes.

## Notarized macOS builds

`packaging/macos.sh` signs ad hoc, which only runs on the Mac that built it. For a build other people can open, `packaging/notarize.sh` signs with a Developer ID Application certificate (hardened runtime and secure timestamp), submits it with `notarytool`, staples the ticket, and writes `dist/Larik-Text-<version>-macos-arm64.zip`. It needs a paid Apple Developer Program account; the script header lists the one-time certificate and credential setup.

## Recent files

**File → Open recent** lists the last 10 files you opened, newest first. **Clear menu** empties the list. It is saved between launches.

## Install

Download the build for your system from [GitHub Releases](https://github.com/fikrifirmanf/larik-text/releases/latest): a zip with the app for macOS (Apple Silicon), a zip for Windows x64, and tarballs for Linux x64 and ARM64. Extract it and run Larik Text. Release builds check that feed for updates, so **Help → Check for updates** works out of the box.

## Local builds

A local `packaging/macos.sh` build produces `dist/Larik Text.app`. Local builds have no update feed unless you set `LARIK_TEXT_UPDATE_FEED`. Release archives come from the GitHub Actions workflow in `.github/workflows/release.yml`.

## Build from source

Use a current stable Rust toolchain and run `cargo build --locked --release`. The binary is in `target/release`. To open a file directly from the source checkout, use `cargo run --locked -- path/to/file.py`.

- **macOS.** Select a full Xcode installation with its Metal toolchain. Check with `xcrun metal --version`. Run `packaging/macos.sh` to build a locally signed app bundle with Finder document types. The script also uses Python 3.
- **Windows.** Build from a Visual Studio Developer PowerShell with the Desktop development with C++ workload and Windows SDK. Use the Rust MSVC toolchain for a native build.
- **Linux.** Install a C/C++ toolchain, `pkg-config`, and the Fontconfig, XKB, X11, Wayland, and Vulkan development packages. Run under Wayland or X11 with Vulkan support. Install XDG Desktop Portal and the backend for your desktop environment for the native file picker.

On Debian-based Linux, install the build libraries with:

```sh
sudo apt-get install build-essential clang pkg-config libfontconfig1-dev \
  libxkbcommon-dev libxkbcommon-x11-dev libvulkan-dev libwayland-dev \
  libx11-xcb-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
  libxcb-xkb-dev libasound2-dev libssl-dev
```

GPUI selects the renderer. The macOS build uses Metal. Windows uses Direct3D 11, and Linux uses Vulkan through Blade. [GPUI platform source](https://docs.rs/crate/gpui/0.2.2/source/src/platform/)

Run the regression suite with `cargo test --locked`. See the [design proposal](docs/design-proposal.md) (written under the working name Foldrest) and its [interactive mockup](docs/design-mockup.html) for the visual direction.
