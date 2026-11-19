//! CommonMark parsed once into a small native rendering model. Embedded HTML
//! is reduced to the same blocks (paragraphs, headings, lists, links, images);
//! unknown tags are dropped and never shown as markup. Linked images use the
//! same artwork policy as the gallery.

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
            Event::Start(Tag::HtmlBlock) => {
                push_text(frame, &mut spans);
                let html = collect_text(&mut events, TagEnd::HtmlBlock);
                frame.blocks.extend(html_blocks(&html, base));
            }
            Event::Rule => {
                push_text(frame, &mut spans);
                frame.blocks.push(Block::Rule);
            }
            Event::Text(text) => span(text.into_string(), style),
            Event::InlineHtml(html) => match tag(&html) {
                Some(("br", false, _)) => span("\n".into(), style),
                Some(("b" | "strong", closing, _)) => {
                    bold = if closing {
                        bold.saturating_sub(1)
                    } else {
                        bold + 1
                    }
                }
                Some(("i" | "em", closing, _)) => {
                    italic = if closing {
                        italic.saturating_sub(1)
                    } else {
                        italic + 1
                    }
                }
                Some(("a", false, attrs)) => {
                    link = attr(attrs, "href").and_then(|href| safe_web_link(&href, Some(base)))
                }
                Some(("a", true, _)) => link = None,
                Some(("img", false, attrs)) => {
                    if let Some(alt) = attr(attrs, "alt").filter(|alt| !alt.trim().is_empty()) {
                        span(alt, style)
                    }
                }
                // Other inline tags are presentation only.
                _ => {}
            },
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

/// Parse one tag token (`<a href=..>`, `</p>`, `<br/>`) into its lowercase
/// name, whether it closes, and its raw attribute text.
fn tag(token: &str) -> Option<(&'static str, bool, &str)> {
    let inner = token.trim().strip_prefix('<')?.strip_suffix('>')?;
    let (closing, inner) = match inner.strip_prefix('/') {
        Some(rest) => (true, rest),
        None => (false, inner),
    };
    let inner = inner.trim_end_matches('/');
    let end = inner
        .find(|c: char| c.is_ascii_whitespace())
        .unwrap_or(inner.len());
    let name = inner[..end].to_ascii_lowercase();
    const KNOWN: &[&str] = &[
        "a",
        "b",
        "strong",
        "i",
        "em",
        "code",
        "br",
        "img",
        "p",
        "div",
        "center",
        "h1",
        "h2",
        "h3",
        "h4",
        "h5",
        "h6",
        "ul",
        "ol",
        "li",
        "blockquote",
        "hr",
        "tr",
        "summary",
        "details",
        "table",
        "pre",
        "script",
        "style",
        "iframe",
    ];
    let name = KNOWN.iter().find(|known| **known == name)?;
    Some((name, closing, &inner[end..]))
}

/// Value of a quoted or bare attribute, with common entities decoded.
fn attr(attrs: &str, name: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let mut from = 0;
    while let Some(found) = lower[from..].find(name) {
        let start = from + found;
        from = start + name.len();
        let before = lower[..start].chars().next_back();
        if before.is_some_and(|c| !c.is_ascii_whitespace()) {
            continue;
        }
        let rest = attrs[from..].trim_start();
        let Some(rest) = rest.strip_prefix('=') else {
            continue;
        };
        let rest = rest.trim_start();
        let value = match rest.chars().next()? {
            quote @ ('"' | '\'') => rest[1..].split(quote).next()?,
            _ => rest.split(|c: char| c.is_ascii_whitespace()).next()?,
        };
        return Some(decode(value));
    }
    None
}

fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let window = rest
            .char_indices()
            .take(12)
            .map(|(i, _)| i)
            .last()
            .unwrap_or(0);
        let Some(end) = rest[..=window].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ => entity
                .strip_prefix("#x")
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|n| n.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

struct HtmlState {
    blocks: Vec<Block>,
    spans: Vec<Span>,
    heading: Option<u8>,
    lists: Vec<(Option<u64>, Vec<Block>)>,
    bold: usize,
    italic: usize,
    code: usize,
    link: Option<String>,
}

impl HtmlState {
    fn flush(&mut self) {
        while self
            .spans
            .last()
            .is_some_and(|span| span.text.trim().is_empty())
        {
            self.spans.pop();
        }
        if let Some(first) = self.spans.first_mut() {
            first.text = first.text.trim_start().to_owned();
        }
        if let Some(last) = self.spans.last_mut() {
            last.text = last.text.trim_end().to_owned();
        }
        if self.spans.is_empty() {
            return;
        }
        let spans = std::mem::take(&mut self.spans);
        let block = match self.heading {
            Some(level) => Block::Heading(level, spans),
            None => Block::Text(spans),
        };
        match self.lists.last_mut() {
            Some((_, items)) => items.push(Block::Item(vec![block])),
            None => self.blocks.push(block),
        }
    }

    fn text(&mut self, text: &str) {
        let mut collapsed = text.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
        if text.starts_with(|c: char| c.is_ascii_whitespace()) {
            collapsed.insert(0, ' ');
        }
        if text.ends_with(|c: char| c.is_ascii_whitespace()) && !collapsed.ends_with(' ') {
            collapsed.push(' ');
        }
        if collapsed.is_empty() || (collapsed == " " && self.spans.is_empty()) {
            return;
        }
        self.spans.push(Span {
            text: decode(&collapsed),
            style: Style {
                bold: self.bold != 0,
                italic: self.italic != 0,
                code: self.code != 0,
            },
            link: self.link.clone(),
        });
    }

    fn close_list(&mut self) {
        if let Some((start, items)) = self.lists.pop() {
            let list = Block::List(start, items);
            match self.lists.last_mut() {
                Some((_, outer)) => outer.push(Block::Item(vec![list])),
                None => self.blocks.push(list),
            }
        }
    }
}

/// Reduce an HTML block to native blocks. Text is whitespace-collapsed,
/// links pass the same safety check as Markdown links, and content of
/// script/style/iframe elements is discarded.
fn html_blocks(html: &str, base: &str) -> Vec<Block> {
    const MAX_LISTS: usize = 8;
    let mut state = HtmlState {
        blocks: Vec::new(),
        spans: Vec::new(),
        heading: None,
        lists: Vec::new(),
        bold: 0,
        italic: 0,
        code: 0,
        link: None,
    };
    let mut skip: Option<&str> = None;
    let mut rest = html;
    while !rest.is_empty() {
        let Some(open) = rest.find('<') else {
            if skip.is_none() {
                state.text(rest);
            }
            break;
        };
        if skip.is_none() {
            state.text(&rest[..open]);
        }
        rest = &rest[open..];
        if let Some(comment) = rest.strip_prefix("<!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        let Some(close) = rest.find('>') else {
            if skip.is_none() {
                state.text(rest);
            }
            break;
        };
        let token = &rest[..=close];
        rest = &rest[close + 1..];
        let Some((name, closing, attrs)) = tag(token) else {
            continue;
        };
        if let Some(skipped) = skip {
            if closing && name == skipped {
                skip = None;
            }
            continue;
        }
        match (name, closing) {
            ("script" | "style" | "iframe", false) => skip = Some(name),
            ("br", _) => state.spans.push(Span {
                text: "\n".into(),
                style: Style::default(),
                link: None,
            }),
            ("b" | "strong", false) => state.bold += 1,
            ("b" | "strong", true) => state.bold = state.bold.saturating_sub(1),
            ("i" | "em", false) => state.italic += 1,
            ("i" | "em", true) => state.italic = state.italic.saturating_sub(1),
            ("code", false) => state.code += 1,
            ("code", true) => state.code = state.code.saturating_sub(1),
            ("a", false) => {
                state.link = attr(attrs, "href").and_then(|href| safe_web_link(&href, Some(base)))
            }
            ("a", true) => state.link = None,
            ("img", _) => {
                let alt = attr(attrs, "alt").unwrap_or_default();
                if state.link.is_some() {
                    // Badge rows: keep a readable linked label, not a picture.
                    if !alt.trim().is_empty() {
                        state.text(&format!(" {} ", alt.trim()));
                    }
                } else {
                    state.flush();
                    state.blocks.push(Block::Image {
                        alt,
                        url: attr(attrs, "src").and_then(|src| safe_web_link(&src, Some(base))),
                    });
                }
            }
            ("hr", _) => {
                state.flush();
                state.blocks.push(Block::Rule);
            }
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", false) => {
                state.flush();
                state.heading = name[1..].parse().ok();
            }
            ("h1" | "h2" | "h3" | "h4" | "h5" | "h6", true) => {
                state.flush();
                state.heading = None;
            }
            ("ul" | "ol", false) => {
                state.flush();
                if state.lists.len() < MAX_LISTS {
                    state.lists.push(((name == "ol").then_some(1), Vec::new()));
                }
            }
            ("ul" | "ol", true) => {
                state.flush();
                state.close_list();
            }
            // Remaining block-level tags only separate paragraphs.
            _ => state.flush(),
        }
    }
    state.heading = None;
    state.flush();
    while !state.lists.is_empty() {
        state.close_list();
    }
    state.blocks
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
        assert!(matches!(&document[6], Block::Text(spans) if spans[0].text == "Plain HTML"));
        assert!(matches!(&document[7], Block::Text(spans) if spans[0].text == "Final paragraph."));
    }

    #[test]
    fn html_blocks_become_native_blocks_without_markup() {
        let document = parse(
            "<p align=\"center\">\n  <a href=\"https://discord.gg/x\"><img src=\"https://e.io/d.png\" alt=\"Discord\"></a>\n  <a href='javascript:alert(1)'>Bad</a>\n</p>\n<h2>Features &amp; more</h2>\n<ul><li>One <b>bold</b></li><li>Two</li></ul>\n<script>alert(1)</script>\n<img src=\"https://cdn.modrinth.com/data/a.png\" alt=\"Shot\">",
            "https://modrinth.com/mod/example",
        );
        let Block::Text(spans) = &document[0] else {
            panic!("paragraph: {document:?}")
        };
        assert!(spans.iter().any(
            |s| s.text.contains("Discord") && s.link.as_deref() == Some("https://discord.gg/x")
        ));
        assert!(
            spans
                .iter()
                .any(|s| s.text.contains("Bad") && s.link.is_none())
        );
        assert!(
            matches!(&document[1], Block::Heading(2, spans) if spans[0].text == "Features & more")
        );
        assert!(
            matches!(&document[2], Block::List(None, items) if items.len() == 2
            && matches!(&items[0], Block::Item(blocks)
                if matches!(&blocks[0], Block::Text(spans)
                    if spans.iter().any(|s| s.style.bold && s.text == "bold"))))
        );
        assert!(matches!(&document[3], Block::Image { alt, url: Some(url) }
            if alt == "Shot" && url == "https://cdn.modrinth.com/data/a.png"));
        assert_eq!(document.len(), 4);
        assert!(!format!("{document:?}").contains("alert"));
    }

    #[test]
    fn untrusted_html_and_unsafe_urls_are_passive_text() {
        let document = parse(
            "[Unsafe](javascript:alert(1)) <img src=x> ![Local](file:///private.png)",
            "https://modrinth.com/mod/example",
        );
        assert!(
            matches!(&document[0], Block::Text(spans) if spans.iter().all(|s| s.link.is_none())
            && spans.iter().all(|s| !s.text.contains('<')))
        );
        assert!(matches!(&document[1], Block::Image { url: None, alt } if alt == "Local"));
    }
}
