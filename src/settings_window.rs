use crate::theme::{
    self, Appearance, EditorFont, LineSpacing, MAX_FONT_SIZE, MIN_FONT_SIZE, Palette, Settings,
};
use gpui::{
    AnyElement, AnyWindowHandle, App, Bounds, Context, IntoElement, ParentElement as _, Render,
    SharedString, Styled as _, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use gpui_component::{
    Disableable as _, Root, Sizable as _,
    button::{Button, ButtonVariants as _},
    switch::Switch,
};

struct Open(Option<AnyWindowHandle>);
impl gpui::Global for Open {}

/// Opens the settings window, or brings the existing one forward.
pub fn open(cx: &mut App) {
    if let Some(handle) = cx.try_global::<Open>().and_then(|open| open.0)
        && cx.windows().contains(&handle)
        && handle
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(560.), px(640.)),
            cx,
        ))),
        window_min_size: Some(size(px(440.), px(380.))),
        app_id: Some("app.larik.LarikText".to_owned()),
        titlebar: Some(gpui::TitlebarOptions {
            title: Some("Settings".into()),
            ..Default::default()
        }),
        ..Default::default()
    };
    match cx.open_window(options, |window, cx| {
        let view = cx.new(|_| SettingsView);
        cx.new(|cx| Root::new(view, window, cx))
    }) {
        Ok(window) => cx.set_global(Open(Some(window.into()))),
        Err(error) => eprintln!("Could not open settings: {error:#}"),
    }
}

struct SettingsView;

fn section(title: &'static str, p: &Palette, rows: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .pb_1()
                .text_size(px(11.))
                .text_color(p.muted)
                .child(title.to_uppercase()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .rounded_lg()
                .border_1()
                .border_color(p.rule)
                .bg(p.chrome)
                .children(rows.into_iter().enumerate().map(|(i, row)| {
                    div()
                        .when(i > 0, |d| d.border_t_1().border_color(p.rule))
                        .child(row)
                })),
        )
        .into_any_element()
}

fn row(
    label: &'static str,
    hint: &'static str,
    p: &Palette,
    control: impl IntoElement,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .px_3()
        .py_2()
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(div().text_size(px(13.)).child(label))
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(p.muted)
                        .child(SharedString::from(hint)),
                ),
        )
        .child(div().flex_shrink_0().child(control))
        .into_any_element()
}

fn choice<T: Copy + PartialEq + 'static>(
    id: &'static str,
    options: &[(T, &'static str)],
    current: T,
    set: fn(&mut Settings, T),
) -> impl IntoElement {
    div()
        .flex()
        .gap_1()
        .children(options.iter().map(|&(value, label)| {
            {
                let button = Button::new(SharedString::from(format!("{id}-{label}")))
                    .small()
                    .label(label);
                if current == value {
                    button.primary()
                } else {
                    button.outline()
                }
            }
            .on_click(move |_, _, cx| theme::update_settings(cx, |s| set(s, value)))
        }))
}

fn toggle(id: &'static str, on: bool, set: fn(&mut Settings, bool)) -> impl IntoElement {
    Switch::new(id)
        .checked(on)
        .on_click(move |value: &bool, _, cx| {
            let value = *value;
            theme::update_settings(cx, |s| set(s, value))
        })
}

impl Render for SettingsView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = Palette::current(cx);
        let s = cx.global::<Settings>().clone();
        let stepper = div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                Button::new("size-down")
                    .small()
                    .ghost()
                    .label("−")
                    .disabled(s.font_size <= MIN_FONT_SIZE)
                    .on_click(|_, _, cx| theme::update_settings(cx, |s| s.font_size -= 1.)),
            )
            .child(
                div()
                    .w(px(44.))
                    .text_size(px(12.))
                    .flex()
                    .justify_center()
                    .child(format!("{} pt", s.font_size as u32)),
            )
            .child(
                Button::new("size-up")
                    .small()
                    .ghost()
                    .label("+")
                    .disabled(s.font_size >= MAX_FONT_SIZE)
                    .on_click(|_, _, cx| theme::update_settings(cx, |s| s.font_size += 1.)),
            )
            .child(
                Button::new("size-reset")
                    .small()
                    .ghost()
                    .label("Reset")
                    .on_click(|_, _, cx| {
                        theme::update_settings(cx, |s| s.font_size = theme::DEFAULT_FONT_SIZE)
                    }),
            );

        div()
            .id("settings")
            .size_full()
            .overflow_y_scroll()
            .bg(p.paper)
            .text_color(p.ink)
            .font_family(theme::SANS)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_5()
                    .p_5()
                    .child(section(
                        "Appearance",
                        &p,
                        vec![row(
                            "Theme",
                            "Follow the system or choose one.",
                            &p,
                            choice(
                                "theme",
                                &[
                                    (Appearance::System, "System"),
                                    (Appearance::Light, "Light"),
                                    (Appearance::Dark, "Dark"),
                                ],
                                s.appearance,
                                |s, v| s.appearance = v,
                            ),
                        )],
                    ))
                    .child(section(
                        "Text",
                        &p,
                        vec![
                            row(
                                "Font",
                                "Used in the editor and preview.",
                                &p,
                                choice(
                                    "font",
                                    &EditorFont::ALL.map(|f| (f, f.label())),
                                    s.font,
                                    |s, v| s.font = v,
                                ),
                            ),
                            row("Text size", "Also changes with ⌘+ and ⌘−.", &p, stepper),
                            row(
                                "Line spacing",
                                "Space between lines.",
                                &p,
                                choice(
                                    "spacing",
                                    &LineSpacing::ALL.map(|l| (l, l.label())),
                                    s.line_spacing,
                                    |s, v| s.line_spacing = v,
                                ),
                            ),
                        ],
                    ))
                    .child(section(
                        "Editor",
                        &p,
                        vec![
                            row(
                                "Wrap text and Markdown",
                                "Long lines continue on the next line.",
                                &p,
                                toggle("wrap-text", s.wrap_text, |s, v| s.wrap_text = v),
                            ),
                            row(
                                "Wrap code",
                                "Off keeps code lines intact and scrolls sideways.",
                                &p,
                                toggle("wrap-code", s.wrap_code, |s, v| s.wrap_code = v),
                            ),
                            row(
                                "Line numbers",
                                "Shown for code files.",
                                &p,
                                toggle("line-numbers", s.line_numbers, |s, v| s.line_numbers = v),
                            ),
                        ],
                    ))
                    .child(section(
                        "Files",
                        &p,
                        vec![row(
                            "Beautify JSON on save",
                            "Reformat valid JSON files when saving.",
                            &p,
                            toggle("beautify", s.beautify_json_on_save, |s, v| {
                                s.beautify_json_on_save = v
                            }),
                        )],
                    ))
                    .child(section(
                        "Updates",
                        &p,
                        vec![row(
                            "Check on launch",
                            "Look for a new version shortly after opening.",
                            &p,
                            toggle("updates", s.check_updates_on_launch, |s, v| {
                                s.check_updates_on_launch = v
                            }),
                        )],
                    )),
            )
    }
}
