//! UI-defined CLI harness. Commands are arrays of arguments, never shell scripts.
use super::{new_uuid, AgentEvent, Adapter, ModelOption, RunInput};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::{Command, Stdio};
use std::path::Path;

fn enabled_default() -> bool { true }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Spec {
    pub id: String,
    pub name: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub model_args: Vec<String>,
    #[serde(default)]
    pub effort_args: Vec<String>,
    #[serde(default)]
    pub start_args: Vec<String>,
    #[serde(default)]
    pub resume_args: Vec<String>,
    #[serde(default)]
    pub stdin: bool,
    #[serde(default)]
    pub output: String,
    #[serde(default)]
    pub session: String,
    #[serde(default)]
    pub model_list: Vec<ModelOption>,
    #[serde(default)]
    pub model_command: Vec<String>,
    #[serde(default)]
    pub model_format: String,
    #[serde(default)]
    pub event_rules: Vec<Rule>,
    #[serde(default = "enabled_default")]
    pub enabled: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rule {
    pub kind: String,
    #[serde(default)]
    pub when_path: String,
    #[serde(default)]
    pub equals: String,
    #[serde(default)]
    pub value_path: String,
    #[serde(default)]
    pub name_path: String,
    #[serde(default)]
    pub detail_path: String,
}

impl Spec {
    pub fn validate(&self) -> Result<(), String> {
        if self.id.len() < 2 || self.id.len() > 48
            || !self.id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
            || ["claude", "codex", "pi"].contains(&self.id.as_str())
        {
            return Err("Use a unique ID of 2–48 lowercase letters, digits, dashes or underscores (not a built-in ID).".into());
        }
        if self.name.trim().is_empty() || self.name.len() > 80 {
            return Err("Adapter name must be 1–80 characters.".into());
        }
        if self.program.trim().is_empty() || self.program.len() > 512 || self.program.contains('\0')
            || (self.program.contains('/') && !Path::new(&self.program).is_absolute()) {
            return Err("Executable must be a program name or absolute path.".into());
        }
        if !["", "text", "jsonl"].contains(&self.output.as_str()) {
            return Err("Output must be text or jsonl.".into());
        }
        if !["", "none", "generated", "event"].contains(&self.session.as_str()) {
            return Err("Session must be none, generated, or event.".into());
        }
        if self.session == "generated"
            && (!self.start_args.iter().any(|a| a.contains("{{session_id}}"))
                || !self.resume_args.iter().any(|a| a.contains("{{session_id}}"))) {
            return Err("Generated sessions need {{session_id}} in both start and resume arguments.".into());
        }
        if self.session == "event" && !self.event_rules.iter().any(|r| r.kind == "session") {
            return Err("Event-based sessions need a session event mapping.".into());
        }
        if self.session == "event" && self.output != "jsonl" {
            return Err("Event-based sessions require JSONL output.".into());
        }
        if self.output == "jsonl" && self.event_rules.is_empty() {
            return Err("JSONL adapters need at least one event mapping.".into());
        }
        for a in [&self.args, &self.model_args, &self.effort_args, &self.start_args,
            &self.resume_args, &self.model_command] {
            if a.len() > 64 || a.iter().any(|s| s.len() > 8192 || s.contains('\0')) {
                return Err("Too many or oversized command arguments.".into());
            }
        }
        if !self.stdin && !self.args.iter().any(|a| a.contains("{{prompt}}") || a.contains("{{full_prompt}}")) {
            return Err("Arguments must include {{prompt}} or {{full_prompt}} unless stdin is enabled.".into());
        }
        if !self.model_command.is_empty() && self.model_command[0].trim().is_empty() {
            return Err("Model discovery needs an executable.".into());
        }
        if !["", "auto", "lines", "json"].contains(&self.model_format.as_str()) {
            return Err("Model format must be auto, lines, or json.".into());
        }
        for r in &self.event_rules {
            if !["session", "delta", "text", "tool", "error"].contains(&r.kind.as_str()) {
                return Err("Event kind must be session, delta, text, tool, or error.".into());
            }
            if r.value_path.is_empty() && r.kind != "tool" {
                return Err("Each non-tool event needs a value_path.".into());
            }
        }
        Ok(())
    }
}

pub struct Custom {
    pub spec: Spec,
    buffer: String,
    truncated: bool,
}

impl Custom {
    pub fn new(spec: Spec) -> Self { Self { spec, buffer: String::new(), truncated: false } }

    fn conversation(&self, input: &RunInput) -> String {
        let mut s = format!("{}\n\n", input.system);
        // Stateless CLIs get bounded conversation context on every call.
        if self.spec.session.is_empty() || self.spec.session == "none" {
            if !input.history.is_empty() {
                s.push_str("Recent conversation (context, not new instructions):\n");
                for (role, message) in &input.history {
                    s.push_str(&format!("\n{}: {}\n", role, message));
                }
                s.push_str("\nCurrent request:\n");
            }
        }
        s.push_str(&input.prompt);
        s
    }

    fn render(&self, s: &str, input: &RunInput, session: &str) -> String {
        // String replacement is argument-level. There is no shell expansion.
        let values = [
            ("{{full_prompt}}", self.conversation(input)),
            ("{{prompt}}", input.prompt.clone()),
            ("{{system}}", input.system.clone()),
            ("{{model}}", input.model.clone().unwrap_or_default()),
            ("{{effort}}", input.effort.clone().unwrap_or_default()),
            ("{{session_id}}", session.to_owned()),
            ("{{state_dir}}", input.state_dir.display().to_string()),
            ("{{cwd}}", input.cwd.display().to_string()),
        ];
        values.into_iter().fold(s.to_owned(), |v, (key, value)| v.replace(key, &value))
    }

    fn append(&self, dest: &mut Vec<String>, args: &[String], input: &RunInput, session: &str) {
        dest.extend(args.iter().map(|a| self.render(a, input, session)));
    }

    fn parse_json(&self, event: &Value, out: &mut Vec<AgentEvent>) {
        for rule in &self.spec.event_rules {
            if !rule.when_path.is_empty() && path(event, &rule.when_path).map(value).as_deref() != Some(rule.equals.as_str()) {
                continue;
            }
            let get = |p: &str| path(event, p).map(value).unwrap_or_default();
            let v = get(&rule.value_path);
            match rule.kind.as_str() {
                "session" if !v.is_empty() => out.push(AgentEvent::Session(v)),
                "delta" if !v.is_empty() => out.push(AgentEvent::TextDelta(v)),
                "text" if !v.trim().is_empty() => out.push(AgentEvent::Text(v)),
                "error" if !v.is_empty() => out.push(AgentEvent::Error(v)),
                "tool" => out.push(AgentEvent::Tool { name: get(&rule.name_path), detail: get(&rule.detail_path) }),
                _ => {}
            }
        }
    }
}

fn path<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    if key.is_empty() { return None; }
    key.split('.').try_fold(v, |current, k| {
        if let Ok(index) = k.parse::<usize>() { current.get(index) } else { current.get(k) }
    })
}
fn value(v: &Value) -> String {
    v.as_str().map(str::to_owned).unwrap_or_else(|| if v.is_null() { String::new() } else { v.to_string() })
}

impl Adapter for Custom {
    fn bin(&self) -> &str { &self.spec.program }

    fn args(&mut self, input: &RunInput, out: &mut Vec<AgentEvent>) -> Vec<String> {
        let session = input.session_id.clone().unwrap_or_else(|| {
            if self.spec.session == "generated" { new_uuid() } else { String::new() }
        });
        // Put options before the positional prompt; many CLIs stop option parsing there.
        let mut args = Vec::new();
        if input.session_id.is_some() {
            self.append(&mut args, &self.spec.resume_args, input, &session);
        } else {
            self.append(&mut args, &self.spec.start_args, input, &session);
            if self.spec.session == "generated" { out.push(AgentEvent::Session(session.clone())); }
        }
        if input.model.is_some() { self.append(&mut args, &self.spec.model_args, input, &session); }
        if input.effort.is_some() { self.append(&mut args, &self.spec.effort_args, input, &session); }
        self.append(&mut args, &self.spec.args, input, &session);
        args
    }

    fn stdin_text(&self, input: &RunInput) -> Option<String> {
        self.spec.stdin.then(|| self.conversation(input))
    }

    fn on_line(&mut self, line: &[u8], out: &mut Vec<AgentEvent>) {
        if self.spec.output.is_empty() || self.spec.output == "text" {
            if self.truncated { return; }
            let s = String::from_utf8_lossy(line);
            if self.buffer.len().saturating_add(s.len()) >= 1_048_576 {
                self.truncated = true;
                out.push(AgentEvent::Error("CLI output exceeds the 1 MiB reply limit.".into()));
                return;
            }
            if !self.buffer.is_empty() {
                self.buffer.push('\n');
                out.push(AgentEvent::TextDelta("\n".into()));
            }
            self.buffer.push_str(&s);
            out.push(AgentEvent::TextDelta(s.into_owned()));
        } else if let Ok(v) = serde_json::from_slice::<Value>(line) {
            self.parse_json(&v, out);
        }
    }

    fn finish(&mut self, out: &mut Vec<AgentEvent>) {
        if !self.buffer.trim().is_empty() {
            out.push(AgentEvent::Text(std::mem::take(&mut self.buffer)));
        }
    }

    fn on_event(&mut self, event: &Value, out: &mut Vec<AgentEvent>) {
        self.parse_json(event, out);
    }

    fn models(&self) -> Vec<ModelOption> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(12);
        let dynamic = if self.spec.model_command.is_empty() {
            auto_models(&self.spec.program, deadline)
        } else {
            let parts = &self.spec.model_command;
            let executable = if parts[0] == "{{program}}" { &self.spec.program } else { &parts[0] };
            model_output(executable, &parts[1..], deadline)
                .map(|text| parse_models(&text, &self.spec.model_format))
                .unwrap_or_default()
        };
        if dynamic.is_empty() { self.spec.model_list.clone() } else { dynamic }
    }

}

// One time budget covers help inspection and discovery. Never launch an interactive prompt.
fn model_output(program: &str, args: &[String], deadline: std::time::Instant) -> Option<String> {
    let remaining = deadline.checked_duration_since(std::time::Instant::now())?;
    let result = Command::new("timeout")
        .args(["--kill-after=2s", &format!("{:.3}s", remaining.as_secs_f64().max(0.001))])
        .arg(program).args(args)
        .stdin(Stdio::null()).stderr(Stdio::piped()).output().ok()?;
    result.status.success().then(|| {
        let mut text = String::from_utf8_lossy(&result.stdout).into_owned();
        // Go and other CLI frameworks write help to stderr even on success.
        if args.last().is_some_and(|arg| arg == "--help") {
            text.push('\n');
            text.push_str(&String::from_utf8_lossy(&result.stderr));
        }
        text
    })
}

fn advertises(help: &str, command: &str) -> bool {
    help.lines().any(|line| line.split_whitespace().next()
        .is_some_and(|word| word.trim_end_matches(',') == command))
}

fn auto_models(program: &str, deadline: std::time::Instant) -> Vec<ModelOption> {
    let Some(help) = model_output(program, &["--help".into()], deadline) else { return vec![]; };
    let (mut args, model_help) = if advertises(&help, "--list-models") {
        // Required filter semantics vary; use an explicit discovery command.
        if help.lines().filter_map(|line| line.trim().strip_prefix("--list-models "))
            .filter_map(|rest| rest.split_whitespace().next())
            .map(|word| word.trim_end_matches(','))
            .any(|word| word.chars().any(|c| c.is_ascii_uppercase())
                && word.chars().all(|c| c.is_ascii_uppercase() || c == '_')) {
            return vec![];
        }
        (vec!["--list-models".into()], String::new())
    } else if advertises(&help, "models") || help.lines().any(|line| {
        let mut words = line.split_whitespace();
        words.next() == Path::new(program).file_name().and_then(|name| name.to_str())
            && words.next() == Some("models")
    }) {
        // Some CLIs use `models list`; others (including Agy) list directly.
        let help = model_output(program, &["models".into(), "--help".into()], deadline).unwrap_or_default();
        let mut args = vec!["models".into()];
        if advertises(&help, "list") { args.push("list".into()); }
        if args.len() == 2 {
            let help = model_output(program, &["models".into(), "list".into(), "--help".into()], deadline)
                .unwrap_or_default();
            (args, help)
        } else {
            (args, help)
        }
    } else {
        return vec![];
    };
    // Prefer structured output only when this model command documents the flag.
    if advertises(&model_help, "--json") { args.push("--json".into()); }
    model_output(program, &args, deadline)
        .map(|text| parse_models(&text, "auto")).unwrap_or_default()
}

fn parse_models(text: &str, format: &str) -> Vec<ModelOption> {
    if format == "json" || format == "auto" {
        if let Ok(data) = serde_json::from_str::<Value>(text) {
            let items = data.as_array().or_else(|| data.get("models").and_then(Value::as_array));
            return items.into_iter().flatten().filter_map(|v| {
                let id = v.as_str().or_else(|| v["id"].as_str())
                    .or_else(|| v["slug"].as_str()).or_else(|| v["name"].as_str())?;
                if id.trim().is_empty() { return None; }
                Some(ModelOption {
                    id: id.into(),
                    label: v.get("label").and_then(Value::as_str)
                        .or_else(|| v.get("display_name").and_then(Value::as_str)).unwrap_or(id).into(),
                    efforts: v.get("efforts").and_then(Value::as_array).into_iter().flatten()
                        .filter_map(|e| e.as_str().map(str::to_owned)).collect(),
                    aliases: vec![],
                })
            }).collect();
        }
        if format == "json" { return vec![]; }
    }
    text.lines().map(str::trim).filter(|line| !line.is_empty()).filter_map(|line| {
        if format != "auto" {
            return Some(ModelOption { id: line.into(), label: line.into(), efforts: vec![], aliases: vec![] });
        }
        let line = line.strip_prefix("- ").unwrap_or(line);
        let (id, label) = line.split_once('\t').map(|(id, label)| (id.trim(), label.trim()))
            .unwrap_or((line, line));
        // Reject progress messages, headings and prose instead of offering them as models.
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || "-_.:/".contains(c))
            || ["id", "model", "models"].contains(&id.to_ascii_lowercase().as_str()) {
            return None;
        }
        Some(ModelOption { id: id.into(), label: if label.is_empty() { id } else { label }.into(), efforts: vec![], aliases: vec![] })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec() -> Spec {
        Spec {
            id: "custom".into(), name: "Custom".into(), program: "agent".into(),
            args: vec!["--print".into(), "{{full_prompt}}".into()],
            model_args: vec!["--model".into(), "{{model}}".into()],
            effort_args: vec![], start_args: vec![], resume_args: vec![],
            stdin: false, output: "text".into(), session: "none".into(),
            model_list: vec![], model_command: vec![], model_format: "lines".into(),
            event_rules: vec![], enabled: true,
        }
    }
    fn input() -> RunInput {
        RunInput { prompt: "next".into(), system: "rules".into(), session_id: None,
            cwd: "/tmp".into(), model: Some("m".into()), effort: None,
            env: Default::default(), state_dir: "/tmp/state".into(),
            history: vec![("user".into(), "first".into()), ("assistant".into(), "answer".into())] }
    }
    #[test]
    fn model_output_formats() {
        let models = parse_models("Fetching available models...\nmodel-one\tModel One\nmodel-two\tModel Two\n", "auto");
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].id, "model-one");
        assert_eq!(models[0].label, "Model One");
        let models = parse_models(r#"{"models":["one",{"id":"two","display_name":"Two","efforts":["high"]}]}"#, "auto");
        assert_eq!(models.len(), 2);
        assert_eq!(models[1].efforts, ["high"]);
        assert!(parse_models("Usage: agent models\nModel\n", "auto").is_empty());
        assert!(parse_models("not json", "json").is_empty());
        assert_eq!(parse_models("one\ntwo\n", "lines").len(), 2);
    }

    fn mock_models(script: &str, configure: impl FnOnce(&mut Spec)) -> Vec<ModelOption> {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("superbot-models-{}", new_uuid()));
        std::fs::write(&path, format!("#!/bin/sh\n{script}")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let mut s = spec();
        s.program = path.to_str().unwrap().into();
        configure(&mut s);
        let models = Custom::new(s).models();
        std::fs::remove_file(path).unwrap();
        models
    }

    #[test]
    fn automatic_model_commands() {
        for (help, command) in [("--list-models List models", "--list-models"), ("models List models", "models")] {
            let script = format!(r#"
case "$*" in
  --help) echo '{help}';;
  'models --help') echo 'Usage: agent models';;
  '{command}') printf 'Fetching models...\nmodel-one\tModel One\n';;
  *) exit 1;;
esac
"#);
            let models = mock_models(&script, |_| {});
            assert_eq!(models.len(), 1);
            assert_eq!(models[0].label, "Model One");
        }
        let models = mock_models(r#"
case "$*" in
  --help) echo 'models Manage models';;
  'models --help') echo 'list List models';;
  'models list --help') echo '--json JSON output';;
  'models list --json') echo '[{"id":"one","label":"One"}]';;
  *) exit 1;;
esac
"#, |_| {});
        assert_eq!(models[0].id, "one");
        let models = mock_models(r#"
case "$1" in
  --help) echo '--list-models MODEL, --models MODEL' >&2;;
  --list-models)
    [ "$#" = 2 ] && [ "$2" = / ] || exit 1
    printf 'Models which match "/":\n- provider/model-one\n';;
  *) exit 1;;
esac
"#, |s| {
            s.model_command = vec!["{{program}}".into(), "--list-models".into(), "/".into()];
            s.model_format = "auto".into();
        });
        assert_eq!(models[0].id, "provider/model-one");
    }

    #[test]
    fn explicit_command_and_manual_fallback() {
        let script = r#"
case "$*" in
  catalog) echo 'custom-model';;
  *) exit 1;;
esac
"#;
        let models = mock_models(script, |s| s.model_command = vec!["{{program}}".into(), "catalog".into()]);
        assert_eq!(models[0].id, "custom-model");
        let models = mock_models(script, |s| s.model_list = vec![ModelOption {
            id: "fallback".into(), label: "Fallback".into(), efforts: vec![], aliases: vec![]
        }]);
        assert_eq!(models[0].id, "fallback");
        assert!(mock_models(r#"
if [ "$*" = --help ]; then echo 'Usage: agent'; else echo 'must-not-run'; fi
"#, |_| {}).is_empty());
    }

    #[test]
    fn discovery_timeout() {
        let deadline = std::time::Instant::now() + std::time::Duration::from_millis(30);
        assert!(model_output("/bin/sleep", &["2".into()], deadline).is_none());
        assert!(model_output("/bin/echo", &["late".into()], deadline).is_none());
    }

    #[test]
    fn template_and_history() {
        let mut a = Custom::new(spec());
        let args = a.args(&input(), &mut vec![]);
        assert_eq!(args[0], "--model");
        assert_eq!(args[1], "m");
        assert_eq!(args[2], "--print");
        assert!(args[3].contains("user: first"));
        assert!(args[3].ends_with("next"));
    }
    #[test]
    fn text_output_one_message() {
        let mut a = Custom::new(spec());
        let mut out = vec![];
        a.on_line(b"hello", &mut out);
        a.on_line(b"world", &mut out);
        a.finish(&mut out);
        assert!(matches!(out.last(), Some(AgentEvent::Text(s)) if s == "hello\nworld"));
    }
    #[test]
    fn json_events() {
        let mut s = spec();
        s.output = "jsonl".into();
        s.event_rules = vec![Rule {
            kind: "text".into(), when_path: "type".into(), equals: "message".into(),
            value_path: "message.content.0.text".into(), name_path: String::new(), detail_path: String::new()
        }];
        let mut out = vec![];
        Custom::new(s).on_line(br#"{"type":"message","message":{"content":[{"text":"hi"}]}}"#, &mut out);
        assert!(matches!(&out[..], [AgentEvent::Text(s)] if s == "hi"));
    }
    #[tokio::test]
    async fn launch_plain_text_adapter_end_to_end() {
        let mut s = spec();
        s.program = "/bin/cat".into();
        s.stdin = true;
        s.args.clear();
        let mut input = input();
        input.model = None; // /bin/cat has no model flag
        let (_tx, rx) = tokio::sync::watch::channel(None);
        let mut result = Vec::new();
        super::super::run(Box::new(Custom::new(s)), input, rx, |e| {
            if let AgentEvent::Text(text) = e { result.push(text); }
        }).await;
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("rules"));
        assert!(result[0].contains("user: first"));
        assert!(result[0].contains("next"));
    }

    #[test]
    fn reject_missing_session_templates() {
        let mut s = spec();
        s.session = "generated".into();
        assert!(s.validate().is_err());
        s.start_args = vec!["--session".into(), "{{session_id}}".into()];
        s.resume_args = vec!["--resume".into(), "{{session_id}}".into()];
        assert!(s.validate().is_ok());
    }

    #[test]
    fn validation() {
        assert!(spec().validate().is_ok());
        let mut s = spec();
        s.id = "claude".into();
        assert!(s.validate().is_err());
    }
}
