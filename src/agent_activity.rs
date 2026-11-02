use serde_json::{Value, json};
use std::collections::HashMap;

#[derive(Default)]
pub struct Tracker {
    active: HashMap<String, Value>,
}
fn detail(input: &Value) -> String {
    ["command", "file_path", "path", "pattern", "query", "url"]
        .into_iter()
        .filter_map(|key| input[key].as_str())
        .next()
        .unwrap_or("")
        .chars()
        .take(2000)
        .collect()
}
impl Tracker {
    fn start(&mut self, runtime: &str, id: &str, tool: &str, input: &Value) -> Option<Value> {
        if self.active.contains_key(id) {
            return None;
        }
        let event =
            json!({"runtime":runtime,"id":id,"tool":tool,"phase":"start","detail":detail(input)});
        self.active.insert(id.into(), event.clone());
        Some(event)
    }
    fn end(&mut self, runtime: &str, id: &str, tool: &str, input: &Value, failed: bool) -> Value {
        let mut event = self.active.remove(id).unwrap_or_else(
            || json!({"runtime":runtime,"id":id,"tool":tool,"detail":detail(input)}),
        );
        event["phase"] = json!(if failed { "error" } else { "end" });
        event
    }
    pub fn observe(&mut self, runtime: &str, v: &Value) -> Vec<Value> {
        let mut out = vec![];
        match v["type"].as_str().unwrap_or("") {
            "assistant" if runtime == "claude" => {
                for block in v["message"]["content"].as_array().into_iter().flatten() {
                    if block["type"] == "tool_use"
                        && let (Some(id), Some(name)) =
                            (block["id"].as_str(), block["name"].as_str())
                        && let Some(event) = self.start(runtime, id, name, &block["input"])
                    {
                        out.push(event);
                    }
                }
            }
            "user" if runtime == "claude" => {
                for block in v["message"]["content"].as_array().into_iter().flatten() {
                    if block["type"] == "tool_result"
                        && let Some(id) = block["tool_use_id"].as_str()
                    {
                        out.push(self.end(
                            runtime,
                            id,
                            "tool",
                            &Value::Null,
                            block["is_error"] == true,
                        ));
                    }
                }
            }
            "tool_execution_start" => {
                if let Some(event) = self.start(
                    runtime,
                    v["toolCallId"].as_str().unwrap_or("tool"),
                    v["toolName"].as_str().unwrap_or("tool"),
                    &v["args"],
                ) {
                    out.push(event);
                }
            }
            // Extensions such as todo and web_search report failures in details.error, not isError.
            "tool_execution_end" => out.push(self.end(
                runtime,
                v["toolCallId"].as_str().unwrap_or("tool"),
                v["toolName"].as_str().unwrap_or("tool"),
                &v["args"],
                v["isError"] == true
                    || !matches!(
                        v["result"]["details"]["error"],
                        Value::Null | Value::Bool(false)
                    ),
            )),
            "item.started" | "item.completed" => {
                let item = &v["item"];
                let kind = item["type"].as_str().unwrap_or("");
                let tool = match kind {
                    "command_execution" => "Shell",
                    "file_change" => "Edit",
                    "mcp_tool_call" => item["tool"].as_str().unwrap_or("MCP"),
                    "web_search" => "WebSearch",
                    _ => return out,
                };
                let id = item["id"].as_str().unwrap_or(kind);
                let mut input = item.clone();
                if kind == "file_change" {
                    input["path"] = json!(
                        item["changes"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|c| c["path"].as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                }
                if v["type"] == "item.started" {
                    if let Some(event) = self.start(runtime, id, tool, &input) {
                        out.push(event);
                    }
                } else {
                    let failed = item["exit_code"].as_i64().is_some_and(|n| n != 0)
                        || item["status"] == "failed";
                    let mut event = self.end(runtime, id, tool, &input, failed);
                    if event["detail"] == "" {
                        event["detail"] = json!(detail(&input));
                    }
                    if let Some(code) = item["exit_code"].as_i64() {
                        event["exit_code"] = json!(code);
                    }
                    out.push(event);
                }
            }
            _ => {}
        }
        out
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn claude_tools_are_paired_without_exposing_reasoning_or_file_contents() {
        let mut t = Tracker::default();
        let message = json!({"type":"assistant","message":{"content":[{"type":"thinking","thinking":"private"},{"type":"tool_use","id":"a","name":"Bash","input":{"command":"cargo test","content":"private file contents"}}]}});
        let start = t.observe("claude", &message);
        assert_eq!(start.len(), 1);
        assert_eq!(start[0]["detail"], "cargo test");
        assert!(!start[0].to_string().contains("private"));
        assert!(t.observe("claude", &message).is_empty());
        let end=t.observe("claude",&json!({"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"a","is_error":true,"content":"private output"}]}}));
        assert_eq!(end[0]["tool"], "Bash");
        assert_eq!(end[0]["phase"], "error");
    }
    #[test]
    fn codex_and_pi_report_commands_and_completion() {
        let mut t = Tracker::default();
        assert_eq!(t.observe("codex",&json!({"type":"item.started","item":{"id":"1","type":"command_execution","command":"ls"}}))[0]["phase"],"start");
        let end=t.observe("codex",&json!({"type":"item.completed","item":{"id":"1","type":"command_execution","exit_code":2}}));
        assert_eq!(end[0]["detail"], "ls");
        assert_eq!(end[0]["phase"], "error");
        let start=t.observe("pi",&json!({"type":"tool_execution_start","toolCallId":"2","toolName":"read","args":{"path":"src/main.rs"}}));
        assert_eq!(start[0]["detail"], "src/main.rs");
        assert_eq!(
            t.observe(
                "pi",
                &json!({"type":"tool_execution_end","toolCallId":"2","toolName":"read"})
            )[0]["phase"],
            "end"
        );
        for error in [
            json!("#1 not found"),
            json!("unsupported_model"),
            json!(true),
        ] {
            let end = t.observe("pi",&json!({"type":"tool_execution_end","toolCallId":"3","toolName":"todo","isError":false,"result":{"details":{"error":error}}}));
            assert_eq!(end[0]["phase"], "error");
        }
        let end = t.observe("pi",&json!({"type":"tool_execution_end","toolCallId":"4","toolName":"web_fetch","isError":false,"result":{"details":{"url":"https://example.com"}}}));
        assert_eq!(end[0]["phase"], "end");
    }
}
