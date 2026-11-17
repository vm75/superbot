//! UI-defined CLI harness. Commands are arrays of arguments, never shell scripts.
use super::{new_uuid, AgentEvent, Adapter, ModelOption, RunInput};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::process::{Command, Stdio};

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
        if self.program.trim().is_empty() || self.program.len() > 512 || self.program.contains('\0') {
            return Err("Executable must be a program name or absolute path.".into());
        }
        if !["", "text", "jsonl"].contains(&self.output.as_str()) {
            return Err("Output must be text or jsonl.".into());
        }
        if !["", "none", "generated", "event"].contains(&self.session.as_str()) {
            return Err("Session must be none, generated, or event.".into());
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
        if !["", "lines", "json"].contains(&self.model_format.as_str()) {
            return Err("Model format must be lines or json.".into());
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
}

impl Custom {
    pub fn new(spec: Spec) -> Self { Self { spec, buffer: String::new() } }

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
        if input.model.is_some() { self.append(&mut args, &self.spec.model_args, input, &session); }
        if input.effort.is_some() { self.append(&mut args, &self.spec.effort_args, input, &session); }
        if input.session_id.is_some() {
            self.append(&mut args, &self.spec.resume_args, input, &session);
        } else {
            self.append(&mut args, &self.spec.start_args, input, &session);
            if self.spec.session == "generated" { out.push(AgentEvent::Session(session.clone())); }
        }
        self.append(&mut args, &self.spec.args, input, &session);
        args
    }

    fn stdin_text(&self, input: &RunInput) -> Option<String> {
        self.spec.stdin.then(|| self.conversation(input))
    }

    fn on_line(&mut self, line: &[u8], out: &mut Vec<AgentEvent>) {
        if self.spec.output.is_empty() || self.spec.output == "text" {
            let s = String::from_utf8_lossy(line);
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
        if self.spec.model_command.is_empty() { return self.spec.model_list.clone(); }
        let parts = &self.spec.model_command;
        let executable = if parts[0] == "{{program}}" { &self.spec.program } else { &parts[0] };
        let result = Command::new(executable).args(&parts[1..]).stdin(Stdio::null()).stderr(Stdio::null()).output();
        let Ok(result) = result else { return self.spec.model_list.clone(); };
        if !result.status.success() { return self.spec.model_list.clone(); }
        let text = String::from_utf8_lossy(&result.stdout);
        let dynamic: Vec<ModelOption> = if self.spec.model_format == "json" {
            let data: Value = serde_json::from_str(&text).unwrap_or_default();
            let items = data.as_array().or_else(|| data.get("models").and_then(Value::as_array));
            items.into_iter().flatten().filter_map(|v| {
                if let Some(id) = v.as_str() {
                    Some(ModelOption { id: id.into(), label: id.into(), efforts: vec![], aliases: vec![] })
                } else {
                    let id = v["id"].as_str().or(v["name"].as_str())?;
                    Some(ModelOption {
                        id: id.into(),
                        label: v["label"].as_str().or(v["display_name"].as_str()).unwrap_or(id).into(),
                        efforts: v["efforts"].as_array().into_iter().flatten().filter_map(|e| e.as_str().map(str::to_owned)).collect(),
                        aliases: vec![],
                    })
                }
            }).collect()
        } else {
            text.lines().map(str::trim).filter(|line| !line.is_empty())
                .map(|id| ModelOption { id: id.into(), label: id.into(), efforts: vec![], aliases: vec![] }).collect()
        };
        if dynamic.is_empty() { self.spec.model_list.clone() } else { dynamic }
    }
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
    fn template_and_history() {
        let mut a = Custom::new(spec());
        let args = a.args(&input(), &mut vec![]);
        assert_eq!(args[0], "--print");
        assert!(args[1].contains("user: first"));
        assert!(args[1].ends_with("next"));
        assert_eq!(&args[2..], &["--model", "m"]);
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
    #[test]
    fn validation() {
        assert!(spec().validate().is_ok());
        let mut s = spec();
        s.id = "claude".into();
        assert!(s.validate().is_err());
    }
}
