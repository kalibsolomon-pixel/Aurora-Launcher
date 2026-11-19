//! CommonMark parsed once into a small native rendering model. Raw HTML is
//! kept as text and linked images use the same artwork policy as the gallery.

use aurora_launcher_lib::modrinth::safe_web_link;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Style {
    pub bold: bool,
    pub italic: bool,
    pub code: bool,
}

#[derive(Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub style: Style,
    pub link: Option<String>,
}

#[derive(Debug, PartialEq)]
pub enum Block {
    Text(Vec<Span>),
    Heading(u8, Vec<Span>),
    Quote(Vec<Block>),
    List(Option<u64>, Vec<Block>),
    Item(Vec<Block>),
    Code(String),
    Html(String),
    Image { alt: String, url: Option<String> },
    Rule,
}

enum Container {
    Document,
    Quote,
    List(Option<u64>),
    Item,
}

struct Frame {
    container: Container,
    blocks: Vec<Block>,
}

pub fn parse(text: &str, base: &str) -> Vec<Block> {
    // Preserve text when a publisher nests beyond a usable native layout.
    const MAX_DEPTH: usize = 32;
    let mut events = Parser::new_ext(text, Options::ENABLE_GFM);
    let mut stack = vec![Frame {
        container: Container::Document,
        blocks: Vec::new(),
    }];
    let mut spans = Vec::new();
    let mut heading = None;
    let (mut bold, mut italic) = (0usize, 0usize);
    let mut link: Option<String> = None;
    let mut flattened = 0usize;
    while let Some(event) = events.next() {
        let depth = stack.len();
        let frame = stack.last_mut().unwrap();
        let push_text = |frame: &mut Frame, spans: &mut Vec<Span>| {
            if !spans.is_empty() {
                let spans = std::mem::take(spans);
                frame.blocks.push(match heading {
                    Some(level) => Block::Heading(level, spans),
                    None => Block::Text(spans),
                });
            }
        };
        let style = Style {
            bold: bold != 0,
            italic: italic != 0,
            code: false,
        };
        let mut span = |text: String, style| {
            spans.push(Span {
                text,
                style,
                link: link.clone(),
            })
        };
        match event {
            Event::Start(Tag::Heading { level, .. }) => heading = Some(level as u8),
            Event::End(TagEnd::Heading(_) | TagEnd::Paragraph) => {
                push_text(frame, &mut spans);
                heading = None;
            }
            Event::Start(Tag::Strong) => bold += 1,
            Event::End(TagEnd::Strong) => bold -= 1,
            Event::Start(Tag::Emphasis) => italic += 1,
            Event::End(TagEnd::Emphasis) => italic -= 1,
            Event::Start(Tag::Link { dest_url, .. }) => link = safe_web_link(&dest_url, Some(base)),
            Event::End(TagEnd::Link) => link = None,
            Event::Start(Tag::Image { dest_url, .. }) => {
                push_text(frame, &mut spans);
                frame.blocks.push(Block::Image {
                    alt: collect_text(&mut events, TagEnd::Image),
                    url: safe_web_link(&dest_url, Some(base)),
                });
            }
            Event::Start(tag @ (Tag::BlockQuote(_) | Tag::List(_) | Tag::Item)) => {
                push_text(frame, &mut spans);
                if depth >= MAX_DEPTH {
                    flattened += 1;
                    continue;
                }
                let container = match tag {
                    Tag::BlockQuote(_) => Container::Quote,
                    Tag::List(start) => Container::List(start),
                    _ => Container::Item,
                };
                stack.push(Frame {
                    container,
                    blocks: Vec::new(),
                });
            }
            Event::End(TagEnd::BlockQuote(_) | TagEnd::List(_) | TagEnd::Item) => {
                push_text(frame, &mut spans);
                if flattened > 0 {
                    flattened -= 1;
                    continue;
                }
                let frame = stack.pop().unwrap();
                let block = match frame.container {
                    Container::Quote => Block::Quote(frame.blocks),
                    Container::List(start) => Block::List(start, frame.blocks),
                    Container::Item => Block::Item(frame.blocks),
                    Container::Document => unreachable!("CommonMark containers are balanced"),
                };
                stack.last_mut().unwrap().blocks.push(block);
            }
            Event::Start(Tag::CodeBlock(_)) => frame
                .blocks
                .push(Block::Code(collect_text(&mut events, TagEnd::CodeBlock))),
            Event::Start(Tag::HtmlBlock) => frame
                .blocks
                .push(Block::Html(collect_text(&mut events, TagEnd::HtmlBlock))),
            Event::Rule => {
                push_text(frame, &mut spans);
                frame.blocks.push(Block::Rule);
            }
            Event::Text(text) | Event::InlineHtml(text) => span(text.into_string(), style),
            Event::Code(text) => span(
                text.into_string(),
                Style {
                    code: true,
                    ..style
                },
            ),
            Event::SoftBreak => span(" ".into(), style),
            Event::HardBreak => span("\n".into(), style),
            _ => {}
        }
    }
    let mut blocks = stack.pop().unwrap().blocks;
    if !spans.is_empty() {
        blocks.push(Block::Text(spans));
    }
    blocks
}

fn collect_text(events: &mut Parser<'_>, end: TagEnd) -> String {
    let mut text = String::new();
    let mut depth = 0;
    for event in events {
        match event {
            Event::End(tag) if tag == end && depth == 0 => break,
            Event::Start(tag) if tag.to_end() == end => depth += 1,
            Event::End(tag) if tag == end => depth -= 1,
            Event::Text(value)
            | Event::Code(value)
            | Event::Html(value)
            | Event::InlineHtml(value) => {
                text.push_str(&value);
            }
            Event::SoftBreak | Event::HardBreak => text.push('\n'),
            _ => {}
        }
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commonmark_preserves_structure_styles_and_full_text() {
        let document = parse(
            "# Heading\n\nStart **bold and *italic***, `code` and [docs](/help).\n\n> A quote\n\n3. First\n   - Child\n4. Second\n\n```rs\nlet x = 1;\n```\n\n![Picture](https://cdn.modrinth.com/data/image.png)\n\n<div>Plain HTML</div>\n\nFinal paragraph.",
            "https://modrinth.com/mod/example",
        );
        assert!(matches!(&document[0], Block::Heading(1, spans) if spans[0].text == "Heading"));
        let Block::Text(spans) = &document[1] else {
            panic!("paragraph")
        };
        assert!(
            spans
                .iter()
                .any(|s| s.style.bold && s.style.italic && s.text == "italic")
        );
        assert!(spans.iter().any(|s| s.style.code && s.text == "code"));
        assert!(
            spans
                .iter()
                .any(|s| s.link.as_deref() == Some("https://modrinth.com/help"))
        );
        assert!(
            matches!(&document[2], Block::Quote(blocks) if matches!(&blocks[0], Block::Text(_)))
        );
        assert!(matches!(&document[3], Block::List(Some(3), items)
            if matches!(&items[0], Block::Item(blocks) if matches!(&blocks[1], Block::List(None, _)))));
        assert_eq!(document[4], Block::Code("let x = 1;\n".into()));
        assert!(matches!(&document[5], Block::Image { alt, url: Some(_) } if alt == "Picture"));
        assert!(
            matches!(&document[6], Block::Html(text) if text.contains("<div>Plain HTML</div>"))
        );
        assert!(matches!(&document[7], Block::Text(spans) if spans[0].text == "Final paragraph."));
    }

    #[test]
    fn untrusted_html_and_unsafe_urls_are_passive_text() {
        let document = parse(
            "[Unsafe](javascript:alert(1)) <img src=x> ![Local](file:///private.png)",
            "https://modrinth.com/mod/example",
        );
        assert!(
            matches!(&document[0], Block::Text(spans) if spans.iter().all(|s| s.link.is_none())
            && spans.iter().any(|s| s.text == "<img src=x>"))
        );
        assert!(matches!(&document[1], Block::Image { url: None, alt } if alt == "Local"));
    }
}
