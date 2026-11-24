use crate::{
    CloseFile, Editor, NewFile, NextTab, PreviousTab, document::LoadedFile, session, theme::Palette,
};
use gpui::{
    App, AppContext as _, Context, Entity, EntityId, FocusHandle, Subscription, Task, Window, div,
    prelude::*, px,
};
use gpui_component::{
    IconName, TitleBar,
    button::{Button, ButtonVariants as _},
    menu::AppMenuBar,
};
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

struct Tab {
    editor: Entity<Editor>,
    _observer: Subscription,
    revision: u64,
}

pub struct Workspace {
    tabs: Vec<Tab>,
    active: usize,
    session_path: Option<PathBuf>,
    pending: Option<Task<()>>,
    menu: Option<Entity<AppMenuBar>>,
    focus: FocusHandle,
    _appearance: Subscription,
    _resize: Subscription,
}

impl Workspace {
    pub fn new(
        file: Option<LoadedFile>,
        untitled: bool,
        session_path: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let menu = (!cfg!(target_os = "macos")).then(|| AppMenuBar::new(window, cx));
        let mut workspace = Self {
            tabs: Vec::new(),
            active: 0,
            session_path,
            pending: None,
            menu,
            focus: cx.focus_handle(),
            _resize: cx.observe_window_bounds(window, |workspace, _, cx| {
                workspace
                    .active()
                    .update(cx, |editor, cx| editor.viewport_changed(cx));
                cx.notify();
            }),
            _appearance: window.observe_window_appearance(|window, cx| {
                use crate::theme::{self, Appearance, Settings};
                if cx.global::<Settings>().appearance == Appearance::System {
                    theme::apply(Appearance::System, Some(window), cx);
                }
            }),
        };
        if let Some(path) = &workspace.session_path {
            match session::load(path) {
                Ok(session) => {
                    let mut selected = None;
                    for (i, tab) in session.tabs.into_iter().enumerate() {
                        let keep_empty = tab.path.is_none();
                        let language = tab.language.clone();
                        let (file, draft) = tab.restore();
                        if file.is_some() || draft.is_some() || keep_empty {
                            workspace.push(file, draft, window, cx);
                            workspace.active().update(cx, |editor, cx| {
                                editor.restore_presentation(language.as_deref(), window, cx);
                            });
                            if i == session.active {
                                selected = Some(workspace.active);
                            }
                        }
                    }
                    if let Some(active) = selected {
                        workspace.active = active;
                    }
                }
                Err(error) => {
                    eprintln!("Couldn’t restore tabs: {error:#}");
                    // Preserve an unreadable checkpoint for manual recovery.
                    workspace.session_path = None;
                }
            }
        }
        if file.is_some() || untitled || workspace.tabs.is_empty() {
            workspace.push(file, None, window, cx);
        }
        workspace.activate(window, cx);
        workspace
    }

    pub fn active(&self) -> &Entity<Editor> {
        &self.tabs[self.active].editor
    }

    pub fn busy(&self, cx: &App) -> bool {
        self.tabs.iter().any(|tab| tab.editor.read(cx).busy())
    }

    pub fn has_path(&self, path: &Path, except: Option<EntityId>, cx: &App) -> bool {
        self.tabs.iter().any(|tab| {
            Some(tab.editor.entity_id()) != except && tab.editor.read(cx).path() == Some(path)
        })
    }

    pub fn open(&mut self, file: Option<LoadedFile>, window: &mut Window, cx: &mut Context<Self>) {
        let existing = file.as_ref().and_then(|file| {
            self.tabs
                .iter()
                .position(|tab| tab.editor.read(cx).path() == Some(&file.path))
        });
        if let Some(index) = existing {
            self.active = index;
        } else {
            match file {
                Some(file)
                    if self.tabs.len() == 1 && self.active().read(cx).is_empty_untitled(cx) =>
                {
                    self.active()
                        .update(cx, |editor, cx| editor.load(file, window, cx));
                }
                file => self.push(file, None, window, cx),
            }
        }
        self.activate(window, cx);
        self.schedule(window, cx);
    }

    fn push(
        &mut self,
        file: Option<LoadedFile>,
        draft: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editor = cx.new(|cx| {
            let editor = Editor::new(file, window, cx);
            if let Some(draft) = draft {
                editor.restore_draft(draft, window, cx);
            }
            editor
        });
        let revision = editor.read(cx).revision;
        let observer = cx.observe_in(&editor, window, |workspace, editor, window, cx| {
            let current = editor.read(cx);
            let Some(index) = workspace.tabs.iter().position(|tab| tab.editor == editor) else {
                return;
            };
            if workspace.tabs[index].revision == current.revision {
                return;
            }
            workspace.tabs[index].revision = current.revision;
            if index == workspace.active {
                window.set_window_title(&current.title());
            }
            workspace.schedule(window, cx);
            cx.notify();
        });
        self.tabs.push(Tab {
            editor,
            _observer: observer,
            revision,
        });
        self.active = self.tabs.len() - 1;
    }

    pub fn remove(&mut self, id: EntityId, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self
            .tabs
            .iter()
            .position(|tab| tab.editor.entity_id() == id)
        {
            self.tabs.remove(index);
            if index < self.active {
                self.active -= 1;
            }
            if self.tabs.is_empty() {
                self.push(None, None, window, cx);
            }
            self.active = self.active.min(self.tabs.len() - 1);
            self.activate(window, cx);
            self.schedule(window, cx);
        }
    }

    pub fn persist(&self, cx: &App) -> anyhow::Result<()> {
        if let Some(path) = &self.session_path {
            session::save(
                path,
                session::Session {
                    active: self.active,
                    tabs: self
                        .tabs
                        .iter()
                        .map(|tab| tab.editor.read(cx).snapshot(cx))
                        .collect(),
                },
            )?;
        }
        Ok(())
    }

    fn schedule(&mut self, window: &Window, cx: &Context<Self>) {
        if self.session_path.is_none() {
            return;
        }
        self.pending = Some(cx.spawn_in(window, async |workspace, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(300))
                .await;
            let _ = cx.update(|_, cx| {
                if let Some(workspace) = workspace.upgrade()
                    && let Err(error) = workspace.read(cx).persist(cx)
                {
                    eprintln!("Couldn’t save tabs: {error:#}");
                }
            });
        }));
    }

    fn cycle(&mut self, offset: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.tabs.len() < 2 {
            return;
        }
        self.active = (self.active + offset) % self.tabs.len();
        self.activate(window, cx);
        self.schedule(window, cx);
    }

    fn activate(&self, window: &mut Window, cx: &mut Context<Self>) {
        let editor = self.active().read(cx);
        window.set_window_title(&editor.title());
        let focus = editor.input_focus(cx);
        window.focus(&focus);
        cx.notify();
    }
}

impl Render for Workspace {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(cx);
        let active = self.active().read(cx);
        let title = active.title();
        let unsaved = active.show_dot(cx);
        let tabs = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, tab)| {
                let editor = tab.editor.read(cx);
                let selected = index == self.active;
                let dirty = editor.show_dot(cx);
                let name = editor.title();
                div()
                    .flex()
                    .items_center()
                    .min_w(px(110.))
                    .max_w(px(220.))
                    .flex_1()
                    .flex_shrink_0()
                    .overflow_hidden()
                    .h(px(24.))
                    .rounded_full()
                    .bg(if selected { p.active_line } else { p.chrome })
                    .child(
                        div()
                            .id(("tab", index))
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .flex()
                            .items_center()
                            .pl_3()
                            .pr_1()
                            .cursor_pointer()
                            .text_size(px(12.))
                            .child(div().min_w_0().truncate().child(name))
                            .on_click(cx.listener(move |workspace, _, window, cx| {
                                workspace.active = index;
                                workspace.activate(window, cx);
                                workspace.schedule(window, cx);
                            })),
                    )
                    .when(dirty, |tab| {
                        tab.child(div().flex_shrink_0().size(px(6.)).rounded_full().bg(p.ink))
                    })
                    .child(
                        Button::new(("close-tab", index))
                            .ghost()
                            .icon(IconName::Close)
                            .tooltip("Close tab")
                            .flex_shrink_0()
                            .size(px(22.))
                            .on_click(cx.listener(move |workspace, _, window, cx| {
                                workspace.active = index;
                                workspace.activate(window, cx);
                                let editor = workspace.active().clone();
                                window.defer(cx, move |window, cx| {
                                    editor.update(cx, |editor, cx| editor.request_close(window, cx))
                                });
                            })),
                    )
            })
            .collect::<Vec<_>>();
        let strip = div()
            .id("tabs")
            .flex()
            .items_center()
            .flex_shrink_0()
            .gap_2()
            .px_2()
            .h(px(32.))
            .bg(p.chrome)
            .border_b_1()
            .border_color(p.rule)
            .child(
                div()
                    .id("tab-list")
                    .flex()
                    .flex_1()
                    .min_w_0()
                    .gap_1()
                    .overflow_x_scroll()
                    .children(tabs),
            )
            .child(
                Button::new("new-tab")
                    .ghost()
                    .icon(IconName::Plus)
                    .tooltip("New tab")
                    .size(px(26.))
                    .on_click(
                        cx.listener(|workspace, _, window, cx| workspace.open(None, window, cx)),
                    ),
            );
        div()
            .id("workspace")
            .track_focus(&self.focus)
            .flex()
            .flex_col()
            .size_full()
            .bg(p.paper)
            .on_action(cx.listener(|workspace, _: &NewFile, window, cx| {
                cx.stop_propagation();
                workspace.open(None, window, cx);
            }))
            .on_action(
                cx.listener(|workspace, _: &NextTab, window, cx| workspace.cycle(1, window, cx)),
            )
            .on_action(cx.listener(|workspace, _: &PreviousTab, window, cx| {
                workspace.cycle(workspace.tabs.len().saturating_sub(1), window, cx)
            }))
            .on_action(cx.listener(|workspace, _: &CloseFile, window, cx| {
                let editor = workspace.active().clone();
                window.defer(cx, move |window, cx| {
                    editor.update(cx, |editor, cx| editor.request_close(window, cx))
                });
            }))
            .when(cfg!(target_os = "macos"), |root| {
                root.child(
                    TitleBar::new().h(px(34.)).pr(px(80.)).child(
                        div()
                            .flex()
                            .justify_center()
                            .min_w_0()
                            .w_full()
                            .text_size(px(12.))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .items_center()
                            .gap_1()
                            .child(div().truncate().child(title))
                            .when(unsaved, |row| {
                                row.child(
                                    div()
                                        .size(px(5.))
                                        .flex_shrink_0()
                                        .rounded_full()
                                        .bg(p.caret),
                                )
                            }),
                    ),
                )
            })
            .when_some(self.menu.clone(), |root, menu| {
                root.child(div().h(px(32.)).flex_shrink_0().bg(p.chrome).child(menu))
            })
            .child(strip)
            .child(
                div()
                    .min_h_0()
                    .min_w_0()
                    .flex_1()
                    .overflow_hidden()
                    .child(self.active().clone()),
            )
    }
}
