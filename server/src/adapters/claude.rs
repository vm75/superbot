//! Claude Code CLI: `claude -p --output-format stream-json`.
//! Resumes with `--resume <session id>`. The system prompt is applied fresh every run.

use super::{describe, new_uuid, str_of, Adapter, AgentEvent, ModelOption, RunInput};
use serde_json::Value;

#[derive(Default)]
pub struct Claude {
    id: String,
    saw_turn: bool,
}

impl Adapter for Claude {
    fn bin(&self) -> &str {
        "claude"
    }

    fn args(&mut self, input: &RunInput, _out: &mut Vec<AgentEvent>) -> Vec<String> {
        self.id = input.session_id.clone().unwrap_or_else(new_uuid);
        let mut a: Vec<String> = [
            "-p", "--output-format", "stream-json", "--verbose", "--include-partial-messages",
            "--dangerously-skip-permissions", "--append-system-prompt", &input.system,
        ]
        .map(String::from)
        .into();
        a.extend([if input.session_id.is_some() { "--resume" } else { "--session-id" }.into(), self.id.clone()]);
        if let Some(m) = &input.model {
            a.extend(["--model".into(), m.clone()]);
        }
        if let Some(e) = &input.effort {
            a.extend(["--effort".into(), e.clone()]);
        }
        a.extend(["--".into(), input.prompt.clone()]);
        a
    }

    fn on_event(&mut self, e: &Value, out: &mut Vec<AgentEvent>) {
        match str_of(e, "type") {
            "stream_event" => {
                let ev = &e["event"];
                if str_of(ev, "type") == "content_block_delta" && str_of(&ev["delta"], "type") == "text_delta" {
                    out.push(AgentEvent::TextDelta(str_of(&ev["delta"], "text").into()));
                }
            }
            "assistant" => {
                // Save the session only once it really exists.
                if !self.saw_turn {
                    self.saw_turn = true;
                    out.push(AgentEvent::Session(self.id.clone()));
                }
                for c in e["message"]["content"].as_array().into_iter().flatten() {
                    match str_of(c, "type") {
                        "text" if !str_of(c, "text").trim().is_empty() => out.push(AgentEvent::Text(str_of(c, "text").into())),
                        "tool_use" => out.push(AgentEvent::Tool { name: str_of(c, "name").into(), detail: describe(&c["input"]) }),
                        _ => {}
                    }
                }
            }
            "result" if e["is_error"].as_bool() == Some(true) => {
                let msg = e["result"].as_str().unwrap_or("Claude Code failed");
                out.push(AgentEvent::Error(msg.into()));
            }
            _ => {}
        }
    }

    /// Claude Code has no command to list models, so this is the current lineup.
    /// Update it when Anthropic ships new models.
    fn models(&self) -> Vec<ModelOption> {
        let efforts: Vec<String> = ["low", "medium", "high", "xhigh", "max"].map(String::from).into();
        [
            ("claude-fable-5-2", "Fable 5.2", "fable"),
            ("claude-opus-5-5", "Opus 5.5", "opus"),
            ("claude-sonnet-5-5", "Sonnet 5.5", "sonnet"),
            ("claude-haiku-5-5", "Haiku 5.5", "haiku"),
        ]
        .map(|(id, label, alias)| ModelOption { id: id.into(), label: label.into(), efforts: efforts.clone(), aliases: vec![alias.into()] })
        .into()
    }
}
