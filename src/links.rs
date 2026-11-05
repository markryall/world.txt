//! Links in what people write. Plain https links, and markdown-style ones,
//! `[the notes](https://…)`, are the only formatting there is: no bold, no
//! headings, nothing else. The browser draws them as links; the terminal
//! shows them as "the notes (https://…)".

/// The longest label a link can have.
const MAX_LABEL: usize = 100;

/// A piece of text: words, or a link with what it says.
#[derive(Debug, PartialEq)]
pub enum Piece<'a> {
    Text(&'a str),
    Link { label: &'a str, url: &'a str },
}

/// Split out markdown-style links: `[label](https://…)`. Anything that isn't
/// quite one (no https, a newline in the label, an unclosed bracket) stays
/// as it was.
pub fn pieces(text: &str) -> Vec<Piece<'_>> {
    let mut out = Vec::new();
    let mut rest = text;
    let mut start = 0; // where the current run of plain text began, in `text`
    let mut at = 0; // how far through `text` we've looked
    while let Some(open) = rest.find('[') {
        let from = at + open;
        if let Some((label, url, len)) = link_at(&text[from..]) {
            if from > start {
                out.push(Piece::Text(&text[start..from]));
            }
            out.push(Piece::Link { label, url });
            start = from + len;
            at = start;
        } else {
            at = from + 1;
        }
        rest = &text[at..];
    }
    if start < text.len() {
        out.push(Piece::Text(&text[start..]));
    }
    out
}

/// `[label](https://url)` at the start of `s`: (label, url, its length).
fn link_at(s: &str) -> Option<(&str, &str, usize)> {
    let close = s.find("](")?;
    let label = &s[1..close];
    if label.is_empty() || label.contains(['[', ']', '\n']) || label.chars().count() > MAX_LABEL {
        return None;
    }
    let after = &s[close + 2..];
    let end = after.find(')')?;
    let url = &after[..end];
    let ok = url.starts_with("https://") && url.len() > "https://".len() && !url.chars().any(|c| c.is_whitespace() || matches!(c, '<' | '>' | '"' | '\''));
    ok.then_some((label.trim(), url, close + 2 + end + 1))
}

/// For the terminal: "the notes (https://…)".
pub fn plain(text: &str) -> String {
    pieces(text)
        .into_iter()
        .map(|p| match p {
            Piece::Text(t) => t.to_string(),
            Piece::Link { label, url } => format!("{label} ({url})"),
        })
        .collect()
}

/// Just what the links say, for somewhere small (a side panel): "the notes".
pub fn labels(text: &str) -> String {
    pieces(text)
        .into_iter()
        .map(|p| match p {
            Piece::Text(t) => t,
            Piece::Link { label, .. } => label,
        })
        .collect()
}

/// Slack writes links as `<https://…|the notes>` (or just `<https://…>`):
/// turn them into the markdown kind, so a hook that thinks it's talking to
/// Slack still comes out as a proper link.
pub fn from_slack(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(open) = rest.find('<') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('>') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let inner = &after[..close];
        let (url, label) = inner.split_once('|').unwrap_or((inner, ""));
        if url.starts_with("https://") || url.starts_with("http://") {
            let label = label.trim();
            match (label.is_empty(), url.starts_with("https://")) {
                (true, _) => out.push_str(url),
                (false, true) => out.push_str(&format!("[{label}]({url})")),
                (false, false) => out.push_str(&format!("{label} ({url})")),
            }
        } else if let Some(name) = inner.strip_prefix('@').or_else(|| inner.strip_prefix('#')).and_then(|i| i.split_once('|')).map(|(_, n)| n) {
            // <@U123|poppy>, <#C123|general>: just the name.
            out.push_str(name);
        } else {
            out.push('<');
            out.push_str(inner);
            out.push('>');
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_markdown_links() {
        assert_eq!(
            pieces("See [the notes](https://example.com/notes) and [cake](https://example.com/cake)!"),
            vec![
                Piece::Text("See "),
                Piece::Link { label: "the notes", url: "https://example.com/notes" },
                Piece::Text(" and "),
                Piece::Link { label: "cake", url: "https://example.com/cake" },
                Piece::Text("!"),
            ]
        );
        assert_eq!(pieces("just words"), vec![Piece::Text("just words")]);
        assert_eq!(pieces("[not a link](http://example.com)"), vec![Piece::Text("[not a link](http://example.com)")], "https only");
        assert_eq!(pieces("[a] b [c](https://x.y)"), vec![Piece::Text("[a] b "), Piece::Link { label: "c", url: "https://x.y" }]);
        assert_eq!(pieces("[unclosed](https://x.y"), vec![Piece::Text("[unclosed](https://x.y")]);
        assert_eq!(pieces("[](https://x.y)"), vec![Piece::Text("[](https://x.y)")]);
        assert_eq!(pieces("**bold** stays as it is"), vec![Piece::Text("**bold** stays as it is")]);
    }

    #[test]
    fn terminal_shows_where_it_goes() {
        assert_eq!(plain("See [the notes](https://example.com/n) now"), "See the notes (https://example.com/n) now");
        assert_eq!(plain("https://example.com"), "https://example.com");
        assert_eq!(labels("[build 42](https://ci.example.com/42) failed"), "build 42 failed");
    }

    #[test]
    fn slack_links_become_markdown() {
        assert_eq!(from_slack("Build <https://ci.example.com/42|#42> passed"), "Build [#42](https://ci.example.com/42) passed");
        assert_eq!(from_slack("See <https://example.com>"), "See https://example.com");
        assert_eq!(from_slack("Old <http://example.com|site>"), "Old site (http://example.com)");
        assert_eq!(from_slack("Hi <@U123|poppy> in <#C9|general>"), "Hi poppy in general");
        assert_eq!(from_slack("1 < 2 and 3 > 2"), "1 < 2 and 3 > 2");
        assert_eq!(from_slack("a <b"), "a <b");
    }
}
