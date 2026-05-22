use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::{Parser, Subcommand};
use miette::miette;
use mq_exec::{ExecMarkdownRuntime, LocalFileLoader, MockFileLoader};

#[derive(Parser)]
#[command(name = "mq-exec")]
#[command(version)]
#[command(about = "Executable Markdown: run embedded mq queries and replace code blocks with results")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Process a Markdown file once and write the result.
    Process {
        /// Input Markdown file
        input: PathBuf,
        /// Write output to this file instead of stdout
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Overwrite the input file with the result
        #[arg(long, conflicts_with = "output")]
        in_place: bool,
    },
    /// Watch a Markdown file and re-process whenever it (or related files) change.
    Watch {
        /// Input Markdown file
        input: PathBuf,
        /// Write output to this file (defaults to stdout)
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    /// Run a built-in demo showing both usage styles.
    Demo,
}

fn main() -> miette::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Process { input, output, in_place } => {
            let result = process_path(&input)?;

            if in_place {
                std::fs::write(&input, &result)
                    .map_err(|e| miette!("Cannot write {}: {}", input.display(), e))?;
                eprintln!("Updated {} in-place.", input.display());
            } else if let Some(out) = output {
                std::fs::write(&out, &result)
                    .map_err(|e| miette!("Cannot write {}: {}", out.display(), e))?;
                eprintln!("{} → {}", input.display(), out.display());
            } else {
                print!("{}", result);
            }
        }

        Commands::Watch { input, output } => {
            watch(&input, output.as_deref())?;
        }

        Commands::Demo => {
            demo()?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// process
// ---------------------------------------------------------------------------

fn process_path(input: &Path) -> miette::Result<String> {
    // Canonicalize so the path stays valid after we change CWD.
    let input = input
        .canonicalize()
        .map_err(|e| miette!("Cannot resolve {}: {}", input.display(), e))?;

    let source = std::fs::read_to_string(&input)
        .map_err(|e| miette!("Cannot read {}: {}", input.display(), e))?;

    let base_dir = input.parent().unwrap_or(Path::new("."));

    // Change CWD so that both LocalFileLoader (input: attribute) and
    // mq's built-in load_markdown() resolve relative paths against the
    // directory that contains the Markdown file.
    let prev_dir =
        std::env::current_dir().map_err(|e| miette!("Cannot get cwd: {}", e))?;
    std::env::set_current_dir(base_dir)
        .map_err(|e| miette!("Cannot cd to {}: {}", base_dir.display(), e))?;

    let result = ExecMarkdownRuntime::new(LocalFileLoader::new("."))
        .process(&source)
        .map_err(|e| miette!("{}", e));

    std::env::set_current_dir(&prev_dir).ok();
    result
}

// ---------------------------------------------------------------------------
// watch
// ---------------------------------------------------------------------------

fn watch(input: &Path, output: Option<&Path>) -> miette::Result<()> {
    use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;

    eprintln!("Watching {} — Ctrl-C to stop.", input.display());
    emit(input, output)?;

    let (tx, rx) = mpsc::channel();
    let mut watcher = RecommendedWatcher::new(tx, Config::default())
        .map_err(|e| miette!("Watcher init error: {}", e))?;

    watcher
        .watch(input, RecursiveMode::NonRecursive)
        .map_err(|e| miette!("Cannot watch {}: {}", input.display(), e))?;

    // Also watch the directory so changes to loaded sibling files are picked up.
    if let Some(dir) = input.parent() {
        let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
    }

    loop {
        match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(event)) => {
                use notify::EventKind;
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    eprintln!("Change detected — reprocessing…");
                    if let Err(e) = emit(input, output) {
                        eprintln!("Error: {e}");
                    }
                }
            }
            Ok(Err(e)) => eprintln!("Watch error: {e}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}

fn emit(input: &Path, output: Option<&Path>) -> miette::Result<()> {
    let result = process_path(input)?;
    match output {
        Some(out) => {
            std::fs::write(out, &result)
                .map_err(|e| miette!("Cannot write {}: {}", out.display(), e))?;
            eprintln!("→ {}", out.display());
        }
        None => print!("{}", result),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// demo
// ---------------------------------------------------------------------------

fn demo() -> miette::Result<()> {
    let mut loader = MockFileLoader::new();

    // Virtual file contents served by MockFileLoader (no filesystem needed).
    loader.insert(
        "tasks.md",
        "\
# Tasks

- [ ] Implement user authentication
- [x] Design database schema
- [ ] Create API endpoints
- [x] Write tests
- [ ] Configure deployment
",
    );
    loader.insert(
        "backlog.md",
        "\
# Backlog

- [ ] Performance improvements
- [ ] Documentation
",
    );

    // Style A: input: attribute  — FileLoader (Wasm-compatible)
    // Style B: load_markdown()   — mq built-in (native only)
    let source = r#"# Project Progress Report

## Pending tasks (input attribute — via FileLoader)

```mq { exec: true, render: "list", input: "tasks.md" }
.todo | .value
```

## Completed tasks (input attribute)

```mq { exec: true, render: "numbered_list", input: "tasks.md" }
.done | .value
```

## Multiple files merged (input array)

```mq { exec: true, render: "list", input: ["tasks.md", "backlog.md"] }
.todo | .value
```
"#;

    println!("=== Input Markdown ===\n\n{source}");

    let result = ExecMarkdownRuntime::new(loader)
        .process(source)
        .map_err(|e| miette!("{e}"))?;

    println!("=== Output Markdown ===\n\n{result}");

    Ok(())
}
