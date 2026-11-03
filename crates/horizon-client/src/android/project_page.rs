//! A project's page in the catalog: its description (Markdown, often with HTML in it) as plain
//! lines the launcher can draw, wrapped to a width. No JNI: the host content tests cover it.

/// How a line of the description is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Heading,
    Text,
    /// A list item's lines, indented.
    Item,
    /// A gap between paragraphs.
    Gap,
}

/// The description as paragraphs: each a style and its text, with links' text kept, images,
/// HTML tags and emphasis marks dropped.
pub fn paragraphs(markdown: &str) -> Vec<(Style, String)> {
    let text = strip_html(markdown);
    let mut out: Vec<(Style, String)> = Vec::new();
    let mut paragraph = String::new();
    let mut fenced = false;
    let flush = |out: &mut Vec<(Style, String)>, paragraph: &mut String| {
        let p = paragraph.trim();
        if !p.is_empty() {
            out.push((Style::Text, p.to_owned()));
        }
        paragraph.clear();
    };
    for raw in text.lines() {
        let line = raw.trim();
        if line.starts_with("```") || line.starts_with("~~~") {
            flush(&mut out, &mut paragraph);
            fenced = !fenced;
            continue;
        }
        if fenced {
            if !line.is_empty() {
                out.push((Style::Text, line.to_owned()));
            }
            continue;
        }
        if line.is_empty() {
            flush(&mut out, &mut paragraph);
            if out.last().is_some_and(|(s, _)| *s != Style::Gap) {
                out.push((Style::Gap, String::new()));
            }
            continue;
        }
        // Table rules and horizontal rules.
        if line
            .chars()
            .all(|c| matches!(c, '-' | '|' | ':' | ' ' | '*' | '_' | '='))
        {
            flush(&mut out, &mut paragraph);
            continue;
        }
        if let Some(heading) = heading(line) {
            flush(&mut out, &mut paragraph);
            let heading = inline(heading);
            if !heading.is_empty() {
                out.push((Style::Heading, heading));
            }
            continue;
        }
        if let Some(item) = list_item(line) {
            flush(&mut out, &mut paragraph);
            let item = inline(item);
            if !item.is_empty() {
                out.push((Style::Item, item));
            }
            continue;
        }
        let line = line.trim_start_matches('>').trim();
        let line = inline(&line.replace('|', "  "));
        if !line.is_empty() {
            if !paragraph.is_empty() {
                paragraph.push(' ');
            }
            paragraph.push_str(&line);
        }
    }
    flush(&mut out, &mut paragraph);
    while out.last().is_some_and(|(s, _)| *s == Style::Gap) {
        out.pop();
    }
    while out.first().is_some_and(|(s, _)| *s == Style::Gap) {
        out.remove(0);
    }
    out
}

fn heading(line: &str) -> Option<&str> {
    let rest = line.trim_start_matches('#');
    let level = line.len() - rest.len();
    ((1..=6).contains(&level) && (rest.is_empty() || rest.starts_with(' ')))
        .then(|| rest.trim().trim_end_matches('#').trim())
}

fn list_item(line: &str) -> Option<&str> {
    for bullet in ["- ", "* ", "+ "] {
        if let Some(rest) = line.strip_prefix(bullet) {
            return Some(rest.trim_start_matches("[ ] ").trim_start_matches("[x] "));
        }
    }
    let digits = line.bytes().take_while(u8::is_ascii_digit).count();
    (digits > 0 && digits < 4)
        .then(|| &line[digits..])
        .and_then(|rest| rest.strip_prefix(". ").or_else(|| rest.strip_prefix(") ")))
}

/// HTML tags and comments removed (a `<br>` or a block's end becomes a new line), and the
/// common entities decoded.
fn strip_html(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('<') {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        if let Some(comment) = after.strip_prefix("<!--") {
            rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
            continue;
        }
        let Some(end) = after.find('>') else {
            out.push_str(after);
            rest = "";
            break;
        };
        let tag = after[1..end].trim_start_matches('/').to_ascii_lowercase();
        let name: String = tag
            .chars()
            .take_while(char::is_ascii_alphanumeric)
            .collect();
        if name.is_empty() {
            // Not a tag: "a < b".
            out.push('<');
            rest = &after[1..];
            continue;
        }
        if matches!(
            name.as_str(),
            "p" | "div" | "li" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "tr" | "center"
        ) {
            out.push('\n');
        } else if name == "br" {
            // A line break of its own: a new paragraph.
            out.push_str("\n\n");
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

/// One line's inline Markdown: images dropped, links as their text, emphasis and code marks
/// removed.
fn inline(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while !rest.is_empty() {
        let image = rest.starts_with("![");
        if (image || rest.starts_with('['))
            && let Some(close) = rest.find("](")
            && let Some(end) = rest[close..].find(')')
        {
            if !image {
                out.push_str(&rest[1..close]);
            }
            rest = &rest[close + end + 1..];
            continue;
        }
        let c = rest.chars().next().unwrap_or_default();
        rest = &rest[c.len_utf8()..];
        out.push(c);
    }
    let out = out
        .replace("__", "")
        .replace("~~", "")
        .replace(['*', '`'], "");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` in lines no wider than `width` by `measure`, broken between words (and inside a word
/// wider than a line).
pub fn wrap(text: &str, width: f32, measure: impl Fn(&str) -> f32) -> Vec<String> {
    let mut lines = Vec::new();
    let mut line = String::new();
    for word in text.split_whitespace() {
        let candidate = if line.is_empty() {
            word.to_owned()
        } else {
            format!("{line} {word}")
        };
        if measure(&candidate) <= width {
            line = candidate;
            continue;
        }
        if !line.is_empty() {
            lines.push(std::mem::take(&mut line));
        }
        if measure(word) <= width {
            line = word.to_owned();
            continue;
        }
        // A word wider than the line: as many characters as fit, then the rest.
        for c in word.chars() {
            line.push(c);
            if measure(&line) > width && line.chars().count() > 1 {
                line.pop();
                lines.push(std::mem::take(&mut line));
                line.push(c);
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_and_html_become_plain_paragraphs() {
        let page = "<center><img src=\"https://x/banner.png\"></center>\n\n\
            # Fast **Render**\n\n\
            A [quick](https://example.com) renderer,\nwith `options`.<br>Second line &amp; more.\n\n\
            ![screenshot](https://x/s.png)\n\n\
            - Fewer *draw* calls\n2. Smooth __chunks__\n\n---\n\n\
            ```\ncode stays\n```\n<!-- hidden -->";
        let gap = (Style::Gap, String::new());
        assert_eq!(
            paragraphs(page),
            vec![
                (Style::Heading, "Fast Render".into()),
                gap.clone(),
                (Style::Text, "A quick renderer, with options.".into()),
                gap.clone(),
                (Style::Text, "Second line & more.".into()),
                gap.clone(),
                (Style::Item, "Fewer draw calls".into()),
                (Style::Item, "Smooth chunks".into()),
                gap,
                (Style::Text, "code stays".into()),
            ]
        );
    }

    #[test]
    fn lines_break_between_words_and_inside_long_ones() {
        // Each character 1 wide.
        let measure = |s: &str| s.chars().count() as f32;
        assert_eq!(
            wrap("one two three four", 9.0, measure),
            ["one two", "three", "four"]
        );
        assert_eq!(
            wrap("abcdefghij xy", 4.0, measure),
            ["abcd", "efgh", "ij", "xy"]
        );
        assert!(wrap("   ", 5.0, measure).is_empty());
    }
}
