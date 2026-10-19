//! The bundled LSP resolver guest (chunk:m5-lsp-resolver-plugin).
//!
//! A minimal LSP client over the subprocess capability: for each language id
//! the host's detection table serves, it opens the detected server, runs the
//! initialize handshake, locates the symbol via `workspace/symbol` (see
//! [`Session::locate`] for why that takes two queries), and asks
//! for `textDocument/references`. The JSON-RPC protocol lives HERE, in the
//! guest — the host provides only framed stdio transport (wit/subprocess.wit).
//!
//! Failure discipline: every protocol problem — timeout, dead server,
//! malformed JSON, missing capability — degrades to an empty answer for that
//! language. The guest never traps on server behavior; the host's symbolic
//! lane is the permanent floor (spec US2).
//!
//! Pooled sessions (specs/007-resolver-server-pool, subprocess@0.2.0): the
//! host may hand back a WARM session from an earlier invoke. `sp::reused`
//! says so, and LSP allows `initialize` once per connection, so the guest
//! skips the handshake on a warm session. A session that answered is handed
//! back with `sp::release` (never killed); a session judged UNUSABLE — dead,
//! silent past the read timeout, unframeable — is `sp::close`d so the next
//! invoke starts fresh. "No such symbol" and a server's error reply are not
//! unusable: the server is healthy and keeps its warm session.
//!
//! `no_std` + `alloc` by construction: `std` on wasm32-wasip1 links wasi-libc
//! startup that imports `wasi_snapshot_preview1` (environ_get), and the
//! host's default-deny would refuse the component. With no std there is no
//! WASI symbol to import — the module's only import is magistr:subprocess.

#![no_std]

extern crate alloc;

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

#[global_allocator]
static ALLOC: dlmalloc::GlobalDlmalloc = dlmalloc::GlobalDlmalloc;

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // A guest panic is a deterministic trap the host classifies as
    // guest-trap — never an ambient-authority escape.
    core::arch::wasm32::unreachable()
}

/// wasi-libc normally provides `memcmp`; without std nothing does, and the
/// module must import NOTHING but magistr:subprocess. One byte-wise
/// definition keeps the import surface empty (speed is irrelevant here).
#[unsafe(no_mangle)]
unsafe extern "C" fn memcmp(a: *const u8, b: *const u8, n: usize) -> i32 {
    let mut i = 0;
    while i < n {
        let x = unsafe { *a.add(i) };
        let y = unsafe { *b.add(i) };
        if x != y {
            return i32::from(x) - i32::from(y);
        }
        i += 1;
    }
    0
}

#[allow(warnings)]
mod bindings {
    wit_bindgen::generate!({
        path: "wit",
        world: "lsp-resolver",
        generate_all,
        // no_std guest: gate the generated `std::error::Error` impls behind a
        // "std" feature this crate deliberately does not define.
        std_feature,
    });
}

use bindings::exports::magistr::resolver::resolver::{
    Diagnostic, Guest as ResolverGuest, RenameEdit, SourceLocation,
};
use bindings::magistr::subprocess::subprocess as sp;
use serde_json::{json, Value};

/// Per-frame read timeout. The host's per-request budget is the outer bound;
/// this inner one keeps a silent server from eating the whole invoke.
const READ_TIMEOUT_MS: u32 = 5_000;
/// Frames to scan past server-initiated notifications before giving up on a
/// response id. A loading server (rust-analyzer indexing a fresh project)
/// interleaves many `$/progress`, `publishDiagnostics`, and
/// `workspace/diagnostic/refresh` frames ahead of the reply; the bound is the
/// per-frame READ_TIMEOUT, so this only caps a pathological chatterer.
const MAX_FRAMES_PER_RESPONSE: usize = 4096;

struct Component;

impl bindings::Guest for Component {
    /// The standard plugin-tool convention (TCK conformance): identity as JSON.
    fn invoke(_payload: String) -> String {
        json!({
            "resolver": "magistr.lsp-resolver",
            "protocol": "lsp",
            "surface": "magistr:resolver@0.1.0",
            "subprocess": "magistr:subprocess@0.2.0",
        })
        .to_string()
    }
}

impl ResolverGuest for Component {
    fn resolve_references(symbol: String) -> Vec<SourceLocation> {
        let mut out = Vec::new();
        for language in sp::specs() {
            if let Ok(session) = sp::open(&language) {
                let warm = sp::reused(session);
                match Session::new(session).references(&symbol, warm) {
                    Ok(mut locations) => {
                        out.append(&mut locations);
                        sp::release(session);
                    }
                    Err(Unusable) => sp::close(session),
                }
            }
        }
        out
    }

    /// MVP: rename and diagnostics answer empty (additive later — the WIT
    /// surface is already carried, so growing these is not a contract change).
    fn rename(_symbol: String, _new_name: String) -> Vec<RenameEdit> {
        Vec::new()
    }

    fn diagnostics(_path: String) -> Vec<Diagnostic> {
        let _ = Diagnostic {
            location: SourceLocation {
                path: String::new(),
                line: 1,
                column: 1,
            },
            message: String::new(),
            severity: bindings::exports::magistr::resolver::resolver::Severity::Info,
        };
        Vec::new()
    }
}

/// The session cannot be trusted any further (transport failure, silence
/// past the read timeout, unframeable bytes, a refused handshake): the
/// caller kills it.
struct Unusable;

struct Session {
    handle: u32,
    next_id: u64,
}

impl Session {
    fn new(handle: u32) -> Self {
        Session { handle, next_id: 1 }
    }

    /// Full flow: initialize (cold sessions only) → workspace/symbol →
    /// textDocument/references. `Ok(empty)` when the server is healthy but
    /// has nothing (no such symbol, declined request); `Err(Unusable)` only
    /// when the session itself is broken.
    fn references(&mut self, symbol: &str, warm: bool) -> Result<Vec<SourceLocation>, Unusable> {
        let root = sp::workspace_root();
        let root_uri = file_uri(&root);

        if !warm {
            let init = self.request(
                "initialize",
                json!({
                    "processId": null,
                    "rootUri": if root_uri.is_empty() { Value::Null } else { Value::String(root_uri.clone()) },
                    "capabilities": {},
                }),
            )?;
            if init.is_none() {
                // The server refused the handshake; a fresh process next time.
                return Err(Unusable);
            }
            self.notify("initialized", json!({}));
        }

        let Some(hit) = self.locate(symbol)? else {
            return Ok(Vec::new());
        };
        let (Some(uri), Some(position)) = (
            hit.pointer("/location/uri")
                .and_then(Value::as_str)
                .map(str::to_string),
            hit.pointer("/location/range/start").cloned(),
        ) else {
            return Ok(Vec::new());
        };

        let Some(refs) = self.request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": uri },
                "position": position,
                "context": { "includeDeclaration": true },
            }),
        )?
        else {
            return Ok(Vec::new());
        };
        let mut locations = Vec::new();
        for entry in refs.as_array().into_iter().flatten() {
            let Some(loc) = lsp_location(entry, &root_uri) else {
                continue;
            };
            locations.push(loc);
        }
        Ok(locations)
    }

    /// Find the symbol's declaration via `workspace/symbol`, returning the
    /// exact-name hit.
    ///
    /// Two queries, because `workspace/symbol` is a FUZZY search whose default
    /// scope is server-defined: rust-analyzer answers a bare query with TYPES
    /// ONLY, falling back to all symbols just when the type search is empty —
    /// so in any real Rust project (which has types) a bare `greet` returns
    /// `Greeter` and never the function. When the plain query yields no
    /// exact-name hit, retry once with the `#` all-symbols modifier
    /// rust-analyzer documents. A server that does not know the modifier
    /// simply answers nothing or an error, which is the same empty answer the
    /// caller already had — the fallback can only add hits, never lose them.
    fn locate(&mut self, symbol: &str) -> Result<Option<Value>, Unusable> {
        for query in [symbol.to_string(), format!("#{symbol}")] {
            let Some(symbols) = self.request("workspace/symbol", json!({ "query": query }))? else {
                continue;
            };
            let hit = symbols.as_array().and_then(|list| {
                list.iter()
                    .find(|s| s.get("name").and_then(Value::as_str) == Some(symbol))
                    .cloned()
            });
            if hit.is_some() {
                return Ok(hit);
            }
        }
        Ok(None)
    }

    /// Send one request and scan frames until its response id arrives.
    /// `Ok(None)` is the server's error reply (declined, not broken);
    /// `Err(Unusable)` is a transport failure, silence, or unframeable data.
    fn request(&mut self, method: &str, params: Value) -> Result<Option<Value>, Unusable> {
        let id = self.next_id;
        self.next_id += 1;
        let body = json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params });
        sp::write_frame(self.handle, body.to_string().as_bytes()).map_err(|_| Unusable)?;
        for _ in 0..MAX_FRAMES_PER_RESPONSE {
            let frame = sp::read_frame(self.handle, READ_TIMEOUT_MS).map_err(|_| Unusable)?;
            let value: Value = serde_json::from_slice(&frame).map_err(|_| Unusable)?;
            // A server-initiated REQUEST also carries an id; only a frame
            // without `method` is a response to us.
            if value.get("method").is_none() && value.get("id").and_then(Value::as_u64) == Some(id)
            {
                if value.get("error").is_some() {
                    return Ok(None);
                }
                return Ok(Some(value.get("result").cloned().unwrap_or(Value::Null)));
            }
            // Server-initiated notification/request: skip and keep scanning.
        }
        Err(Unusable)
    }

    fn notify(&mut self, method: &str, params: Value) {
        let body = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        let _ = sp::write_frame(self.handle, body.to_string().as_bytes());
    }
}

/// One LSP Location → SourceLocation (1-based, workspace-relative when the
/// URI sits under the root; otherwise the decoded absolute path is passed
/// through for the HOST's jail validation to reject — the guest never
/// silently drops what the host must judge).
fn lsp_location(entry: &Value, root_uri: &str) -> Option<SourceLocation> {
    let uri = entry.get("uri")?.as_str()?;
    let start = entry.pointer("/range/start")?;
    let line = u32::try_from(start.get("line")?.as_u64()?).ok()? + 1;
    let column = u32::try_from(start.get("character")?.as_u64()?).ok()? + 1;
    Some(SourceLocation {
        path: relativize(uri, root_uri),
        line,
        column,
    })
}

/// `file://` URI for a host path: `/`-separated, drive letters prefixed with
/// a slash. No percent-encoding — the host controls both producers.
fn file_uri(root: &str) -> String {
    if root.is_empty() {
        return String::new();
    }
    let slashed = root.replace('\\', "/");
    if slashed.starts_with('/') {
        format!("file://{slashed}")
    } else {
        format!("file:///{slashed}")
    }
}

/// Strip the root URI prefix (case-insensitive on the drive-letter form,
/// tolerant of `%3A`-encoded colons) to a workspace-relative `/` path.
fn relativize(uri: &str, root_uri: &str) -> String {
    let decoded = uri.replace("%3A", ":").replace("%3a", ":");
    if !root_uri.is_empty() {
        let (head, tail) = split_at_prefix(&decoded, root_uri);
        if head && !tail.is_empty() {
            return tail.trim_start_matches('/').to_string();
        }
    }
    decoded
        .strip_prefix("file:///")
        .or_else(|| decoded.strip_prefix("file://"))
        .unwrap_or(&decoded)
        .to_string()
}

/// ASCII-case-insensitive prefix test returning the remainder.
fn split_at_prefix<'a>(value: &'a str, prefix: &str) -> (bool, &'a str) {
    if value.len() >= prefix.len() && value[..prefix.len()].eq_ignore_ascii_case(prefix) {
        (true, &value[prefix.len()..])
    } else {
        (false, value)
    }
}

bindings::export!(Component with_types_in bindings);
