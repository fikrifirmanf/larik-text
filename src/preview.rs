use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};

pub fn render(markdown: &str) -> String {
    let events = Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    );
    let safe = events.filter_map(|event| match event {
        // The preview has no active links, embedded files, or network loads.
        Event::Start(Tag::Link { .. } | Tag::Image { .. })
        | Event::End(TagEnd::Link | TagEnd::Image) => None,
        Event::Html(text) | Event::InlineHtml(text) => Some(Event::Text(text)),
        other => Some(other),
    });
    let mut output = String::new();
    html::push_html(&mut output, safe);
    output
}

#[cfg(test)]
mod tests {
    #[test]
    fn preview_renders_markdown_without_links_images_or_raw_html() {
        let html = super::render(
            "# Reading notes\n\n- **Finish chapter two**\n- ![Cover](https://example.com/cover.png)\n\n[Notes](https://example.com)\n\n<img src=\"file:///notes.png\">",
        );
        assert!(html.contains("<h1>Reading notes</h1>"));
        assert!(html.contains("<li><strong>Finish chapter two</strong></li>"));
        assert!(html.contains("<li>Cover</li>"));
        assert!(html.contains("<p>Notes</p>"));
        assert!(html.contains("&lt;img"));
        assert!(!html.contains("<img"));
        assert!(!html.contains("<a "));
    }
}
