//! `update` and `view` are pure: no file under
//! `src/cockpit` or `src/turn` (their tests aside) may reach a clock, the
//! file system, a process, the environment, a thread, a database or
//! stdout. The runtime owns every effect and hands the core what it needs
//! as messages and the frame clock.

use std::path::{Path, PathBuf};

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs")
            && !path.to_string_lossy().ends_with(".tests.rs")
        {
            out.push(path);
        }
    }
}

#[test]
fn the_core_reaches_no_effect() {
    let files = core_files();
    assert!(files.len() >= 16, "walked only {} files", files.len());
    let forbidden = [
        "Instant::now",
        "SystemTime",
        "std::fs",
        "std::process",
        "std::env",
        "std::thread",
        // Not `std::io` wholesale: `io::Cursor` is an in-memory adapter
        // with no effect behind it, and the image decoders take a reader
        // rather than a slice. What the ban is actually for is the three
        // standard streams — a file reached through `io::Read` is already
        // caught by `std::fs`.
        "io::stdout",
        "io::stdin",
        "io::stderr",
        "std::net",
        "rusqlite",
        "tokio",
        "println!",
        "eprintln!",
        "print!",
        "unsafe",
        "from_os",
        "Command::new",
        "block_on",
    ];
    let mut offenders = Vec::new();
    for file in &files {
        let source = std::fs::read_to_string(file).expect("read source");
        for (idx, line) in source.lines().enumerate() {
            for token in forbidden {
                if line.contains(token) {
                    offenders.push(format!("{}:{}: {token}", file.display(), idx + 1));
                }
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "effects in the core:\n{}",
        offenders.join("\n")
    );
}

/// Semantic effect ports and result values cannot execute runtime work.
#[test]
fn semantic_ports_and_results_own_no_runtime_effects() {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    for module in ["job", "engine", "checkpoint", "workspace"] {
        walk(&src.join(module), &mut files);
    }
    assert!(files.len() >= 6, "walked only {} files", files.len());
    let forbidden = [
        "std::process",
        "Command::new",
        "rusqlite",
        "tokio::",
        "std::fs::write",
        "File::create",
        "OpenOptions",
        "println!",
        "eprintln!",
        "print!",
        "unsafe",
    ];
    let offenders = offenders(&files, &forbidden);
    assert!(
        offenders.is_empty(),
        "effects in the producers:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn the_core_never_names_an_effectful_producer_handle() {
    let files = core_files();
    let forbidden = [
        "EngineClient",
        "NativeEngines",
        "NativeConfig",
        "spawn_feed",
        "spawn_session",
        "AcpTransport",
        "AcpEvent",
        "Jobs",
        "spawn_job",
        "spawn_blocking",
        "Inbox",
        "UnixStream",
        // The loop driver (the root crate's src/orchestration/loop_driver)
        // reports to the core as LoopEvent messages and takes LoopCmd
        // decisions back; none of its machinery is named here.
        "LoopDriver",
        "IterationLoop",
        "WorkflowConfig",
        "Connector",
        "connect_transport",
    ];
    let mut offenders = offenders(&files, &forbidden);
    for file in &files {
        let source = std::fs::read_to_string(file).expect("read source");
        if names_producer_handle(&source) {
            offenders.push(format!("{}: runtime producer handle", file.display()));
        }
    }
    assert!(
        offenders.is_empty(),
        "producer handles in the core:\n{}",
        offenders.join("\n")
    );
}

/// `Commands` also names a visible menu and a Widget variant. Inspect Rust
/// paths, imports and macro identifiers so copy and variant declarations remain
/// legal while every direct or renamed producer import is still rejected.
/// `run_command::RunCommand` is a semantic value; the `run_command` function
/// is an effect, identified by the final path segment rather than module text.
fn names_producer_handle(source: &str) -> bool {
    use syn::visit::Visit;

    #[derive(Default)]
    struct Handles {
        found: bool,
        widget_impl: bool,
    }

    impl Handles {
        fn path(&mut self, names: &[String]) {
            self.found |= names.last().is_some_and(|name| name == "run_command");
            self.found |= names.iter().enumerate().any(|(index, name)| {
                name == "Commands"
                    && !index.checked_sub(1).is_some_and(|previous| {
                        names[previous] == "Widget"
                            || (self.widget_impl && names[previous] == "Self")
                    })
            });
        }

        fn imports(&mut self, tree: &syn::UseTree, prefix: &mut Vec<String>) {
            match tree {
                syn::UseTree::Path(path) => {
                    prefix.push(path.ident.to_string());
                    self.imports(&path.tree, prefix);
                    prefix.pop();
                }
                syn::UseTree::Name(name) => {
                    prefix.push(name.ident.to_string());
                    self.path(prefix);
                    prefix.pop();
                }
                syn::UseTree::Rename(rename) => {
                    prefix.push(rename.ident.to_string());
                    self.path(prefix);
                    prefix.pop();
                }
                syn::UseTree::Group(group) => {
                    for item in &group.items {
                        self.imports(item, prefix);
                    }
                }
                syn::UseTree::Glob(_) => {}
            }
        }

        fn tokens(&mut self, stream: proc_macro2::TokenStream) {
            use proc_macro2::TokenTree;
            let tokens: Vec<_> = stream.into_iter().collect();
            for (index, token) in tokens.iter().enumerate() {
                match token {
                    TokenTree::Group(group) => self.tokens(group.stream()),
                    TokenTree::Ident(ident) if ident == "run_command" => {
                        let module_path = matches!(
                            (tokens.get(index + 1), tokens.get(index + 2)),
                            (Some(TokenTree::Punct(a)), Some(TokenTree::Punct(b)))
                                if a.as_char() == ':' && b.as_char() == ':'
                        );
                        self.found |= !module_path;
                    }
                    TokenTree::Ident(ident) if ident == "Commands" => {
                        let qualifier = index.checked_sub(3).and_then(|start| {
                            match (&tokens[start], &tokens[start + 1], &tokens[start + 2]) {
                                (
                                    TokenTree::Ident(name),
                                    TokenTree::Punct(a),
                                    TokenTree::Punct(b),
                                ) if a.as_char() == ':' && b.as_char() == ':' => Some(name),
                                _ => None,
                            }
                        });
                        if !qualifier.is_some_and(|name| {
                            name == "Widget" || (self.widget_impl && name == "Self")
                        }) {
                            self.found = true;
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    impl<'ast> Visit<'ast> for Handles {
        fn visit_path(&mut self, path: &'ast syn::Path) {
            self.path(
                &path
                    .segments
                    .iter()
                    .map(|segment| segment.ident.to_string())
                    .collect::<Vec<_>>(),
            );
            syn::visit::visit_path(self, path);
        }

        fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
            self.imports(&item.tree, &mut Vec::new());
        }

        fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
            let previous = self.widget_impl;
            self.widget_impl = matches!(item.self_ty.as_ref(), syn::Type::Path(path)
                if path.path.segments.last().is_some_and(|segment| segment.ident == "Widget"));
            syn::visit::visit_item_impl(self, item);
            self.widget_impl = previous;
        }

        fn visit_macro(&mut self, node: &'ast syn::Macro) {
            self.tokens(node.tokens.clone());
            syn::visit::visit_macro(self, node);
        }
    }

    let syntax = syn::parse_file(source).expect("core source must parse for purity verification");
    let mut handles = Handles::default();
    handles.visit_file(&syntax);
    handles.found
}

#[test]
fn command_producer_paths_and_renamed_imports_remain_forbidden() {
    for source in [
        "fn run() { run_command(); }",
        "use driver::run_command as execute; fn run() { execute(); }",
        "fn run() { invoke!(driver::run_command()); }",
        "fn run(commands: Commands) {}",
        "fn run(commands: crate::channel::Commands) {}",
        "type Handle = Option<Commands>;",
        "fn run() { Commands::new(); }",
        "fn run() { crate::channel::Commands::new(); }",
        "use crate::channel::Commands;",
        "use crate::channel::{Commands, Msg};",
        "use crate::channel::{self, Commands as Producer};",
        "use crate::{channel::{Commands as Producer}}; fn run(commands: Producer) {}",
        "use crate::channel as producer; fn run(commands: producer::Commands) {}",
        "struct Other; impl Other { fn run() { Self::Commands(); } }",
        "fn run() { invoke!(Commands::new()); }",
        "fn run() { invoke!({ crate::channel::Commands::new() }); }",
    ] {
        assert!(names_producer_handle(source), "{source}");
    }
}

#[test]
fn semantic_run_command_module_and_value_are_not_effectful_producers() {
    let source = "pub mod run_command; pub use run_command::RunCommand;";
    assert!(!names_producer_handle(source));
}

/// Shared semantic conversation, workflow and session contracts.
fn core_files() -> Vec<PathBuf> {
    let src = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    for module in ["cockpit", "turn"] {
        walk(&src.join(module), &mut files);
    }
    files
}

fn offenders(files: &[PathBuf], forbidden: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for file in files {
        let source = std::fs::read_to_string(file).expect("read source");
        for (idx, line) in source.lines().enumerate() {
            for token in forbidden {
                if line.contains(token) {
                    out.push(format!("{}:{}: {token}", file.display(), idx + 1));
                }
            }
        }
    }
    out
}
