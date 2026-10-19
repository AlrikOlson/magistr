//! Current Magistr plugin contract. Parse every WIT definition and compare its
//! full vocabulary with the reviewed snapshot. Update the snapshot deliberately
//! when changing the current contract; old interfaces are not retained.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use wit_parser::{Resolve, Type, TypeDefKind, WorldKey};

const EXPECTED_PACKAGES: [&str; 6] = [
    "magistr:lang-pack",
    "magistr:resolver",
    "magistr:runner",
    "magistr:tool",
    "magistr:ranking",
    // The narrow host capability reserved to blessed plugins (SDD §8.3
    // Resolvers; specs/001-lsp-resolver-plugin), added with its own stamp.
    "magistr:subprocess",
];

fn wit_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("wit")
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

#[test]
fn all_six_interfaces_defined_with_independent_version_stamps() {
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
fn current_contract_matches_reviewed_snapshot() {
    let current = current_vocabulary();
    let path = baseline_path();
    if std::env::var_os("WIT_BASELINE_BLESS").is_some() {
        let json = serde_json::to_string_pretty(&current).expect("serialize vocabulary");
        std::fs::write(&path, format!("{json}\n")).expect("write vocabulary");
        return;
    }
    let raw = std::fs::read_to_string(&path).expect("read wit/baseline.json");
    let expected: BTreeMap<String, PackageVocabulary> =
        serde_json::from_str(&raw).expect("parse vocabulary");
    assert_eq!(
        current, expected,
        "review the current WIT contract and update its snapshot"
    );
}
