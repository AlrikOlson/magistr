use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};

use super::{
    RustDeclarationChange, RustDeclarationKind, RustMovePlan, RustMoveWrite, RustUnrewritten,
    declaration_item::DeclarationItem,
    declarations::{
        declaration_item_text, declared_modules, find_declaration, insert_declaration,
        remove_declaration, rename_declaration,
    },
    manifest::{manifest_target_paths, rewrite_manifest_paths},
    module_location::ModuleLocation,
    module_tree::ModuleTree,
    paths::{
        base_of, crate_dir_of, dir_of, is_under, join_path, parent_dir_of, relative_to, stem_of,
    },
    rewrite::rewrite_paths,
    rewrite_context::RewriteContext,
};
use crate::query::SourceEntry;

/// Everything a plan reads about one crate, with per-path read caching.
struct Context<'r> {
    crate_dir: String,
    read: &'r dyn Fn(&str) -> Option<String>,
    cache: RefCell<BTreeMap<String, Option<String>>>,
    tree: ModuleTree,
    crate_files: BTreeSet<String>,
    manifest_path: String,
    manifest_content: Option<String>,
    from: &'r str,
    to: &'r str,
    from_rel: String,
    to_rel: Option<String>,
}

impl Context<'_> {
    fn abs(&self, rel: &str) -> String {
        join_path(&self.crate_dir, rel)
    }

    fn read_rel(&self, rel: &str) -> Option<String> {
        if let Some(cached) = self.cache.borrow().get(rel) {
            return cached.clone();
        }
        let content = (self.read)(&self.abs(rel));
        self.cache
            .borrow_mut()
            .insert(rel.to_owned(), content.clone());
        content
    }
}

struct Declaring {
    rel: String,
    content: String,
    item: DeclarationItem,
}

/// A move whose derivation was confirmed: the old module, its declaring
/// parents, and the new module path and parent directory.
struct Located {
    location: ModuleLocation,
    declaring: Vec<Declaring>,
    old_module: Vec<String>,
    new_module: Vec<String>,
    new_parent_dir: String,
}

fn render_module(segments: &[String]) -> String {
    if segments.is_empty() {
        "crate".to_owned()
    } else {
        format!("crate::{}", segments.join("::"))
    }
}

fn plural(n: usize, word: &str) -> String {
    format!("{n} {word}{}", if n == 1 { "" } else { "s" })
}

fn quote_all(paths: &[String]) -> String {
    paths
        .iter()
        .map(|path| format!("{path:?}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Compute every rewrite a Rust-aware move needs; never writes anything.
/// `files` is the workspace listing (junk entries are never scanned) and
/// `read` returns one workspace file's text, or None when unreadable.
pub(crate) fn plan_rust_move(
    from: &str,
    to: &str,
    from_content: &str,
    files: &[SourceEntry],
    read: &dyn Fn(&str) -> Option<String>,
) -> RustMovePlan {
    let all: BTreeSet<String> = files
        .iter()
        .filter(|entry| !entry.is_dir)
        .map(|entry| entry.path.clone())
        .collect();
    let junk: BTreeSet<&str> = files
        .iter()
        .filter(|entry| entry.junk)
        .map(|entry| entry.path.as_str())
        .collect();
    let crate_dir = crate_dir_of(from, &all);
    let manifest_path = join_path(&crate_dir, "Cargo.toml");
    let manifest_content = all
        .contains(&manifest_path)
        .then(|| read(&manifest_path))
        .flatten();
    let manifest_targets: BTreeSet<String> = manifest_content
        .as_deref()
        .map(manifest_target_paths)
        .unwrap_or_default()
        .into_iter()
        .collect();
    let crate_files: BTreeSet<String> = all
        .iter()
        .filter(|path| {
            is_under(path, &crate_dir) && path.ends_with(".rs") && !junk.contains(path.as_str())
        })
        .map(|path| relative_to(path, &crate_dir).to_owned())
        .collect();
    let ctx = Context {
        tree: ModuleTree::build(crate_files.clone(), manifest_targets),
        crate_files,
        from_rel: relative_to(from, &crate_dir).to_owned(),
        to_rel: is_under(to, &crate_dir).then(|| relative_to(to, &crate_dir).to_owned()),
        crate_dir,
        read,
        cache: RefCell::new(BTreeMap::new()),
        manifest_path,
        manifest_content,
        from,
        to,
    };
    let (writes, manifest, trailing_notes) = manifest_step(&ctx);
    match locate(&ctx).and_then(|(location, old_module)| destination(&ctx, location, old_module)) {
        Ok(located) => finish(
            &ctx,
            from_content,
            located,
            writes,
            manifest,
            trailing_notes,
        ),
        Err((reason, old_module)) => RustMovePlan {
            crate_dir: ctx.crate_dir.clone(),
            old_module,
            new_module: None,
            declaration: None,
            moved_content: from_content.to_owned(),
            moved_rewrites: 0,
            writes,
            path_rewrites: Vec::new(),
            manifest,
            unrewritten: Vec::new(),
            notes: std::iter::once(format!(
                "rust: no module rewrite — {reason}; the file was moved as-is"
            ))
            .chain(trailing_notes)
            .collect(),
            skipped: Some(reason),
        },
    }
}

type ManifestStep = (Vec<RustMoveWrite>, Option<(String, usize)>, Vec<String>);

/// Manifest target entries follow the file whatever else happens.
fn manifest_step(ctx: &Context<'_>) -> ManifestStep {
    let (Some(content), Some(to_rel)) = (&ctx.manifest_content, &ctx.to_rel) else {
        return (Vec::new(), None, Vec::new());
    };
    if !ctx.tree.manifest_targets.contains(&ctx.from_rel) {
        return (Vec::new(), None, Vec::new());
    }
    let (next, count) = rewrite_manifest_paths(content, &ctx.from_rel, to_rel);
    if count == 0 {
        return (Vec::new(), None, Vec::new());
    }
    (
        vec![RustMoveWrite {
            path: ctx.manifest_path.clone(),
            before: content.clone(),
            content: next,
        }],
        Some((ctx.manifest_path.clone(), count)),
        vec![format!(
            "{}: {} rewritten to {to_rel:?}",
            ctx.manifest_path,
            plural(count, "target path entry")
        )],
    )
}

type Skipped = (String, Option<Vec<String>>);

/// The file's place in the module tree, refusing roots, style conversions,
/// non-leaf modules and undiscoverable chains with their exact reasons.
fn locate(ctx: &Context<'_>) -> Result<(ModuleLocation, Vec<String>), Skipped> {
    let from = ctx.from;
    let from_rel = &ctx.from_rel;
    let location = ctx.tree.locate(from_rel);
    if let Some(location) = &location
        && location.segments.is_empty()
    {
        let reason = if ctx.tree.manifest_targets.contains(from_rel) {
            format!(
                "{from:?} is a crate root named by a Cargo.toml target path; its module tree is not rewritten"
            )
        } else {
            format!(
                "{from:?} is a crate root discovered by location (src/lib.rs, src/main.rs, src/bin/, tests/, examples/, benches/); moving it changes or removes that Cargo target — pin it with a [lib]/[[bin]]/[[test]]/[[example]]/[[bench]] path entry if that is not intended"
            )
        };
        return Err((reason, Some(Vec::new())));
    }
    let stem = stem_of(from_rel).unwrap_or("");
    let child_dir = if stem == "mod" {
        dir_of(from_rel).to_owned()
    } else {
        join_path(dir_of(from_rel), stem)
    };
    let segments = location.as_ref().map(|location| location.segments.clone());
    let same_module = ctx.to_rel.as_deref().is_some_and(|to_rel| {
        (stem == "mod" && to_rel == format!("{}.rs", dir_of(from_rel)))
            || (stem != "mod" && to_rel == format!("{child_dir}/mod.rs"))
    });
    if same_module {
        return Err((
            format!(
                "module path unchanged ({from_rel:?} ↔ {:?} is a style conversion of the same module); nothing to rewrite",
                ctx.to_rel.as_deref().unwrap_or_default()
            ),
            segments,
        ));
    }
    let children: Vec<String> = ctx
        .crate_files
        .iter()
        .filter(|file| *file != from_rel && is_under(file, &child_dir))
        .map(|file| relative_to(file, &child_dir).to_owned())
        .collect();
    if !children.is_empty() {
        return Err((
            format!(
                "{from:?} has child modules ({}) and directories are not movable in v1, so they cannot follow",
                children.join(", ")
            ),
            segments,
        ));
    }
    let location = location.ok_or_else(|| {
        (
            format!("no module chain from {from:?} to a crate root was found by the file-module conventions (is it declared with #[path], or not declared at all?)"),
            None,
        )
    })?;
    let old_module = location.segments.clone();
    Ok((location, old_module))
}

/// Confirm the derivation (the parent must actually say `mod <name>;`) and
/// derive the new module path under the same root directory by convention.
fn destination(
    ctx: &Context<'_>,
    location: ModuleLocation,
    old_module: Vec<String>,
) -> Result<Located, Skipped> {
    let old_name = old_module.last().cloned().unwrap_or_default();
    let declaring: Vec<Declaring> = location
        .parents
        .iter()
        .filter_map(|parent| {
            let content = ctx.read_rel(parent)?;
            let item = find_declaration(&content, &old_name)?;
            Some(Declaring {
                rel: parent.clone(),
                content,
                item,
            })
        })
        .collect();
    if declaring.is_empty() {
        return Err((
            format!(
                "no `mod {old_name};` declaration found in {}",
                location
                    .parents
                    .iter()
                    .map(|parent| format!("{:?}", ctx.abs(parent)))
                    .collect::<Vec<_>>()
                    .join(" or ")
            ),
            Some(old_module),
        ));
    }
    if let Some(path_declared) = declaring.iter().find(|p| p.item.has_path_attr) {
        return Err((
            format!(
                "`mod {old_name};` in {:?} carries a #[path] attribute; #[path]-declared modules are out of scope (edit the attribute's path by hand)",
                ctx.abs(&path_declared.rel)
            ),
            Some(old_module),
        ));
    }
    let to_stem = ctx.to_rel.as_deref().and_then(stem_of);
    let Some((to_rel, to_stem)) = ctx
        .to_rel
        .as_deref()
        .zip(to_stem)
        .filter(|(to_rel, _)| is_under(to_rel, &location.root_dir))
    else {
        return Err((
            format!(
                "destination {:?} is outside the module tree rooted at {}; the old `mod {old_name};` in {} would dangle",
                ctx.to,
                quote_all(
                    &location
                        .roots
                        .iter()
                        .map(|r| ctx.abs(r))
                        .collect::<Vec<_>>()
                ),
                quote_all(
                    &declaring
                        .iter()
                        .map(|p| ctx.abs(&p.rel))
                        .collect::<Vec<_>>()
                )
            ),
            Some(old_module),
        ));
    };
    if to_stem == "mod" && dir_of(to_rel) == location.root_dir {
        return Err((
            format!(
                "destination {:?} is not a module file position (a mod.rs directly in the root directory)",
                ctx.to
            ),
            Some(old_module),
        ));
    }
    let new_parent_dir = parent_dir_of(to_rel).to_owned();
    let mut new_module: Vec<String> = relative_to(&new_parent_dir, &location.root_dir)
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(str::to_owned)
        .collect();
    new_module.push(if to_stem == "mod" {
        base_of(dir_of(to_rel)).to_owned()
    } else {
        to_stem.to_owned()
    });
    Ok(Located {
        location,
        declaring,
        old_module,
        new_module,
        new_parent_dir,
    })
}

/// Declaration edits: renamed in place, or removed from the old parent and
/// added to the existing new parent (nothing synthesized).
struct DeclarationEdits {
    content_after: BTreeMap<String, String>,
    declared_after: BTreeMap<String, BTreeSet<String>>,
    declaration: RustDeclarationChange,
}

fn declaration_edits(ctx: &Context<'_>, located: &Located) -> DeclarationEdits {
    let Located {
        location,
        declaring,
        old_module,
        new_module,
        new_parent_dir,
    } = located;
    let old_name = old_module.last().cloned().unwrap_or_default();
    let new_name = new_module.last().cloned().unwrap_or_default();
    let same_parent = parent_dir_of(&ctx.from_rel) == new_parent_dir;
    let mut edits = DeclarationEdits {
        content_after: BTreeMap::new(),
        declared_after: BTreeMap::new(),
        declaration: RustDeclarationChange {
            kind: if same_parent {
                RustDeclarationKind::Rename
            } else {
                RustDeclarationKind::Move
            },
            from: Vec::new(),
            to: None,
        },
    };
    if same_parent {
        for p in declaring {
            edits.content_after.insert(
                p.rel.clone(),
                rename_declaration(&p.content, &old_name, &new_name),
            );
            edits.declaration.from.push(ctx.abs(&p.rel));
            let mut declared = declared_modules(&p.content);
            declared.remove(&old_name);
            declared.insert(new_name.clone());
            edits.declared_after.insert(p.rel.clone(), declared);
        }
        return edits;
    }
    let mut item_text: Option<String> = None;
    for p in declaring {
        edits
            .content_after
            .insert(p.rel.clone(), remove_declaration(&p.content, &p.item));
        item_text.get_or_insert_with(|| declaration_item_text(&p.content, &p.item));
        edits.declaration.from.push(ctx.abs(&p.rel));
        let mut declared = declared_modules(&p.content);
        declared.remove(&old_name);
        edits.declared_after.insert(p.rel.clone(), declared);
    }
    // Where the new declaration goes: `<dir>.rs` / `<dir>/mod.rs`, or, at the
    // root directory, the root(s) that already own this module's top segment.
    let new_parent: Option<String> = if *new_parent_dir == location.root_dir {
        location
            .roots
            .iter()
            .find(|root| {
                ctx.read_rel(root)
                    .is_some_and(|content| declared_modules(&content).contains(&old_module[0]))
            })
            .or_else(|| location.roots.first())
            .cloned()
    } else {
        [
            format!("{new_parent_dir}.rs"),
            format!("{new_parent_dir}/mod.rs"),
        ]
        .into_iter()
        .find(|candidate| ctx.crate_files.contains(candidate))
    };
    let base = new_parent.as_ref().and_then(|parent| {
        edits
            .content_after
            .get(parent)
            .cloned()
            .or_else(|| ctx.read_rel(parent))
    });
    match (new_parent, base, item_text) {
        (Some(new_parent), Some(base), Some(item_text)) => {
            edits.content_after.insert(
                new_parent.clone(),
                insert_declaration(&base, &rename_declaration(&item_text, &old_name, &new_name)),
            );
            edits.declaration.to = Some(ctx.abs(&new_parent));
            edits
                .declared_after
                .entry(new_parent)
                .or_insert_with(|| declared_modules(&base))
                .insert(new_name);
        }
        _ => edits.declaration.kind = RustDeclarationKind::RemoveOnly,
    }
    edits
}

fn report_groups(unrewritten: &mut Vec<RustUnrewritten>, path: &str, sites: Vec<(usize, String)>) {
    for (line, text) in sites {
        unrewritten.push(RustUnrewritten {
            path: path.to_owned(),
            line,
            reason: format!("use-tree member names the moved module ({text}); split it by hand"),
        });
    }
}

/// Path rewrites in the moved file (resolve at old, emit at new) and every
/// other file sharing its module tree, then the rendered notes.
fn finish(
    ctx: &Context<'_>,
    from_content: &str,
    located: Located,
    mut writes: Vec<RustMoveWrite>,
    manifest: Option<(String, usize)>,
    trailing_notes: Vec<String>,
) -> RustMovePlan {
    let edits = declaration_edits(ctx, &located);
    let Located {
        location,
        declaring,
        old_module,
        new_module,
        new_parent_dir,
    } = located;
    let mut path_rewrites = Vec::new();
    let mut unrewritten = Vec::new();
    let moved_declared = declared_modules(from_content);
    let (moved_content, moved_rewrites, moved_groups) = rewrite_paths(
        from_content,
        &RewriteContext {
            resolve_module: Some(&old_module),
            emit_module: Some(&new_module),
            declared_before: &moved_declared,
            declared_after: &moved_declared,
            old_module: &old_module,
            new_module: &new_module,
        },
    );
    report_groups(&mut unrewritten, ctx.to, moved_groups);
    for rel in &ctx.crate_files {
        if *rel == ctx.from_rel {
            continue;
        }
        let Some(loc) = ctx.tree.locate(rel) else {
            continue;
        };
        if !loc.roots.iter().any(|root| location.roots.contains(root)) {
            continue;
        }
        let Some(before) = ctx.read_rel(rel) else {
            continue;
        };
        let working = edits
            .content_after
            .get(rel)
            .cloned()
            .unwrap_or_else(|| before.clone());
        let declared_before = declared_modules(&before);
        let declared_after = edits
            .declared_after
            .get(rel)
            .cloned()
            .unwrap_or_else(|| declared_before.clone());
        let (content, count, groups) = rewrite_paths(
            &working,
            &RewriteContext {
                resolve_module: Some(&loc.segments),
                emit_module: Some(&loc.segments),
                declared_before: &declared_before,
                declared_after: &declared_after,
                old_module: &old_module,
                new_module: &new_module,
            },
        );
        report_groups(&mut unrewritten, &ctx.abs(rel), groups);
        if count > 0 {
            path_rewrites.push((ctx.abs(rel), count));
        }
        if content != before {
            writes.push(RustMoveWrite {
                path: ctx.abs(rel),
                before,
                content,
            });
        }
    }
    let notes = render_notes(
        ctx,
        &edits.declaration,
        &old_module,
        &new_module,
        &new_parent_dir,
        &location,
        &path_rewrites,
        moved_rewrites,
        &unrewritten,
        declaring.iter().any(|p| p.item.is_pub),
        trailing_notes,
    );
    RustMovePlan {
        crate_dir: ctx.crate_dir.clone(),
        old_module: Some(old_module),
        new_module: Some(new_module),
        declaration: Some(edits.declaration),
        moved_content,
        moved_rewrites,
        writes,
        path_rewrites,
        manifest,
        unrewritten,
        skipped: None,
        notes,
    }
}

#[allow(clippy::too_many_arguments)] // Every argument is one rendered fact of the plan.
fn render_notes(
    ctx: &Context<'_>,
    declaration: &RustDeclarationChange,
    old_module: &[String],
    new_module: &[String],
    new_parent_dir: &str,
    location: &ModuleLocation,
    path_rewrites: &[(String, usize)],
    moved_rewrites: usize,
    unrewritten: &[RustUnrewritten],
    any_pub: bool,
    trailing_notes: Vec<String>,
) -> Vec<String> {
    let new_name = new_module.last().cloned().unwrap_or_default();
    let total: usize = path_rewrites.iter().map(|(_, count)| count).sum();
    let declaration_note = match declaration.kind {
        RustDeclarationKind::Rename => {
            format!("declaration renamed in {}", quote_all(&declaration.from))
        }
        RustDeclarationKind::Move => format!(
            "declaration moved from {} to {:?}",
            quote_all(&declaration.from),
            declaration.to.as_deref().unwrap_or_default()
        ),
        RustDeclarationKind::RemoveOnly => format!(
            "declaration removed from {} (it would name a missing file)",
            quote_all(&declaration.from)
        ),
    };
    let mut notes = vec![format!(
        "rust: module {} → {}; {declaration_note}; {} rewritten across {}{} (contiguous crate::/super::/self::/uniform paths only; nothing synthesized)",
        render_module(old_module),
        render_module(new_module),
        plural(total, "path"),
        plural(path_rewrites.len(), "file"),
        if moved_rewrites > 0 {
            format!(
                " plus {} re-rooted in the moved file",
                plural(moved_rewrites, "path")
            )
        } else {
            String::new()
        }
    )];
    if declaration.kind == RustDeclarationKind::RemoveOnly {
        let wanted = quote_all(&[
            ctx.abs(&format!("{new_parent_dir}.rs")),
            ctx.abs(&format!("{new_parent_dir}/mod.rs")),
        ]);
        let grand_dir = parent_dir_of(&format!("{new_parent_dir}.rs")).to_owned();
        let grand = if grand_dir == location.root_dir {
            quote_all(
                &location
                    .roots
                    .iter()
                    .map(|r| ctx.abs(r))
                    .collect::<Vec<_>>(),
            )
        } else {
            quote_all(&[
                ctx.abs(&format!("{grand_dir}.rs")),
                ctx.abs(&format!("{grand_dir}/mod.rs")),
            ])
        };
        notes.push(format!(
            "no parent module file for {:?}/ ({wanted}): create one declaring `mod {new_name};` and declare `mod {};` in {grand} yourself — nothing was synthesized",
            ctx.abs(new_parent_dir),
            base_of(new_parent_dir)
        ));
    }
    for site in unrewritten {
        notes.push(format!(
            "not rewritten: {}:{} — {}",
            site.path, site.line, site.reason
        ));
    }
    if any_pub {
        notes.push(format!(
            "paths naming {} from OTHER crates in the workspace (<crate_name>::{}) are not rewritten",
            render_module(old_module),
            old_module.join("::")
        ));
    }
    notes.extend(trailing_notes);
    notes
}

#[cfg(test)]
#[path = "planner.tests.rs"]
mod tests;
