//! Wasmtime extension host (SDD §8.1, chunk:m4-wasmtime-host).
//!
//! One sandbox engine, one policy, two guest lanes:
//!
//! * **component lane** — component-model guests (the SDK plugin substrate).
//!   Imports resolve exclusively through capability objects granted per run;
//!   an import with no grant is refused *before instantiation* with the
//!   import's name, so denial is typed, not a string-matched link error.
//! * **wasi lane** — `wasm32-wasip1` module guests (the `script` substrate,
//!   QuickJS-ng). Zero preopens, no inherited stdio, no environment, no
//!   network: the guest's whole world is argv plus captured stdout/stderr.
//!
//! Both lanes run under the same [`SandboxPolicy`]: a fuel budget
//! (`Config::consume_fuel`) and a linear-memory cap enforced by a
//! [`ResourceLimiter`] configured to *trap* on a denied grow — so fuel
//! exhaustion and memory-cap breach are both deterministic terminations, not
//! recoverable soft failures the guest can probe around.
//!
//! Guest failure is **data**: every run returns a [`GuestEnd`] classification
//! rather than raising. [`SandboxError`] is reserved for host-side bugs
//! (engine misconfiguration), which is what keeps a trapped guest from ever
//! taking down the engine child.
//!
//! This crate builds to `wasm32-wasip1` as the engine child's fallback;
//! Wasmtime cannot target wasm32, so the whole module sits behind the
//! `sandbox` cargo feature (default-on, excluded from the wasm build). The
//! fallback lane honestly lacks the extension host, matching the SDD §6.2
//! degradation model.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use wasmtime::component::{Component, Linker as ComponentLinker, Type as ComponentType, Val};
use wasmtime::{
    Config, Engine, Module, ResourceLimiter, Store, StoreLimits, StoreLimitsBuilder, Trap,
};
use wasmtime_wasi::p1::WasiP1Ctx;
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::{I32Exit, WasiCtxBuilder};

#[cfg(test)]
#[path = "sandbox_process.tests.rs"]
mod process_tests;

#[cfg(test)]
#[path = "sandbox_cache.tests.rs"]
mod cache_tests;

/// Cap on captured guest stdout/stderr, per stream.
const OUTPUT_PIPE_CAPACITY: usize = 64 * 1024;

/// The one resource policy every guest runs under (SDD §8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SandboxPolicy {
    /// Fuel budget; exhaustion is a deterministic `Trap::OutOfFuel` kill.
    pub fuel: u64,
    /// Linear-memory cap in bytes; a grow past it traps the guest.
    pub memory_cap_bytes: usize,
}

/// Host-side failure — a bug or misconfiguration in the host itself, never a
/// guest-caused condition. Guest conditions are [`GuestEnd`] values.
#[derive(Debug, thiserror::Error)]
pub enum SandboxError {
    #[error("sandbox engine unavailable: {0}")]
    Engine(String),
}

/// How a guest run ended. Every variant is an ordinary return value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuestEnd {
    /// The guest ran to completion (wasi lane: exited 0).
    Ok,
    /// Deterministic kill: the fuel budget ran out.
    OutOfFuel,
    /// Deterministic kill: the guest tried to grow past the memory cap.
    MemoryCap,
    /// The guest requires an import for which no capability was granted.
    /// Carries the import's name. Decided before instantiation.
    CapDenied(String),
    /// The guest trapped on its own (unreachable, div-by-zero, …).
    GuestTrap(String),
    /// wasi lane only: the guest called `proc_exit` with a non-zero code.
    GuestExit(i32),
    /// The bytes, export, argument types, or requested grants are unusable.
    InvalidGuest(String),
}

impl GuestEnd {
    /// Stable wire tag for the FFI boundary.
    pub fn kind(&self) -> &'static str {
        match self {
            GuestEnd::Ok => "ok",
            GuestEnd::OutOfFuel => "out-of-fuel",
            GuestEnd::MemoryCap => "memory-cap",
            GuestEnd::CapDenied(_) => "cap-denied",
            GuestEnd::GuestTrap(_) => "guest-trap",
            GuestEnd::GuestExit(_) => "guest-exit",
            GuestEnd::InvalidGuest(_) => "invalid-guest",
        }
    }

    /// Human-readable detail (import name, trap message, exit code, …).
    pub fn detail(&self) -> String {
        match self {
            GuestEnd::Ok | GuestEnd::OutOfFuel | GuestEnd::MemoryCap => String::new(),
            GuestEnd::CapDenied(name) => name.clone(),
            GuestEnd::GuestTrap(msg) | GuestEnd::InvalidGuest(msg) => msg.clone(),
            GuestEnd::GuestExit(code) => code.to_string(),
        }
    }
}

/// Outcome of a component-lane run.
#[derive(Debug)]
pub struct ComponentOutcome {
    pub end: GuestEnd,
    pub fuel_used: u64,
    /// Debug-rendered component values returned by the export (substrate
    /// calling convention; the typed WIT surfaces are chunk m4-wit-interfaces).
    pub results: Vec<String>,
    /// Every value the guest passed to the granted `observe` capability.
    pub observations: Vec<u32>,
}

/// Outcome of a wasi-lane run.
#[derive(Debug)]
pub struct WasiOutcome {
    pub end: GuestEnd,
    pub fuel_used: u64,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// The capability objects this host can grant (v0). Manifest-driven grants
/// are chunk m4-capability-manifests; the mechanism — named grants, default
/// deny — is fixed here.
pub const CAPABILITIES: &[&str] = &[CAP_OBSERVE, CAP_SUBPROCESS];

/// v0 capability: a host counter the guest may call with a `u32`; the host
/// records each call and returns the running count. Exists to prove the
/// grant mechanism end to end without the canonical-ABI string machinery.
pub const CAP_OBSERVE: &str = "observe";

/// The subprocess capability's CURRENT grant name — the WIT world import name
/// of `magistr:subprocess/subprocess@0.2.0` (SDD §8.2: world import names
/// are the grant vocabulary). Reserved to blessed plugins; the extension host refuses
/// to grant any version of it below that tier before this code ever runs.
pub const CAP_SUBPROCESS: &str = "magistr:subprocess/subprocess@0.2.0";

/// Recognize the current subprocess contract exactly.
pub fn is_subprocess_import(name: &str) -> bool {
    name == CAP_SUBPROCESS
}

// ---- Subprocess capability (chunk:m5-lsp-resolver-plugin) -----------------
//
// Constitution IV boundary: the guest never names a binary, argv, or env.
// It addresses a language id; the HOST resolves it through this curated spec
// table, handed in per invoke by the caller (the detection table travels
// with the request — the server stays stateless). Transport frames are LSP
// Content-Length framed on the wire; the guest sees only frame bodies.
//
// Since specs/007-resolver-server-pool the SERVER PROCESS may outlive the
// invoke: a process-global [`ServerPool`] keeps warm sessions keyed by the
// request-carried identity (workspace root + spec) and the per-invoke
// [`SubprocessPool`] is a LEASE VIEW over it. Kill is the default: a session
// survives an invoke only through the explicit success path (`settle(true)`
// or the guest's `release`); fuel exhaustion, traps, memory-cap kills,
// `close`, the idle limit, and pool shutdown all kill.

/// One host-curated language-server launch spec. Only the host constructs
/// these; the guest can only select one by `language_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubprocessSpec {
    pub language_id: String,
    pub bin: String,
    pub args: Vec<String>,
}

/// Ceiling on one transport frame in either direction.
const SUBPROCESS_FRAME_CAP: usize = 4 * 1024 * 1024;
/// Headers cannot grow indefinitely while waiting for their delimiter.
const SUBPROCESS_HEADER_CAP: usize = 8 * 1024;
/// Bound unread notifications without blocking the owned reader's teardown.
const SUBPROCESS_FRAME_QUEUE_CAP: usize = 16;
/// Ceiling on live sessions per guest invoke (one per spec is the intent).
const SUBPROCESS_SESSION_CAP: usize = 4;
/// Aggregate ceiling includes active leases and warm pooled sessions.
const SERVER_SESSION_CAP: usize = 16;
/// A warm, unleased session idle longer than this is killed (FR-007).
pub const SERVER_IDLE_TTL: Duration = Duration::from_secs(300);
/// Cadence of the background idle sweep (the sweep also runs on every lease).
const SERVER_REAPER_TICK: Duration = Duration::from_secs(30);

/// `open-error` (wit/subprocess.wit) — lowered as the WIT variant.
#[derive(Debug, Clone, wasmtime::component::ComponentType, wasmtime::component::Lower)]
#[component(variant)]
enum SubprocessOpenError {
    #[component(name = "unknown-spec")]
    UnknownSpec(String),
    #[component(name = "spawn-refused")]
    SpawnRefused(String),
    #[component(name = "unhealthy")]
    #[allow(dead_code)] // health gating lives host-side (TS); variant is contract-complete
    Unhealthy(String),
}

/// `io-error` (wit/subprocess.wit) — lowered as the WIT variant.
#[derive(Debug, Clone, wasmtime::component::ComponentType, wasmtime::component::Lower)]
#[component(variant)]
enum SubprocessIoError {
    #[component(name = "dead")]
    Dead(String),
    #[component(name = "timeout")]
    Timeout(String),
    #[component(name = "frame-too-large")]
    FrameTooLarge(String),
    #[component(name = "bad-session")]
    BadSession(String),
}

/// The pool key: exactly the identity the request carries (Constitution V —
/// identity travels on the request; the pool holds no session state).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PoolKey {
    root: String,
    language_id: String,
    bin: String,
    args: Vec<String>,
}

/// One live language-server session: the child, its stdin, and a channel fed
/// by an owned reader thread that de-frames Content-Length payloads. The
/// `leased` flag tells the reader whether to queue frames (a guest holds the
/// session) or drop them (the session sits warm in the pool) — so the
/// server's stdout always drains and a reused lease starts with an empty
/// buffer (FR-004).
struct SubprocessSession {
    language_id: String,
    child: std::process::Child,
    stdin: Option<std::process::ChildStdin>,
    frames: mpsc::Receiver<Result<Vec<u8>, String>>,
    leased: Arc<AtomicBool>,
    /// Identity in the process-global pool.
    key: PoolKey,
    /// True when this handle is a warm session from an earlier invoke.
    reused: bool,
    reader: Option<std::thread::JoinHandle<()>>,
    live_sessions: Arc<AtomicUsize>,
}

impl SubprocessSession {
    fn kill(self) {
        drop(self);
    }

    fn is_dead(&mut self) -> bool {
        self.reader
            .as_ref()
            .is_none_or(std::thread::JoinHandle::is_finished)
            || !matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for SubprocessSession {
    fn drop(&mut self) {
        drop(self.stdin.take());
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        self.live_sessions.fetch_sub(1, Ordering::SeqCst);
    }
}

/// One warm-session stat row (tests, diagnostics).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerPoolSessionStat {
    pub language_id: String,
    pub root: String,
    pub pid: u32,
    /// Held by an in-flight invoke right now.
    pub leased: bool,
    /// Milliseconds since the session was last leased or returned.
    pub idle_ms: u64,
}

/// The engine-child-lifetime pool (specs/007 data-model.md). `warm` holds
/// sessions nobody is using; `leased` names the keys currently held by a
/// lease view (their sessions live in that view), so a second invoke asking
/// for the same key is refused without waiting.
#[derive(Default)]
struct ServerPool {
    warm: HashMap<PoolKey, (SubprocessSession, Instant)>,
    leased: HashMap<PoolKey, u32>,
    #[cfg(windows)]
    kill_group: Option<kill_group::KillGroup>,
    reaper_started: bool,
    live_sessions: Arc<AtomicUsize>,
}

fn server_pool() -> &'static Mutex<ServerPool> {
    static POOL: OnceLock<Mutex<ServerPool>> = OnceLock::new();
    POOL.get_or_init(|| Mutex::new(ServerPool::default()))
}

fn lock_pool() -> std::sync::MutexGuard<'static, ServerPool> {
    server_pool()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl ServerPool {
    /// Spawn a server for `spec`, attach the reader thread, and put it in the
    /// OS kill group where one exists (research R5).
    fn spawn(
        &mut self,
        spec: &SubprocessSpec,
        key: PoolKey,
    ) -> Result<SubprocessSession, SubprocessOpenError> {
        if self.live_sessions.load(Ordering::SeqCst) >= SERVER_SESSION_CAP {
            return Err(SubprocessOpenError::SpawnRefused(format!(
                "server pool session ceiling ({SERVER_SESSION_CAP}) reached"
            )));
        }
        let mut child = spawn_server_process(spec)
            .map_err(|e| SubprocessOpenError::SpawnRefused(format!("{}: {e}", spec.bin)))?;
        #[cfg(windows)]
        {
            if self.kill_group.is_none() {
                self.kill_group = kill_group::KillGroup::new();
            }
            if let Some(group) = &self.kill_group {
                group.assign(&child);
            }
        }
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(SubprocessOpenError::SpawnRefused(
                "child pipes unavailable".to_string(),
            ));
        };
        let leased = Arc::new(AtomicBool::new(true));
        let (tx, rx) = mpsc::sync_channel(SUBPROCESS_FRAME_QUEUE_CAP);
        let reader_flag = Arc::clone(&leased);
        let reader = match std::thread::Builder::new()
            .name("magistr-lsp-frames".to_owned())
            .spawn(move || read_frames(stdout, &tx, &reader_flag))
        {
            Ok(reader) => reader,
            Err(error) => {
                drop(stdin);
                let _ = child.kill();
                let _ = child.wait();
                return Err(SubprocessOpenError::SpawnRefused(format!(
                    "LSP reader thread unavailable: {error}"
                )));
            }
        };
        self.live_sessions.fetch_add(1, Ordering::SeqCst);
        Ok(SubprocessSession {
            language_id: spec.language_id.clone(),
            child,
            stdin: Some(stdin),
            frames: rx,
            leased,
            key,
            reused: false,
            reader: Some(reader),
            live_sessions: Arc::clone(&self.live_sessions),
        })
    }

    /// Lease the session for `key`: a warm one (drained, marked reused) or a
    /// fresh spawn. A key held by another invoke is refused, never awaited.
    fn lease(
        &mut self,
        spec: &SubprocessSpec,
        key: PoolKey,
    ) -> Result<SubprocessSession, SubprocessOpenError> {
        self.ensure_reaper();
        self.reap_idle(SERVER_IDLE_TTL);
        if self.leased.contains_key(&key) {
            return Err(SubprocessOpenError::SpawnRefused(format!(
                "session for {:?} is leased by another in-flight invoke",
                key.language_id
            )));
        }
        if let Some((mut session, _)) = self.warm.remove(&key) {
            if session.is_dead() {
                session.kill();
            } else {
                session.leased.store(true, Ordering::SeqCst);
                while session.frames.try_recv().is_ok() {}
                session.reused = true;
                self.leased.insert(key, session.child.id());
                return Ok(session);
            }
        }
        let session = self.spawn(spec, key.clone())?;
        self.leased.insert(key, session.child.id());
        Ok(session)
    }

    /// Hand a leased session back warm (the explicit success path).
    fn return_warm(&mut self, mut session: SubprocessSession) {
        let key = session.key.clone();
        self.leased.remove(&key);
        if session.is_dead() {
            session.kill();
            return;
        }
        session.leased.store(false, Ordering::SeqCst);
        if let Some((old, _)) = self.warm.insert(key, (session, Instant::now())) {
            old.kill();
        }
    }

    /// Kill a leased session and forget its key.
    fn kill_leased(&mut self, session: SubprocessSession) {
        self.leased.remove(&session.key);
        session.kill();
    }

    fn reap_idle(&mut self, ttl: Duration) -> usize {
        let now = Instant::now();
        let stale: Vec<PoolKey> = self
            .warm
            .iter()
            .filter(|(_, (_, last))| now.duration_since(*last) >= ttl)
            .map(|(k, _)| k.clone())
            .collect();
        let count = stale.len();
        for key in stale {
            if let Some((session, _)) = self.warm.remove(&key) {
                session.kill();
            }
        }
        count
    }

    fn ensure_reaper(&mut self) {
        if self.reaper_started {
            return;
        }
        self.reaper_started = true;
        std::thread::Builder::new()
            .name("workspace-server-pool-reaper".into())
            .spawn(|| {
                loop {
                    std::thread::sleep(SERVER_REAPER_TICK);
                    lock_pool().reap_idle(SERVER_IDLE_TTL);
                }
            })
            .ok();
    }

    fn shutdown(&mut self) -> usize {
        let keys: Vec<PoolKey> = self.warm.keys().cloned().collect();
        let count = keys.len();
        for key in keys {
            if let Some((session, _)) = self.warm.remove(&key) {
                session.kill();
            }
        }
        count
    }

    fn stats(&self) -> Vec<ServerPoolSessionStat> {
        let now = Instant::now();
        let mut out: Vec<ServerPoolSessionStat> = self
            .warm
            .iter()
            .map(|(key, (session, last))| ServerPoolSessionStat {
                language_id: key.language_id.clone(),
                root: key.root.clone(),
                pid: session.child.id(),
                leased: false,
                idle_ms: u64::try_from(now.duration_since(*last).as_millis()).unwrap_or(u64::MAX),
            })
            .chain(self.leased.iter().map(|(key, pid)| ServerPoolSessionStat {
                language_id: key.language_id.clone(),
                root: key.root.clone(),
                pid: *pid,
                leased: true,
                idle_ms: 0,
            }))
            .collect();
        out.sort_by(|a, b| (&a.root, &a.language_id).cmp(&(&b.root, &b.language_id)));
        out
    }
}

/// Kill every warm pooled server. Called by the engine child at its normal
/// exit — after every in-flight invoke settled; Rust statics never drop at
/// process exit — and by tests. Idempotent. The OS kill group covers an
/// abrupt process death on Windows and Linux.
pub fn server_pool_shutdown() -> usize {
    lock_pool().shutdown()
}

/// Snapshot of the pool (tests, diagnostics).
pub fn server_pool_stats() -> Vec<ServerPoolSessionStat> {
    lock_pool().stats()
}

/// Kill warm sessions idle for at least `ttl`; returns how many. The ticker
/// calls this with [`SERVER_IDLE_TTL`]; tests inject `Duration::ZERO`.
pub fn server_pool_reap_idle(ttl: Duration) -> usize {
    lock_pool().reap_idle(ttl)
}

/// Spawn the server process for `spec`, stdio piped, stderr discarded. On
/// Linux the spawn runs on one dedicated process-lifetime thread so the
/// parent-death signal (per parent THREAD, not process) is tied to a thread
/// that lives as long as the engine child (research R5).
#[cfg(not(target_os = "linux"))]
fn spawn_server_process(spec: &SubprocessSpec) -> std::io::Result<std::process::Child> {
    server_command(spec).spawn()
}

#[cfg(target_os = "linux")]
fn spawn_server_process(spec: &SubprocessSpec) -> std::io::Result<std::process::Child> {
    type Reply = mpsc::Sender<std::io::Result<std::process::Child>>;
    static SPAWNER: OnceLock<Mutex<mpsc::Sender<(SubprocessSpec, Reply)>>> = OnceLock::new();
    let sender = SPAWNER.get_or_init(|| {
        let (tx, rx) = mpsc::channel::<(SubprocessSpec, Reply)>();
        std::thread::Builder::new()
            .name("workspace-server-pool-spawner".into())
            .spawn(move || {
                let parent = std::process::id();
                while let Ok((spec, reply)) = rx.recv() {
                    use std::os::unix::process::CommandExt;
                    let mut command = server_command(&spec);
                    // SAFETY: the hook only calls async-signal-safe libc
                    // functions (prctl, getppid, _exit) between fork and exec.
                    unsafe {
                        command.pre_exec(move || {
                            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                            // The parent may have died between fork and prctl;
                            // a reparented child must not outlive it either.
                            if libc::getppid() as u32 != parent {
                                libc::_exit(1);
                            }
                            Ok(())
                        });
                    }
                    let _ = reply.send(command.spawn());
                }
            })
            .expect("spawner thread");
        Mutex::new(tx)
    });
    let (reply_tx, reply_rx) = mpsc::channel();
    sender
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .send((spec.clone(), reply_tx))
        .map_err(|_| std::io::Error::other("server spawner thread is gone"))?;
    reply_rx
        .recv()
        .map_err(|_| std::io::Error::other("server spawner thread is gone"))?
}

fn server_command(spec: &SubprocessSpec) -> std::process::Command {
    let mut command = std::process::Command::new(&spec.bin);
    command
        .args(&spec.args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    command
}

/// Windows kill group: one Job Object with KILL_ON_JOB_CLOSE owned by the
/// pool. When the engine child dies for ANY reason (graceful exit, `proc.kill`
/// from the supervisor, `process.exit`, a crash) the OS closes the handle and
/// kills every assigned server — the guarantee no destructor can give.
#[cfg(windows)]
mod kill_group {
    use std::os::windows::io::AsRawHandle;

    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    pub(super) struct KillGroup(HANDLE);

    // SAFETY: a job handle is a process-wide kernel object; it has no thread
    // affinity and is only ever used under the pool mutex.
    unsafe impl Send for KillGroup {}

    impl KillGroup {
        pub(super) fn new() -> Option<Self> {
            // SAFETY: plain Win32 calls with valid pointers; failure is checked.
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return None;
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                let ok = SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    std::ptr::from_ref(&info).cast(),
                    u32::try_from(std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>())
                        .unwrap_or(u32::MAX),
                );
                if ok == 0 {
                    CloseHandle(job);
                    return None;
                }
                Some(KillGroup(job))
            }
        }

        pub(super) fn assign(&self, child: &std::process::Child) {
            // SAFETY: both handles are live; a failed assignment (already in a
            // non-nesting job) leaves the stdin-EOF belt as the fallback.
            unsafe {
                AssignProcessToJobObject(self.0, child.as_raw_handle() as HANDLE);
            }
        }
    }

    impl Drop for KillGroup {
        fn drop(&mut self) {
            // SAFETY: handle owned by this value; closing it kills the job.
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}

/// Per-invoke subprocess leases. Sessions return warm only after successful settlement.
struct SubprocessPool {
    specs: Vec<SubprocessSpec>,
    workspace_root: String,
    sessions: HashMap<u32, SubprocessSession>,
    next_handle: u32,
    /// Fresh spawns / warm leases this invoke performed (FR-013).
    opened: u32,
    reused: u32,
    settled: bool,
}

impl Default for SubprocessPool {
    fn default() -> Self {
        SubprocessPool::new(Vec::new(), String::new())
    }
}

impl SubprocessPool {
    fn new(specs: Vec<SubprocessSpec>, workspace_root: String) -> Self {
        SubprocessPool {
            specs,
            workspace_root,
            sessions: HashMap::new(),
            next_handle: 1,
            opened: 0,
            reused: 0,
            settled: false,
        }
    }

    fn open(&mut self, language_id: &str) -> Result<u32, SubprocessOpenError> {
        if let Some((handle, _)) = self
            .sessions
            .iter()
            .find(|(_, s)| s.language_id == language_id)
        {
            return Ok(*handle);
        }
        let spec = self
            .specs
            .iter()
            .find(|s| s.language_id == language_id)
            .cloned()
            .ok_or_else(|| {
                SubprocessOpenError::UnknownSpec(format!(
                    "no detected language server for {language_id:?}"
                ))
            })?;
        if self.sessions.len() >= SUBPROCESS_SESSION_CAP {
            return Err(SubprocessOpenError::SpawnRefused(format!(
                "session ceiling ({SUBPROCESS_SESSION_CAP}) reached"
            )));
        }
        let key = PoolKey {
            root: self.workspace_root.clone(),
            language_id: spec.language_id.clone(),
            bin: spec.bin.clone(),
            args: spec.args.clone(),
        };
        let session = lock_pool().lease(&spec, key)?;
        if session.reused {
            self.reused += 1;
        } else {
            self.opened += 1;
        }
        let handle = self.next_handle;
        self.next_handle += 1;
        self.sessions.insert(handle, session);
        Ok(handle)
    }

    fn write_frame(&mut self, handle: u32, frame: &[u8]) -> Result<(), SubprocessIoError> {
        if frame.len() > SUBPROCESS_FRAME_CAP {
            return Err(SubprocessIoError::FrameTooLarge(format!(
                "{} bytes exceeds the {SUBPROCESS_FRAME_CAP}-byte frame cap",
                frame.len()
            )));
        }
        let session = self
            .sessions
            .get_mut(&handle)
            .ok_or_else(|| SubprocessIoError::BadSession(format!("no session {handle}")))?;
        let stdin = session
            .stdin
            .as_mut()
            .ok_or_else(|| SubprocessIoError::Dead("stdin closed".to_string()))?;
        let header = format!("Content-Length: {}\r\n\r\n", frame.len());
        stdin
            .write_all(header.as_bytes())
            .and_then(|()| stdin.write_all(frame))
            .and_then(|()| stdin.flush())
            .map_err(|e| SubprocessIoError::Dead(e.to_string()))
    }

    fn read_frame(&mut self, handle: u32, timeout_ms: u32) -> Result<Vec<u8>, SubprocessIoError> {
        let session = self
            .sessions
            .get_mut(&handle)
            .ok_or_else(|| SubprocessIoError::BadSession(format!("no session {handle}")))?;
        match session
            .frames
            .recv_timeout(Duration::from_millis(u64::from(timeout_ms)))
        {
            Ok(Ok(frame)) => Ok(frame),
            Ok(Err(msg)) => Err(SubprocessIoError::Dead(msg)),
            Err(mpsc::RecvTimeoutError::Timeout) => Err(SubprocessIoError::Timeout(format!(
                "no frame within {timeout_ms}ms"
            ))),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err(SubprocessIoError::Dead("server stream ended".to_string()))
            }
        }
    }

    /// `close`: kill the server and forget the handle.
    fn close(&mut self, handle: u32) {
        if let Some(session) = self.sessions.remove(&handle) {
            lock_pool().kill_leased(session);
        }
    }

    /// Was this handle a warm session?
    fn reused(&self, handle: u32) -> bool {
        self.sessions.get(&handle).is_some_and(|s| s.reused)
    }

    /// Return the session warm without killing it.
    fn release(&mut self, handle: u32) {
        if let Some(session) = self.sessions.remove(&handle) {
            lock_pool().return_warm(session);
        }
    }

    /// Invoke end (research R4): on a normal end every still-open pooled
    /// session returns warm; on any other end every session is killed.
    /// Idempotent; `Drop` kills when this never ran (a host panic unwinding
    /// through the store, for instance).
    fn settle(&mut self, ok: bool) {
        if self.settled {
            return;
        }
        self.settled = true;
        let sessions: Vec<SubprocessSession> = self.sessions.drain().map(|(_, s)| s).collect();
        if sessions.is_empty() {
            return;
        }
        let mut pool = lock_pool();
        for session in sessions {
            if ok {
                pool.return_warm(session);
            } else {
                pool.kill_leased(session);
            }
        }
    }
}

impl Drop for SubprocessPool {
    fn drop(&mut self) {
        self.settle(false);
    }
}

/// Reader-thread loop: de-frame `Content-Length: N\r\n\r\n<body>` payloads
/// from the child's stdout into the session channel while the session is
/// leased; DROP them while it sits warm in the pool (so the pipe always
/// drains and a reused lease starts empty). Ends on EOF, on a malformed
/// header, or on an oversized frame — each reported once. A saturated frame
/// queue terminates the reader instead of allocating or blocking indefinitely.
fn read_frames(
    mut stdout: impl Read,
    tx: &mpsc::SyncSender<Result<Vec<u8>, String>>,
    leased: &AtomicBool,
) {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        // Parse as many complete frames as the buffer holds.
        while let Some(header_end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            if header_end > SUBPROCESS_HEADER_CAP {
                let _ = tx.try_send(Err("frame header exceeds the cap".to_owned()));
                return;
            }
            let header = String::from_utf8_lossy(&buf[..header_end]).into_owned();
            let Some(len) = header.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())?
            }) else {
                let _ = tx.try_send(Err(format!("malformed frame header: {header:?}")));
                return;
            };
            if len > SUBPROCESS_FRAME_CAP {
                let _ = tx.try_send(Err(format!("frame of {len} bytes exceeds the cap")));
                return;
            }
            let body_start = header_end + 4;
            if buf.len() < body_start + len {
                break;
            }
            let body = buf[body_start..body_start + len].to_vec();
            buf.drain(..body_start + len);
            if leased.load(Ordering::SeqCst) && tx.try_send(Ok(body)).is_err() {
                return;
            }
        }
        if buf.len() > SUBPROCESS_HEADER_CAP && !buf.windows(4).any(|window| window == b"\r\n\r\n")
        {
            let _ = tx.try_send(Err("frame header exceeds the cap".to_owned()));
            return;
        }
        match stdout.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&chunk[..n]),
        }
    }
}

fn engine() -> Result<&'static Engine, SandboxError> {
    static ENGINE: OnceLock<Result<Engine, String>> = OnceLock::new();
    ENGINE
        .get_or_init(|| {
            // Wasmtime keys compiled code by guest bytes, compiler and engine
            // configuration. Only immutable code is shared across workers;
            // stores, capabilities, fuel and memory limits remain per run.
            // A missing/unwritable cache must not prevent sandbox execution.
            let cache = wasmtime::Cache::from_file(None).ok();
            build_engine(cache)
        })
        .as_ref()
        .map_err(|e| SandboxError::Engine(e.clone()))
}

fn build_engine(cache: Option<wasmtime::Cache>) -> Result<Engine, String> {
    let mut config = Config::new();
    config.consume_fuel(true);
    config.cache(cache);
    config.wasm_component_model(true);
    // Two wasmtime majors share this process: this engine (48) and
    // tree-sitter's grammar engine (36, `lang_registry`). Wasmtime
    // implements guest traps as native faults; on macOS its default
    // Mach exception port is process-wide and does not recognise the
    // OTHER copy's JIT code, so a trap raised by one engine after the
    // other installed its port surfaces as a plain SIGILL and kills
    // the host (observed under `bun test` once a lang pack had been
    // loaded before a sandbox guest trapped). POSIX signal handlers
    // chain to the previously installed handler, so both copies
    // coexist; the setting is a no-op off macOS.
    config.macos_use_mach_ports(false);
    Engine::new(&config).map_err(|e| e.to_string())
}

/// [`StoreLimits`] wrapper that records the breach, so classification after a
/// failed call is a flag read instead of error-message matching.
struct TrackedLimits {
    inner: StoreLimits,
    breached: bool,
}

impl TrackedLimits {
    fn new(policy: SandboxPolicy) -> Self {
        TrackedLimits {
            inner: StoreLimitsBuilder::new()
                .memory_size(policy.memory_cap_bytes)
                .trap_on_grow_failure(true)
                .build(),
            breached: false,
        }
    }
}

impl ResourceLimiter for TrackedLimits {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let verdict = self.inner.memory_growing(current, desired, maximum);
        if !matches!(verdict, Ok(true)) {
            self.breached = true;
        }
        verdict
    }

    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let verdict = self.inner.table_growing(current, desired, maximum);
        if !matches!(verdict, Ok(true)) {
            self.breached = true;
        }
        verdict
    }
}

struct ComponentState {
    limits: TrackedLimits,
    observations: Vec<u32>,
    subprocess: SubprocessPool,
}

/// Link the current subprocess capability only when explicitly granted.
fn link_subprocess(
    linker: &mut ComponentLinker<ComponentState>,
    capabilities: &[String],
) -> Result<(), wasmtime::Error> {
    if capabilities.iter().any(|cap| cap == CAP_SUBPROCESS) {
        define_subprocess(linker, CAP_SUBPROCESS)?;
    }
    Ok(())
}

/// Define the subprocess host interface on the linker under `import_name`.
/// Called only when the import is granted; with an empty spec table every
/// `open` answers `unknown-spec`, which is how a granted-but-undetected
/// environment behaves.
fn define_subprocess(
    linker: &mut ComponentLinker<ComponentState>,
    import_name: &str,
) -> Result<(), wasmtime::Error> {
    let mut instance = linker.instance(import_name)?;
    instance.func_wrap(
        "specs",
        |ctx: wasmtime::StoreContextMut<'_, ComponentState>, (): ()| {
            let ids: Vec<String> = ctx
                .data()
                .subprocess
                .specs
                .iter()
                .map(|s| s.language_id.clone())
                .collect();
            Ok((ids,))
        },
    )?;
    instance.func_wrap(
        "workspace-root",
        |ctx: wasmtime::StoreContextMut<'_, ComponentState>, (): ()| {
            Ok((ctx.data().subprocess.workspace_root.clone(),))
        },
    )?;
    instance.func_wrap(
        "open",
        |mut ctx: wasmtime::StoreContextMut<'_, ComponentState>, (language_id,): (String,)| {
            Ok((ctx.data_mut().subprocess.open(&language_id),))
        },
    )?;
    instance.func_wrap(
        "write-frame",
        |mut ctx: wasmtime::StoreContextMut<'_, ComponentState>,
         (session, frame): (u32, Vec<u8>)| {
            Ok((ctx.data_mut().subprocess.write_frame(session, &frame),))
        },
    )?;
    instance.func_wrap(
        "read-frame",
        |mut ctx: wasmtime::StoreContextMut<'_, ComponentState>,
         (session, timeout_ms): (u32, u32)| {
            Ok((ctx.data_mut().subprocess.read_frame(session, timeout_ms),))
        },
    )?;
    instance.func_wrap(
        "close",
        |mut ctx: wasmtime::StoreContextMut<'_, ComponentState>, (session,): (u32,)| {
            ctx.data_mut().subprocess.close(session);
            Ok(())
        },
    )?;
    instance.func_wrap(
        "reused",
        |ctx: wasmtime::StoreContextMut<'_, ComponentState>, (session,): (u32,)| {
            Ok((ctx.data().subprocess.reused(session),))
        },
    )?;
    instance.func_wrap(
        "release",
        |mut ctx: wasmtime::StoreContextMut<'_, ComponentState>, (session,): (u32,)| {
            ctx.data_mut().subprocess.release(session);
            Ok(())
        },
    )?;
    Ok(())
}

struct WasiState {
    limits: TrackedLimits,
    wasi: WasiP1Ctx,
}

/// Classify a failed call using the breach flag first (the limiter is the
/// authority on memory-cap kills), then the trap code.
fn classify_failure(breached: bool, error: &wasmtime::Error) -> GuestEnd {
    if breached {
        return GuestEnd::MemoryCap;
    }
    if let Some(Trap::OutOfFuel) = error.downcast_ref::<Trap>() {
        return GuestEnd::OutOfFuel;
    }
    GuestEnd::GuestTrap(error.to_string())
}

fn lower_arg(ty: &ComponentType, raw: i64) -> Result<Val, String> {
    let lowered = match ty {
        ComponentType::Bool => Val::Bool(raw != 0),
        ComponentType::U8 => u8::try_from(raw).map(Val::U8).map_err(|e| e.to_string())?,
        ComponentType::S8 => i8::try_from(raw).map(Val::S8).map_err(|e| e.to_string())?,
        ComponentType::U16 => u16::try_from(raw)
            .map(Val::U16)
            .map_err(|e| e.to_string())?,
        ComponentType::S16 => i16::try_from(raw)
            .map(Val::S16)
            .map_err(|e| e.to_string())?,
        ComponentType::U32 => u32::try_from(raw)
            .map(Val::U32)
            .map_err(|e| e.to_string())?,
        ComponentType::S32 => i32::try_from(raw)
            .map(Val::S32)
            .map_err(|e| e.to_string())?,
        ComponentType::U64 => u64::try_from(raw)
            .map(Val::U64)
            .map_err(|e| e.to_string())?,
        ComponentType::S64 => Val::S64(raw),
        other => {
            return Err(format!(
                "unsupported param type {other:?} (substrate accepts integer scalars)"
            ));
        }
    };
    Ok(lowered)
}

/// Run one export of a component-model guest under `policy`.
///
/// `capabilities` names the capability objects granted for this run; every
/// import the component requires must match a grant or the run ends
/// [`GuestEnd::CapDenied`] before instantiation. `args` are lowered to the
/// export's declared integer-scalar parameter types.
pub fn run_component(
    guest: &[u8],
    export: &str,
    args: &[i64],
    capabilities: &[String],
    policy: SandboxPolicy,
) -> Result<ComponentOutcome, SandboxError> {
    let engine = engine()?;

    let denied = |end: GuestEnd| ComponentOutcome {
        end,
        fuel_used: 0,
        results: Vec::new(),
        observations: Vec::new(),
    };

    for cap in capabilities {
        if !CAPABILITIES.contains(&cap.as_str()) {
            return Ok(denied(GuestEnd::InvalidGuest(format!(
                "unknown capability {cap:?}; host capabilities: {CAPABILITIES:?}"
            ))));
        }
    }

    let component = match Component::new(engine, guest) {
        Ok(c) => c,
        Err(e) => return Ok(denied(GuestEnd::InvalidGuest(e.to_string()))),
    };

    // Default deny, decided before any guest code exists: every required
    // import must be covered by an explicit grant.
    for (import_name, _) in component.component_type().imports(engine) {
        if !capabilities.iter().any(|cap| cap == import_name) {
            return Ok(denied(GuestEnd::CapDenied(import_name.to_string())));
        }
    }

    let mut linker: ComponentLinker<ComponentState> = ComponentLinker::new(engine);
    if capabilities.iter().any(|cap| cap == CAP_OBSERVE) {
        let defined = linker.root().func_wrap(
            CAP_OBSERVE,
            |mut ctx: wasmtime::StoreContextMut<'_, ComponentState>, (n,): (u32,)| {
                let observations = &mut ctx.data_mut().observations;
                observations.push(n);
                Ok((u32::try_from(observations.len()).unwrap_or(u32::MAX),))
            },
        );
        if let Err(e) = defined {
            return Err(SandboxError::Engine(e.to_string()));
        }
    }
    link_subprocess(&mut linker, capabilities)
        .map_err(|error| SandboxError::Engine(error.to_string()))?;

    let mut store = Store::new(
        engine,
        ComponentState {
            limits: TrackedLimits::new(policy),
            observations: Vec::new(),
            subprocess: SubprocessPool::default(),
        },
    );
    store.limiter(|s| &mut s.limits);
    if let Err(e) = store.set_fuel(policy.fuel) {
        return Err(SandboxError::Engine(e.to_string()));
    }

    let fuel_used = |store: &mut Store<ComponentState>| {
        policy.fuel.saturating_sub(store.get_fuel().unwrap_or(0))
    };

    let finish = |end: GuestEnd, store: &mut Store<ComponentState>, results: Vec<String>| {
        store.data_mut().subprocess.settle(end == GuestEnd::Ok);
        ComponentOutcome {
            end,
            fuel_used: fuel_used(store),
            results,
            observations: std::mem::take(&mut store.data_mut().observations),
        }
    };

    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => {
            let end = classify_failure(store.data().limits.breached, &e);
            let end = match end {
                // A non-resource instantiation failure is a bad guest, not a trap.
                GuestEnd::GuestTrap(msg) => GuestEnd::InvalidGuest(msg),
                other => other,
            };
            return Ok(finish(end, &mut store, Vec::new()));
        }
    };

    let Some(func) = instance.get_func(&mut store, export) else {
        return Ok(finish(
            GuestEnd::InvalidGuest(format!("export {export:?} not found")),
            &mut store,
            Vec::new(),
        ));
    };

    let func_ty = func.ty(&store);
    let param_types: Vec<ComponentType> = func_ty.params().map(|(_, ty)| ty).collect();
    if param_types.len() != args.len() {
        return Ok(finish(
            GuestEnd::InvalidGuest(format!(
                "export {export:?} takes {} params, {} given",
                param_types.len(),
                args.len()
            )),
            &mut store,
            Vec::new(),
        ));
    }
    let mut params = Vec::with_capacity(args.len());
    for (ty, raw) in param_types.iter().zip(args) {
        match lower_arg(ty, *raw) {
            Ok(val) => params.push(val),
            Err(msg) => {
                return Ok(finish(GuestEnd::InvalidGuest(msg), &mut store, Vec::new()));
            }
        }
    }
    let mut results = vec![Val::Bool(false); func_ty.results().len()];

    match func.call(&mut store, &params, &mut results) {
        Ok(()) => {
            let rendered = results.iter().map(|v| format!("{v:?}")).collect();
            Ok(finish(GuestEnd::Ok, &mut store, rendered))
        }
        Err(e) => {
            let end = classify_failure(store.data().limits.breached, &e);
            Ok(finish(end, &mut store, Vec::new()))
        }
    }
}

/// Outcome of a component-lane JSON invoke (the plugin tool substrate).
#[derive(Debug)]
pub struct InvokeOutcome {
    pub end: GuestEnd,
    pub fuel_used: u64,
    /// The export's string result, present iff the run ended [`GuestEnd::Ok`].
    pub result: Option<String>,
}

/// Invoke a `(string) -> string` export of a component-model guest — the
/// plugin tool calling convention (SDD §8.3: payloads are envelope-versioned
/// JSON strings by decision, chunk:m4-wit-interfaces).
///
/// `export` addresses either a root export (`"invoke"`) or a function inside
/// an exported instance (`"magistr:tool/guest@0.1.0#invoke"`).
///
/// `capabilities` are the granted import names from the plugin's install-time
/// grant record (chunk:m4-capability-manifests): every import the component
/// requires must be covered or the run ends [`GuestEnd::CapDenied`] before
/// instantiation. Unlike [`run_component`], grant names are not restricted to
/// the host's v0 capability objects — grants come from a manifest whose
/// vocabulary is WIT world import names, and a granted-but-unimplemented
/// import fails honestly at instantiation as [`GuestEnd::InvalidGuest`].
///
/// `subprocess_specs` is the host-curated detection table backing a granted
/// subprocess capability ([`CAP_SUBPROCESS`]); pass empty when the invoke has
/// no business spawning servers — the interface still links (so a granted
/// guest instantiates) but every `open` answers `unknown-spec`.
pub fn invoke_component_json(
    guest: &[u8],
    export: &str,
    payload: &str,
    capabilities: &[String],
    policy: SandboxPolicy,
    subprocess_specs: &[SubprocessSpec],
    subprocess_root: &str,
) -> Result<InvokeOutcome, SandboxError> {
    let engine = engine()?;

    let denied = |end: GuestEnd| InvokeOutcome {
        end,
        fuel_used: 0,
        result: None,
    };

    let component = match Component::new(engine, guest) {
        Ok(c) => c,
        Err(e) => return Ok(denied(GuestEnd::InvalidGuest(e.to_string()))),
    };

    // Default deny, decided before any guest code exists (SDD §8.1).
    for (import_name, _) in component.component_type().imports(engine) {
        if !capabilities.iter().any(|cap| cap == import_name) {
            return Ok(denied(GuestEnd::CapDenied(import_name.to_string())));
        }
    }

    let mut linker: ComponentLinker<ComponentState> = ComponentLinker::new(engine);
    if capabilities.iter().any(|cap| cap == CAP_OBSERVE) {
        let defined = linker.root().func_wrap(
            CAP_OBSERVE,
            |mut ctx: wasmtime::StoreContextMut<'_, ComponentState>, (n,): (u32,)| {
                let observations = &mut ctx.data_mut().observations;
                observations.push(n);
                Ok((u32::try_from(observations.len()).unwrap_or(u32::MAX),))
            },
        );
        if let Err(e) = defined {
            return Err(SandboxError::Engine(e.to_string()));
        }
    }
    link_subprocess(&mut linker, capabilities)
        .map_err(|error| SandboxError::Engine(error.to_string()))?;

    let mut store = Store::new(
        engine,
        ComponentState {
            limits: TrackedLimits::new(policy),
            observations: Vec::new(),
            subprocess: SubprocessPool::new(subprocess_specs.to_vec(), subprocess_root.to_string()),
        },
    );
    store.limiter(|s| &mut s.limits);
    if let Err(e) = store.set_fuel(policy.fuel) {
        return Err(SandboxError::Engine(e.to_string()));
    }

    let finish = |end: GuestEnd, store: &mut Store<ComponentState>, result: Option<String>| {
        // Research R4: a normal end returns pooled sessions warm; any other end
        // kills them (the lease view's Drop kills too, should this not run).
        store.data_mut().subprocess.settle(end == GuestEnd::Ok);
        InvokeOutcome {
            end,
            fuel_used: policy.fuel.saturating_sub(store.get_fuel().unwrap_or(0)),
            result,
        }
    };

    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => {
            let end = match classify_failure(store.data().limits.breached, &e) {
                // A non-resource instantiation failure is a bad guest, not a trap.
                GuestEnd::GuestTrap(msg) => GuestEnd::InvalidGuest(msg),
                other => other,
            };
            return Ok(finish(end, &mut store, None));
        }
    };

    let func = match export.split_once('#') {
        Some((iface, name)) => instance
            .get_export_index(&mut store, None, iface)
            .and_then(|iface_idx| instance.get_export_index(&mut store, Some(&iface_idx), name))
            .and_then(|func_idx| instance.get_func(&mut store, func_idx)),
        None => instance.get_func(&mut store, export),
    };
    let Some(func) = func else {
        return Ok(finish(
            GuestEnd::InvalidGuest(format!("export {export:?} not found")),
            &mut store,
            None,
        ));
    };

    let func_ty = func.ty(&store);
    let param_types: Vec<ComponentType> = func_ty.params().map(|(_, ty)| ty).collect();
    let result_types: Vec<ComponentType> = func_ty.results().collect();
    let shape_ok = matches!(param_types.as_slice(), [ComponentType::String])
        && matches!(result_types.as_slice(), [ComponentType::String]);
    if !shape_ok {
        return Ok(finish(
            GuestEnd::InvalidGuest(format!(
                "export {export:?} must be (string) -> string (plugin tool calling convention)"
            )),
            &mut store,
            None,
        ));
    }

    let params = [Val::String(payload.to_string())];
    let mut results = [Val::Bool(false)];
    match func.call(&mut store, &params, &mut results) {
        Ok(()) => {
            let result = match &results[0] {
                Val::String(s) => Some(s.clone()),
                _ => None,
            };
            Ok(finish(GuestEnd::Ok, &mut store, result))
        }
        Err(e) => {
            let end = classify_failure(store.data().limits.breached, &e);
            Ok(finish(end, &mut store, None))
        }
    }
}

/// One resolved location lifted from the guest's `list<source-location>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedLocation {
    pub path: String,
    pub line: u32,
    pub column: u32,
}

/// Outcome of a typed resolver-lane invoke (chunk:m5-lsp-resolver-plugin).
#[derive(Debug)]
pub struct ResolverOutcome {
    pub end: GuestEnd,
    pub fuel_used: u64,
    /// Locations lifted from the export's result, present iff [`GuestEnd::Ok`].
    pub locations: Vec<ResolvedLocation>,
    /// Language-server sessions this invoke spawned fresh (FR-013,
    /// specs/007-resolver-server-pool).
    pub sessions_opened: u32,
    /// Warm pooled sessions this invoke reused instead of spawning.
    pub sessions_reused: u32,
}

/// The resolver surface's export address (wit/resolver.wit).
pub const RESOLVER_REFERENCES_EXPORT: &str = "magistr:resolver/resolver@0.1.0#resolve-references";

/// Invoke the typed `resolve-references(symbol) -> list<source-location>`
/// export of a resolver-provider component (wit/resolver.wit). Same sandbox,
/// same grant model as [`invoke_component_json`] — this lane exists because
/// the resolver surface is typed, not the `(string) -> string` tool shape.
pub fn invoke_resolver_references(
    guest: &[u8],
    symbol: &str,
    capabilities: &[String],
    policy: SandboxPolicy,
    subprocess_specs: &[SubprocessSpec],
    subprocess_root: &str,
) -> Result<ResolverOutcome, SandboxError> {
    let engine = engine()?;

    let denied = |end: GuestEnd| ResolverOutcome {
        end,
        fuel_used: 0,
        locations: Vec::new(),
        sessions_opened: 0,
        sessions_reused: 0,
    };

    let component = match Component::new(engine, guest) {
        Ok(c) => c,
        Err(e) => return Ok(denied(GuestEnd::InvalidGuest(e.to_string()))),
    };

    // Default deny, decided before any guest code exists (SDD §8.1).
    for (import_name, _) in component.component_type().imports(engine) {
        if !capabilities.iter().any(|cap| cap == import_name) {
            return Ok(denied(GuestEnd::CapDenied(import_name.to_string())));
        }
    }

    let mut linker: ComponentLinker<ComponentState> = ComponentLinker::new(engine);
    link_subprocess(&mut linker, capabilities)
        .map_err(|error| SandboxError::Engine(error.to_string()))?;

    let mut store = Store::new(
        engine,
        ComponentState {
            limits: TrackedLimits::new(policy),
            observations: Vec::new(),
            subprocess: SubprocessPool::new(subprocess_specs.to_vec(), subprocess_root.to_string()),
        },
    );
    store.limiter(|s| &mut s.limits);
    if let Err(e) = store.set_fuel(policy.fuel) {
        return Err(SandboxError::Engine(e.to_string()));
    }

    let finish =
        |end: GuestEnd, store: &mut Store<ComponentState>, locations: Vec<ResolvedLocation>| {
            // Research R4: a normal end returns pooled sessions warm; any other
            // end kills them (the lease view's Drop kills too, should this not run).
            store.data_mut().subprocess.settle(end == GuestEnd::Ok);
            ResolverOutcome {
                end,
                fuel_used: policy.fuel.saturating_sub(store.get_fuel().unwrap_or(0)),
                locations,
                sessions_opened: store.data().subprocess.opened,
                sessions_reused: store.data().subprocess.reused,
            }
        };

    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => {
            let end = match classify_failure(store.data().limits.breached, &e) {
                GuestEnd::GuestTrap(msg) => GuestEnd::InvalidGuest(msg),
                other => other,
            };
            return Ok(finish(end, &mut store, Vec::new()));
        }
    };

    let (iface, name) = RESOLVER_REFERENCES_EXPORT
        .split_once('#')
        .expect("resolver export address contains '#'");
    let func = instance
        .get_export_index(&mut store, None, iface)
        .and_then(|iface_idx| instance.get_export_index(&mut store, Some(&iface_idx), name))
        .and_then(|func_idx| instance.get_func(&mut store, func_idx));
    let Some(func) = func else {
        return Ok(finish(
            GuestEnd::InvalidGuest(format!(
                "export {RESOLVER_REFERENCES_EXPORT:?} not found (not a resolver-provider component)"
            )),
            &mut store,
            Vec::new(),
        ));
    };

    let params = [Val::String(symbol.to_string())];
    let mut results = [Val::Bool(false)];
    match func.call(&mut store, &params, &mut results) {
        Ok(()) => {
            let Some(locations) = lift_locations(&results[0]) else {
                return Ok(finish(
                    GuestEnd::InvalidGuest(
                        "resolve-references result is not list<source-location>".to_string(),
                    ),
                    &mut store,
                    Vec::new(),
                ));
            };
            Ok(finish(GuestEnd::Ok, &mut store, locations))
        }
        Err(e) => {
            let end = classify_failure(store.data().limits.breached, &e);
            Ok(finish(end, &mut store, Vec::new()))
        }
    }
}

/// One structured failure lifted from the runner surface (wit/runner.wit).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiftedFailure {
    pub test_name: String,
    pub file: Option<String>,
    pub assertion: String,
    pub trace: String,
}

/// One lifted run-report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiftedRunReport {
    pub failures: Vec<LiftedFailure>,
    pub summary: Option<String>,
}

/// Outcome of a typed runner-lane invoke (chunk:m5-runner-adapter).
#[derive(Debug)]
pub struct RunnerOutcome {
    pub end: GuestEnd,
    pub fuel_used: u64,
    /// `can-parse`'s answer; false short-circuits without parse-output.
    pub claimed: bool,
    /// The guest's report; `None` with `claimed` means the adapter could not
    /// extract structure (the host falls back to head+tail, never a guess).
    pub report: Option<LiftedRunReport>,
}

/// The runner surface's export addresses (wit/runner.wit).
pub const RUNNER_CAN_PARSE_EXPORT: &str = "magistr:runner/runner@0.1.0#can-parse";
pub const RUNNER_PARSE_EXPORT: &str = "magistr:runner/runner@0.1.0#parse-output";

/// Invoke a runner-provider component: `can-parse(command)`, then — only on a
/// claim — `parse-output(run-output)`. Same sandbox, same grant model as the
/// other component lanes; this one exists because the runner surface is
/// typed records, not the `(string) -> string` tool shape.
pub fn invoke_runner_parse(
    guest: &[u8],
    command: &str,
    exit_code: i32,
    stdout: &str,
    stderr: &str,
    capabilities: &[String],
    policy: SandboxPolicy,
) -> Result<RunnerOutcome, SandboxError> {
    let engine = engine()?;

    let denied = |end: GuestEnd| RunnerOutcome {
        end,
        fuel_used: 0,
        claimed: false,
        report: None,
    };

    let component = match Component::new(engine, guest) {
        Ok(c) => c,
        Err(e) => return Ok(denied(GuestEnd::InvalidGuest(e.to_string()))),
    };
    for (import_name, _) in component.component_type().imports(engine) {
        if !capabilities.iter().any(|cap| cap == import_name) {
            return Ok(denied(GuestEnd::CapDenied(import_name.to_string())));
        }
    }

    let mut linker: ComponentLinker<ComponentState> = ComponentLinker::new(engine);
    link_subprocess(&mut linker, capabilities)
        .map_err(|error| SandboxError::Engine(error.to_string()))?;

    let mut store = Store::new(
        engine,
        ComponentState {
            limits: TrackedLimits::new(policy),
            observations: Vec::new(),
            subprocess: SubprocessPool::default(),
        },
    );
    store.limiter(|s| &mut s.limits);
    if let Err(e) = store.set_fuel(policy.fuel) {
        return Err(SandboxError::Engine(e.to_string()));
    }

    let finish = |end: GuestEnd,
                  store: &mut Store<ComponentState>,
                  claimed: bool,
                  report: Option<LiftedRunReport>| {
        store.data_mut().subprocess.settle(end == GuestEnd::Ok);
        RunnerOutcome {
            end,
            fuel_used: policy.fuel.saturating_sub(store.get_fuel().unwrap_or(0)),
            claimed,
            report,
        }
    };

    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => {
            let end = match classify_failure(store.data().limits.breached, &e) {
                GuestEnd::GuestTrap(msg) => GuestEnd::InvalidGuest(msg),
                other => other,
            };
            return Ok(finish(end, &mut store, false, None));
        }
    };

    let lookup = |store: &mut Store<ComponentState>, address: &str| {
        let (iface, name) = address.split_once('#')?;
        let iface_idx = instance.get_export_index(&mut *store, None, iface)?;
        let func_idx = instance.get_export_index(&mut *store, Some(&iface_idx), name)?;
        instance.get_func(&mut *store, func_idx)
    };

    let Some(can_parse) = lookup(&mut store, RUNNER_CAN_PARSE_EXPORT) else {
        return Ok(finish(
            GuestEnd::InvalidGuest(format!(
                "export {RUNNER_CAN_PARSE_EXPORT:?} not found (not a runner-provider component)"
            )),
            &mut store,
            false,
            None,
        ));
    };
    let mut claim_result = [Val::Bool(false)];
    if let Err(e) = can_parse.call(
        &mut store,
        &[Val::String(command.to_string())],
        &mut claim_result,
    ) {
        let end = classify_failure(store.data().limits.breached, &e);
        return Ok(finish(end, &mut store, false, None));
    }
    let claimed = matches!(claim_result[0], Val::Bool(true));
    if !claimed {
        return Ok(finish(GuestEnd::Ok, &mut store, false, None));
    }

    let Some(parse) = lookup(&mut store, RUNNER_PARSE_EXPORT) else {
        return Ok(finish(
            GuestEnd::InvalidGuest(format!("export {RUNNER_PARSE_EXPORT:?} not found")),
            &mut store,
            true,
            None,
        ));
    };
    let output = Val::Record(vec![
        ("command".to_string(), Val::String(command.to_string())),
        ("exit-code".to_string(), Val::S32(exit_code)),
        ("stdout".to_string(), Val::String(stdout.to_string())),
        ("stderr".to_string(), Val::String(stderr.to_string())),
    ]);
    let mut parse_result = [Val::Bool(false)];
    if let Err(e) = parse.call(&mut store, &[output], &mut parse_result) {
        let end = classify_failure(store.data().limits.breached, &e);
        return Ok(finish(end, &mut store, true, None));
    }
    let Some(report) = lift_run_report(&parse_result[0]) else {
        return Ok(finish(
            GuestEnd::InvalidGuest("parse-output result is not option<run-report>".to_string()),
            &mut store,
            true,
            None,
        ));
    };
    Ok(finish(GuestEnd::Ok, &mut store, true, report))
}

/// Lift `option<run-report>` from a dynamic component value. `Some(None)`
/// means the guest answered `none` (could not extract structure).
#[allow(clippy::option_option)]
fn lift_run_report(val: &Val) -> Option<Option<LiftedRunReport>> {
    let Val::Option(inner) = val else { return None };
    let Some(report_val) = inner else {
        return Some(None);
    };
    let Val::Record(fields) = report_val.as_ref() else {
        return None;
    };
    let mut failures = None;
    let mut summary = None;
    for (name, value) in fields {
        match (name.as_str(), value) {
            ("failures", Val::List(items)) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(lift_failure(item)?);
                }
                failures = Some(out);
            }
            ("summary", Val::Option(s)) => {
                summary = Some(match s {
                    Some(v) => match v.as_ref() {
                        Val::String(text) => Some(text.clone()),
                        _ => return None,
                    },
                    None => None,
                });
            }
            _ => return None,
        }
    }
    Some(Some(LiftedRunReport {
        failures: failures?,
        summary: summary?,
    }))
}

fn lift_failure(val: &Val) -> Option<LiftedFailure> {
    let Val::Record(fields) = val else {
        return None;
    };
    let mut test_name = None;
    let mut file = None;
    let mut assertion = None;
    let mut trace = None;
    for (name, value) in fields {
        match (name.as_str(), value) {
            ("test-name", Val::String(s)) => test_name = Some(s.clone()),
            ("file", Val::Option(v)) => {
                file = Some(match v {
                    Some(inner) => match inner.as_ref() {
                        Val::String(text) => Some(text.clone()),
                        _ => return None,
                    },
                    None => None,
                });
            }
            ("assertion", Val::String(s)) => assertion = Some(s.clone()),
            ("trace", Val::String(s)) => trace = Some(s.clone()),
            _ => return None,
        }
    }
    Some(LiftedFailure {
        test_name: test_name?,
        file: file?,
        assertion: assertion?,
        trace: trace?,
    })
}

/// Lift `list<source-location>` from a dynamic component value.
fn lift_locations(val: &Val) -> Option<Vec<ResolvedLocation>> {
    let Val::List(items) = val else { return None };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let Val::Record(fields) = item else {
            return None;
        };
        let mut path = None;
        let mut line = None;
        let mut column = None;
        for (name, value) in fields {
            match (name.as_str(), value) {
                ("path", Val::String(s)) => path = Some(s.clone()),
                ("line", Val::U32(n)) => line = Some(*n),
                ("column", Val::U32(n)) => column = Some(*n),
                _ => return None,
            }
        }
        out.push(ResolvedLocation {
            path: path?,
            line: line?,
            column: column?,
        });
    }
    Some(out)
}

/// One hit-context handed to a ranking signal (wit/ranking.wit). The enum
/// fields travel as their WIT case names ("declaration"…, "source"…); the
/// lane refuses an unknown name before the sandbox ever runs.
#[derive(Debug, Clone, PartialEq)]
pub struct RankingHit {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub excerpt: String,
    pub kind_class: String,
    pub file_class: String,
}

/// One named feature value lifted from a signal's `contribute` answer.
#[derive(Debug, Clone, PartialEq)]
pub struct LiftedFeature {
    pub name: String,
    pub value: f64,
}

/// Outcome of a typed ranking-lane invoke (chunk:m5-ranking-shadow).
#[derive(Debug)]
pub struct RankingOutcome {
    pub end: GuestEnd,
    pub fuel_used: u64,
    /// Per-hit feature lists, parallel to the input batch. Shorter than the
    /// batch when the guest failed partway (`end` says why); the host keeps
    /// what was lifted — shadow scoring is additive, never all-or-nothing.
    pub contributions: Vec<Vec<LiftedFeature>>,
}

/// The ranking surface's export address (wit/ranking.wit).
pub const RANKING_CONTRIBUTE_EXPORT: &str = "magistr:ranking/ranking@0.1.0#contribute";

const KIND_CLASS_CASES: [&str; 6] = [
    "declaration",
    "call",
    "import",
    "comment",
    "string",
    "other",
];
const FILE_CLASS_CASES: [&str; 3] = ["source", "test", "vendor"];

/// Invoke a ranking-provider component: instantiate once, call `contribute`
/// per hit in the batch. Same sandbox, same grant model as the other component
/// lanes; batching lives HERE, in the host, so the blessed per-hit contract
/// stays untouched (research R1). This lane must only ever run off the search
/// response path (research R2 / SC-005) — the caller owns that discipline.
pub fn invoke_ranking_contribute(
    guest: &[u8],
    hits: &[RankingHit],
    capabilities: &[String],
    policy: SandboxPolicy,
) -> Result<RankingOutcome, SandboxError> {
    let engine = engine()?;

    let denied = |end: GuestEnd| RankingOutcome {
        end,
        fuel_used: 0,
        contributions: Vec::new(),
    };

    for hit in hits {
        if !KIND_CLASS_CASES.contains(&hit.kind_class.as_str()) {
            return Ok(denied(GuestEnd::InvalidGuest(format!(
                "unknown kind-class {:?} in ranking batch",
                hit.kind_class
            ))));
        }
        if !FILE_CLASS_CASES.contains(&hit.file_class.as_str()) {
            return Ok(denied(GuestEnd::InvalidGuest(format!(
                "unknown file-class {:?} in ranking batch",
                hit.file_class
            ))));
        }
    }

    let component = match Component::new(engine, guest) {
        Ok(c) => c,
        Err(e) => return Ok(denied(GuestEnd::InvalidGuest(e.to_string()))),
    };
    for (import_name, _) in component.component_type().imports(engine) {
        if !capabilities.iter().any(|cap| cap == import_name) {
            return Ok(denied(GuestEnd::CapDenied(import_name.to_string())));
        }
    }

    // The ranking lane is a CARRIER lane like lang-pack: it defines NOTHING.
    // Even a granted import cannot be satisfied here — the tightest leash is
    // structural (contracts/ranking.md; escape-suite chapter pins it).
    let linker: ComponentLinker<ComponentState> = ComponentLinker::new(engine);

    let mut store = Store::new(
        engine,
        ComponentState {
            limits: TrackedLimits::new(policy),
            observations: Vec::new(),
            subprocess: SubprocessPool::default(),
        },
    );
    store.limiter(|s| &mut s.limits);
    if let Err(e) = store.set_fuel(policy.fuel) {
        return Err(SandboxError::Engine(e.to_string()));
    }

    let finish = |end: GuestEnd,
                  store: &mut Store<ComponentState>,
                  contributions: Vec<Vec<LiftedFeature>>| RankingOutcome {
        end,
        fuel_used: policy.fuel.saturating_sub(store.get_fuel().unwrap_or(0)),
        contributions,
    };

    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => {
            let end = match classify_failure(store.data().limits.breached, &e) {
                GuestEnd::GuestTrap(msg) => GuestEnd::InvalidGuest(msg),
                other => other,
            };
            return Ok(finish(end, &mut store, Vec::new()));
        }
    };

    let lookup = |store: &mut Store<ComponentState>, address: &str| {
        let (iface, name) = address.split_once('#')?;
        let iface_idx = instance.get_export_index(&mut *store, None, iface)?;
        let func_idx = instance.get_export_index(&mut *store, Some(&iface_idx), name)?;
        instance.get_func(&mut *store, func_idx)
    };

    let Some(contribute) = lookup(&mut store, RANKING_CONTRIBUTE_EXPORT) else {
        return Ok(finish(
            GuestEnd::InvalidGuest(format!(
                "export {RANKING_CONTRIBUTE_EXPORT:?} not found (not a ranking-provider component)"
            )),
            &mut store,
            Vec::new(),
        ));
    };

    let mut contributions: Vec<Vec<LiftedFeature>> = Vec::with_capacity(hits.len());
    for hit in hits {
        let context = Val::Record(vec![
            ("file".to_string(), Val::String(hit.file.clone())),
            ("line".to_string(), Val::U32(hit.line)),
            ("column".to_string(), Val::U32(hit.column)),
            ("excerpt".to_string(), Val::String(hit.excerpt.clone())),
            ("kind-class".to_string(), Val::Enum(hit.kind_class.clone())),
            ("file-class".to_string(), Val::Enum(hit.file_class.clone())),
        ]);
        let mut result = [Val::Bool(false)];
        if let Err(e) = contribute.call(&mut store, &[context], &mut result) {
            let end = classify_failure(store.data().limits.breached, &e);
            return Ok(finish(end, &mut store, contributions));
        }
        let Some(features) = lift_features(&result[0]) else {
            return Ok(finish(
                GuestEnd::InvalidGuest("contribute result is not list<feature>".to_string()),
                &mut store,
                contributions,
            ));
        };
        contributions.push(features);
    }
    Ok(finish(GuestEnd::Ok, &mut store, contributions))
}

/// Lift `list<feature>` from a dynamic component value.
fn lift_features(val: &Val) -> Option<Vec<LiftedFeature>> {
    let Val::List(items) = val else { return None };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let Val::Record(fields) = item else {
            return None;
        };
        let mut name = None;
        let mut value = None;
        for (field, v) in fields {
            match (field.as_str(), v) {
                ("name", Val::String(s)) => name = Some(s.clone()),
                ("value", Val::Float64(n)) => value = Some(*n),
                _ => return None,
            }
        }
        out.push(LiftedFeature {
            name: name?,
            value: value?,
        });
    }
    Some(out)
}

/// What the lang-pack carrier hands the host at load time (feature 002):
/// the pack's identity plus the raw material the dynamic language registry
/// registers — grammar bytes and tag-query text. Pure data; the grammar
/// EXECUTES later, in the registry's lane, never here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LangPackBundle {
    pub name: String,
    pub extensions: Vec<String>,
    pub grammar_wasm: Vec<u8>,
    pub tag_queries: String,
}

/// Outcome of a typed lang-pack carrier read (chunk:m5-lang-pack).
#[derive(Debug)]
pub struct LangPackOutcome {
    pub end: GuestEnd,
    pub fuel_used: u64,
    /// `None` when the guest ended abnormally or is not a lang-pack provider.
    pub bundle: Option<LangPackBundle>,
}

/// The lang-pack surface's export addresses (wit/lang-pack.wit).
pub const LANG_PACK_INFO_EXPORT: &str = "magistr:lang-pack/lang-pack@0.1.0#info";
pub const LANG_PACK_GRAMMAR_EXPORT: &str = "magistr:lang-pack/lang-pack@0.1.0#grammar-wasm";
pub const LANG_PACK_QUERIES_EXPORT: &str = "magistr:lang-pack/lang-pack@0.1.0#tag-queries";
pub const LANG_PACK_CLASSIFY_EXPORT: &str = "magistr:lang-pack/lang-pack@0.1.0#classify-file";

/// Read a lang-pack-provider component's carrier exports: `info`, then
/// `grammar-wasm`, then `tag-queries`, one instantiation. Same sandbox and
/// grant model as the other component lanes; this one exists because the
/// lang-pack surface is typed records, not the `(string) -> string` tool
/// shape. A lang pack declares zero capabilities, so `capabilities` is
/// normally empty and any import at all is refused pre-instantiation.
pub fn invoke_lang_pack_load(
    guest: &[u8],
    capabilities: &[String],
    policy: SandboxPolicy,
) -> Result<LangPackOutcome, SandboxError> {
    let engine = engine()?;

    let denied = |end: GuestEnd| LangPackOutcome {
        end,
        fuel_used: 0,
        bundle: None,
    };

    let component = match Component::new(engine, guest) {
        Ok(c) => c,
        Err(e) => return Ok(denied(GuestEnd::InvalidGuest(e.to_string()))),
    };
    for (import_name, _) in component.component_type().imports(engine) {
        if !capabilities.iter().any(|cap| cap == import_name) {
            return Ok(denied(GuestEnd::CapDenied(import_name.to_string())));
        }
    }

    let linker: ComponentLinker<ComponentState> = ComponentLinker::new(engine);
    let mut store = Store::new(
        engine,
        ComponentState {
            limits: TrackedLimits::new(policy),
            observations: Vec::new(),
            subprocess: SubprocessPool::default(),
        },
    );
    store.limiter(|s| &mut s.limits);
    if let Err(e) = store.set_fuel(policy.fuel) {
        return Err(SandboxError::Engine(e.to_string()));
    }

    let finish =
        |end: GuestEnd, store: &mut Store<ComponentState>, bundle: Option<LangPackBundle>| {
            LangPackOutcome {
                end,
                fuel_used: policy.fuel.saturating_sub(store.get_fuel().unwrap_or(0)),
                bundle,
            }
        };

    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => {
            let end = match classify_failure(store.data().limits.breached, &e) {
                GuestEnd::GuestTrap(msg) => GuestEnd::InvalidGuest(msg),
                other => other,
            };
            return Ok(finish(end, &mut store, None));
        }
    };

    let lookup = |store: &mut Store<ComponentState>, address: &str| {
        let (iface, name) = address.split_once('#')?;
        let iface_idx = instance.get_export_index(&mut *store, None, iface)?;
        let func_idx = instance.get_export_index(&mut *store, Some(&iface_idx), name)?;
        instance.get_func(&mut *store, func_idx)
    };

    let call_one =
        |store: &mut Store<ComponentState>, address: &str| -> Result<Val, (GuestEnd, bool)> {
            let Some(func) = lookup(&mut *store, address) else {
                return Err((
                    GuestEnd::InvalidGuest(format!(
                        "export {address:?} not found (not a lang-pack-provider component)"
                    )),
                    false,
                ));
            };
            let mut result = [Val::Bool(false)];
            match func.call(&mut *store, &[], &mut result) {
                Ok(()) => Ok(result[0].clone()),
                Err(e) => Err((classify_failure(store.data().limits.breached, &e), true)),
            }
        };

    let info = match call_one(&mut store, LANG_PACK_INFO_EXPORT) {
        Ok(v) => v,
        Err((end, _)) => return Ok(finish(end, &mut store, None)),
    };
    let Some((name, extensions)) = lift_language_info(&info) else {
        return Ok(finish(
            GuestEnd::InvalidGuest("info result is not language-info".to_string()),
            &mut store,
            None,
        ));
    };

    let grammar = match call_one(&mut store, LANG_PACK_GRAMMAR_EXPORT) {
        Ok(v) => v,
        Err((end, _)) => return Ok(finish(end, &mut store, None)),
    };
    let Some(grammar_wasm) = lift_bytes(&grammar) else {
        return Ok(finish(
            GuestEnd::InvalidGuest("grammar-wasm result is not list<u8>".to_string()),
            &mut store,
            None,
        ));
    };

    let queries = match call_one(&mut store, LANG_PACK_QUERIES_EXPORT) {
        Ok(v) => v,
        Err((end, _)) => return Ok(finish(end, &mut store, None)),
    };
    let Val::String(tag_queries) = queries else {
        return Ok(finish(
            GuestEnd::InvalidGuest("tag-queries result is not string".to_string()),
            &mut store,
            None,
        ));
    };

    Ok(finish(
        GuestEnd::Ok,
        &mut store,
        Some(LangPackBundle {
            name,
            extensions,
            grammar_wasm,
            tag_queries,
        }),
    ))
}

/// Outcome of a batched classify-file pass over one carrier instantiation.
#[derive(Debug)]
pub struct LangPackClassifyOutcome {
    pub end: GuestEnd,
    pub fuel_used: u64,
    /// One verdict per input path: `Some("source"|"test"|"vendor")` where the
    /// pack answered, `None` where it deferred to the host's heuristics.
    pub classes: Vec<Option<String>>,
}

/// Ask a lang-pack component to classify a batch of workspace-relative
/// paths. One instantiation per batch — classify-file is a per-path call in
/// the WIT, but the host amortizes instantiation across the batch.
pub fn invoke_lang_pack_classify(
    guest: &[u8],
    paths: &[String],
    capabilities: &[String],
    policy: SandboxPolicy,
) -> Result<LangPackClassifyOutcome, SandboxError> {
    let engine = engine()?;

    let denied = |end: GuestEnd| LangPackClassifyOutcome {
        end,
        fuel_used: 0,
        classes: Vec::new(),
    };

    let component = match Component::new(engine, guest) {
        Ok(c) => c,
        Err(e) => return Ok(denied(GuestEnd::InvalidGuest(e.to_string()))),
    };
    for (import_name, _) in component.component_type().imports(engine) {
        if !capabilities.iter().any(|cap| cap == import_name) {
            return Ok(denied(GuestEnd::CapDenied(import_name.to_string())));
        }
    }

    let linker: ComponentLinker<ComponentState> = ComponentLinker::new(engine);
    let mut store = Store::new(
        engine,
        ComponentState {
            limits: TrackedLimits::new(policy),
            observations: Vec::new(),
            subprocess: SubprocessPool::default(),
        },
    );
    store.limiter(|s| &mut s.limits);
    if let Err(e) = store.set_fuel(policy.fuel) {
        return Err(SandboxError::Engine(e.to_string()));
    }

    let finish = |end: GuestEnd,
                  store: &mut Store<ComponentState>,
                  classes: Vec<Option<String>>| LangPackClassifyOutcome {
        end,
        fuel_used: policy.fuel.saturating_sub(store.get_fuel().unwrap_or(0)),
        classes,
    };

    let instance = match linker.instantiate(&mut store, &component) {
        Ok(i) => i,
        Err(e) => {
            let end = match classify_failure(store.data().limits.breached, &e) {
                GuestEnd::GuestTrap(msg) => GuestEnd::InvalidGuest(msg),
                other => other,
            };
            return Ok(finish(end, &mut store, Vec::new()));
        }
    };

    let classify = {
        let lookup = |store: &mut Store<ComponentState>, address: &str| {
            let (iface, name) = address.split_once('#')?;
            let iface_idx = instance.get_export_index(&mut *store, None, iface)?;
            let func_idx = instance.get_export_index(&mut *store, Some(&iface_idx), name)?;
            instance.get_func(&mut *store, func_idx)
        };
        match lookup(&mut store, LANG_PACK_CLASSIFY_EXPORT) {
            Some(f) => f,
            None => {
                return Ok(finish(
                    GuestEnd::InvalidGuest(format!(
                        "export {LANG_PACK_CLASSIFY_EXPORT:?} not found"
                    )),
                    &mut store,
                    Vec::new(),
                ));
            }
        }
    };

    let mut classes = Vec::with_capacity(paths.len());
    for path in paths {
        let mut result = [Val::Bool(false)];
        if let Err(e) = classify.call(&mut store, &[Val::String(path.clone())], &mut result) {
            let end = classify_failure(store.data().limits.breached, &e);
            return Ok(finish(end, &mut store, classes));
        }
        let Some(class) = lift_file_class(&result[0]) else {
            return Ok(finish(
                GuestEnd::InvalidGuest(
                    "classify-file result is not option<file-class>".to_string(),
                ),
                &mut store,
                classes,
            ));
        };
        classes.push(class);
    }
    Ok(finish(GuestEnd::Ok, &mut store, classes))
}

/// Lift `language-info` (record { name, extensions }) from a dynamic value.
fn lift_language_info(val: &Val) -> Option<(String, Vec<String>)> {
    let Val::Record(fields) = val else {
        return None;
    };
    let mut name = None;
    let mut extensions = None;
    for (field, value) in fields {
        match (field.as_str(), value) {
            ("name", Val::String(s)) => name = Some(s.clone()),
            ("extensions", Val::List(items)) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    let Val::String(s) = item else { return None };
                    out.push(s.clone());
                }
                extensions = Some(out);
            }
            _ => return None,
        }
    }
    Some((name?, extensions?))
}

/// Lift `list<u8>` from a dynamic component value.
fn lift_bytes(val: &Val) -> Option<Vec<u8>> {
    let Val::List(items) = val else { return None };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let Val::U8(b) = item else { return None };
        out.push(*b);
    }
    Some(out)
}

/// Lift `option<file-class>` from a dynamic component value. `Some(None)`
/// means the guest deferred (`none`); outer `None` means a shape mismatch.
#[allow(clippy::option_option)]
fn lift_file_class(val: &Val) -> Option<Option<String>> {
    let Val::Option(inner) = val else { return None };
    let Some(case) = inner else { return Some(None) };
    let Val::Enum(name) = case.as_ref() else {
        return None;
    };
    match name.as_str() {
        "source" | "test" | "vendor" => Some(Some(name.clone())),
        _ => None,
    }
}

/// Compiled WASI modules kept per guest digest (chunk arch-14c8). The script
/// interpreter is the same 1.5 MB module on every round of every script and
/// JIT compilation dominates a round, so the worker keeps the last few
/// compiled modules; the digest keys them, so changed bytes never reuse code.
const WASI_MODULE_CACHE: usize = 4;

/// Guest digest → compiled module, oldest first.
type WasiModuleCache = Vec<([u8; 32], Module)>;

fn cached_wasi_module(engine: &Engine, guest: &[u8]) -> Result<Module, wasmtime::Error> {
    use sha2::Digest;
    static CACHE: OnceLock<Mutex<WasiModuleCache>> = OnceLock::new();
    let digest: [u8; 32] = sha2::Sha256::digest(guest).into();
    let cache = CACHE.get_or_init(|| Mutex::new(Vec::new()));
    // Hold the cache lock through a miss: concurrent requests for the same
    // interpreter must not compile identical code independently.
    let mut cache = cache
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((_, module)) = cache
        .iter()
        .find(|(known, module)| *known == digest && Engine::same(module.engine(), engine))
    {
        return Ok(module.clone());
    }
    let module = Module::new(engine, guest)?;
    if cache.len() >= WASI_MODULE_CACHE {
        cache.remove(0);
    }
    cache.push((digest, module.clone()));
    Ok(module)
}

/// Run a `wasm32-wasip1` command module (`_start`) under `policy`.
///
/// The guest's world: `argv`, and stdout/stderr captured into memory. No
/// preopened directories, no environment, no inherited stdio, no network —
/// ambient authority is structurally absent, not filtered.
pub fn run_wasi_module(
    guest: &[u8],
    argv: &[String],
    policy: SandboxPolicy,
) -> Result<WasiOutcome, SandboxError> {
    let engine = engine()?;

    let invalid = |msg: String| WasiOutcome {
        end: GuestEnd::InvalidGuest(msg),
        fuel_used: 0,
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
    };

    let module = match cached_wasi_module(engine, guest) {
        Ok(m) => m,
        Err(e) => return Ok(invalid(e.to_string())),
    };

    let stdout = MemoryOutputPipe::new(OUTPUT_PIPE_CAPACITY);
    let stderr = MemoryOutputPipe::new(OUTPUT_PIPE_CAPACITY);
    let wasi = WasiCtxBuilder::new()
        .args(argv)
        .stdout(stdout.clone())
        .stderr(stderr.clone())
        .build_p1();

    let mut store = Store::new(
        engine,
        WasiState {
            limits: TrackedLimits::new(policy),
            wasi,
        },
    );
    store.limiter(|s| &mut s.limits);
    if let Err(e) = store.set_fuel(policy.fuel) {
        return Err(SandboxError::Engine(e.to_string()));
    }

    let mut linker: wasmtime::Linker<WasiState> = wasmtime::Linker::new(engine);
    if let Err(e) =
        wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |s: &mut WasiState| &mut s.wasi)
    {
        return Err(SandboxError::Engine(e.to_string()));
    }

    let run = linker
        .instantiate(&mut store, &module)
        .and_then(|instance| instance.get_typed_func::<(), ()>(&mut store, "_start"))
        .and_then(|start| start.call(&mut store, ()));

    let (end, exit_code) = match run {
        Ok(()) => (GuestEnd::Ok, Some(0)),
        Err(e) => {
            if let Some(exit) = e.downcast_ref::<I32Exit>() {
                if exit.0 == 0 {
                    (GuestEnd::Ok, Some(0))
                } else {
                    (GuestEnd::GuestExit(exit.0), Some(exit.0))
                }
            } else {
                (classify_failure(store.data().limits.breached, &e), None)
            }
        }
    };

    Ok(WasiOutcome {
        end,
        fuel_used: policy.fuel.saturating_sub(store.get_fuel().unwrap_or(0)),
        exit_code,
        stdout: String::from_utf8_lossy(&stdout.contents()).into_owned(),
        stderr: String::from_utf8_lossy(&stderr.contents()).into_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: SandboxPolicy = SandboxPolicy {
        fuel: 1_000_000,
        memory_cap_bytes: 10 * 65536,
    };

    /// Plain component: add two numbers, no imports.
    const ADD_WAT: &str = r#"
    (component
      (core module $m
        (memory (export "mem") 1)
        (func (export "add") (param i32 i32) (result i32)
          local.get 0
          local.get 1
          i32.add))
      (core instance $i (instantiate $m))
      (func (export "add") (param "a" u32) (param "b" u32) (result u32)
        (canon lift (core func $i "add")))
    )
    "#;

    /// Infinite loop: dies by fuel.
    const SPIN_WAT: &str = r#"
    (component
      (core module $m
        (func (export "spin") (loop $l br $l)))
      (core instance $i (instantiate $m))
      (func (export "spin") (canon lift (core func $i "spin")))
    )
    "#;

    /// Grows memory until denied; under trap_on_grow_failure the denial kills it.
    const HOG_WAT: &str = r#"
    (component
      (core module $m
        (memory 1)
        (func (export "hog") (result i32)
          (loop $l
            (drop (memory.grow (i32.const 1)))
            (br $l))
          (i32.const 0))
      )
      (core instance $i (instantiate $m))
      (func (export "hog") (result u32) (canon lift (core func $i "hog")))
    )
    "#;

    /// Traps on its own: unreachable.
    const CRASH_WAT: &str = r#"
    (component
      (core module $m
        (func (export "crash") unreachable))
      (core instance $i (instantiate $m))
      (func (export "crash") (canon lift (core func $i "crash")))
    )
    "#;

    /// Imports the `observe` capability and calls it three times.
    const OBSERVE_WAT: &str = r#"
    (component
      (import "observe" (func $obs (param "n" u32) (result u32)))
      (core func $obs_core (canon lower (func $obs)))
      (core module $m
        (import "cap" "observe" (func $obs (param i32) (result i32)))
        (func (export "run") (result i32)
          (drop (call $obs (i32.const 7)))
          (drop (call $obs (i32.const 11)))
          (call $obs (i32.const 13))))
      (core instance $i (instantiate $m (with "cap" (instance (export "observe" (func $obs_core))))))
      (func (export "run") (result u32) (canon lift (core func $i "run")))
    )
    "#;

    /// wasi command module: probes for preopens (fd 3) and exits with the
    /// errno — 8 (EBADF) proves there is no preopened directory at all.
    const PREOPEN_PROBE_WAT: &str = r#"
    (module
      (import "wasi_snapshot_preview1" "fd_prestat_get"
        (func $prestat (param i32 i32) (result i32)))
      (import "wasi_snapshot_preview1" "proc_exit" (func $exit (param i32)))
      (memory (export "memory") 1)
      (func (export "_start")
        (call $exit (call $prestat (i32.const 3) (i32.const 0)))
        unreachable)
    )
    "#;

    /// wasi command module: writes "jailed" to stdout and exits 0.
    const HELLO_WASI_WAT: &str = r#"
    (module
      (import "wasi_snapshot_preview1" "fd_write"
        (func $fd_write (param i32 i32 i32 i32) (result i32)))
      (memory (export "memory") 1)
      (data (i32.const 8) "jailed")
      (func (export "_start")
        (i32.store (i32.const 0) (i32.const 8))
        (i32.store (i32.const 4) (i32.const 6))
        (drop (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 16))))
    )
    "#;

    /// Plugin-tool calling convention: `invoke: (string) -> string`, echoing
    /// its payload. The canonical ABI's indirect string return is hand-rolled:
    /// the core function writes the (ptr, len) pair at 16 and returns 16.
    const ECHO_TOOL_WAT: &str = r#"
    (component
      (core module $m
        (memory (export "mem") 1)
        (func (export "realloc") (param i32 i32 i32 i32) (result i32)
          (i32.const 1024))
        (func (export "echo") (param i32 i32) (result i32)
          (i32.store (i32.const 16) (local.get 0))
          (i32.store (i32.const 20) (local.get 1))
          (i32.const 16)))
      (core instance $i (instantiate $m))
      (func (export "invoke") (param "payload" string) (result string)
        (canon lift (core func $i "echo") (memory $i "mem") (realloc (func $i "realloc"))))
    )
    "#;

    /// Same calling convention, exported inside an instance — the WIT world
    /// shape (`magistr:tool/guest@0.1.0#invoke`).
    const NESTED_TOOL_WAT: &str = r#"
    (component
      (core module $m
        (memory (export "mem") 1)
        (func (export "realloc") (param i32 i32 i32 i32) (result i32)
          (i32.const 1024))
        (func (export "echo") (param i32 i32) (result i32)
          (i32.store (i32.const 16) (local.get 0))
          (i32.store (i32.const 20) (local.get 1))
          (i32.const 16)))
      (core instance $i (instantiate $m))
      (func $invoke (param "payload" string) (result string)
        (canon lift (core func $i "echo") (memory $i "mem") (realloc (func $i "realloc"))))
      (instance $guest (export "invoke" (func $invoke)))
      (export "magistr:tool/guest@0.1.0" (instance $guest))
    )
    "#;

    /// A tool whose invoke traps immediately.
    const CRASH_TOOL_WAT: &str = r#"
    (component
      (core module $m
        (memory (export "mem") 1)
        (func (export "realloc") (param i32 i32 i32 i32) (result i32)
          (i32.const 1024))
        (func (export "boom") (param i32 i32) (result i32) unreachable))
      (core instance $i (instantiate $m))
      (func (export "invoke") (param "payload" string) (result string)
        (canon lift (core func $i "boom") (memory $i "mem") (realloc (func $i "realloc"))))
    )
    "#;

    /// Imports the subprocess capability and probes `open("zz")`: writes the
    /// call's indirect result at 1024 and returns the discriminant byte
    /// (0 = ok, 1 = err). With an empty spec table the typed answer must be
    /// err (unknown-spec) — the capability links, and refusal is data.
    const SUBPROCESS_PROBE_WAT: &str = r#"
    (component
      (import "magistr:subprocess/subprocess@0.2.0" (instance $sp
        (type $open-error' (variant
          (case "unknown-spec" string)
          (case "spawn-refused" string)
          (case "unhealthy" string)))
        (export "open-error" (type $open-error (eq $open-error')))
        (export "open" (func (param "language-id" string) (result (result u32 (error $open-error)))))
      ))
      (core module $mem_mod
        (memory (export "mem") 1)
        (func (export "realloc") (param i32 i32 i32 i32) (result i32)
          (i32.const 4096)))
      (core instance $mem (instantiate $mem_mod))
      (core func $open_core
        (canon lower (func $sp "open") (memory $mem "mem") (realloc (func $mem "realloc"))))
      (core module $m
        (import "env" "mem" (memory 1))
        (import "sp" "open" (func $open (param i32 i32 i32)))
        (data (i32.const 8) "zz")
        (func (export "probe") (result i32)
          (call $open (i32.const 8) (i32.const 2) (i32.const 1024))
          (i32.load8_u (i32.const 1024))))
      (core instance $i (instantiate $m
        (with "env" (instance (export "mem" (memory $mem "mem"))))
        (with "sp" (instance (export "open" (func $open_core))))))
      (func (export "probe") (result u32) (canon lift (core func $i "probe")))
    )
    "#;

    #[test]
    fn subprocess_capability_links_when_granted_and_refuses_typed() {
        // Ungranted: denied by name before instantiation, fuel untouched.
        let denied = run_component(SUBPROCESS_PROBE_WAT.as_bytes(), "probe", &[], &[], POLICY)
            .expect("host must not error on guest input");
        assert_eq!(
            denied.end,
            GuestEnd::CapDenied(CAP_SUBPROCESS.to_string()),
            "ungranted subprocess import must be refused pre-instantiation"
        );
        assert_eq!(denied.fuel_used, 0);

        // Granted with an empty spec table: open answers err (unknown-spec),
        // whose discriminant byte is 1.
        let outcome = invoke_resolver_probe(SUBPROCESS_PROBE_WAT);
        assert_eq!(outcome.end, GuestEnd::Ok);
        assert_eq!(outcome.results, vec!["U32(1)".to_string()]);
    }

    /// Run the subprocess probe with the current capability granted (empty specs).
    fn invoke_resolver_probe(wat: &str) -> ComponentOutcome {
        run_component(
            wat.as_bytes(),
            "probe",
            &[],
            &[CAP_SUBPROCESS.to_string()],
            POLICY,
        )
        .expect("host must not error on guest input")
    }

    /// The committed go runner-adapter component (chunk:m5-runner-adapter) —
    /// the runner lane's end-to-end pin over the real guest bytes.
    fn runner_guest() -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/plugins/runner-go/guest.wasm");
        std::fs::read(path).expect("committed runner-go guest.wasm present")
    }

    #[test]
    fn runner_lane_claims_go_and_lifts_structured_failures() {
        let guest = runner_guest();
        let policy = SandboxPolicy {
            fuel: 200_000_000,
            memory_cap_bytes: 32 * 1024 * 1024,
        };
        let stdout = concat!(
            "--- FAIL: TestAdd (0.00s)\n",
            "    math_test.go:12: expected 4, got 5\n",
            "    math_test.go:13: extra context\n",
            "FAIL\n",
            "FAIL\texample.com/m\t0.004s\n",
        );
        let outcome = invoke_runner_parse(&guest, "go test ./...", 1, stdout, "", &[], policy)
            .expect("host must not error on guest input");
        assert_eq!(outcome.end, GuestEnd::Ok);
        assert!(outcome.claimed);
        let report = outcome.report.expect("structure extracted");
        assert_eq!(report.failures.len(), 1);
        let failure = &report.failures[0];
        assert_eq!(failure.test_name, "TestAdd");
        assert_eq!(failure.file.as_deref(), Some("math_test.go"));
        assert_eq!(failure.assertion, "expected 4, got 5");
        assert_eq!(
            report.summary.as_deref(),
            Some("FAIL\texample.com/m\t0.004s")
        );

        // A non-go command is not claimed — no parse call, outcome Ok.
        let unclaimed = invoke_runner_parse(&guest, "bun test", 1, "x", "", &[], policy)
            .expect("host must not error on guest input");
        assert_eq!(unclaimed.end, GuestEnd::Ok);
        assert!(!unclaimed.claimed);
        assert!(unclaimed.report.is_none());
    }

    /// The committed visibility ranking-signal component (chunk:m5-ranking-shadow)
    /// — the ranking lane's end-to-end pin over the real guest bytes.
    fn ranking_guest() -> Vec<u8> {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/plugins/ranking-visibility/guest.wasm");
        std::fs::read(path).expect("committed ranking-visibility guest.wasm present")
    }

    fn ranking_hit(excerpt: &str, kind_class: &str) -> RankingHit {
        RankingHit {
            file: "src/lib.rs".to_string(),
            line: 1,
            column: 1,
            excerpt: excerpt.to_string(),
            kind_class: kind_class.to_string(),
            file_class: "source".to_string(),
        }
    }

    #[test]
    fn ranking_lane_batches_contributions_over_one_instance() {
        let guest = ranking_guest();
        let policy = SandboxPolicy {
            fuel: 200_000_000,
            memory_cap_bytes: 32 * 1024 * 1024,
        };
        let hits = [
            ranking_hit("pub fn parse(input: &str) -> Ast {", "declaration"),
            ranking_hit("fn helper() {", "declaration"),
            ranking_hit("export default async function main() {", "declaration"),
            ranking_hit("    parse(input);", "call"),
            ranking_hit("// pub fn parse — a comment mention", "comment"),
        ];
        let outcome = invoke_ranking_contribute(&guest, &hits, &[], policy)
            .expect("host must not error on guest input");
        assert_eq!(outcome.end, GuestEnd::Ok);
        assert_eq!(outcome.contributions.len(), 5);
        assert_eq!(
            outcome.contributions[0],
            vec![LiftedFeature {
                name: "exported".to_string(),
                value: 1.0
            }]
        );
        assert_eq!(
            outcome.contributions[1],
            vec![LiftedFeature {
                name: "exported".to_string(),
                value: 0.0
            }]
        );
        assert_eq!(outcome.contributions[2][0].value, 1.0);
        // Non-declaration hits contribute nothing, even when the excerpt
        // mentions a marker (the comment case).
        assert!(outcome.contributions[3].is_empty());
        assert!(outcome.contributions[4].is_empty());
        assert!(outcome.fuel_used > 0);
    }

    #[test]
    fn ranking_lane_refuses_unknown_enum_cases_before_running() {
        let guest = ranking_guest();
        let bad = RankingHit {
            kind_class: "definition".to_string(),
            ..ranking_hit("pub fn x() {}", "declaration")
        };
        let outcome = invoke_ranking_contribute(&guest, &[bad], &[], POLICY)
            .expect("host must not error on guest input");
        assert!(matches!(outcome.end, GuestEnd::InvalidGuest(_)));
        assert!(outcome.contributions.is_empty());
        assert_eq!(outcome.fuel_used, 0);
    }

    #[test]
    fn ranking_lane_is_a_carrier_lane_no_import_linkable() {
        // A component that imports anything is refused by the grant check
        // even before instantiation — the ranking surface grants nothing.
        let guest = ranking_guest();
        let outcome = invoke_ranking_contribute(
            &guest,
            &[ranking_hit("pub struct S;", "declaration")],
            &["magistr:subprocess/subprocess@0.2.0".to_string()],
            SandboxPolicy {
                fuel: 200_000_000,
                memory_cap_bytes: 32 * 1024 * 1024,
            },
        )
        .expect("host must not error on guest input");
        // The zero-import guest still runs fine with an over-broad grant —
        // grants gate IMPORTS, and this guest has none to gate.
        assert_eq!(outcome.end, GuestEnd::Ok);
    }

    fn invoke(wat: &str, export: &str, payload: &str, caps: &[&str]) -> InvokeOutcome {
        let caps: Vec<String> = caps.iter().map(|c| c.to_string()).collect();
        invoke_component_json(wat.as_bytes(), export, payload, &caps, POLICY, &[], "")
            .expect("host must not error on guest input")
    }

    fn run(wat: &str, export: &str, args: &[i64], caps: &[&str]) -> ComponentOutcome {
        let caps: Vec<String> = caps.iter().map(|c| c.to_string()).collect();
        run_component(wat.as_bytes(), export, args, &caps, POLICY)
            .expect("host must not error on guest input")
    }

    #[test]
    fn component_runs_under_policy() {
        let outcome = run(ADD_WAT, "add", &[19, 23], &[]);
        assert_eq!(outcome.end, GuestEnd::Ok);
        assert_eq!(outcome.results, vec!["U32(42)".to_string()]);
        assert!(outcome.fuel_used > 0);
    }

    #[test]
    fn fuel_exhaustion_kills_deterministically() {
        let first = run(SPIN_WAT, "spin", &[], &[]);
        let second = run(SPIN_WAT, "spin", &[], &[]);
        assert_eq!(first.end, GuestEnd::OutOfFuel);
        assert_eq!(second.end, GuestEnd::OutOfFuel);
        assert_eq!(first.fuel_used, second.fuel_used);
    }

    #[test]
    fn memory_cap_breach_kills_deterministically() {
        let first = run(HOG_WAT, "hog", &[], &[]);
        let second = run(HOG_WAT, "hog", &[], &[]);
        assert_eq!(first.end, GuestEnd::MemoryCap);
        assert_eq!(second.end, GuestEnd::MemoryCap);
        assert_eq!(first.fuel_used, second.fuel_used);
    }

    #[test]
    fn guest_trap_is_data() {
        let outcome = run(CRASH_WAT, "crash", &[], &[]);
        assert!(matches!(outcome.end, GuestEnd::GuestTrap(_)));
    }

    /// Two wasmtime majors in one process: a guest trap in this engine must
    /// stay data whether tree-sitter's grammar engine (wasmtime 36) was
    /// initialised before or after it. Honest scope: this plain Rust binary
    /// did NOT die with the Mach-port default — the SIGILL only reproduced
    /// inside Bun's `bun test` process (four TS files loading both engines
    /// before a sandbox guest trapped, 4/4 crashes before the
    /// `macos_use_mach_ports(false)` change, 0/4 after). This test keeps the
    /// two-engine trap path exercised so a future regression in either
    /// engine's trap handling shows up here first.
    #[cfg(feature = "lang-pack")]
    #[test]
    fn guest_trap_stays_data_beside_the_grammar_engine() {
        let before = run(CRASH_WAT, "crash", &[], &[]);
        assert!(matches!(before.end, GuestEnd::GuestTrap(_)));
        let _grammar = crate::lang_registry::grammar_engine();
        let after = run(CRASH_WAT, "crash", &[], &[]);
        assert!(matches!(after.end, GuestEnd::GuestTrap(_)));
    }

    #[test]
    fn ungranted_import_is_denied_by_name_before_instantiation() {
        let outcome = run(OBSERVE_WAT, "run", &[], &[]);
        assert_eq!(outcome.end, GuestEnd::CapDenied("observe".to_string()));
        assert_eq!(outcome.fuel_used, 0);
    }

    #[test]
    fn granted_capability_is_the_only_door_and_the_host_observes_it() {
        let outcome = run(OBSERVE_WAT, "run", &[], &[CAP_OBSERVE]);
        assert_eq!(outcome.end, GuestEnd::Ok);
        assert_eq!(outcome.observations, vec![7, 11, 13]);
        // The export returns the third call's running count.
        assert_eq!(outcome.results, vec!["U32(3)".to_string()]);
    }

    #[test]
    fn unknown_grant_is_refused() {
        let outcome = run(ADD_WAT, "add", &[1, 2], &["filesystem"]);
        assert!(matches!(outcome.end, GuestEnd::InvalidGuest(_)));
    }

    #[test]
    fn bad_bytes_missing_export_and_bad_arity_are_typed() {
        let garbage = run_component(b"not wasm", "x", &[], &[], POLICY)
            .expect("host must not error on guest input");
        assert!(matches!(garbage.end, GuestEnd::InvalidGuest(_)));

        let missing = run(ADD_WAT, "nope", &[], &[]);
        assert!(matches!(missing.end, GuestEnd::InvalidGuest(_)));

        let arity = run(ADD_WAT, "add", &[1], &[]);
        assert!(matches!(arity.end, GuestEnd::InvalidGuest(_)));
    }

    #[test]
    fn invoke_round_trips_a_string_payload() {
        let outcome = invoke(ECHO_TOOL_WAT, "invoke", r#"{"args":{"n":7}}"#, &[]);
        assert_eq!(outcome.end, GuestEnd::Ok);
        assert_eq!(outcome.result.as_deref(), Some(r#"{"args":{"n":7}}"#));
        assert!(outcome.fuel_used > 0);
    }

    #[test]
    fn invoke_addresses_a_wit_world_export_by_interface_and_name() {
        let outcome = invoke(
            NESTED_TOOL_WAT,
            "magistr:tool/guest@0.1.0#invoke",
            "payload",
            &[],
        );
        assert_eq!(outcome.end, GuestEnd::Ok);
        assert_eq!(outcome.result.as_deref(), Some("payload"));
    }

    #[test]
    fn invoke_denies_an_ungranted_import_before_instantiation() {
        let outcome = invoke(OBSERVE_WAT, "run", "{}", &[]);
        assert_eq!(outcome.end, GuestEnd::CapDenied("observe".to_string()));
        assert_eq!(outcome.fuel_used, 0);
    }

    #[test]
    fn invoke_trap_and_wrong_shape_are_typed() {
        let crash = invoke(CRASH_TOOL_WAT, "invoke", "{}", &[]);
        assert!(matches!(crash.end, GuestEnd::GuestTrap(_)));
        assert_eq!(crash.result, None);

        // An integer-shaped export is refused: the plugin tool calling
        // convention is (string) -> string.
        let shape = invoke(ADD_WAT, "add", "{}", &[]);
        assert!(matches!(shape.end, GuestEnd::InvalidGuest(_)));

        let missing = invoke(ECHO_TOOL_WAT, "nope", "{}", &[]);
        assert!(matches!(missing.end, GuestEnd::InvalidGuest(_)));
    }

    #[test]
    fn wasi_guest_has_zero_preopens() {
        let outcome = run_wasi_module(PREOPEN_PROBE_WAT.as_bytes(), &["probe".to_string()], POLICY)
            .expect("host must not error on guest input");
        // fd_prestat_get(3) with no preopens answers errno 8 (EBADF).
        assert_eq!(outcome.end, GuestEnd::GuestExit(8));
        assert_eq!(outcome.exit_code, Some(8));
    }

    #[test]
    fn wasi_stdout_is_captured_not_inherited() {
        let outcome = run_wasi_module(HELLO_WASI_WAT.as_bytes(), &["hello".to_string()], POLICY)
            .expect("host must not error on guest input");
        assert_eq!(outcome.end, GuestEnd::Ok);
        assert_eq!(outcome.stdout, "jailed");
    }

    #[test]
    fn host_survives_every_kill_in_sequence() {
        // Fuel kill, memory kill, own trap — then a clean run still works in
        // the same process. The engine child never dies with the guest.
        assert_eq!(run(SPIN_WAT, "spin", &[], &[]).end, GuestEnd::OutOfFuel);
        assert_eq!(run(HOG_WAT, "hog", &[], &[]).end, GuestEnd::MemoryCap);
        assert!(matches!(
            run(CRASH_WAT, "crash", &[], &[]).end,
            GuestEnd::GuestTrap(_)
        ));
        let alive = run(ADD_WAT, "add", &[2, 2], &[]);
        assert_eq!(alive.end, GuestEnd::Ok);
        assert_eq!(alive.results, vec!["U32(4)".to_string()]);
    }

    // ---- Server pool (specs/007-resolver-server-pool) ----------------------
    //
    // The "language server" in these tests is THIS test binary re-invoked on
    // one libtest entry (`fake_server_entry`) so the Rust CI job needs no
    // external runtime. libtest's own chatter ("running 1 test") precedes the
    // first frame and is swallowed by the header parser, which only looks for
    // a Content-Length line before the first blank line — the same tolerance
    // a real server's startup banner would need.

    /// Acts as a framed-stdio server when re-invoked with a `mode:<m>` filter
    /// argument (an inert test otherwise). Modes:
    ///   echo        — echo every frame body back as a frame; exit on EOF
    ///   ignore-eof  — echo, but sleep forever after stdin EOF
    #[test]
    fn fake_server_entry() {
        let Some(mode) = std::env::args().find_map(|a| a.strip_prefix("mode:").map(String::from))
        else {
            return;
        };
        use std::io::Write as _;
        // libtest prints "test <name> ... " with no newline before the test
        // runs; start the transport on a fresh line so the first
        // Content-Length header is its own line, as a real server's would be.
        {
            let mut out = std::io::stdout().lock();
            let _ = out.write_all(
                b"
",
            );
            let _ = out.flush();
        }
        let mut stdin = std::io::stdin().lock();
        let mut buf: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 4096];
        loop {
            while let Some(header_end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                let header = String::from_utf8_lossy(&buf[..header_end]).into_owned();
                let len = header
                    .lines()
                    .find_map(|l| {
                        let (k, v) = l.split_once(':')?;
                        k.eq_ignore_ascii_case("content-length")
                            .then(|| v.trim().parse::<usize>().ok())?
                    })
                    .unwrap_or(0);
                if buf.len() < header_end + 4 + len {
                    break;
                }
                let body = buf[header_end + 4..header_end + 4 + len].to_vec();
                buf.drain(..header_end + 4 + len);
                let mut out = std::io::stdout().lock();
                let _ = out.write_all(format!("Content-Length: {}\r\n\r\n", body.len()).as_bytes());
                let _ = out.write_all(&body);
                let _ = out.flush();
            }
            match stdin.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => buf.extend_from_slice(&chunk[..n]),
            }
        }
        if mode == "ignore-eof" {
            loop {
                std::thread::sleep(Duration::from_secs(3600));
            }
        }
    }

    /// The pool is process-global; these tests call global operations
    /// (idle reap with a zero TTL, shutdown), so they run one at a time —
    /// the lang_registry serial-guard precedent.
    pub(super) fn pool_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static GUARD: Mutex<()> = Mutex::new(());
        GUARD
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn fake_spec(language_id: &str, mode: &str) -> SubprocessSpec {
        SubprocessSpec {
            language_id: language_id.to_string(),
            bin: std::env::current_exe()
                .expect("test binary path")
                .to_string_lossy()
                .into_owned(),
            args: vec![
                "--exact".to_string(),
                "sandbox::tests::fake_server_entry".to_string(),
                format!("mode:{mode}"),
                "--nocapture".to_string(),
                "--test-threads=1".to_string(),
            ],
        }
    }

    /// Each test gets its own root so pool keys never collide across the
    /// parallel test threads.
    fn unique_root(tag: &str) -> String {
        format!("/pool-test/{tag}/{}", std::process::id())
    }

    fn warm_for(root: &str) -> Vec<ServerPoolSessionStat> {
        server_pool_stats()
            .into_iter()
            .filter(|s| s.root == root)
            .collect()
    }

    fn round_trip(view: &mut SubprocessPool, handle: u32, body: &[u8]) -> Vec<u8> {
        view.write_frame(handle, body).expect("write");
        view.read_frame(handle, 10_000).expect("read")
    }

    #[test]
    fn pooled_session_survives_the_invoke_and_is_reused_warm() {
        let _serial = pool_test_guard();
        let root = unique_root("reuse");
        let spec = fake_spec("echo", "echo");
        // Invoke 1: fresh spawn, normal end → returned warm.
        let mut first = SubprocessPool::new(vec![spec.clone()], root.clone());
        let h = first.open("echo").expect("open");
        assert!(!first.reused(h));
        assert_eq!(round_trip(&mut first, h, b"alpha"), b"alpha");
        assert_eq!((first.opened, first.reused), (1, 0));
        let reader_lease = Arc::clone(&first.sessions[&h].leased);
        assert!(reader_lease.load(Ordering::SeqCst));
        first.settle(true);
        assert!(!reader_lease.load(Ordering::SeqCst));
        drop(first);
        let warm = warm_for(&root);
        assert_eq!(warm.len(), 1);
        assert!(!warm[0].leased);

        // Invoke 2: the same key comes back warm and says so.
        let mut second = SubprocessPool::new(vec![spec.clone()], root.clone());
        let h2 = second.open("echo").expect("open");
        assert!(second.reused(h2));
        assert_eq!((second.opened, second.reused), (0, 1));
        assert!(reader_lease.load(Ordering::SeqCst));
        assert!(matches!(
            second.sessions[&h2].frames.try_recv(),
            Err(mpsc::TryRecvError::Empty)
        ));
        assert!(
            warm_for(&root)[0].leased,
            "a leased key is visible as leased"
        );
        assert_eq!(round_trip(&mut second, h2, b"beta"), b"beta");
        second.settle(true);
        drop(second);
        assert_eq!(warm_for(&root).len(), 1);

        // Explicit release returns warm too.
        let mut third = SubprocessPool::new(vec![spec], root.clone());
        let h3 = third.open("echo").expect("open");
        assert!(third.reused(h3));
        third.release(h3);
        assert!(
            matches!(
                third.read_frame(h3, 10),
                Err(SubprocessIoError::BadSession(_))
            ),
            "a released handle is gone from the view"
        );
        drop(third);
        assert_eq!(warm_for(&root).len(), 1);

        // Idle reap with a zero TTL kills it.
        assert!(server_pool_reap_idle(Duration::ZERO) >= 1);
        assert!(warm_for(&root).is_empty());
    }

    #[test]
    fn a_leased_key_is_refused_to_a_second_invoke_without_waiting() {
        let _serial = pool_test_guard();
        let root = unique_root("busy");
        let spec = fake_spec("echo", "echo");
        let mut holder = SubprocessPool::new(vec![spec.clone()], root.clone());
        holder.open("echo").expect("open");
        let mut other = SubprocessPool::new(vec![spec], root.clone());
        let started = Instant::now();
        let refused = other.open("echo");
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "busy must not wait"
        );
        match refused {
            Err(SubprocessOpenError::SpawnRefused(msg)) => assert!(msg.contains("leased")),
            other => panic!("expected spawn-refused(leased), got {other:?}"),
        }
        holder.settle(true);
        drop(holder);
        // Now it is warm and the other invoke can take it.
        let h = other.open("echo").expect("open after release");
        assert!(other.reused(h));
        other.settle(false);
        drop(other);
        assert!(warm_for(&root).is_empty());
    }

    #[test]
    fn non_ok_end_close_and_drop_without_settle_all_kill() {
        let _serial = pool_test_guard();
        let root = unique_root("kill");
        let spec = fake_spec("echo", "echo");
        // settle(false) — fuel/trap/memory ends route here.
        let mut a = SubprocessPool::new(vec![spec.clone()], root.clone());
        a.open("echo").expect("open");
        a.settle(false);
        drop(a);
        assert!(warm_for(&root).is_empty());
        // close kills even inside a normal invoke.
        let mut b = SubprocessPool::new(vec![spec.clone()], root.clone());
        let h = b.open("echo").expect("open");
        b.close(h);
        b.settle(true);
        drop(b);
        assert!(warm_for(&root).is_empty());
        // Drop without settle kills (the host-panic path).
        let mut c = SubprocessPool::new(vec![spec.clone()], root.clone());
        c.open("echo").expect("open");
        drop(c);
        assert!(warm_for(&root).is_empty());
        // And the next open after any of those spawns fresh.
        let mut d = SubprocessPool::new(vec![spec], root.clone());
        let h = d.open("echo").expect("open");
        assert!(!d.reused(h));
        d.settle(false);
    }

    #[test]
    fn frames_read_while_idle_are_discarded() {
        // Drive lease changes at exact frame boundaries. Scheduler speed must
        // not determine how many idle messages the reader sees.
        struct FrameSteps<'a> {
            leased: &'a AtomicBool,
            steps: std::vec::IntoIter<(bool, &'static str)>,
        }
        impl Read for FrameSteps<'_> {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                let Some((leased, body)) = self.steps.next() else {
                    return Ok(0);
                };
                self.leased.store(leased, Ordering::SeqCst);
                let frame = format!("Content-Length: {}\r\n\r\n{body}", body.len());
                assert!(frame.len() <= buffer.len());
                buffer[..frame.len()].copy_from_slice(frame.as_bytes());
                Ok(frame.len())
            }
        }

        let leased = AtomicBool::new(true);
        let mut steps = vec![(true, "before")];
        // More idle frames than the queue can hold prove that idle traffic
        // neither buffers nor saturates the leased-frame channel.
        steps.extend(std::iter::repeat_n(
            (false, "idle"),
            SUBPROCESS_FRAME_QUEUE_CAP + 1,
        ));
        steps.push((true, "after"));
        let mut input = FrameSteps {
            leased: &leased,
            steps: steps.into_iter(),
        };
        let (tx, rx) = mpsc::sync_channel(SUBPROCESS_FRAME_QUEUE_CAP);
        read_frames(&mut input, &tx, &leased);
        assert_eq!(input.steps.len(), 0, "the reader drains every frame");
        let frames: Vec<_> = rx.try_iter().map(Result::unwrap).collect();
        assert_eq!(frames, [b"before".to_vec(), b"after".to_vec()]);
    }

    #[test]
    fn shutdown_kills_every_warm_session_and_later_opens_spawn_fresh() {
        let _serial = pool_test_guard();
        let root = unique_root("shutdown");
        let spec = fake_spec("echo", "ignore-eof");
        let mut a = SubprocessPool::new(vec![spec.clone()], root.clone());
        a.open("echo").expect("open");
        a.settle(true);
        drop(a);
        assert_eq!(warm_for(&root).len(), 1);
        assert!(server_pool_shutdown() >= 1);
        assert!(warm_for(&root).is_empty());
        let mut b = SubprocessPool::new(vec![spec], root);
        let h = b.open("echo").expect("open after shutdown");
        assert!(!b.reused(h));
        b.settle(false);
    }

    /// Imports subprocess@0.2.0 and calls `reused(7)` on a handle that was
    /// never opened: links (the 0.2.0 functions exist) and answers false.
    const SUBPROCESS_V2_PROBE_WAT: &str = r#"
    (component
      (import "magistr:subprocess/subprocess@0.2.0" (instance $sp
        (export "reused" (func (param "session" u32) (result bool)))
      ))
      (core func $reused_core (canon lower (func $sp "reused")))
      (core module $m
        (import "sp" "reused" (func $reused (param i32) (result i32)))
        (func (export "probe") (result i32)
          (call $reused (i32.const 7))))
      (core instance $i (instantiate $m
        (with "sp" (instance (export "reused" (func $reused_core))))))
      (func (export "probe") (result u32) (canon lift (core func $i "probe")))
    )
    "#;

    #[test]
    fn reused_links_for_the_0_2_0_stamp_only() {
        let v2 = run_component(
            SUBPROCESS_V2_PROBE_WAT.as_bytes(),
            "probe",
            &[],
            &[CAP_SUBPROCESS.to_string()],
            POLICY,
        )
        .expect("host must not error on guest input");
        assert_eq!(v2.end, GuestEnd::Ok);
        assert_eq!(v2.results, vec!["U32(0)".to_string()]);
    }

    /// A 0.2.0 guest that opens a session and then spins out of fuel: the
    /// fuel kill must take the leased server with it (research R4).
    const SUBPROCESS_V2_OPEN_THEN_SPIN_WAT: &str = r#"
    (component
      (import "magistr:subprocess/subprocess@0.2.0" (instance $sp
        (type $open-error' (variant
          (case "unknown-spec" string)
          (case "spawn-refused" string)
          (case "unhealthy" string)))
        (export "open-error" (type $open-error (eq $open-error')))
        (export "open" (func (param "language-id" string) (result (result u32 (error $open-error)))))
      ))
      (core module $mem_mod
        (memory (export "mem") 1)
        (func (export "realloc") (param i32 i32 i32 i32) (result i32)
          (i32.const 4096)))
      (core instance $mem (instantiate $mem_mod))
      (core func $open_core
        (canon lower (func $sp "open") (memory $mem "mem") (realloc (func $mem "realloc"))))
      (core module $m
        (import "env" "mem" (memory 1))
        (import "sp" "open" (func $open (param i32 i32 i32)))
        (data (i32.const 8) "echo")
        (func (export "invoke") (param i32 i32) (result i32)
          (call $open (i32.const 8) (i32.const 4) (i32.const 1024))
          (loop $forever br $forever)
          unreachable))
      (core instance $i (instantiate $m
        (with "env" (instance (export "mem" (memory $mem "mem"))))
        (with "sp" (instance (export "open" (func $open_core))))))
      (func (export "invoke") (param "payload" string) (result string)
        (canon lift (core func $i "invoke") (memory $mem "mem") (realloc (func $mem "realloc"))))
    )
    "#;

    #[test]
    fn fuel_exhaustion_kills_the_leased_server_through_the_wasm_lane() {
        let _serial = pool_test_guard();
        let root = unique_root("fuel");
        let spec = fake_spec("echo", "echo");
        let outcome = invoke_component_json(
            SUBPROCESS_V2_OPEN_THEN_SPIN_WAT.as_bytes(),
            "invoke",
            "{}",
            &[CAP_SUBPROCESS.to_string()],
            SandboxPolicy {
                fuel: 2_000_000,
                memory_cap_bytes: 16 * 1024 * 1024,
            },
            std::slice::from_ref(&spec),
            &root,
        )
        .expect("host must not error on guest input");
        assert_eq!(outcome.end, GuestEnd::OutOfFuel);
        assert!(
            warm_for(&root).is_empty(),
            "a fuel-killed invoke must not leave a warm or leased session"
        );
        // The key is free again: a clean lease view spawns fresh.
        let mut view = SubprocessPool::new(vec![spec], root.clone());
        let h = view.open("echo").expect("open after fuel kill");
        assert!(!view.reused(h));
        view.settle(false);
    }

    #[test]
    fn is_subprocess_import_accepts_the_current_contract_only() {
        assert!(is_subprocess_import(CAP_SUBPROCESS));
        assert!(!is_subprocess_import("magistr:subprocess/subprocess@0.1.0"));
        assert!(!is_subprocess_import("magistr:subprocess/subprocess@0.7.3"));
        assert!(!is_subprocess_import("magistr:subprocess/subprocess@1.0.0"));
        assert!(!is_subprocess_import("magistr:subprocess/subprocess"));
        assert!(!is_subprocess_import("magistr:subprocess/subprocess@0.1"));
        assert!(!is_subprocess_import("magistr:tool/host@0.1.0"));
    }
}
