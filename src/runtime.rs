use mq_lang::{null_input, parse_markdown_input, DefaultEngine, RuntimeValue};
use mq_markdown::Node;

use crate::{
    block::{BlockAttributes, ExecBlock},
    loader::FileLoader,
    renderer::render_values,
};

#[derive(Debug, thiserror::Error)]
pub enum ExecError {
    #[error("Failed to parse markdown: {0}")]
    ParseError(String),
    #[error("Failed to load '{path}': {source}")]
    LoadError {
        path: String,
        #[source]
        source: crate::loader::LoadError,
    },
    #[error("mq evaluation error: {0}")]
    EvalError(String),
}

/// Executable Markdown runtime.
///
/// Walks a Markdown document using the mq engine (`.code` selector),
/// finds code blocks tagged with `exec: true`, runs their mq queries,
/// and replaces each block with the rendered result.
///
/// # Input resolution (in priority order)
///
/// 1. `input:` attribute on the fence line → loaded via `FileLoader` (Wasm-safe).
/// 2. No `input:` → query runs with `null_input()`; use `load_markdown("file")`
///    inside the query for native filesystem access.
pub struct ExecMarkdownRuntime<L: FileLoader> {
    loader: L,
}

impl<L: FileLoader> ExecMarkdownRuntime<L> {
    pub fn new(loader: L) -> Self {
        Self { loader }
    }

    /// Process a Markdown source string and return the result with exec blocks replaced.
    pub fn process(&self, source: &str) -> Result<String, ExecError> {
        let blocks = self.find_exec_blocks(source)?;
        if blocks.is_empty() {
            return Ok(source.to_string());
        }

        let mut replacements: Vec<(usize, usize, String)> = Vec::new();
        for block in &blocks {
            let values = self.execute(block)?;
            let rendered = render_values(&values, &block.attributes.render);
            replacements.push((block.start_line, block.end_line, rendered));
        }

        Ok(apply_line_replacements(source, replacements))
    }

    /// Use the mq engine (`.code` selector) to find all executable code blocks.
    /// The returned `RuntimeValue::Markdown(Node::Code(...))` preserves position info.
    fn find_exec_blocks(&self, source: &str) -> Result<Vec<ExecBlock>, ExecError> {
        let input =
            parse_markdown_input(source).map_err(|e| ExecError::ParseError(e.to_string()))?;

        let mut engine = DefaultEngine::default();
        engine.load_builtin_module();

        // .code selects every fenced code block node, keeping position info intact.
        let result = engine
            .eval(".code", input.into_iter())
            .map_err(|e| ExecError::EvalError(e.to_string()))?;

        let mut blocks = Vec::new();

        for value in result.into_iter() {
            let RuntimeValue::Markdown(node, _) = value else {
                continue;
            };
            let Node::Code(code) = *node else {
                continue;
            };

            if code.lang.as_deref() != Some("mq") {
                continue;
            }
            let Some(meta) = &code.meta else {
                continue;
            };
            let Some(attrs) = BlockAttributes::parse(meta) else {
                continue;
            };
            let Some(pos) = &code.position else {
                continue;
            };

            blocks.push(ExecBlock {
                query: code.value.clone(),
                attributes: attrs,
                start_line: pos.start.line,
                end_line: pos.end.line,
            });
        }

        Ok(blocks)
    }

    fn execute(&self, block: &ExecBlock) -> Result<Vec<RuntimeValue>, ExecError> {
        let input = self.resolve_input(&block.attributes)?;

        let mut engine = DefaultEngine::default();
        engine.load_builtin_module();

        let result = engine
            .eval(&block.query, input.into_iter())
            .map_err(|e| ExecError::EvalError(e.to_string()))?;

        Ok(result.into_iter().collect())
    }

    /// Resolve mq input values from the `input:` attribute via `FileLoader`.
    /// Falls back to `null_input()` so the query can use `load_markdown()` itself.
    fn resolve_input(&self, attrs: &BlockAttributes) -> Result<Vec<RuntimeValue>, ExecError> {
        match &attrs.input {
            None => Ok(null_input()),
            Some(paths) => {
                let mut values = Vec::new();
                for path in paths {
                    let content = self.loader.load(path).map_err(|e| ExecError::LoadError {
                        path: path.clone(),
                        source: e,
                    })?;
                    let nodes = parse_markdown_input(&content)
                        .map_err(|e| ExecError::ParseError(e.to_string()))?;
                    values.extend(nodes);
                }
                Ok(values)
            }
        }
    }
}

/// Replace line ranges in `source` with new content strings.
/// Applied from the bottom of the file upward so earlier line numbers stay valid.
fn apply_line_replacements(source: &str, mut replacements: Vec<(usize, usize, String)>) -> String {
    if replacements.is_empty() {
        return source.to_string();
    }

    replacements.sort_by_key(|b| std::cmp::Reverse(b.0));

    let mut lines: Vec<String> = source.split('\n').map(str::to_string).collect();

    for (start_line, end_line, replacement) in &replacements {
        let start = start_line.saturating_sub(1);
        let end = (*end_line).min(lines.len());
        let new: Vec<String> = replacement.split('\n').map(str::to_string).collect();
        lines.splice(start..end, new);
    }

    lines.join("\n")
}
