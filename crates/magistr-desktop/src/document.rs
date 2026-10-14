//! CommonMark becomes a bounded, typed semantic tree, never authored HTML.
//! Canonical source stays beside this expendable presentation for exact copy.
use pulldown_cmark::{Event, Options, Parser, Tag};
use serde::Serialize;

const MAX_NODES: usize = 4096;
const MAX_DEPTH: usize = 64;

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Node {
    pub(crate) tag: &'static str,
    pub(crate) text: String,
    pub(crate) children: Vec<Node>,
    pub(crate) destination: Option<String>,
    pub(crate) start: usize,
    pub(crate) number: Option<u64>,
}
impl Node {
    fn element(tag: &'static str, start: usize) -> Self {
        Self {
            tag,
            text: String::new(),
            children: Vec::new(),
            destination: None,
            start,
            number: None,
        }
    }
    fn text(text: impl Into<String>, start: usize) -> Self {
        Self {
            text: text.into(),
            ..Self::element("text", start)
        }
    }
    fn append_text(&mut self, text: &str, start: usize) {
        if let Some(last) = self.children.last_mut()
            && last.tag == "text"
        {
            last.text.push_str(text);
        } else {
            self.children.push(Self::text(text, start));
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub(crate) struct Document {
    pub(crate) nodes: Vec<Node>,
    pub(crate) limited: bool,
}
impl Document {
    pub(crate) fn parse(source: &str) -> Self {
        let mut stack = vec![Node::element("div", 0)];
        let mut count = 0;
        for (event, range) in
            Parser::new_ext(source, Options::ENABLE_STRIKETHROUGH).into_offset_iter()
        {
            count += 1;
            if count > MAX_NODES || stack.len() > MAX_DEPTH {
                return Self {
                    nodes: vec![Node {
                        text: source.into(),
                        ..Node::element("pre", 0)
                    }],
                    limited: true,
                };
            }
            match event {
                Event::Start(tag) => {
                    let mut node = Node::element(
                        match &tag {
                            Tag::Paragraph => "p",
                            Tag::Heading { level, .. } => match *level as u8 {
                                1 => "h1",
                                2 => "h2",
                                3 => "h3",
                                4 => "h4",
                                5 => "h5",
                                _ => "h6",
                            },
                            Tag::BlockQuote(_) => "blockquote",
                            Tag::CodeBlock(_) => "pre",
                            Tag::List(Some(_)) => "ol",
                            Tag::List(None) => "ul",
                            Tag::Item => "li",
                            Tag::Emphasis => "em",
                            Tag::Strong => "strong",
                            Tag::Strikethrough => "s",
                            Tag::Link { .. } => "a",
                            _ => "span",
                        },
                        range.start,
                    );
                    if let Tag::List(start) = &tag {
                        node.number = *start;
                    }
                    if let Tag::Link { dest_url, .. } = tag {
                        node.destination =
                            crate::links::web_address(&dest_url).map(|url| url.into());
                        if node.destination.is_none() {
                            node.tag = "span";
                        }
                    }
                    stack.push(node);
                }
                Event::End(_) => {
                    if let Some(node) = stack.pop()
                        && let Some(parent) = stack.last_mut()
                    {
                        parent.children.push(node);
                    }
                }
                Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                    if let Some(parent) = stack.last_mut() {
                        parent.append_text(&text, range.start);
                    }
                }
                Event::Code(text) => {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(Node {
                            text: text.into_string(),
                            ..Node::element("code", range.start)
                        });
                    }
                }
                Event::SoftBreak => {
                    if let Some(parent) = stack.last_mut() {
                        parent.append_text("\n", range.start);
                    }
                }
                Event::HardBreak | Event::Rule => {
                    let tag = if matches!(event, Event::Rule) {
                        "hr"
                    } else {
                        "br"
                    };
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(Node::element(tag, range.start));
                    }
                }
                _ => {}
            }
        }
        Self {
            nodes: stack.pop().map_or_else(Vec::new, |root| root.children),
            limited: false,
        }
    }
}

#[cfg(test)]
#[path = "document.tests.rs"]
mod tests;
