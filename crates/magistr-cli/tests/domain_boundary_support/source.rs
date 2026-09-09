use std::path::{Component, Path};

use syn::parse::{Parse, ParseStream, Parser};
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Expr, Meta, Token};

/// Scan every Rust source, including cfg-disabled modules and separate tests.
pub(super) fn check_tree(root: &Path) -> Vec<String> {
    check_tree_with_async(root, false)
}

/// Application contracts await injected ports without depending on an executor.
pub(super) fn check_tree_with_async(root: &Path, allow_async: bool) -> Vec<String> {
    let mut errors = Vec::new();
    let lib = root.join("src/lib.rs");
    match std::fs::read_to_string(&lib) {
        Ok(text) => errors.extend(check_source_with_async(&text, true, allow_async)),
        Err(error) => errors.push(format!("{}: {error}", lib.display())),
    }
    if root.join("build.rs").exists() {
        errors.push("contract build.rs is an effect escape".into());
    }
    scan_directory(root, &lib, allow_async, &mut errors);
    errors
}

fn scan_directory(directory: &Path, lib: &Path, allow_async: bool, errors: &mut Vec<String>) {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) => {
            errors.push(format!("{}: {error}", directory.display()));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                errors.push(error.to_string());
                continue;
            }
        };
        let path = entry.path();
        let kind = match entry.file_type() {
            Ok(kind) => kind,
            Err(error) => {
                errors.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        if kind.is_symlink() {
            errors.push(format!(
                "{}: contract source symlink escape",
                path.display()
            ));
        } else if kind.is_dir() {
            scan_directory(&path, lib, allow_async, errors);
        } else if path != lib && path.extension().is_some_and(|ext| ext == "rs") {
            match std::fs::read_to_string(&path) {
                Ok(text) => errors.extend(
                    check_source_with_async(&text, false, allow_async)
                        .into_iter()
                        .map(|error| format!("{}: {error}", path.display())),
                ),
                Err(error) => errors.push(format!("{}: {error}", path.display())),
            }
        }
    }
}

pub(super) fn check_source(text: &str, crate_root: bool) -> Vec<String> {
    check_source_with_async(text, crate_root, false)
}

pub(super) fn check_source_with_async(
    text: &str,
    crate_root: bool,
    allow_async: bool,
) -> Vec<String> {
    let file = match syn::parse_file(text) {
        Ok(file) => file,
        Err(error) => return vec![format!("unparsed contract source: {error}")],
    };
    let mut guard = SourceGuard {
        allow_async,
        errors: Vec::new(),
    };
    if crate_root {
        if !file
            .attrs
            .iter()
            .any(|attr| matches!(&attr.meta, Meta::Path(path) if path.is_ident("no_std")))
        {
            guard.reject("contract lib.rs must declare #![no_std]");
        }
        if !file.attrs.iter().any(|attr| {
            attr.path().is_ident("forbid")
                && attr
                    .parse_args::<syn::Path>()
                    .is_ok_and(|path| path.is_ident("unsafe_code"))
        }) {
            guard.reject("contract lib.rs must declare #![forbid(unsafe_code)]");
        }
    }
    guard.visit_file(&file);
    guard.errors
}

struct SourceGuard {
    allow_async: bool,
    errors: Vec<String>,
}

impl SourceGuard {
    fn reject(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }

    fn effect_name(&self, name: &syn::Ident) -> bool {
        name == "std"
            || (self.allow_async
                && matches!(
                    name.to_string().as_str(),
                    "tokio"
                        | "async_std"
                        | "smol"
                        | "reqwest"
                        | "ureq"
                        | "libc"
                        | "magistr_core"
                        | "magistr"
                        | "magistr_engine"
                        | "magistr_store"
                        | "magistr_protocol"
                        | "magistr_mcp"
                        | "rmcp"
                        | "think_and_ship"
                        | "magistr_acp"
                        | "magistr_acp_agent"
                        | "magistr_cockpit"
                        | "magistr_theme"
                        | "magistr_tui"
                        | "magistr_desktop"
                        | "magistr_gui"
                        | "ratatui"
                        | "crossterm"
                        | "rusqlite"
                ))
    }

    fn check_meta(&mut self, meta: &Meta) {
        let path = meta.path();
        let Some(name) = path.get_ident().map(ToString::to_string) else {
            self.reject("unapproved attribute macro");
            return;
        };
        match name.as_str() {
            "doc" => {
                if !matches!(meta, Meta::NameValue(value) if matches!(&value.value, Expr::Lit(lit) if matches!(lit.lit, syn::Lit::Str(_))))
                {
                    self.reject("documentation must be literal text; macro expansion is forbidden");
                }
            }
            "no_std" | "test" | "ignore" | "should_panic" | "inline" | "must_use"
            | "non_exhaustive" | "default" | "allow" | "warn" | "deny" | "forbid" | "cfg" => {}
            "cfg_attr" => {
                if let Meta::List(list) = meta {
                    match list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) {
                        Ok(metas) => {
                            for meta in metas.iter().skip(1) {
                                self.check_meta(meta);
                            }
                        }
                        Err(_) => self.reject("unparsed cfg_attr"),
                    }
                }
            }
            "path" => {
                let safe = matches!(meta, Meta::NameValue(value) if matches!(&value.value,
                    Expr::Lit(lit) if matches!(&lit.lit, syn::Lit::Str(path) if sibling_source(&path.value()))));
                if !safe {
                    self.reject("module #[path] must name a sibling .rs file");
                }
            }
            "derive" => {
                let Meta::List(list) = meta else {
                    self.reject("unparsed derive");
                    return;
                };
                match list.parse_args_with(Punctuated::<syn::Path, Token![,]>::parse_terminated) {
                    Ok(paths) => {
                        for path in paths {
                            let name = path_name(&path);
                            if !matches!(
                                name.as_str(),
                                "Clone"
                                    | "Copy"
                                    | "Debug"
                                    | "Default"
                                    | "Eq"
                                    | "PartialEq"
                                    | "Ord"
                                    | "PartialOrd"
                                    | "Hash"
                                    | "Serialize"
                                    | "Deserialize"
                                    | "serde::Serialize"
                                    | "serde::Deserialize"
                            ) {
                                self.reject(format!("unapproved derive macro {name}"));
                            }
                        }
                    }
                    Err(_) => self.reject("unparsed derive"),
                }
            }
            "serde" => self.check_serde(meta),
            _ => self.reject(format!("unapproved attribute {name}")),
        }
    }

    fn check_serde(&mut self, meta: &Meta) {
        let Meta::List(list) = meta else {
            self.reject("unparsed serde attribute");
            return;
        };
        let Ok(metas) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        else {
            self.reject("unparsed serde attribute");
            return;
        };
        for meta in metas {
            let name = path_name(meta.path());
            match name.as_str() {
                "rename"
                | "rename_all"
                | "alias"
                | "tag"
                | "content"
                | "untagged"
                | "transparent"
                | "deny_unknown_fields"
                | "skip"
                | "skip_serializing"
                | "skip_deserializing"
                | "flatten" => {}
                "default"
                | "skip_serializing_if"
                | "with"
                | "serialize_with"
                | "deserialize_with"
                | "from"
                | "try_from"
                | "into" => {
                    if let Meta::NameValue(value) = meta {
                        if let Expr::Lit(lit) = value.value
                            && let syn::Lit::Str(text) = lit.lit
                            && let Ok(path) = text.parse::<syn::Path>()
                        {
                            self.visit_path(&path);
                        } else {
                            self.reject("serde hook must be a parsed domain path");
                        }
                    }
                }
                _ => self.reject(format!("unapproved serde attribute {name}")),
            }
        }
    }
}

fn sibling_source(value: &str) -> bool {
    let path = Path::new(value);
    path.components().count() == 1
        && matches!(path.components().next(), Some(Component::Normal(_)))
        && path.extension().is_some_and(|ext| ext == "rs")
        && !value.contains('\\')
}

fn path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

impl<'ast> Visit<'ast> for SourceGuard {
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        self.check_meta(&attribute.meta);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        if path
            .segments
            .iter()
            .any(|segment| self.effect_name(&segment.ident))
        {
            self.reject(format!("adapter effect path {}", path_name(path)));
        }
        visit::visit_path(self, path);
    }

    fn visit_use_path(&mut self, path: &'ast syn::UsePath) {
        if self.effect_name(&path.ident) {
            self.reject(format!("adapter effect import {}", path.ident));
        }
        visit::visit_use_path(self, path);
    }

    fn visit_use_name(&mut self, name: &'ast syn::UseName) {
        if self.effect_name(&name.ident) {
            self.reject(format!("adapter effect import {}", name.ident));
        }
    }

    fn visit_use_rename(&mut self, rename: &'ast syn::UseRename) {
        if self.effect_name(&rename.ident) {
            self.reject(format!("aliased adapter effect import {}", rename.ident));
        }
    }

    fn visit_item_extern_crate(&mut self, item: &'ast syn::ItemExternCrate) {
        if item.ident != "alloc" || item.rename.is_some() || !item.attrs.is_empty() {
            self.reject("only unaliased extern crate alloc is permitted");
        }
    }

    fn visit_item_foreign_mod(&mut self, _: &'ast syn::ItemForeignMod) {
        self.reject("foreign effect interface");
    }

    fn visit_abi(&mut self, _: &'ast syn::Abi) {
        self.reject("foreign ABI effect interface");
    }

    fn visit_type_bare_fn(&mut self, function: &'ast syn::TypeBareFn) {
        if function.unsafety.is_some() {
            self.reject("unsafe function pointer");
        }
        visit::visit_type_bare_fn(self, function);
    }

    fn visit_expr_unsafe(&mut self, _: &'ast syn::ExprUnsafe) {
        self.reject("unsafe effect escape");
    }

    fn visit_signature(&mut self, signature: &'ast syn::Signature) {
        if signature.unsafety.is_some() || (!self.allow_async && signature.asyncness.is_some()) {
            self.reject("unsafe or async domain function");
        }
        visit::visit_signature(self, signature);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if item.unsafety.is_some() {
            self.reject("unsafe domain implementation");
        }
        visit::visit_item_impl(self, item);
    }

    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        if item.unsafety.is_some() {
            self.reject("unsafe domain trait");
        }
        visit::visit_item_trait(self, item);
    }

    fn visit_expr_async(&mut self, expression: &'ast syn::ExprAsync) {
        if self.allow_async {
            visit::visit_expr_async(self, expression);
        } else {
            self.reject("async runtime boundary");
        }
    }

    fn visit_expr_await(&mut self, expression: &'ast syn::ExprAwait) {
        if self.allow_async {
            visit::visit_expr_await(self, expression);
        } else {
            self.reject("async runtime boundary");
        }
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        let name = path_name(&mac.path);
        let leaf = name
            .strip_prefix("core::")
            .or_else(|| name.strip_prefix("alloc::"))
            .unwrap_or(&name);
        // Macro token trees are not visited by syn. Parse every allowed macro's
        // arguments explicitly; unknown grammars fail closed, including macro_rules.
        if self.allow_async && matches!(name.as_str(), "core::pin::pin" | "pin") {
            match syn::parse2::<Expr>(mac.tokens.clone()) {
                Ok(expression) => self.visit_expr(&expression),
                Err(_) => self.reject("unparsed pin! argument"),
            }
        } else if leaf == "matches" {
            match syn::parse2::<MatchArguments>(mac.tokens.clone()) {
                Ok(args) => {
                    self.visit_expr(&args.expression);
                    self.visit_pat(&args.pattern);
                    if let Some(guard) = args.guard {
                        self.visit_expr(&guard);
                    }
                }
                Err(_) => self.reject("unparsed matches! arguments"),
            }
        } else if leaf == "vec" {
            match syn::parse_str::<Expr>(&format!("[{}]", mac.tokens)) {
                Ok(expr) => self.visit_expr(&expr),
                Err(_) => self.reject("unparsed vec! arguments"),
            }
        } else if matches!(
            leaf,
            "format"
                | "format_args"
                | "write"
                | "writeln"
                | "assert"
                | "assert_eq"
                | "assert_ne"
                | "debug_assert"
                | "debug_assert_eq"
                | "debug_assert_ne"
                | "panic"
                | "unreachable"
                | "todo"
        ) {
            match Punctuated::<Expr, Token![,]>::parse_terminated.parse2(mac.tokens.clone()) {
                Ok(expressions) => {
                    for expr in expressions {
                        self.visit_expr(&expr);
                    }
                }
                Err(_) => self.reject(format!("unparsed {name}! arguments")),
            }
        } else {
            self.reject(format!("unapproved macro {name}!"));
        }
    }

    fn visit_expr(&mut self, expression: &'ast Expr) {
        if matches!(expression, Expr::Verbatim(_)) {
            self.reject("unparsed expression tokens");
        }
        visit::visit_expr(self, expression);
    }

    fn visit_item(&mut self, item: &'ast syn::Item) {
        if matches!(item, syn::Item::Verbatim(_)) {
            self.reject("unparsed item tokens");
        }
        visit::visit_item(self, item);
    }
}

struct MatchArguments {
    expression: Expr,
    pattern: syn::Pat,
    guard: Option<Expr>,
}

impl Parse for MatchArguments {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let expression = input.parse()?;
        input.parse::<Token![,]>()?;
        let pattern = syn::Pat::parse_multi_with_leading_vert(input)?;
        let guard = if input.peek(Token![if]) {
            input.parse::<Token![if]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
        }
        Ok(Self {
            expression,
            pattern,
            guard,
        })
    }
}
