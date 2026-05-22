use mq_lang::RuntimeValue;

/// Renders a slice of runtime values to a Markdown string.
///
/// `render_format` controls the output shape:
/// - `"list"` — unordered Markdown list (`- item`)
/// - `"ol"` / `"numbered_list"` — ordered list (`1. item`)
/// - `"table"` — single-column Markdown table
/// - `"code"` — fenced code block
/// - anything else (including `"markdown"`) — raw markdown, values separated by blank lines
pub fn render_values(values: &[RuntimeValue], render_format: &str) -> String {
    match render_format {
        "list" => render_list(values, false),
        "ol" | "numbered_list" => render_list(values, true),
        "table" => render_table(values),
        "code" => {
            let lines: Vec<String> = values.iter().flat_map(flatten_value).collect();
            if lines.is_empty() {
                String::new()
            } else {
                format!("```\n{}\n```", lines.join("\n"))
            }
        }
        _ => {
            let parts: Vec<String> = values.iter().flat_map(flatten_value).collect();
            parts.join("\n\n")
        }
    }
}

fn render_list(values: &[RuntimeValue], ordered: bool) -> String {
    values
        .iter()
        .flat_map(flatten_value)
        .enumerate()
        .map(|(i, text)| {
            if ordered {
                format!("{}. {}", i + 1, text)
            } else {
                format!("- {}", text)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_table(values: &[RuntimeValue]) -> String {
    let rows: Vec<String> = values
        .iter()
        .flat_map(flatten_value)
        .filter(|s| !s.is_empty())
        .collect();

    if rows.is_empty() {
        return String::new();
    }

    let mut out = String::from("| Value |\n|-------|\n");
    for row in &rows {
        out.push_str(&format!("| {} |\n", row.replace('|', "\\|")));
    }
    out
}

/// Recursively flatten a `RuntimeValue` into a list of non-empty strings.
fn flatten_value(value: &RuntimeValue) -> Vec<String> {
    match value {
        RuntimeValue::None => vec![],
        RuntimeValue::Array(items) => items.iter().flat_map(flatten_value).collect(),
        other => {
            let s = other.to_string();
            if s.is_empty() { vec![] } else { vec![s] }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn str_values(items: &[&str]) -> Vec<RuntimeValue> {
        items.iter().map(|s| RuntimeValue::String(s.to_string())).collect()
    }

    #[test]
    fn render_list_unordered() {
        let values = str_values(&["a", "b", "c"]);
        assert_eq!(render_values(&values, "list"), "- a\n- b\n- c");
    }

    #[test]
    fn render_list_ordered() {
        let values = str_values(&["x", "y"]);
        assert_eq!(render_values(&values, "ol"), "1. x\n2. y");
        assert_eq!(render_values(&values, "numbered_list"), "1. x\n2. y");
    }

    #[test]
    fn render_table_output() {
        let values = str_values(&["alpha", "beta"]);
        let out = render_values(&values, "table");
        assert!(out.contains("| alpha |"));
        assert!(out.contains("| beta |"));
        assert!(out.starts_with("| Value |"));
    }

    #[test]
    fn render_code_block() {
        let values = str_values(&["fn main() {}", "}"]);
        let out = render_values(&values, "code");
        assert!(out.starts_with("```\n"));
        assert!(out.ends_with("\n```"));
        assert!(out.contains("fn main() {}"));
    }

    #[test]
    fn render_markdown_default() {
        let values = str_values(&["# Hello", "World"]);
        assert_eq!(render_values(&values, "markdown"), "# Hello\n\nWorld");
    }

    #[test]
    fn render_empty_values() {
        assert_eq!(render_values(&[], "list"), "");
        assert_eq!(render_values(&[], "table"), "");
        assert_eq!(render_values(&[], "code"), "");
    }

    #[test]
    fn render_none_values_skipped() {
        let values = vec![
            RuntimeValue::String("keep".to_string()),
            RuntimeValue::None,
            RuntimeValue::String("this".to_string()),
        ];
        assert_eq!(render_values(&values, "list"), "- keep\n- this");
    }

    #[test]
    fn render_nested_array() {
        let values = vec![RuntimeValue::Array(vec![
            RuntimeValue::String("a".to_string()),
            RuntimeValue::String("b".to_string()),
        ])];
        assert_eq!(render_values(&values, "list"), "- a\n- b");
    }

    #[test]
    fn render_table_escapes_pipe() {
        let values = str_values(&["foo|bar"]);
        let out = render_values(&values, "table");
        assert!(out.contains("foo\\|bar"));
    }
}
