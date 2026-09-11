/// The honest cut of a raw stream: head and tail lines with the count of
/// lines between them, so a manifest can name what was dropped.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(super) struct HeadTail {
    pub(super) head: Vec<String>,
    pub(super) tail: Vec<String>,
    pub(super) dropped: usize,
}

pub(super) fn head_tail(text: &str, head_lines: usize, tail_lines: usize) -> HeadTail {
    let mut lines: Vec<String> = text
        .replace("\r\n", "\n")
        .split('\n')
        .map(str::to_owned)
        .collect();
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    if lines.len() <= head_lines + tail_lines {
        return HeadTail {
            head: lines,
            tail: Vec::new(),
            dropped: 0,
        };
    }
    let dropped = lines.len() - head_lines - tail_lines;
    let tail = lines.split_off(lines.len() - tail_lines);
    lines.truncate(head_lines);
    HeadTail {
        head: lines,
        tail,
        dropped,
    }
}
