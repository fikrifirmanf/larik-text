#![cfg_attr(windows, windows_subsystem = "windows")]

mod document;
mod editor;
mod formatting;
mod integration;
mod language;
mod preview;
mod recent;
mod session;
mod settings_window;
mod theme;
mod updates;
mod workspace;
use workspace::Workspace;
const APP_NAME: &str = "Larik Text";

use std::{
    borrow::Cow,
    path::{Path, PathBuf},
};

use document::{Encoding, LoadedFile, OpenError};
use editor::Editor;
use gpui::{
    Action, AnyWindowHandle, App, AppContext as _, Application, Bounds, Entity, EntityId, Global,
    KeyBinding, Menu, MenuItem, PromptButton, PromptLevel, TitlebarOptions, WeakEntity, Window,
    WindowBounds, WindowOptions, actions, px, size,
};
use gpui_component::{
    Root,
    input::{Copy, Cut, Paste, Redo, Search, SelectAll, Undo},
};
use theme::{Appearance, Settings};

actions!(
    foldrest,
    [
        NewFile,
        OpenFile,
        SaveFile,
        SaveAs,
        SaveUtf8,
        CloseFile,
        Quit,
        TogglePreview,
        ToggleWrap,
        ZoomIn,
        ZoomOut,
        ZoomReset,
        BeautifyJson,
        ClearRecent,
        NoRecentFiles,
        Recent0,
        Recent1,
        Recent2,
        Recent3,
        Recent4,
        Recent5,
        Recent6,
        Recent7,
        Recent8,
        Recent9,
        OpenSettings,
        CheckForUpdates,
        About,
        NextTab,
        PreviousTab,
        SystemAppearance,
        LightAppearance,
        DarkAppearance,
    ]
);

#[derive(Default)]
struct Windows(Vec<(AnyWindowHandle, WeakEntity<Workspace>)>);
impl Global for Windows {}

/// What the View menu shows as checked. Rebuilt into the native menu whenever it changes.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct MenuState {
    pub preview: bool,
    pub wrap: bool,
    pub appearance: Appearance,
    pub recent: u64,
}

struct MenuCache(Option<MenuState>);
impl Global for MenuCache {}

/// Called from the active editor's render; only touches the menu bar when something changed.
/// Re-sends the menu bar after a setting changed from another window.
pub fn menus_changed(cx: &mut App) {
    if let Some(state) = cx.try_global::<MenuCache>().and_then(|cache| cache.0) {
        sync_menus(state, cx);
    }
}

pub fn sync_menus(mut state: MenuState, cx: &mut App) {
    if !cx.has_global::<MenuCache>() || !cx.has_global::<Settings>() {
        return;
    }
    state.appearance = cx.global::<Settings>().appearance;
    state.recent = cx.try_global::<recent::Recent>().map_or(0, |r| r.revision);
    if cx.global::<MenuCache>().0 == Some(state) {
        return;
    }
    cx.global_mut::<MenuCache>().0 = Some(state);
    cx.defer(move |cx| set_menus(state, cx));
}

struct SessionPath(Option<PathBuf>);
impl Global for SessionPath {}

fn main() {
    let (sender, receiver) = async_channel::unbounded::<Vec<String>>();
    let app = Application::new().with_assets(gpui_component_assets::Assets);
    app.on_open_urls(move |urls| {
        let _ = sender.try_send(urls);
    });
    app.on_reopen(|cx| {
        if cx.windows().is_empty() {
            create_window(None, false, cx);
        }
    });
    let arguments: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    app.run(move |cx| {
        gpui_component::init(cx);
        cx.set_global(Windows::default());
        cx.set_global(MenuCache(None));
        cx.set_global(recent::Recent::load(
            dirs::data_local_dir().map(|dir| dir.join("Larik Text/recent-v1.json")),
        ));
        cx.set_global(SessionPath(
            dirs::data_local_dir().map(|dir| dir.join("Larik Text/session-v1.json")),
        ));
        let settings_path = dirs::config_dir().map(|dir| dir.join("Larik Text/settings.json"));
        cx.set_global(Settings::load(
            settings_path.as_deref().unwrap_or(std::path::Path::new("")),
        ));
        cx.set_global(theme::SettingsPath(settings_path));
        let fonts = [
            include_bytes!("../assets/fonts/instrument-sans.ttf").as_slice(),
            include_bytes!("../assets/fonts/instrument-serif.ttf").as_slice(),
            include_bytes!("../assets/fonts/source-code-pro.ttf").as_slice(),
        ];
        if let Err(error) = cx
            .text_system()
            .add_fonts(fonts.into_iter().map(Cow::Borrowed).collect())
        {
            eprintln!("Could not load Larik Text fonts: {error:#}");
        }
        theme::apply(cx.global::<Settings>().appearance, None, cx);
        install_keys_and_menus(cx);
        #[cfg(not(target_os = "macos"))]
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let welcome = create_window(None, false, cx);
        for path in arguments {
            open_path(path, None, welcome, cx);
        }
        cx.spawn(async move |cx| {
            while let Ok(urls) = receiver.recv().await {
                let _ = cx.update(|cx| {
                    for url in urls {
                        if let Some(path) = url::Url::parse(&url)
                            .ok()
                            .and_then(|url| url.to_file_path().ok())
                        {
                            open_path(path, None, cx.active_window(), cx);
                        }
                    }
                });
            }
        })
        .detach();
        integration::register();
        if cx.global::<Settings>().check_updates_on_launch {
            updates::check_on_launch(cx);
        }
        cx.activate(false);
    });
}

fn windows(cx: &App) -> Vec<(AnyWindowHandle, Entity<Workspace>)> {
    cx.global::<Windows>()
        .0
        .iter()
        .filter_map(|(window, view)| view.upgrade().map(|view| (*window, view)))
        .collect()
}

pub fn opened_file(path: &Path, except: Option<EntityId>, cx: &App) -> bool {
    windows(cx)
        .into_iter()
        .any(|(_, workspace)| workspace.read(cx).has_path(path, except, cx))
}

pub fn close_tab(id: EntityId, window: &mut Window, cx: &mut App) {
    let handle = window.window_handle();
    window.defer(cx, move |window, cx| {
        if let Some((_, workspace)) = windows(cx)
            .into_iter()
            .find(|(candidate, _)| *candidate == handle)
        {
            workspace.update(cx, |workspace, cx| workspace.remove(id, window, cx));
        }
    });
}

fn create_window(
    file: Option<LoadedFile>,
    untitled: bool,
    cx: &mut App,
) -> Option<AnyWindowHandle> {
    let has_windows = !cx.windows().is_empty();
    let bounds = (!has_windows)
        .then(|| WindowBounds::Windowed(Bounds::centered(None, size(px(900.), px(620.)), cx)));
    let options = WindowOptions {
        window_bounds: bounds,
        window_min_size: Some(size(px(420.), px(320.))),
        app_id: Some("app.larik.LarikText".to_owned()),
        titlebar: Some(if cfg!(target_os = "macos") {
            gpui_component::TitleBar::title_bar_options()
        } else {
            TitlebarOptions {
                title: Some(APP_NAME.into()),
                ..Default::default()
            }
        }),
        ..Default::default()
    };
    match cx.open_window(options, move |window, cx| {
        let session_path = cx
            .try_global::<SessionPath>()
            .and_then(|path| path.0.clone());
        let view = cx.new(|cx| Workspace::new(file, untitled, session_path, window, cx));
        let handle = window.window_handle();
        let weak = view.downgrade();
        cx.global_mut::<Windows>()
            .0
            .retain(|(_, view)| view.upgrade().is_some());
        cx.global_mut::<Windows>().0.push((handle, weak.clone()));
        window.on_window_should_close(cx, move |window, cx| {
            let Some(workspace) = weak.upgrade() else {
                return true;
            };
            if workspace.read(cx).busy(cx) {
                return false;
            }
            if let Err(error) = workspace.read(cx).persist(cx) {
                window.defer(cx, move |window, cx| {
                    let result = window.prompt(
                        PromptLevel::Warning,
                        "Couldn’t save open tabs.",
                        Some(&format!("{error:#}")),
                        &["OK"],
                        cx,
                    );
                    window
                        .spawn(cx, async move |_| {
                            let _ = result.await;
                        })
                        .detach();
                });
                return false;
            }
            true
        });
        cx.new(|cx| Root::new(view, window, cx))
    }) {
        Ok(window) => Some(window.into()),
        Err(error) => {
            eprintln!("Could not open a Larik Text window: {error:#}");
            None
        }
    }
}

pub fn choose_file(window: &mut Window, cx: &mut App) {
    let origin = window.window_handle();
    let receiver = cx.prompt_for_paths(gpui::PathPromptOptions {
        files: true,
        directories: false,
        multiple: false,
        prompt: Some("Open file".into()),
    });
    cx.spawn(async move |cx| {
        let result = receiver.await;
        let _ = cx.update(|cx| match result {
            Ok(Ok(Some(paths))) => {
                for path in paths {
                    open_path(path, None, Some(origin), cx);
                }
            }
            Ok(Err(error)) => show_error(
                Some(origin),
                "Couldn’t show the file picker.",
                &error.to_string(),
                cx,
            ),
            _ => {}
        });
    })
    .detach();
}

pub fn open_path(
    path: PathBuf,
    encoding: Option<Encoding>,
    origin: Option<AnyWindowHandle>,
    cx: &mut App,
) {
    let name = document::file_name(&path);
    let read = cx
        .background_executor()
        .spawn(async move { document::open(path, encoding) });
    cx.spawn(async move |cx| {
        let result = read.await;
        let _ = cx.update(move |cx| match result {
            Ok(file) => open_loaded(file, origin, cx),
            Err(OpenError::Encoding(path)) if encoding.is_none() => {
                choose_encoding(path, origin, cx)
            }
            Err(OpenError::Encoding(_)) => show_error(
                origin,
                &format!("Couldn’t open {name}."),
                "The file could not be decoded using that encoding.",
                cx,
            ),
            Err(OpenError::Binary) => show_error(
                origin,
                &format!("Couldn’t open {name}."),
                "This file appears to contain binary data.",
                cx,
            ),
            Err(OpenError::Read(error)) => show_error(
                origin,
                &format!("Couldn’t open {name}."),
                &format!("{error:#}"),
                cx,
            ),
        });
    })
    .detach();
}

fn open_loaded(file: LoadedFile, origin: Option<AnyWindowHandle>, cx: &mut App) {
    recent::add(&file.path, cx);
    open_in_workspace(Some(file), origin, cx);
}

fn open_in_workspace(file: Option<LoadedFile>, origin: Option<AnyWindowHandle>, cx: &mut App) {
    let preferred = windows(cx)
        .into_iter()
        .min_by_key(|(handle, _)| Some(*handle) != origin);
    if let Some((handle, workspace)) = preferred {
        let _ = handle.update(cx, |_, window, cx| {
            workspace.update(cx, |workspace, cx| workspace.open(file, window, cx));
            window.activate_window();
        });
    } else {
        let untitled = file.is_none();
        create_window(file, untitled, cx);
    }
}

fn choose_encoding(path: PathBuf, origin: Option<AnyWindowHandle>, cx: &mut App) {
    let Some(handle) = dialog_window(origin, cx) else {
        return;
    };
    let name = document::file_name(&path);
    let _ = handle.update(cx, |_, window, cx| {
        let receiver = window.prompt(
            PromptLevel::Warning,
            &format!("Choose an encoding for {name}"),
            Some("Larik Text couldn’t read this file as UTF-8. If you know its encoding, choose it below."),
            &[PromptButton::cancel("Cancel"), PromptButton::new("Windows-1252"),
              PromptButton::new("UTF-16 LE"), PromptButton::new("UTF-16 BE")],
            cx,
        );
        cx.spawn(async move |cx| {
            let encoding = match receiver.await {
                Ok(1) => Encoding::Windows1252,
                Ok(2) => Encoding::Utf16Le,
                Ok(3) => Encoding::Utf16Be,
                _ => return,
            };
            let _ = cx.update(|cx| open_path(path, Some(encoding), Some(handle), cx));
        }).detach();
    });
}

pub(crate) fn dialog_window(
    origin: Option<AnyWindowHandle>,
    cx: &mut App,
) -> Option<AnyWindowHandle> {
    let live = cx.windows();
    origin
        .filter(|origin| live.contains(origin))
        .or_else(|| cx.active_window())
        .or_else(|| live.first().copied())
        .or_else(|| create_window(None, false, cx))
}

pub fn show_error(origin: Option<AnyWindowHandle>, title: &str, detail: &str, cx: &mut App) {
    if let Some(handle) = dialog_window(origin, cx) {
        let _ = handle.update(cx, |_, window, cx| {
            let receiver = window.prompt(PromptLevel::Warning, title, Some(detail), &["OK"], cx);
            window
                .spawn(cx, async move |_| {
                    let _ = receiver.await;
                })
                .detach();
        });
    }
}

pub(crate) fn quit(cx: &mut App) {
    for (handle, workspace) in windows(cx) {
        if workspace.read(cx).busy(cx) {
            return;
        }
        if let Err(error) = workspace.read(cx).persist(cx) {
            show_error(
                Some(handle),
                "Couldn’t save open tabs.",
                &format!("{error:#}"),
                cx,
            );
            return;
        }
    }
    cx.quit();
}

fn set_appearance(appearance: Appearance, cx: &mut App) {
    theme::update_settings(cx, |s| s.appearance = appearance);
}

fn with_active_editor(
    cx: &mut App,
    action: impl FnOnce(&mut Editor, &mut Window, &mut gpui::Context<Editor>),
) {
    let active = cx.active_window();
    if let Some((handle, workspace)) = windows(cx)
        .into_iter()
        .min_by_key(|(handle, _)| Some(*handle) != active)
    {
        let editor = workspace.read(cx).active().clone();
        let _ = handle.update(cx, |_, window, cx| {
            editor.update(cx, |editor, cx| action(editor, window, cx));
        });
    }
}

fn install_keys_and_menus(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("secondary-n", NewFile, None),
        KeyBinding::new("secondary-t", NewFile, None),
        KeyBinding::new("ctrl-tab", NextTab, None),
        KeyBinding::new("ctrl-shift-tab", PreviousTab, None),
        KeyBinding::new("secondary-o", OpenFile, None),
        KeyBinding::new("secondary-s", SaveFile, None),
        KeyBinding::new("secondary-shift-s", SaveAs, None),
        KeyBinding::new("secondary-w", CloseFile, None),
        KeyBinding::new("secondary-q", Quit, None),
        KeyBinding::new("secondary-alt-p", TogglePreview, None),
        KeyBinding::new("secondary-,", OpenSettings, None),
        KeyBinding::new("secondary-=", ZoomIn, None),
        KeyBinding::new("secondary--", ZoomOut, None),
        KeyBinding::new("secondary-0", ZoomReset, None),
    ]);
    cx.on_action(|_: &NewFile, cx| {
        cx.defer(|cx| open_in_workspace(None, cx.active_window(), cx));
    });
    cx.on_action(|_: &OpenFile, cx| {
        cx.defer(|cx| {
            if let Some(handle) = dialog_window(None, cx) {
                let _ = handle.update(cx, |_, window, cx| choose_file(window, cx));
            }
        });
    });
    cx.on_action(|_: &BeautifyJson, cx| {
        cx.defer(|cx| with_active_editor(cx, Editor::format_json));
    });
    macro_rules! recent_actions {
        ($($index:literal => $action:ident),*) => {$(
            cx.on_action(|_: &$action, cx| cx.defer(|cx| open_recent($index, cx)));
        )*};
    }
    recent_actions!(0 => Recent0, 1 => Recent1, 2 => Recent2, 3 => Recent3, 4 => Recent4,
        5 => Recent5, 6 => Recent6, 7 => Recent7, 8 => Recent8, 9 => Recent9);
    cx.on_action(|_: &ClearRecent, cx| cx.defer(recent::clear));
    cx.on_action(|_: &NoRecentFiles, _| {});
    cx.on_action(|_: &OpenSettings, cx| cx.defer(settings_window::open));
    cx.on_action(|_: &ZoomIn, cx| theme::update_settings(cx, |s| s.font_size += 1.));
    cx.on_action(|_: &ZoomOut, cx| theme::update_settings(cx, |s| s.font_size -= 1.));
    cx.on_action(|_: &ZoomReset, cx| {
        theme::update_settings(cx, |s| s.font_size = theme::DEFAULT_FONT_SIZE)
    });
    cx.on_action(|_: &About, cx| cx.defer(updates::about));
    cx.on_action(|_: &CheckForUpdates, cx| cx.defer(updates::check));
    cx.on_action(|_: &Quit, cx| quit(cx));
    cx.on_action(|_: &SystemAppearance, cx| set_appearance(Appearance::System, cx));
    cx.on_action(|_: &LightAppearance, cx| set_appearance(Appearance::Light, cx));
    cx.on_action(|_: &DarkAppearance, cx| set_appearance(Appearance::Dark, cx));
    set_menus(MenuState::default(), cx);
}

fn open_recent(index: usize, cx: &mut App) {
    let path = cx
        .try_global::<recent::Recent>()
        .and_then(|recent| recent.files().get(index).cloned());
    if let Some(path) = path {
        open_path(path, None, cx.active_window(), cx);
    }
}

fn recent_menu(cx: &App) -> MenuItem {
    let files: Vec<PathBuf> = cx
        .try_global::<recent::Recent>()
        .map(|recent| recent.files().to_vec())
        .unwrap_or_default();
    let mut items: Vec<MenuItem> = files
        .iter()
        .enumerate()
        .map(|(i, path)| {
            let name = recent::label(path);
            match i {
                0 => MenuItem::action(name, Recent0),
                1 => MenuItem::action(name, Recent1),
                2 => MenuItem::action(name, Recent2),
                3 => MenuItem::action(name, Recent3),
                4 => MenuItem::action(name, Recent4),
                5 => MenuItem::action(name, Recent5),
                6 => MenuItem::action(name, Recent6),
                7 => MenuItem::action(name, Recent7),
                8 => MenuItem::action(name, Recent8),
                _ => MenuItem::action(name, Recent9),
            }
        })
        .collect();
    if items.is_empty() {
        items.push(MenuItem::action("No recent files", NoRecentFiles));
    } else {
        items.push(MenuItem::separator());
        items.push(MenuItem::action("Clear menu", ClearRecent));
    }
    MenuItem::submenu(Menu {
        name: "Open recent".into(),
        items,
    })
}

fn set_menus(state: MenuState, cx: &mut App) {
    // GPUI menus have no native check state, so a leading mark stands in for it.
    fn item<A: Action>(on: bool, name: &str, action: A) -> MenuItem {
        let mark = if on { "✓  " } else { "\u{2003}\u{2002} " };
        MenuItem::action(format!("{mark}{name}"), action)
    }
    cx.set_menus(vec![
        Menu {
            name: APP_NAME.into(),
            items: vec![
                MenuItem::action("Settings…", OpenSettings),
                MenuItem::separator(),
                MenuItem::action("Quit Larik Text", Quit),
            ],
        },
        Menu {
            name: "File".into(),
            items: vec![
                MenuItem::action("New tab", NewFile),
                MenuItem::action("Open file…", OpenFile),
                recent_menu(cx),
                MenuItem::separator(),
                MenuItem::action("Save", SaveFile),
                MenuItem::action("Save as…", SaveAs),
                MenuItem::action("Save as UTF-8…", SaveUtf8),
                MenuItem::separator(),
                MenuItem::action("Close tab", CloseFile),
            ],
        },
        Menu {
            name: "Edit".into(),
            items: vec![
                MenuItem::action("Undo", Undo),
                MenuItem::action("Redo", Redo),
                MenuItem::separator(),
                MenuItem::action("Cut", Cut),
                MenuItem::action("Copy", Copy),
                MenuItem::action("Paste", Paste),
                MenuItem::action("Select all", SelectAll),
                MenuItem::separator(),
                MenuItem::action("Find and replace", Search),
                MenuItem::separator(),
                MenuItem::action("Beautify JSON", BeautifyJson),
            ],
        },
        Menu {
            name: "View".into(),
            items: vec![
                item(state.preview, "Markdown preview", TogglePreview),
                MenuItem::separator(),
                item(
                    state.appearance == Appearance::System,
                    "System appearance",
                    SystemAppearance,
                ),
                item(
                    state.appearance == Appearance::Light,
                    "Light appearance",
                    LightAppearance,
                ),
                item(
                    state.appearance == Appearance::Dark,
                    "Dark appearance",
                    DarkAppearance,
                ),
                MenuItem::separator(),
                item(state.wrap, "Wrap lines", ToggleWrap),
                MenuItem::separator(),
                item(false, "Increase text size", ZoomIn),
                item(false, "Decrease text size", ZoomOut),
                item(false, "Reset text size", ZoomReset),
            ],
        },
        Menu {
            name: "Help".into(),
            items: vec![
                MenuItem::action("Check for updates…", CheckForUpdates),
                MenuItem::action("About Larik Text", About),
            ],
        },
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use std::fs;

    #[gpui::test]
    fn keyboard_edit_saves_to_the_open_file_and_cancel_preserves_edits(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notes.custom");
        fs::write(&path, b"original\r\n").unwrap();
        let window = cx.update(|app| {
            gpui_component::init(app);
            app.set_global(Windows::default());
            app.set_global(Settings {
                appearance: Appearance::Light,
                ..Settings::default()
            });
            theme::apply(Appearance::Light, None, app);
            install_keys_and_menus(app);
            let window = create_window(None, false, app).unwrap();
            open_path(path.clone(), None, Some(window), app);
            window
        });
        cx.run_until_parked();
        cx.update(|app| assert!(app.windows() == [window], "expected one document window"));
        cx.update(|app| open_path(path.clone(), None, Some(window), app));
        cx.run_until_parked();
        cx.update(|app| assert!(app.windows() == [window], "expected one document window"));

        cx.simulate_keystrokes(window, "b");
        cx.simulate_keystrokes(
            window,
            if cfg!(target_os = "macos") {
                "cmd-s"
            } else {
                "ctrl-s"
            },
        );
        assert_eq!(fs::read(&path).unwrap(), b"boriginal\r\n");

        cx.simulate_keystrokes(window, "c");
        cx.simulate_keystrokes(
            window,
            if cfg!(target_os = "macos") {
                "cmd-w"
            } else {
                "ctrl-w"
            },
        );
        let (title, _) = cx
            .pending_prompt()
            .expect("closing with unsaved edits should prompt");
        assert_eq!(title, "Save changes to notes.custom?");
        cx.simulate_prompt_answer("Cancel");
        cx.run_until_parked();
        cx.update(|app| assert!(app.windows() == [window], "expected one document window"));

        cx.simulate_keystrokes(
            window,
            if cfg!(target_os = "macos") {
                "cmd-s"
            } else {
                "ctrl-s"
            },
        );
        assert_eq!(fs::read(&path).unwrap(), b"bcoriginal\r\n");
    }
    #[gpui::test]
    fn closing_an_untitled_tab_keeps_other_tabs_and_restores_draft(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("notes.md");
        let checkpoint = directory.path().join("session.json");
        fs::write(&path, b"notes\n").unwrap();
        let window = cx.update(|app| {
            gpui_component::init(app);
            app.set_global(Windows::default());
            app.set_global(SessionPath(Some(checkpoint.clone())));
            app.set_global(Settings {
                appearance: Appearance::Dark,
                ..Settings::default()
            });
            theme::apply(Appearance::Dark, None, app);
            install_keys_and_menus(app);
            let window = create_window(None, false, app).unwrap();
            open_path(path.clone(), None, Some(window), app);
            window
        });
        cx.run_until_parked();
        let (new_tab, close_tab) = if cfg!(target_os = "macos") {
            ("cmd-t", "cmd-w")
        } else {
            ("ctrl-t", "ctrl-w")
        };
        cx.simulate_keystrokes(window, new_tab);
        cx.simulate_keystrokes(window, close_tab);
        cx.simulate_keystrokes(window, "x");
        cx.update(|app| windows(app)[0].1.read(app).persist(app).unwrap());
        let session = session::load(&checkpoint).unwrap();
        assert_eq!(session.tabs.len(), 1);
        assert_eq!(session.tabs[0].draft.as_deref(), Some("xnotes\n"));
        cx.update(|app| {
            window
                .update(app, |_, window, _| window.remove_window())
                .unwrap();
            create_window(None, false, app).unwrap()
        });
        cx.run_until_parked();
        cx.update(|app| {
            let windows = windows(app);
            let editor = windows.last().unwrap().1.read(app).active().read(app);
            assert_eq!(editor.snapshot(app).draft.as_deref(), Some("xnotes\n"));
            assert_eq!(editor.title(), "notes.md");
        });
    }
    #[gpui::test]
    fn settings_round_trip_and_clamp(_cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        fs::write(
            &path,
            br#"{"font_size": 400, "wrap_code": true, "unknown": 1}"#,
        )
        .unwrap();
        let loaded = Settings::load(&path);
        assert_eq!(loaded.font_size, theme::MAX_FONT_SIZE);
        assert!(loaded.wrap_code && loaded.wrap_text && loaded.line_numbers);
        fs::write(&path, b"not json").unwrap();
        assert_eq!(Settings::load(&path), Settings::default());
    }

    #[gpui::test]
    fn settings_apply_to_open_editors_and_persist(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        let window = cx.update(|app| {
            gpui_component::init(app);
            app.set_global(Windows::default());
            app.set_global(Settings::default());
            app.set_global(theme::SettingsPath(Some(path.clone())));
            app.set_global(MenuCache(None));
            theme::apply(Appearance::Light, None, app);
            install_keys_and_menus(app);
            create_window(None, false, app).unwrap()
        });
        let mut visual = gpui::VisualTestContext::from_window(window, cx);
        visual.run_until_parked();
        visual.dispatch_action(ZoomIn);
        visual.run_until_parked();
        cx.update(|app| {
            assert_eq!(
                app.global::<Settings>().font_size,
                theme::DEFAULT_FONT_SIZE + 1.
            );
            theme::update_settings(app, |s| s.appearance = Appearance::Dark);
            assert!(gpui_component::Theme::global(app).is_dark());
        });
        let saved = Settings::load(&path);
        assert_eq!(saved.font_size, theme::DEFAULT_FONT_SIZE + 1.);
        assert_eq!(saved.appearance, Appearance::Dark);
    }

    #[gpui::test]
    fn preview_panes_follow_window_resize(cx: &mut TestAppContext) {
        let window = cx.update(|app| {
            gpui_component::init(app);
            app.set_global(Windows::default());
            app.set_global(Settings {
                appearance: Appearance::Light,
                ..Settings::default()
            });
            theme::apply(Appearance::Light, None, app);
            install_keys_and_menus(app);
            create_window(None, false, app).unwrap()
        });
        let mut visual = gpui::VisualTestContext::from_window(window, cx);
        visual.simulate_resize(size(px(500.), px(390.)));
        visual.run_until_parked();
        visual.dispatch_action(TogglePreview);
        visual.run_until_parked();
        let small = visual
            .debug_bounds("editor-pane")
            .expect("small editor pane should be laid out");
        assert_eq!(small.size.width, px(500.));
        let narrow_preview = visual
            .debug_bounds("editor-preview")
            .expect("preview should occupy the narrow editor");
        assert_eq!(narrow_preview.size.width, small.size.width);
        visual.simulate_resize(size(px(1140.), px(740.)));
        visual.run_until_parked();
        let large = visual
            .debug_bounds("editor-pane")
            .expect("maximized editor pane should be laid out");
        assert_eq!(large.size.width, px(1140.));
        assert!(large.size.height > small.size.height);
        let source = visual
            .debug_bounds("editor-source")
            .expect("source should reappear beside wide preview");
        let wide_preview = visual
            .debug_bounds("editor-preview")
            .expect("preview should remain visible after resizing");
        assert!(source.size.width > px(500.) && source.size.width < large.size.width);
        assert_eq!(
            source.size.width + wide_preview.size.width,
            large.size.width
        );
    }

    #[gpui::test]
    fn json_beautify_is_opt_in_on_save(cx: &mut TestAppContext) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("settings.json");
        fs::write(&path, br#"{"count":2}"#).unwrap();
        let window = cx.update(|app| {
            gpui_component::init(app);
            app.set_global(Windows::default());
            app.set_global(Settings {
                appearance: Appearance::Light,
                ..Settings::default()
            });
            theme::apply(Appearance::Light, None, app);
            install_keys_and_menus(app);
            let window = create_window(None, false, app).unwrap();
            open_path(path.clone(), None, Some(window), app);
            window
        });
        cx.run_until_parked();
        let mut visual = gpui::VisualTestContext::from_window(window, cx);
        visual.dispatch_action(SaveFile);
        visual.run_until_parked();
        assert_eq!(fs::read(&path).unwrap(), br#"{"count":2}"#);
        cx.update(|app| theme::update_settings(app, |s| s.beautify_json_on_save = true));
        visual.run_until_parked();
        visual.dispatch_action(SaveFile);
        visual.run_until_parked();
        assert_eq!(fs::read(&path).unwrap(), b"{\n  \"count\": 2\n}");
    }
}
