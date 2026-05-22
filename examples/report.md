# Project Progress Report

## Pending tasks (via `input:` attribute — FileLoader, Wasm-compatible)

```mq { exec: true, render: "list", input: "tasks.md" }
.todo | .value
```

## Completed tasks (via `input:` attribute)

```mq { exec: true, render: "numbered_list", input: "tasks.md" }
.done | .value
```

## All tasks (via `load_markdown()` — mq built-in, native only)

```mq { exec: true, render: "list" }
load_markdown("tasks.md") | .list | .value
```
