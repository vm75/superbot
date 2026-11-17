//! The only contract between the server and a harness (Claude Code, Codex, Pi).
//! Each adapter builds a command line and maps the harness's JSON lines to `AgentEvent`s.
//! Spawning, streaming, stopping, and exit handling are shared here.

mod claude;
mod codex;
mod pi;
mod custom;
pub use custom::Spec;

use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, RwLock};
use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::watch;

pub enum AgentEvent {
    /// Harness session to resume next time.
    Session(String),
    /// Streaming piece of the current text block.
    TextDelta(String),
    /// A finished text block: one chat bubble.
    Text(String),
    /// The agent started using a tool.
    Tool { name: String, detail: String },
    Error(String),
}

pub struct RunInput {
    pub prompt: String,
    /// Bot identity, rules, and memory. Rebuilt every run.
    pub system: String,
    /// None = start a new harness session.
    pub session_id: Option<String>,
    pub cwd: PathBuf,
    pub model: Option<String>,
    /// Reasoning effort, in the harness's own words (e.g. "high").
    pub effort: Option<String>,
    pub env: HashMap<String, String>,
    /// A private folder for this Bot's harness files.
    pub state_dir: PathBuf,
    /// Bounded recent chat history used by stateless custom CLIs.
    pub history: Vec<(String, String)>,
}

pub trait Adapter: Send {
    fn bin(&self) -> &str;
    /// Command-line arguments. May emit events up front (e.g. a session id it chose).
    fn args(&mut self, input: &RunInput, out: &mut Vec<AgentEvent>) -> Vec<String>;
    /// Map one JSON line from the harness to zero or more events.
    fn on_event(&mut self, e: &Value, out: &mut Vec<AgentEvent>);
    fn on_line(&mut self, line: &[u8], out: &mut Vec<AgentEvent>) {
        if let Ok(v) = serde_json::from_slice::<Value>(line) { self.on_event(&v, out); }
    }
    fn stdin_text(&self, _input: &RunInput) -> Option<String> { None }
    fn finish(&mut self, _out: &mut Vec<AgentEvent>) {}
    /// Models this harness can run here, with their reasoning efforts. Blocking.
    fn models(&self) -> Vec<ModelOption>;
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    /// Efforts this model accepts. Empty = no effort setting.
    #[serde(default)]
    pub efforts: Vec<String>,
    /// Other names the harness accepts for this model (e.g. "opus").
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default)]
    pub aliases: Vec<String>,
}

// Shared runtime registry; only built-in Rust adapters need a code change.
static CUSTOM: OnceLock<RwLock<HashMap<String, Spec>>> = OnceLock::new();
type ModelCache = HashMap<String, (std::time::Instant, Vec<ModelOption>)>;
static MODEL_CACHE: OnceLock<Mutex<ModelCache>> = OnceLock::new();

fn custom() -> &'static RwLock<HashMap<String, Spec>> {
    CUSTOM.get_or_init(|| RwLock::new(HashMap::new()))
}
fn cache() -> &'static Mutex<ModelCache> {
    MODEL_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}
pub fn load_custom(specs: Vec<Spec>) {
    *custom().write().unwrap() = specs.into_iter().map(|s| (s.id.clone(), s)).collect();
    cache().lock().unwrap().clear();
}
pub fn put_custom(spec: Spec) {
    cache().lock().unwrap().remove(&spec.id);
    custom().write().unwrap().insert(spec.id.clone(), spec);
}
pub fn delete_custom(id: &str) {
    custom().write().unwrap().remove(id);
    cache().lock().unwrap().remove(id);
}
pub fn custom_specs() -> Vec<Spec> {
    let mut v: Vec<_> = custom().read().unwrap().values().cloned().collect();
    v.sort_by(|a, b| a.name.cmp(&b.name));
    v
}
pub fn names() -> Vec<String> {
    let mut v: Vec<String> = NAMES.into_iter().map(str::to_owned).collect();
    v.extend(custom_specs().into_iter().filter(|s| s.enabled).map(|s| s.id));
    v
}
pub fn on_path(bin: &str) -> bool {
    if bin.is_empty() { return false; }
    let executable = |p: &Path| {
        use std::os::unix::fs::PermissionsExt;
        p.is_file() && p.metadata().is_ok_and(|m| m.permissions().mode() & 0o111 != 0)
    };
    if bin.contains('/') { return executable(Path::new(bin)); }
    std::env::var_os("PATH").into_iter().flat_map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .any(|dir| executable(&dir.join(bin)))
}

/// Model lists, cached for ten minutes per harness. Discovery failures surface to the UI.
pub fn models(name: &str) -> Result<Vec<ModelOption>, String> {
    use std::time::Duration;
    let adapter = get(name).ok_or_else(|| format!("Unknown or disabled adapter: {name}"))?;
    if !on_path(adapter.bin()) {
        return Err(format!("Executable '{}' for {name} is not available to the botd user.", adapter.bin()));
    }
    if let Some((at, list)) = cache().lock().unwrap().get(name) {
        if at.elapsed() < Duration::from_secs(600) { return Ok(list.clone()); }
    }
    let list = adapter.models();
    if list.is_empty() && NAMES.contains(&name) {
        return Err(format!("No models discovered for {name}. Check CLI authentication and its model-list command as the botd user."));
    }
    cache().lock().unwrap().insert(name.to_owned(), (std::time::Instant::now(), list.clone()));
    Ok(list)
}

/// Runs a command and returns its stdout, or "" if it fails.
pub fn output(bin: &str, args: &[&str]) -> String {
    std::process::Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
        .unwrap_or_default()
}

/// The bundled agents stay native; user-defined agents run through the generic CLI adapter.
pub const NAMES: [&str; 3] = ["claude", "codex", "pi"];

pub fn get(name: &str) -> Option<Box<dyn Adapter>> {
    match name {
        "claude" => Some(Box::new(claude::Claude::default())),
        "codex" => Some(Box::new(codex::Codex)),
        "pi" => Some(Box::new(pi::Pi)),
        _ => custom().read().unwrap().get(name).filter(|s| s.enabled)
            .cloned().map(|s| Box::new(custom::Custom::new(s)) as Box<dyn Adapter>),
    }
}

/// Enabled harnesses whose executable is available on the service's PATH.
pub fn installed() -> Vec<String> {
    names().into_iter().filter(|name| get(name).is_some_and(|a| on_path(a.bin()))).collect()
}

/// Why a run was cut short.
pub type AbortReason = Option<&'static str>;

/// Runs the harness, calling `emit` for every event, until it exits or `abort` fires.
pub async fn run(
    mut adapter: Box<dyn Adapter>,
    input: RunInput,
    mut abort: watch::Receiver<AbortReason>,
    mut emit: impl FnMut(AgentEvent),
) {
    let mut out = Vec::new();
    let args = adapter.args(&input, &mut out);
    out.drain(..).for_each(&mut emit);
    let stdin_text = adapter.stdin_text(&input);

    let mut child = match Command::new(adapter.bin())
        .args(&args)
        .current_dir(&input.cwd)
        .envs(&input.env)
        .stdin(if stdin_text.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0) // own group, so stop kills the whole tree
        .kill_on_drop(true)
        .spawn()
    {
        Ok(c) => c,
        Err(e) => return emit(AgentEvent::Error(format!("Couldn't start {}: {e}", adapter.bin()))),
    };
    if let Some(contents) = stdin_text {
        if let Some(mut stdin) = child.stdin.take() {
            tokio::spawn(async move { let _ = stdin.write_all(contents.as_bytes()).await; });
        }
    }
    let pid = child.id();

    let mut stderr = child.stderr.take().unwrap();
    let stderr_task = tokio::spawn(async move {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s).await;
        s
    });

    // JSON lines. Split only on "\n": some harnesses put U+2028 inside JSON strings.
    let mut lines = BufReader::new(child.stdout.take().unwrap()).split(b'\n');
    let mut saw_error = false;
    let mut aborted = false;
    loop {
        tokio::select! {
            line = lines.next_segment() => match line {
                Ok(Some(bytes)) => {
                    adapter.on_line(&bytes, &mut out);
                    for e in out.drain(..) {
                        saw_error |= matches!(e, AgentEvent::Error(_));
                        emit(e);
                    }
                }
                _ => break,
            },
            _ = abort.wait_for(|r| r.is_some()), if !aborted => {
                aborted = true;
                if let Some(pid) = pid {
                    kill_group(pid, "TERM");
                    tokio::spawn(async move {
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                        kill_group(pid, "KILL");
                    });
                }
            }
        }
    }
    let status = child.wait().await;
    if !aborted && status.as_ref().is_ok_and(|s| s.success()) {
        adapter.finish(&mut out);
        for e in out.drain(..) { saw_error |= matches!(e, AgentEvent::Error(_)); emit(e); }
    }
    let stderr = stderr_task.await.unwrap_or_default();
    let ok = status.as_ref().is_ok_and(|s| s.success());
    if !ok && !saw_error && !aborted {
        let tail: Vec<&str> = stderr.trim().lines().rev().take(3).collect();
        let msg = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
        emit(AgentEvent::Error(if msg.is_empty() {
            format!("{} exited with {}", adapter.bin(), status.map(|s| s.to_string()).unwrap_or_default())
        } else {
            msg
        }));
    }
}

fn kill_group(pid: u32, signal: &str) {
    let _ = std::process::Command::new("kill")
        .args([format!("-{signal}"), "--".into(), format!("-{pid}")])
        .stderr(Stdio::null())
        .status();
}

/// One short line describing a tool call, for the "working" status.
pub fn describe(input: &Value) -> String {
    let v = ["command", "cmd", "file_path", "path", "url", "query", "pattern", "description"]
        .iter()
        .find_map(|k| input.get(*k));
    let s = match v {
        Some(Value::String(s)) => s.clone(),
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" "),
        _ => String::new(),
    };
    short(&s)
}

pub fn short(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(120).collect()
}

pub fn str_of<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

pub fn new_uuid() -> String {
    let mut b = crate::random_bytes(16);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let h: String = b.iter().map(|x| format!("{x:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[0..8], &h[8..12], &h[12..16], &h[16..20], &h[20..32])
}
