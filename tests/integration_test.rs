use mq_exec::{ExecMarkdownRuntime, MockFileLoader};

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

fn runtime_with_tasks() -> ExecMarkdownRuntime<MockFileLoader> {
    let mut loader = MockFileLoader::new();
    loader.insert(
        "tasks.md",
        "# タスク\n\n- [ ] タスクA\n- [x] タスクB\n- [ ] タスクC\n",
    );
    loader.insert(
        "extra.md",
        "# 追加\n\n- [ ] タスクD\n",
    );
    ExecMarkdownRuntime::new(loader)
}

// ---------------------------------------------------------------------------
// no exec blocks → document returned unchanged
// ---------------------------------------------------------------------------

#[test]
fn passthrough_when_no_exec_blocks() {
    let source = "# Hello\n\nJust a paragraph.\n\n```rust\nfn main() {}\n```\n";
    let result = runtime_with_tasks().process(source).unwrap();
    assert_eq!(result, source);
}

#[test]
fn passthrough_mq_block_without_exec() {
    let source = "```mq { render: \"list\" }\n.todo\n```\n";
    let result = runtime_with_tasks().process(source).unwrap();
    assert_eq!(result, source);
}

// ---------------------------------------------------------------------------
// input: attribute — FileLoader path
// ---------------------------------------------------------------------------

#[test]
fn todo_items_rendered_as_list() {
    let source = indoc(
        r#"
        # Report

        ```mq { exec: true, render: "list", input: "tasks.md" }
        .todo | .value
        ```
        "#,
    );
    let result = runtime_with_tasks().process(&source).unwrap();

    assert!(result.contains("- タスクA"));
    assert!(result.contains("- タスクC"));
    assert!(!result.contains("タスクB"), "completed task must not appear");
    assert!(!result.contains("```mq"), "fence must be replaced");
}

#[test]
fn done_items_rendered_as_numbered_list() {
    let source = indoc(
        r#"
        # Report

        ```mq { exec: true, render: "ol", input: "tasks.md" }
        .done | .value
        ```
        "#,
    );
    let result = runtime_with_tasks().process(&source).unwrap();

    assert!(result.contains("1. タスクB"));
    assert!(!result.contains("タスクA"));
}

#[test]
fn multiple_input_files_concatenated() {
    let source = indoc(
        r#"
        # All TODOs

        ```mq { exec: true, render: "list", input: ["tasks.md", "extra.md"] }
        .todo | .value
        ```
        "#,
    );
    let result = runtime_with_tasks().process(&source).unwrap();

    assert!(result.contains("- タスクA"));
    assert!(result.contains("- タスクC"));
    assert!(result.contains("- タスクD"));
}

// ---------------------------------------------------------------------------
// multiple blocks in one document
// ---------------------------------------------------------------------------

#[test]
fn multiple_blocks_replaced_independently() {
    let source = indoc(
        r#"
        # Report

        ## TODO

        ```mq { exec: true, render: "list", input: "tasks.md" }
        .todo | .value
        ```

        ## Done

        ```mq { exec: true, render: "ol", input: "tasks.md" }
        .done | .value
        ```
        "#,
    );
    let result = runtime_with_tasks().process(&source).unwrap();

    // Both sections present and correct
    assert!(result.contains("## TODO"));
    assert!(result.contains("- タスクA"));
    assert!(result.contains("## Done"));
    assert!(result.contains("1. タスクB"));

    // No fence markers remain
    assert!(!result.contains("```mq"));
}

// ---------------------------------------------------------------------------
// render formats
// ---------------------------------------------------------------------------

#[test]
fn render_table_format() {
    let source = indoc(
        r#"
        # Table

        ```mq { exec: true, render: "table", input: "tasks.md" }
        .todo | .value
        ```
        "#,
    );
    let result = runtime_with_tasks().process(&source).unwrap();

    assert!(result.contains("| Value |"));
    assert!(result.contains("|-------|"));
    assert!(result.contains("| タスクA |"));
}

#[test]
fn render_code_format() {
    let source = indoc(
        r#"
        # Code

        ```mq { exec: true, render: "code", input: "tasks.md" }
        .todo | .value
        ```
        "#,
    );
    let result = runtime_with_tasks().process(&source).unwrap();

    // Output is wrapped in a generic fenced block (no lang tag)
    assert!(result.contains("```\n"));
    assert!(result.contains("タスクA"));
}

// ---------------------------------------------------------------------------
// error: file not found
// ---------------------------------------------------------------------------

#[test]
fn missing_input_file_returns_error() {
    let source = indoc(
        r#"
        ```mq { exec: true, render: "list", input: "missing.md" }
        .todo | .value
        ```
        "#,
    );
    let err = runtime_with_tasks().process(&source).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("missing.md"), "error should name the file: {msg}");
}

// ---------------------------------------------------------------------------
// surrounding content is preserved
// ---------------------------------------------------------------------------

#[test]
fn prose_around_block_is_preserved() {
    let source = indoc(
        r#"
        # Heading

        Paragraph before.

        ```mq { exec: true, render: "list", input: "tasks.md" }
        .todo | .value
        ```

        Paragraph after.
        "#,
    );
    let result = runtime_with_tasks().process(&source).unwrap();

    assert!(result.contains("# Heading"));
    assert!(result.contains("Paragraph before."));
    assert!(result.contains("Paragraph after."));
    assert!(result.contains("- タスクA"));
}

// ---------------------------------------------------------------------------
// utility
// ---------------------------------------------------------------------------

/// Strip the leading newline and the common leading whitespace from each line.
/// Lets test strings be indented naturally in source without affecting the content.
fn indoc(s: &str) -> String {
    let s = s.trim_start_matches('\n');
    let indent = s
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    s.lines()
        .map(|l| if l.len() >= indent { &l[indent..] } else { l.trim_start() })
        .collect::<Vec<_>>()
        .join("\n")
}
