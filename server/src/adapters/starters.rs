//! Editable custom profiles; all harness-specific defaults stay in the adapter layer.
use super::Spec;
use serde_json::json;

pub fn starters() -> Vec<Spec> {
    serde_json::from_value(json!([
        {
            "id": "agy", "name": "Agy", "program": "agy",
            "args": ["--print", "{{full_prompt}}"],
            "model_args": ["--model", "{{model}}"],
            "output": "text", "session": "none"
        },
        {
            "id": "aider", "name": "Aider", "program": "aider",
            "args": ["--yes-always", "--no-pretty", "--no-stream",
                "--no-auto-commits", "--no-dirty-commits", "--no-check-update",
                "--input-history-file", "{{state_dir}}/aider-input-history",
                "--chat-history-file", "{{state_dir}}/aider-chat-history.md",
                "--message", "{{full_prompt}}"],
            "model_args": ["--model", "{{model}}"],
            "model_command": ["{{program}}", "--no-check-update", "--no-git", "--no-pretty", "--list-models", "/"],
            "model_format": "auto",
            "output": "text", "session": "none"
        },
        {
            "id": "opencode", "name": "OpenCode", "program": "opencode",
            "start_args": ["run"], "resume_args": ["run"],
            "args": ["{{full_prompt}}"],
            "model_args": ["--model", "{{model}}"],
            "output": "text", "session": "none"
        }
    ])).expect("valid built-in starter definitions")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::{Adapter, custom::Custom, RunInput};

    #[test]
    fn starters_validate_and_build_commands() {
        for spec in starters() {
            spec.validate().unwrap();
            // Discovery is automatic or preconfigured; no model IDs to maintain.
            assert!(spec.model_list.is_empty());
            let id = spec.id.clone();
            let is_opencode = id == "opencode";
            let mut adapter = Custom::new(spec);
            for model in [None, Some("provider/model".into())] {
                let input = RunInput {
                    prompt: "hello".into(), system: "rules".into(), session_id: None,
                    cwd: "/tmp".into(), model: model.clone(), effort: None,
                    env: Default::default(), state_dir: "/tmp/state".into(), history: vec![],
                };
                let args = adapter.args(&input, &mut vec![]);
                if is_opencode { assert_eq!(args[0], "run"); }
                assert!(args.last().unwrap().contains("hello"));
                if model.is_some() {
                    let index = args.iter().position(|arg| arg == "--model").unwrap();
                    assert_eq!(args[index + 1], "provider/model");
                }
            }
        }
    }
}
