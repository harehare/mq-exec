<h1 align="center">mq-exec</h1>

**Executable Markdown** — embed [mq](https://mqlang.org) queries inside Markdown code blocks and replace them with live results.

![demo](demo.gif)

```markdown
## Pending tasks

```mq { exec: true, render: "list", input: "tasks.md" }
.todo | .value
```
```

Running `mq-exec process report.md` rewrites the block in-place:

```markdown
## Pending tasks

- Implement user authentication
- Create API endpoints
- Configure deployment
```

---

## Contents

- [Concept](#concept)
- [Installation](#installation)
- [CLI usage](#cli-usage)
- [Code block syntax](#code-block-syntax)
- [Input methods](#input-methods)
- [Render formats](#render-formats)
- [Library API](#library-api)
- [Wasm / custom loaders](#wasm--custom-loaders)
- [Architecture](#architecture)

---

## Concept

Static Markdown is great for documentation, but keeping data sections (task lists, reports, summaries) up to date is manual work.  
`mq-exec` adds a thin **execution layer**: code blocks tagged with `exec: true` are treated as mq queries, evaluated at render time, and their fenced block is replaced with the query output — leaving the rest of the document untouched.

Use cases:

- **Dynamic documentation** — auto-inject filtered data from companion files
- **Living reports** — regenerate progress summaries from task files
- **AI context generation** — produce token-optimised context by selecting only the relevant Markdown nodes

---

## Installation

```sh
# From source (requires Rust ≥ 1.70)
git clone https://github.com/harehare/mq-exec
cd mq-exec
cargo install --path .
```

---

## CLI usage

```
mq-exec <COMMAND>

Commands:
  process   Process a Markdown file once and write the result
  watch     Watch a Markdown file and re-process on changes
  demo      Run the built-in demo
```

### process

```sh
# Print result to stdout
mq-exec process report.md

# Write to a separate file
mq-exec process report.md -o output.md

# Overwrite the source file
mq-exec process report.md --in-place
```

### watch

Watches the input file and its sibling directory.  
Re-processes automatically whenever any `.md` file changes.

```sh
# Stream results to stdout on every change
mq-exec watch report.md

# Write to output file on every change
mq-exec watch report.md -o output.md
```

### demo

Runs an end-to-end demo using in-memory mock files (no filesystem access needed).

```sh
mq-exec demo
```

---

## Code block syntax

Mark a fenced code block with language `mq` and a JSON-like attribute object:

````markdown
```mq { exec: true, render: "list", input: "tasks.md" }
.todo | .value
```
````

| Attribute | Type | Required | Description |
|-----------|------|----------|-------------|
| `exec` | `true` | ✅ | Marks this block as executable |
| `render` | string | — | Output format (default: `"markdown"`) |
| `input` | string \| string[] | — | File(s) to load as mq input |

Blocks without `exec: true` are left untouched, so ordinary code samples are safe.

---

## Input methods

### 1 — `input:` attribute (Wasm-compatible)

The runtime reads the file(s) via the injected `FileLoader` before the query runs.  
This works in any environment, including WebAssembly, because no filesystem call happens inside the engine.

```markdown
```mq { exec: true, render: "list", input: "tasks.md" }
.todo | .value
```
```

Multiple files are concatenated in order:

```markdown
```mq { exec: true, render: "list", input: ["tasks.md", "backlog.md"] }
.todo | .value
```
```

### 2 — `load_markdown()` inside the query (native only)

Uses mq's built-in `load_markdown(path)` function (`read_file | to_markdown`).  
Requires the `file-io` feature and direct filesystem access.

```markdown
```mq { exec: true, render: "list" }
load_markdown("tasks.md") | .list | .value
```
```

---

## Render formats

| `render` value | Output |
|----------------|--------|
| `"markdown"` (default) | Raw mq output, blank-line separated |
| `"list"` | Unordered list: `- value` |
| `"ol"` / `"numbered_list"` | Ordered list: `1. value` |
| `"table"` | Single-column Markdown table |
| `"code"` | Fenced code block |

### Useful mq query patterns

```mq
# Text of unchecked task items
.todo | .value

# Text of checked task items
.done | .value

# All list item text
.list | .value

# First-level headings
.h1 | .value

# All headings
.h | .value
```

---

## Library API

```rust
use mq_exec::{ExecMarkdownRuntime, LocalFileLoader, MockFileLoader};

// Native — reads files from disk
let runtime = ExecMarkdownRuntime::new(LocalFileLoader);
let output = runtime.process(&source)?;

// Mock — inject content in-memory (tests, Wasm)
let mut loader = MockFileLoader::new();
loader.insert("tasks.md", "- [ ] Implement auth\n- [x] Design schema\n");
let runtime = ExecMarkdownRuntime::new(loader);
let output = runtime.process(&source)?;
```

---

## Wasm / custom loaders

The `FileLoader` trait decouples I/O from the engine so the runtime can run in any host environment:

```rust
pub trait FileLoader {
    fn load(&self, path: &str) -> Result<String, LoadError>;
}
```

Implement it to serve content from IndexedDB, a network fetch, a bundled asset, or any other source.

```rust
struct FetchLoader;

impl FileLoader for FetchLoader {
    fn load(&self, path: &str) -> Result<String, LoadError> {
        // e.g. call JS fetch() via wasm-bindgen
        todo!()
    }
}
```

---

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                   ExecMarkdownRuntime<L>                 │
│                                                         │
│  find_exec_blocks()                                     │
│    └─ mq engine: .code                                  │
│         └─ RuntimeValue::Markdown(Node::Code)           │
│              lang="mq", meta has exec:true, position    │
│                                                         │
│  execute()                                              │
│    ├─ resolve_input()                                   │
│    │    ├─ input: ["file"] → FileLoader.load()  ← DI   │
│    │    └─ (none)          → null_input()               │
│    │         └─ query uses load_markdown() internally   │
│    └─ mq engine: eval(query, input)                     │
│                                                         │
│  render_values(format)                                  │
│    └─ list / ol / table / code / markdown               │
│                                                         │
│  apply_line_replacements()                              │
│    └─ replace original fence block by line range        │
└─────────────────────────────────────────────────────────┘
```

### Key files

| File | Role |
|------|------|
| `src/block.rs` | `BlockAttributes` parser, `ExecBlock` struct |
| `src/loader.rs` | `FileLoader` trait, `LocalFileLoader`, `MockFileLoader` |
| `src/renderer.rs` | `RuntimeValue` → Markdown string conversion |
| `src/runtime.rs` | Orchestrates detection → execution → replacement |
| `src/main.rs` | `process` / `watch` / `demo` CLI commands |

---

## License

MIT
