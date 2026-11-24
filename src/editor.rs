use crate::{
    OpenFile, SaveAs, SaveFile, SaveUtf8, TogglePreview, ToggleWrap,
    document::{self, Encoding, FileFormat, LoadedFile, SaveError},
    language, preview,
    theme::{self, EditorFont, Palette, Settings},
};
use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, EntityInputHandler, ExternalPaths,
    FocusHandle, Focusable as _, IntoElement, ParentElement as _, PromptButton, PromptLevel,
    Render, SharedString, Styled as _, Subscription, Window, div, prelude::*, px, relative,
};
use gpui_component::{
    Selectable as _, Sizable as _,
    button::{Button, ButtonVariants as _},
    highlighter::Language,
    input::{Input, InputEvent, InputState, Rope, TabSize},
    menu::{DropdownMenu as _, PopupMenuItem},
    text::TextView,
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum SaveMode {
    Current,
    As,
    Utf8,
}

fn default_wrap(language: Language, cx: &App) -> bool {
    let settings = cx.global::<Settings>();
    if matches!(language, Language::Plain | Language::Markdown) {
        settings.wrap_text
    } else {
        settings.wrap_code
    }
}

fn prefs_key(cx: &App) -> (bool, bool, bool) {
    let settings = cx.global::<Settings>();
    (
        settings.wrap_text,
        settings.wrap_code,
        settings.line_numbers,
    )
}

pub struct Editor {
    input: Entity<InputState>,
    input_observer: Subscription,
    input_changes: Subscription,
    focus: FocusHandle,
    path: Option<PathBuf>,
    original: Option<Arc<[u8]>>,
    saved: Rope,
    format: FileFormat,
    language: Language,
    untitled: bool,
    wrap: bool,
    preview: bool,
    preview_html: SharedString,
    applied: (bool, bool, bool),
    busy: bool,
    close_prompt: bool,
    pub revision: u64,
}

impl Editor {
    pub fn new(file: Option<LoadedFile>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let untitled = file.is_none();
        let (input, input_observer, input_changes) =
            Self::make_input("", Language::Plain, window, cx);
        let mut editor = Self {
            input,
            input_observer,
            input_changes,
            focus: cx.focus_handle(),
            path: None,
            original: None,
            saved: Rope::new(),
            format: FileFormat::default(),
            language: Language::Plain,
            untitled,
            wrap: default_wrap(Language::Plain, cx),
            preview: false,
            preview_html: SharedString::default(),
            applied: prefs_key(cx),
            busy: false,
            close_prompt: false,
            revision: 0,
        };
        if let Some(file) = file {
            editor.load(file, window, cx);
        } else {
            editor.focus_input(window, cx);
        }
        editor
    }

    fn make_input(
        text: &str,
        language: Language,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> (Entity<InputState>, Subscription, Subscription) {
        let code = language != Language::Plain && cx.global::<Settings>().line_numbers;
        let wrap = default_wrap(language, cx);
        let tab_size = TabSize {
            tab_size: 4,
            hard_tabs: language == Language::Make,
        };
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor(language.name())
                .tab_size(tab_size)
                .line_number(code)
                .soft_wrap(wrap)
                .default_value(text.to_owned())
        });
        let observer = cx.observe_in(&input, window, |_, _, _, cx| cx.notify());
        let changes = cx.subscribe_in(&input, window, |editor, _, event, _, cx| {
            if matches!(event, InputEvent::Change) {
                editor.revision += 1;
                editor.refresh_preview(cx);
                cx.notify();
            }
        });
        (input, observer, changes)
    }

    pub fn load(&mut self, file: LoadedFile, window: &mut Window, cx: &mut Context<Self>) {
        self.revision += 1;
        self.language = language::detect(&file.path, &file.text);
        self.wrap = default_wrap(self.language, cx);
        self.preview = false;
        self.preview_html = SharedString::default();
        (self.input, self.input_observer, self.input_changes) =
            Self::make_input(&file.text, self.language, window, cx);
        self.saved = self.input.read(cx).text().clone();
        self.path = Some(file.path);
        self.original = Some(file.bytes);
        self.format = file.format;
        self.untitled = false;
        self.focus_input(window, cx);
        cx.notify();
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }
    pub fn is_empty_untitled(&self, cx: &App) -> bool {
        self.untitled && self.input.read(cx).text().len() == 0 && self.language == Language::Plain
    }
    fn dirty(&self, cx: &App) -> bool {
        self.input.read(cx).text() != &self.saved
    }
    pub fn busy(&self) -> bool {
        self.busy || self.close_prompt
    }
    pub fn show_dot(&self, cx: &App) -> bool {
        self.untitled || self.dirty(cx)
    }
    pub fn viewport_changed(&self, cx: &mut Context<Self>) {
        self.input.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    pub fn menu_state(&self) -> crate::MenuState {
        crate::MenuState {
            preview: self.preview,
            wrap: self.wrap,
            ..Default::default()
        }
    }

    pub fn input_focus(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }

    pub fn snapshot(&self, cx: &App) -> crate::session::Tab {
        let dirty = self.dirty(cx);
        crate::session::Tab {
            path: self.path.clone(),
            encoding: self.format.encoding,
            language: Some(self.language.name().into()),
            draft: (dirty || self.untitled).then(|| self.input.read(cx).text().to_string()),
            original: self
                .original
                .as_ref()
                .filter(|_| dirty)
                .map(|bytes| bytes.to_vec()),
        }
    }

    pub fn restore_presentation(
        &mut self,
        language: Option<&str>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(language) = language {
            self.set_language(Language::from_str(language), window, cx);
        }
        cx.notify();
    }

    pub fn restore_draft(&self, draft: String, window: &mut Window, cx: &mut Context<Self>) {
        self.input
            .update(cx, |input, cx| input.set_value(draft, window, cx));
    }

    pub fn title(&self) -> String {
        self.path
            .as_deref()
            .map_or_else(|| "Untitled".to_owned(), document::file_name)
    }

    fn focus_input(&self, window: &mut Window, cx: &mut App) {
        self.input.read(cx).focus_handle(cx).focus(window);
    }

    pub fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.close_prompt {
            return;
        }
        if !self.dirty(cx) {
            crate::close_tab(cx.entity_id(), window, cx);
            return;
        }
        self.close_prompt = true;
        let prompt = window.prompt(
            PromptLevel::Warning,
            &format!("Save changes to {}?", self.title()),
            Some("Closing this tab without saving discards its changes."),
            &[
                PromptButton::ok("Save"),
                PromptButton::new("Don’t Save"),
                PromptButton::cancel("Cancel"),
            ],
            cx,
        );
        cx.spawn_in(window, async move |editor, cx| {
            let result = prompt.await;
            let _ = cx.update(|window, cx| {
                let _ = editor.update(cx, |editor, cx| {
                    editor.close_prompt = false;
                    match result {
                        Ok(0) => editor.save(SaveMode::Current, true, window, cx),
                        Ok(1) => crate::close_tab(cx.entity_id(), window, cx),
                        _ => editor.focus_input(window, cx),
                    }
                });
            });
        })
        .detach();
    }

    fn save(
        &mut self,
        mode: SaveMode,
        close_after: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.busy || self.close_prompt {
            return;
        }
        if mode == SaveMode::Current
            && let Some(path) = self.path.clone()
        {
            if self.dirty(cx)
                || (self.language == Language::Json
                    && cx.global::<Settings>().beautify_json_on_save)
            {
                self.save_to(path, mode, close_after, window, cx);
            } else if close_after {
                crate::close_tab(cx.entity_id(), window, cx);
            }
            return;
        }
        let directory = self
            .path
            .as_deref()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .or_else(std::env::home_dir)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."));
        let name = if self.path.is_some() {
            self.title()
        } else {
            (match self.language {
                Language::Markdown => "Untitled.md",
                Language::Json => "Untitled.json",
                _ => "Untitled.txt",
            })
            .into()
        };
        self.busy = true;
        cx.notify();
        let result = cx.prompt_for_new_path(&directory, Some(&name));
        cx.spawn_in(window, async move |editor, cx| {
            let result = result.await;
            let _ = cx.update(|window, cx| {
                let _ = editor.update(cx, |editor, cx| {
                    editor.busy = false;
                    match result {
                        Ok(Ok(Some(path))) => editor.save_to(path, mode, close_after, window, cx),
                        Ok(Err(error)) => editor.message(
                            "Couldn’t show the Save dialog.",
                            &error.to_string(),
                            window,
                            cx,
                        ),
                        _ => editor.focus_input(window, cx),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn save_to(
        &mut self,
        path: PathBuf,
        mode: SaveMode,
        close_after: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Canonical paths keep Save As from bypassing protection for a currently open file.
        let path = std::fs::canonicalize(&path).unwrap_or(path);
        if crate::opened_file(&path, Some(cx.entity_id()), cx) {
            self.message(
                "This file is already open.",
                "Choose a different name or save in the window that already has this file.",
                window,
                cx,
            );
            return;
        }
        if self.language == Language::Json && cx.global::<Settings>().beautify_json_on_save {
            let source = self.input.read(cx).value();
            if let Ok(formatted) = crate::formatting::json(&source)
                && formatted != source.as_ref()
            {
                let end = source.encode_utf16().count();
                self.input.update(cx, |input, cx| {
                    input.replace_text_in_range(Some(0..end), &formatted, window, cx)
                });
            }
        }
        if mode == SaveMode::Current && !self.dirty(cx) {
            if close_after {
                crate::close_tab(cx.entity_id(), window, cx);
            }
            return;
        }
        let same_file = self.path.as_ref() == Some(&path);
        let expected = self.original.clone().filter(|_| same_file);
        let original = self.saved.clone();
        let buffer_snapshot = self.input.read(cx).text().clone();
        let text = buffer_snapshot.clone();
        let mut format = self.format.clone();
        if mode == SaveMode::Utf8 {
            format.encoding = Encoding::Utf8;
        }
        self.busy = true;
        cx.notify();
        let task = cx.background_executor().spawn(async move {
            document::save(
                &path,
                expected.as_deref(),
                &original.to_string(),
                &text.to_string(),
                &format,
            )
        });
        cx.spawn_in(window, async move |editor, cx| {
            let result = task.await;
            let _ = cx.update(|window, cx| {
                let _ = editor.update(cx, |editor, cx| {
                    editor.busy = false;
                    match result {
                        Ok(file) => {
                            let new_path = editor.path.as_ref() != Some(&file.path);
                            editor.revision += 1;
                            editor.saved = buffer_snapshot;
                            editor.path = Some(file.path);
                            editor.original = Some(file.bytes);
                            editor.format = file.format;
                            editor.untitled = false;
                            if new_path && let Some(path) = &editor.path {
                                editor.set_language(language::detect(path, &file.text), window, cx);
                            }
                            editor.focus_input(window, cx);
                            if close_after {
                                editor.request_close(window, cx);
                            }
                        }
                        Err(SaveError::Changed) => editor.disk_conflict(close_after, window, cx),
                        Err(SaveError::Other(error)) => {
                            editor.save_failed(mode, close_after, &format!("{error:#}"), window, cx)
                        }
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn save_failed(
        &mut self,
        mode: SaveMode,
        close_after: bool,
        detail: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_prompt = true;
        let prompt = window.prompt(
            PromptLevel::Warning,
            &format!("Couldn’t save {}.", self.title()),
            Some(detail),
            &[
                PromptButton::ok("Try Again"),
                PromptButton::new("Save As"),
                PromptButton::cancel("Cancel"),
            ],
            cx,
        );
        cx.spawn_in(window, async move |editor, cx| {
            let result = prompt.await;
            let _ = cx.update(|window, cx| {
                let _ = editor.update(cx, |editor, cx| {
                    editor.close_prompt = false;
                    match result {
                        Ok(0) => editor.save(mode, close_after, window, cx),
                        Ok(1) => editor.save(SaveMode::As, close_after, window, cx),
                        _ => editor.focus_input(window, cx),
                    }
                });
            });
        })
        .detach();
    }

    fn disk_conflict(&mut self, close_after: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.close_prompt = true;
        let prompt = window.prompt(
            PromptLevel::Warning, &format!("{} changed on disk.", self.title()),
            Some("Save your edits as another file, or reload the disk version and discard your edits."),
            &[PromptButton::ok("Save As"), PromptButton::new("Reload from Disk"), PromptButton::cancel("Cancel")], cx,
        );
        cx.spawn_in(window, async move |editor, cx| {
            let result = prompt.await;
            let _ = cx.update(|window, cx| {
                let _ = editor.update(cx, |editor, cx| {
                    editor.close_prompt = false;
                    match result {
                        Ok(0) => editor.save(SaveMode::As, close_after, window, cx),
                        Ok(1) => editor.reload(close_after, window, cx),
                        _ => editor.focus_input(window, cx),
                    }
                });
            });
        })
        .detach();
    }

    fn reload(&mut self, close_after: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let encoding =
            (self.format.encoding == Encoding::Windows1252).then_some(Encoding::Windows1252);
        self.busy = true;
        cx.notify();
        let read = cx
            .background_executor()
            .spawn(async move { document::open(path, encoding) });
        cx.spawn_in(window, async move |editor, cx| {
            let result = read.await;
            let _ = cx.update(|window, cx| {
                let _ = editor.update(cx, |editor, cx| {
                    editor.busy = false;
                    match result {
                        Ok(file) => {
                            editor.load(file, window, cx);
                            if close_after {
                                crate::close_tab(cx.entity_id(), window, cx);
                            }
                        }
                        Err(_) => editor.message(
                            "Couldn’t reload the file.",
                            "Your edits are still in this window. Save them to another location.",
                            window,
                            cx,
                        ),
                    }
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn message(&self, title: &str, detail: &str, window: &mut Window, cx: &mut Context<Self>) {
        let result = window.prompt(PromptLevel::Warning, title, Some(detail), &["OK"], cx);
        window
            .spawn(cx, async move |_| {
                let _ = result.await;
            })
            .detach();
    }

    fn set_language(&mut self, language: Language, window: &mut Window, cx: &mut Context<Self>) {
        if self.language == language {
            return;
        }
        self.language = language;
        self.revision += 1;
        self.wrap = default_wrap(language, cx);
        self.preview &= language == Language::Markdown;
        let line_numbers = cx.global::<Settings>().line_numbers;
        self.input.update(cx, |input, cx| {
            input.set_highlighter(language.name(), cx);
            input.set_line_number(language != Language::Plain && line_numbers, window, cx);
            input.set_soft_wrap(self.wrap, window, cx);
        });
        self.refresh_preview(cx);
        self.focus_input(window, cx);
        cx.notify();
    }

    fn refresh_preview(&mut self, cx: &App) {
        if self.preview {
            self.preview_html = preview::render(&self.input.read(cx).value()).into();
        }
    }

    fn toggle_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.language == Language::Plain {
            self.set_language(Language::Markdown, window, cx);
        }
        if self.language != Language::Markdown {
            return;
        }
        self.preview = !self.preview;
        self.refresh_preview(cx);
        if self.preview && window.viewport_size().width < px(720.) {
            window.focus(&self.focus);
        } else {
            self.focus_input(window, cx);
        }
        cx.notify();
    }

    fn apply_prefs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.wrap = default_wrap(self.language, cx);
        let line_numbers = self.language != Language::Plain && cx.global::<Settings>().line_numbers;
        let wrap = self.wrap;
        self.input.update(cx, |input, cx| {
            input.set_soft_wrap(wrap, window, cx);
            input.set_line_number(line_numbers, window, cx);
        });
        cx.notify();
    }

    fn toggle_wrap(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.wrap = !self.wrap;
        self.input
            .update(cx, |input, cx| input.set_soft_wrap(self.wrap, window, cx));
        cx.notify();
    }

    pub fn format_json(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy() {
            return;
        }
        let source = self.input.read(cx).value();
        match crate::formatting::json(&source) {
            Ok(formatted) if formatted != source.as_ref() => {
                let end = source.encode_utf16().count();
                self.input.update(cx, |input, cx| {
                    input.replace_text_in_range(Some(0..end), &formatted, window, cx)
                });
                self.focus_input(window, cx);
            }
            Ok(_) => self.focus_input(window, cx),
            Err(error) => self.message("Couldn’t format JSON.", &error.to_string(), window, cx),
        }
    }

    fn status(&self, window: &Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(cx);
        let position = self.input.read(cx).cursor_position();
        let dirty = self.dirty(cx);
        let current = self.language;
        let view = cx.entity().downgrade();
        let languages = Button::new("language")
            .ghost()
            .small()
            .text_size(px(11.))
            .h(px(20.))
            .label(language::label(current))
            .tooltip("Change syntax language")
            .dropdown_menu(move |menu, _, _| {
                let mut choices: Vec<_> = Language::all()
                    .filter(|language| *language != Language::MarkdownInline)
                    .collect();
                choices.sort_by_key(|language| {
                    (*language != Language::Plain, language::label(*language))
                });
                choices
                    .into_iter()
                    .fold(menu.max_h(px(360.)).scrollable(true), |menu, language| {
                        let view = view.clone();
                        menu.item(
                            PopupMenuItem::new(language::label(language))
                                .checked(language == current)
                                .on_click(move |_, window, cx| {
                                    let _ = view.update(cx, |editor, cx| {
                                        editor.set_language(language, window, cx)
                                    });
                                }),
                        )
                    })
            });
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .flex_shrink_0()
            .h(px(25.))
            .px_3()
            .bg(p.chrome)
            .border_t_1()
            .border_color(p.rule)
            .text_size(px(11.))
            .text_color(p.muted)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(format!(
                        "Ln {}, Col {}",
                        position.line + 1,
                        position.character + 1
                    ))
                    .when(
                        current != Language::Plain && window.viewport_size().width >= px(540.),
                        |row| {
                            row.child(if current == Language::Make {
                                "Tabs 4"
                            } else {
                                "Spaces 4"
                            })
                        },
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .when(
                        matches!(current, Language::Plain | Language::Markdown),
                        |row| {
                            row.child(
                                Button::new("preview")
                                    .ghost()
                                    .small()
                                    .text_size(px(11.))
                                    .h(px(20.))
                                    .label("Preview")
                                    .selected(self.preview)
                                    .tooltip("Show or hide the Markdown preview")
                                    .on_click(cx.listener(|editor, _, window, cx| {
                                        editor.toggle_preview(window, cx)
                                    })),
                            )
                        },
                    )
                    .child(self.format.encoding.label())
                    .child(self.format.endings.label())
                    .child(languages)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .when(dirty, |item| {
                                item.text_color(p.caret)
                                    .child(div().size(px(5.)).rounded_full().bg(p.caret))
                            })
                            .child(if self.busy {
                                "Saving…"
                            } else if dirty {
                                "Unsaved"
                            } else if self.untitled {
                                "New file"
                            } else {
                                "Saved"
                            }),
                    ),
            )
    }

    fn document_body(&self, window: &mut Window, cx: &mut App) -> AnyElement {
        let narrow = window.viewport_size().width < px(720.);
        let palette = Palette::current(cx);
        let prefs = cx.global::<Settings>().clone();
        let pane = div()
            .debug_selector(|| "editor-pane".into())
            .flex()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .size_full()
            .overflow_hidden();
        let input = || {
            Input::new(&self.input)
                .appearance(false)
                .h_full()
                .disabled(self.busy)
                .text_size(px(prefs.font_size))
                .line_height(relative(prefs.line_spacing.ratio()))
                .px_4()
                .py_5()
                .font_family(prefs.font.family())
        };
        let pane = if !self.preview || !narrow {
            pane.child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .debug_selector(|| "editor-source".into())
                    .child(input()),
            )
        } else {
            pane
        };
        if !self.preview {
            return pane.into_any_element();
        }
        let rendered = TextView::html("preview-document", self.preview_html.clone(), window, cx)
            .selectable(true)
            .scrollable(true)
            .font_family(if prefs.font == EditorFont::Code {
                theme::SANS
            } else {
                prefs.font.family()
            })
            .text_size(px(prefs.font_size))
            .size_full();
        pane.child(
            div()
                .debug_selector(|| "editor-preview".into())
                .flex()
                .flex_col()
                .h_full()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .bg(palette.chrome)
                .when(!narrow, |panel| {
                    panel.border_l_1().border_color(palette.rule)
                })
                .child(
                    div()
                        .px_6()
                        .pt_4()
                        .pb_2()
                        .text_size(px(12.))
                        .text_color(palette.muted)
                        .font_family(theme::MONO)
                        .child("Preview"),
                )
                .child(div().flex_1().min_h_0().px_6().pb_4().child(rendered)),
        )
        .into_any_element()
    }
}

impl Render for Editor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(cx);
        if window.is_window_active() {
            crate::sync_menus(self.menu_state(), cx);
        }
        let key = prefs_key(cx);
        if key != self.applied {
            self.applied = key;
            cx.defer_in(window, |editor, window, cx| editor.apply_prefs(window, cx));
        }
        let body = self.document_body(window, cx);
        div()
            .id("foldrest")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .bg(p.paper)
            .text_color(p.ink)
            .font_family(theme::SANS)
            .on_action(cx.listener(|editor, _: &SaveFile, window, cx| {
                editor.save(SaveMode::Current, false, window, cx)
            }))
            .on_action(cx.listener(|editor, _: &SaveAs, window, cx| {
                editor.save(SaveMode::As, false, window, cx)
            }))
            .on_action(cx.listener(|editor, _: &SaveUtf8, window, cx| {
                editor.save(SaveMode::Utf8, false, window, cx)
            }))
            .on_action(cx.listener(|editor, _: &TogglePreview, window, cx| {
                editor.toggle_preview(window, cx)
            }))
            .on_action(
                cx.listener(|editor, _: &ToggleWrap, window, cx| editor.toggle_wrap(window, cx)),
            )
            .on_action(|_: &OpenFile, window, cx| {
                cx.stop_propagation();
                crate::choose_file(window, cx);
            })
            .can_drop(|drag, _, _| drag.is::<ExternalPaths>())
            .on_drop(|paths: &ExternalPaths, window, cx| {
                for path in paths.paths() {
                    crate::open_path(path.clone(), None, Some(window.window_handle()), cx);
                }
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .overflow_hidden()
                    .child(body),
            )
            .child(self.status(window, cx))
    }
}
