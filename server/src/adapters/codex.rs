//! Codex CLI: `codex exec --json`. Resumes with `codex exec resume <thread id>`.
//! Limitation: Codex fixes its instructions when a session starts, so later memory
//! changes reach it only through its memory file, not the system prompt.

use super::{output, short, str_of, Adapter, AgentEvent, ModelOption, RunInput};
use serde_json::Value;

pub struct Codex;

impl Adapter for Codex {
    fn bin(&self) -> &str {
        "codex"
    }

    fn args(&mut self, input: &RunInput, _out: &mut Vec<AgentEvent>) -> Vec<String> {
        let mut a: Vec<String> = vec!["exec".into()];
        if input.session_id.is_some() {
            a.push("resume".into());
        }
        a.extend(["--json", "--skip-git-repo-check", "--dangerously-bypass-approvals-and-sandbox", "-c"].map(String::from));
        a.push(format!("developer_instructions={}", Value::String(input.system.clone())));
        if let Some(m) = &input.model {
            a.extend(["-m".into(), m.clone()]);
        }
        if let Some(e) = &input.effort {
            a.extend(["-c".into(), format!("model_reasoning_effort={}", Value::String(e.clone()))]);
        }
        a.push("--".into());
        if let Some(id) = &input.session_id {
            a.push(id.clone());
        }
        a.push(input.prompt.clone());
        a
    }

    fn on_event(&mut self, e: &Value, out: &mut Vec<AgentEvent>) {
        let item = &e["item"];
        let kind = str_of(item, "type");
        match str_of(e, "type") {
            "thread.started" => out.push(AgentEvent::Session(str_of(e, "thread_id").into())),
            "item.completed" if kind == "agent_message" && !str_of(item, "text").trim().is_empty() => {
                out.push(AgentEvent::Text(str_of(item, "text").into()))
            }
            "item.started" if kind == "command_execution" => {
                out.push(AgentEvent::Tool { name: "shell".into(), detail: unwrap(str_of(item, "command")) })
            }
            "item.started" if !kind.is_empty() && kind != "reasoning" => {
                let detail = match (item["server"].as_str(), item["tool"].as_str()) {
                    (Some(s), Some(t)) => format!("{s}.{t}"),
                    _ => str_of(item, "query").into(),
                };
                out.push(AgentEvent::Tool { name: kind.into(), detail })
            }
            "turn.failed" | "error" => {
                let msg = e["error"]["message"].as_str().or(e["message"].as_str()).unwrap_or("Codex failed");
                out.push(AgentEvent::Error(msg.into()))
            }
            _ => {}
        }
    }

    /// From `codex debug models`: the models this login can use, and their efforts.
    fn models(&self) -> Vec<ModelOption> {
        let v: Value = serde_json::from_str(&output("codex", &["debug", "models"])).unwrap_or_default();
        v["models"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| str_of(m, "visibility") == "list")
            .map(|m| ModelOption {
                id: str_of(m, "slug").into(),
                label: m["display_name"].as_str().unwrap_or(str_of(m, "slug")).into(),
                efforts: m["supported_reasoning_levels"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|l| l["effort"].as_str().map(String::from))
                    .collect(),
                aliases: vec![],
            })
            .collect()
    }
}

/// "/bin/bash -lc 'echo hi'" -> "echo hi"
fn unwrap(cmd: &str) -> String {
    let inner = cmd
        .split_once(" -lc '")
        .or_else(|| cmd.split_once(" -c '"))
        .and_then(|(_, rest)| rest.strip_suffix('\''))
        .unwrap_or(cmd);
    short(inner)
}
