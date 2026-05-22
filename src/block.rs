use std::sync::OnceLock;

use regex::Regex;

static EXEC_RE: OnceLock<Regex> = OnceLock::new();
static RENDER_RE: OnceLock<Regex> = OnceLock::new();
static INPUT_SINGLE_RE: OnceLock<Regex> = OnceLock::new();
static INPUT_ARRAY_RE: OnceLock<Regex> = OnceLock::new();

fn exec_re() -> &'static Regex {
    EXEC_RE.get_or_init(|| Regex::new(r#"exec\s*:\s*true"#).unwrap())
}

fn render_re() -> &'static Regex {
    RENDER_RE.get_or_init(|| Regex::new(r#"render\s*:\s*"?([a-zA-Z_][a-zA-Z0-9_]*)"?"#).unwrap())
}

fn input_single_re() -> &'static Regex {
    INPUT_SINGLE_RE.get_or_init(|| Regex::new(r#"input\s*:\s*"([^"]+)""#).unwrap())
}

fn input_array_re() -> &'static Regex {
    INPUT_ARRAY_RE.get_or_init(|| Regex::new(r#"input\s*:\s*\[([^\]]+)\]"#).unwrap())
}

/// Attributes extracted from a code block's info string.
///
/// Supported syntax on the fence line:
///
/// ```text
/// ```mq { exec: true, render: "list", input: "tasks.md" }
/// ```mq { exec: true, render: "list", input: ["a.md", "b.md"] }
/// ```mq { exec: true, render: "list" }   <- query uses load_markdown() inside
/// ```
#[derive(Debug, Clone)]
pub struct BlockAttributes {
    pub exec: bool,
    /// Output rendering format: `"list"`, `"ol"`, `"table"`, `"code"`, or `"markdown"` (default).
    pub render: String,
    /// Files to load and concatenate as mq input.
    /// When set the runtime uses `FileLoader` to read them (Wasm-compatible).
    /// When `None` the query is passed as-is; use `load_markdown()` inside the query for native I/O.
    pub input: Option<Vec<String>>,
}

impl BlockAttributes {
    /// Parse attributes from a code fence info string (the text after `mq`).
    /// Returns `None` if `exec: true` is absent.
    pub fn parse(meta: &str) -> Option<Self> {
        if !exec_re().is_match(meta) {
            return None;
        }

        let render = render_re()
            .captures(meta)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| "markdown".to_string());

        // input: "single.md"
        let input = if let Some(caps) = input_single_re().captures(meta) {
            Some(vec![caps[1].to_string()])
        // input: ["a.md", "b.md"]
        } else if let Some(caps) = input_array_re().captures(meta) {
            let paths = caps[1]
                .split(',')
                .filter_map(|s| {
                    let trimmed = s.trim().trim_matches('"');
                    if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
                })
                .collect::<Vec<_>>();
            if paths.is_empty() { None } else { Some(paths) }
        } else {
            None
        };

        Some(Self { exec: true, render, input })
    }
}

/// A code block identified as executable, with its position in the source document.
#[derive(Debug, Clone)]
pub struct ExecBlock {
    /// The mq query string from inside the code block.
    pub query: String,
    /// Parsed attributes from the fence info string.
    pub attributes: BlockAttributes,
    /// 1-based start line of the opening fence in the source.
    pub start_line: usize,
    /// 1-based end line of the closing fence in the source (inclusive).
    pub end_line: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_returns_none_without_exec() {
        assert!(BlockAttributes::parse(r#"{ render: "list" }"#).is_none());
        assert!(BlockAttributes::parse("{}").is_none());
        assert!(BlockAttributes::parse("").is_none());
    }

    #[test]
    fn parse_exec_only() {
        let attrs = BlockAttributes::parse("{ exec: true }").unwrap();
        assert!(attrs.exec);
        assert_eq!(attrs.render, "markdown");
        assert!(attrs.input.is_none());
    }

    #[test]
    fn parse_render_formats() {
        for fmt in ["list", "ol", "table", "code", "markdown"] {
            let meta = format!(r#"{{ exec: true, render: "{fmt}" }}"#);
            let attrs = BlockAttributes::parse(&meta).unwrap();
            assert_eq!(attrs.render, fmt);
        }
    }

    #[test]
    fn parse_input_single_file() {
        let attrs =
            BlockAttributes::parse(r#"{ exec: true, render: "list", input: "tasks.md" }"#)
                .unwrap();
        assert_eq!(attrs.input, Some(vec!["tasks.md".to_string()]));
    }

    #[test]
    fn parse_input_array() {
        let attrs = BlockAttributes::parse(
            r#"{ exec: true, render: "list", input: ["a.md", "b.md"] }"#,
        )
        .unwrap();
        assert_eq!(
            attrs.input,
            Some(vec!["a.md".to_string(), "b.md".to_string()])
        );
    }

    #[test]
    fn parse_exec_false_returns_none() {
        assert!(BlockAttributes::parse("{ exec: false }").is_none());
    }
}
