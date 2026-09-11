//! The WIT compatibility gate (SDD §8.2, chunk:m4-wit-interfaces).
//!
//! Parses the five public WIT packages under `wit/` with the real component-
//! model parser, renders their whole vocabulary — interfaces, named types
//! (fields and cases included, because the canonical ABI makes them part of
//! the signature), function signatures, and world import/export names (the
//! future capability-grant vocabulary) — into a canonical JSON form, and
//! enforces the additive-evolution rules against the blessed snapshot in
//! `wit/baseline.json`:
//!
//! - every baseline entry must survive verbatim (removal, rename, or any
//!   signature change goes red);
//! - any vocabulary change at all requires that package's version stamp to
//!   move past the baseline's within the same major;
//! - a version regression, or a major bump without a deliberate re-bless,
//!   goes red.
//!
//! Re-blessing IS the deliberate act: `WIT_BASELINE_BLESS=1 cargo test
//! wit_` rewrites the snapshot. See wit/README.md for the rules as prose.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use wit_parser::{Resolve, Type, TypeDefKind, WorldKey};

const EXPECTED_PACKAGES: [&str; 6] = [
    "workbench:lang-pack",
    "workbench:resolver",
    "workbench:runner",
    "workbench:tool",
    "workbench:ranking",
    // The narrow host capability reserved to blessed plugins (SDD §8.3
    // Resolvers; specs/001-lsp-resolver-plugin), added with its own stamp.
    "workbench:subprocess",
];

fn wit_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../subsystems/workspace/reference/wit")
}

fn baseline_path() -> PathBuf {
    wit_dir().join("baseline.json")
}

/// One package's rendered vocabulary. BTreeMaps keep the JSON deterministic.
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct PackageVocabulary {
    version: String,
    interfaces: BTreeMap<String, InterfaceVocabulary>,
    worlds: BTreeMap<String, WorldVocabulary>,
}

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct InterfaceVocabulary {
    types: BTreeMap<String, String>,
    functions: BTreeMap<String, String>,
}

#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
struct WorldVocabulary {
    imports: Vec<String>,
    exports: Vec<String>,
}

fn render_type(resolve: &Resolve, ty: &Type) -> String {
    match ty {
        Type::Bool => "bool".into(),
        Type::U8 => "u8".into(),
        Type::U16 => "u16".into(),
        Type::U32 => "u32".into(),
        Type::U64 => "u64".into(),
        Type::S8 => "s8".into(),
        Type::S16 => "s16".into(),
        Type::S32 => "s32".into(),
        Type::S64 => "s64".into(),
        Type::F32 => "f32".into(),
        Type::F64 => "f64".into(),
        Type::Char => "char".into(),
        Type::String => "string".into(),
        Type::ErrorContext => "error-context".into(),
        Type::Id(id) => {
            let def = &resolve.types[*id];
            match &def.name {
                Some(name) => name.clone(),
                None => render_type_def_kind(resolve, &def.kind),
            }
        }
    }
}

fn render_type_def_kind(resolve: &Resolve, kind: &TypeDefKind) -> String {
    match kind {
        TypeDefKind::Record(record) => {
            let fields: Vec<String> = record
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, render_type(resolve, &f.ty)))
                .collect();
            format!("record {{ {} }}", fields.join(", "))
        }
        TypeDefKind::Enum(e) => {
            let cases: Vec<&str> = e.cases.iter().map(|c| c.name.as_str()).collect();
            format!("enum {{ {} }}", cases.join(", "))
        }
        TypeDefKind::Variant(v) => {
            let cases: Vec<String> = v
                .cases
                .iter()
                .map(|c| match &c.ty {
                    Some(ty) => format!("{}({})", c.name, render_type(resolve, ty)),
                    None => c.name.clone(),
                })
                .collect();
            format!("variant {{ {} }}", cases.join(", "))
        }
        TypeDefKind::Flags(f) => {
            let names: Vec<&str> = f.flags.iter().map(|f| f.name.as_str()).collect();
            format!("flags {{ {} }}", names.join(", "))
        }
        TypeDefKind::Option(ty) => format!("option<{}>", render_type(resolve, ty)),
        TypeDefKind::List(ty) => format!("list<{}>", render_type(resolve, ty)),
        TypeDefKind::Result(r) => {
            let ok = r.ok.as_ref().map(|t| render_type(resolve, t));
            let err = r.err.as_ref().map(|t| render_type(resolve, t));
            match (ok, err) {
                (Some(ok), Some(err)) => format!("result<{ok}, {err}>"),
                (Some(ok), None) => format!("result<{ok}>"),
                (None, Some(err)) => format!("result<_, {err}>"),
                (None, None) => "result".into(),
            }
        }
        TypeDefKind::Tuple(t) => {
            let types: Vec<String> = t.types.iter().map(|ty| render_type(resolve, ty)).collect();
            format!("tuple<{}>", types.join(", "))
        }
        TypeDefKind::Type(ty) => render_type(resolve, ty),
        other => format!("{other:?}"),
    }
}

fn render_world_key(resolve: &Resolve, key: &WorldKey) -> String {
    match key {
        WorldKey::Name(name) => name.clone(),
        WorldKey::Interface(id) => resolve.interfaces[*id]
            .name
            .clone()
            .unwrap_or_else(|| "<anonymous>".into()),
    }
}

/// Parse every wit/*.wit file (each a self-contained package) and render the
/// full vocabulary, keyed by `namespace:name`.
fn current_vocabulary() -> BTreeMap<String, PackageVocabulary> {
    let dir = wit_dir();
    let mut out = BTreeMap::new();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("wit dir {} unreadable: {e}", dir.display()))
        .map(|e| e.expect("dir entry").path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "wit"))
        .collect();
    entries.sort();
    assert!(!entries.is_empty(), "no .wit files under {}", dir.display());

    for path in entries {
        let mut resolve = Resolve::new();
        let (pkg_id, _) = resolve
            .push_path(&path)
            .unwrap_or_else(|e| panic!("{} does not parse as WIT: {e:?}", path.display()));
        let pkg = &resolve.packages[pkg_id];
        let name = format!("{}:{}", pkg.name.namespace, pkg.name.name);
        let version = pkg
            .name
            .version
            .as_ref()
            .unwrap_or_else(|| panic!("{name} carries no version stamp"))
            .to_string();

        let mut interfaces = BTreeMap::new();
        for (iface_name, iface_id) in &pkg.interfaces {
            let iface = &resolve.interfaces[*iface_id];
            let mut types = BTreeMap::new();
            for (type_name, type_id) in &iface.types {
                let def = &resolve.types[*type_id];
                types.insert(type_name.clone(), render_type_def_kind(&resolve, &def.kind));
            }
            let mut functions = BTreeMap::new();
            for (func_name, func) in &iface.functions {
                let params: Vec<String> = func
                    .params
                    .iter()
                    .map(|p| format!("{}: {}", p.name, render_type(&resolve, &p.ty)))
                    .collect();
                let sig = match &func.result {
                    Some(ty) => {
                        format!(
                            "func({}) -> {}",
                            params.join(", "),
                            render_type(&resolve, ty)
                        )
                    }
                    None => format!("func({})", params.join(", ")),
                };
                functions.insert(func_name.clone(), sig);
            }
            interfaces.insert(iface_name.clone(), InterfaceVocabulary { types, functions });
        }

        let mut worlds = BTreeMap::new();
        for (world_name, world_id) in &pkg.worlds {
            let world = &resolve.worlds[*world_id];
            let mut imports: Vec<String> = world
                .imports
                .keys()
                .map(|k| render_world_key(&resolve, k))
                .collect();
            imports.sort();
            let mut exports: Vec<String> = world
                .exports
                .keys()
                .map(|k| render_world_key(&resolve, k))
                .collect();
            exports.sort();
            worlds.insert(world_name.clone(), WorldVocabulary { imports, exports });
        }

        out.insert(
            name,
            PackageVocabulary {
                version,
                interfaces,
                worlds,
            },
        );
    }
    out
}

fn parse_semver(v: &str) -> (u64, u64, u64) {
    let mut parts = v.splitn(3, '.');
    let mut next = || {
        parts
            .next()
            .and_then(|p| p.parse::<u64>().ok())
            .unwrap_or_else(|| panic!("unparseable version stamp {v:?}"))
    };
    (next(), next(), next())
}

#[test]
fn all_five_interfaces_defined_with_independent_version_stamps() {
    let current = current_vocabulary();
    for expected in EXPECTED_PACKAGES {
        let pkg = current
            .get(expected)
            .unwrap_or_else(|| panic!("package {expected} missing from wit/"));
        assert!(
            !pkg.version.is_empty(),
            "{expected} carries no version stamp"
        );
        assert!(
            !pkg.interfaces.is_empty(),
            "{expected} defines no interface"
        );
        assert!(!pkg.worlds.is_empty(), "{expected} defines no world");
    }
    assert_eq!(
        current.len(),
        EXPECTED_PACKAGES.len(),
        "unexpected extra WIT packages: {:?}",
        current.keys().collect::<Vec<_>>()
    );
}

#[test]
fn additive_evolution_enforced_against_blessed_baseline() {
    let current = current_vocabulary();
    let path = baseline_path();

    if std::env::var_os("WIT_BASELINE_BLESS").is_some() {
        let json = serde_json::to_string_pretty(&current).expect("serialize baseline");
        std::fs::write(&path, format!("{json}\n")).expect("write baseline");
        return;
    }

    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "wit/baseline.json unreadable ({e}); bless it deliberately with \
             WIT_BASELINE_BLESS=1 cargo test wit_"
        )
    });
    let baseline: BTreeMap<String, PackageVocabulary> =
        serde_json::from_str(&raw).expect("baseline.json parses");

    for (name, base_pkg) in &baseline {
        let cur_pkg = current.get(name).unwrap_or_else(|| {
            panic!("package {name} was REMOVED — a public WIT package is carried for years")
        });

        let (base_major, ..) = parse_semver(&base_pkg.version);
        let (cur_major, ..) = parse_semver(&cur_pkg.version);
        let base_v = parse_semver(&base_pkg.version);
        let cur_v = parse_semver(&cur_pkg.version);

        assert!(
            cur_v >= base_v,
            "{name}: version regressed {} -> {}",
            base_pkg.version,
            cur_pkg.version
        );
        assert_eq!(
            cur_major, base_major,
            "{name}: major bump {} -> {} without a deliberate re-bless \
             (WIT_BASELINE_BLESS=1 cargo test wit_)",
            base_pkg.version, cur_pkg.version
        );

        // Within a major, every blessed entry survives verbatim.
        for (iface_name, base_iface) in &base_pkg.interfaces {
            let cur_iface = cur_pkg
                .interfaces
                .get(iface_name)
                .unwrap_or_else(|| panic!("{name}/{iface_name}: interface removed or renamed"));
            for (type_name, base_sig) in &base_iface.types {
                let cur_sig = cur_iface.types.get(type_name).unwrap_or_else(|| {
                    panic!("{name}/{iface_name}: type {type_name} removed or renamed")
                });
                assert_eq!(
                    cur_sig, base_sig,
                    "{name}/{iface_name}: type {type_name} changed shape — fields and \
                     cases are canonical-ABI signature, frozen within a major"
                );
            }
            for (func_name, base_sig) in &base_iface.functions {
                let cur_sig = cur_iface.functions.get(func_name).unwrap_or_else(|| {
                    panic!("{name}/{iface_name}: function {func_name} removed or renamed")
                });
                assert_eq!(
                    cur_sig, base_sig,
                    "{name}/{iface_name}: function {func_name} changed signature"
                );
            }
        }
        for (world_name, base_world) in &base_pkg.worlds {
            let cur_world = cur_pkg
                .worlds
                .get(world_name)
                .unwrap_or_else(|| panic!("{name}: world {world_name} removed or renamed"));
            for import in &base_world.imports {
                assert!(
                    cur_world.imports.contains(import),
                    "{name}/{world_name}: import {import} removed — world import names \
                     are the capability-grant vocabulary"
                );
            }
            for export in &base_world.exports {
                assert!(
                    cur_world.exports.contains(export),
                    "{name}/{world_name}: export {export} removed"
                );
            }
        }

        // Any vocabulary difference at all requires the stamp to move.
        if cur_pkg.interfaces != base_pkg.interfaces || cur_pkg.worlds != base_pkg.worlds {
            assert!(
                cur_v > base_v,
                "{name}: vocabulary changed but the version stamp did not move past {}",
                base_pkg.version
            );
        }
    }

    // Baseline covers the whole public surface: a new package must be blessed.
    for name in current.keys() {
        assert!(
            baseline.contains_key(name),
            "package {name} is not in the blessed baseline — bless deliberately with \
             WIT_BASELINE_BLESS=1 cargo test wit_"
        );
    }
}
