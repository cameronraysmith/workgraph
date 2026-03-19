//! Placement output parsing and application.
//!
//! Placement agents produce structured output: a `wg edit` command (or `no-op`)
//! as the last line of their text response. This module parses that output from
//! the raw JSONL stream and optionally executes the command.

use anyhow::{Context, Result};
use std::path::Path;

/// Parsed placement command from agent output.
#[derive(Debug, Clone, PartialEq)]
pub enum PlacementCommand {
    /// A valid `wg edit` command with after/before edges.
    Edit {
        task_id: String,
        after: Vec<String>,
        before: Vec<String>,
    },
    /// No placement changes needed.
    NoOp,
}

/// Result of parsing placement output.
#[derive(Debug)]
pub enum PlacementParseResult {
    /// Successfully parsed a placement command.
    Ok(PlacementCommand),
    /// Output was unparseable.
    Unparseable(String),
    /// No text output found (agent produced nothing).
    Empty,
}

/// Extract text content from a Claude stream-json (JSONL) file.
///
/// Reads the raw_stream.jsonl and extracts text from `type: "assistant"` events.
/// Returns the concatenated text content from all assistant messages.
pub fn extract_text_from_stream(raw_stream_path: &Path) -> Result<String> {
    let content = std::fs::read_to_string(raw_stream_path)
        .with_context(|| format!("Failed to read stream file: {:?}", raw_stream_path))?;

    let mut text_parts: Vec<String> = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        let val: serde_json::Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let event_type = match val.get("type").and_then(|v| v.as_str()) {
            Some(t) => t,
            None => continue,
        };

        if event_type == "assistant" {
            // Extract text from message.content[] blocks
            if let Some(content) = val
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(|c| c.as_array())
            {
                for block in content {
                    if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                        if let Some(t) = block.get("text").and_then(|t| t.as_str()) {
                            text_parts.push(t.to_string());
                        }
                    }
                }
            }
        }
    }

    Ok(text_parts.join("\n"))
}

/// Parse the last non-empty line of text as a placement command.
///
/// Valid formats:
/// - `wg edit <task-id> --after <dep1>,<dep2> --before <dep3>`
/// - `no-op`
pub fn parse_placement_command(text: &str, expected_task_id: &str) -> PlacementParseResult {
    // Find the last non-empty line
    let last_line = text
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim());

    let last_line = match last_line {
        Some(line) => line,
        None => return PlacementParseResult::Empty,
    };

    // Check for no-op
    if last_line.eq_ignore_ascii_case("no-op") || last_line == "no-op" {
        return PlacementParseResult::Ok(PlacementCommand::NoOp);
    }

    // Try to parse as a wg edit command
    match parse_wg_edit_command(last_line, expected_task_id) {
        Some(cmd) => PlacementParseResult::Ok(cmd),
        None => PlacementParseResult::Unparseable(last_line.to_string()),
    }
}

/// Parse a `wg edit <task-id> --after <deps> --before <deps>` command string.
///
/// Returns `None` if the command doesn't match the expected format or targets
/// a different task than expected.
fn parse_wg_edit_command(line: &str, expected_task_id: &str) -> Option<PlacementCommand> {
    // Strip backtick fencing if present (agent may wrap in code block)
    let line = line
        .trim_start_matches('`')
        .trim_end_matches('`')
        .trim();

    let mut parts = line.split_whitespace();

    // Expect: wg edit <task-id> [--after <deps>] [--before <deps>]
    if parts.next()? != "wg" {
        return None;
    }
    if parts.next()? != "edit" {
        return None;
    }
    let task_id = parts.next()?;

    // Validate the task ID matches the expected source task
    if task_id != expected_task_id {
        return None;
    }

    let mut after = Vec::new();
    let mut before = Vec::new();

    // Parse remaining args as --after/--before pairs
    let remaining: Vec<&str> = parts.collect();
    let mut i = 0;
    while i < remaining.len() {
        match remaining[i] {
            "--after" | "--blocked-by" => {
                i += 1;
                if i < remaining.len() {
                    // Support comma-separated deps
                    for dep in remaining[i].split(',') {
                        let dep = dep.trim();
                        if !dep.is_empty() {
                            after.push(dep.to_string());
                        }
                    }
                }
            }
            "--before" => {
                i += 1;
                if i < remaining.len() {
                    for dep in remaining[i].split(',') {
                        let dep = dep.trim();
                        if !dep.is_empty() {
                            before.push(dep.to_string());
                        }
                    }
                }
            }
            _ => {
                // Unknown flag — reject
                return None;
            }
        }
        i += 1;
    }

    // Must have at least one edge
    if after.is_empty() && before.is_empty() {
        return None;
    }

    Some(PlacementCommand::Edit {
        task_id: task_id.to_string(),
        after,
        before,
    })
}

/// Apply a parsed placement command by running `wg edit`.
///
/// Returns Ok(true) if a command was executed, Ok(false) for no-op,
/// or Err if the wg edit command failed.
pub fn apply_placement_command(cmd: &PlacementCommand, dir: &Path) -> Result<bool> {
    match cmd {
        PlacementCommand::NoOp => Ok(false),
        PlacementCommand::Edit {
            task_id,
            after,
            before,
        } => {
            let graph_path = dir.join("graph.jsonl");
            let mut graph =
                workgraph::parser::load_graph(&graph_path).context("Failed to load graph")?;

            let task = graph
                .get_task_mut(task_id)
                .ok_or_else(|| anyhow::anyhow!("Task '{}' not found in graph", task_id))?;

            let mut modified = false;

            for dep in after {
                if !task.after.contains(dep) {
                    task.after.push(dep.clone());
                    modified = true;
                }
            }

            for dep in before {
                if !task.before.contains(dep) {
                    task.before.push(dep.clone());
                    modified = true;
                }
            }

            if modified {
                task.log.push(workgraph::graph::LogEntry {
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    actor: Some("placement".to_string()),
                    message: format!(
                        "Placement applied: {}{}",
                        if !after.is_empty() {
                            format!("--after {}", after.join(","))
                        } else {
                            String::new()
                        },
                        if !before.is_empty() {
                            format!(
                                "{}--before {}",
                                if !after.is_empty() { " " } else { "" },
                                before.join(",")
                            )
                        } else {
                            String::new()
                        },
                    ),
                });
                workgraph::parser::save_graph(&graph, &graph_path)
                    .context("Failed to save graph")?;
            }

            Ok(true)
        }
    }
}

/// Full pipeline: extract text from stream, parse placement command, and apply it.
///
/// Returns:
/// - `Ok(Some(msg))` — command applied or no-op, with a description
/// - `Err` — unparseable output or empty output
pub fn parse_and_apply(
    raw_stream_path: &Path,
    source_task_id: &str,
    workgraph_dir: &Path,
) -> Result<String> {
    let text = extract_text_from_stream(raw_stream_path)?;

    match parse_placement_command(&text, source_task_id) {
        PlacementParseResult::Ok(PlacementCommand::NoOp) => {
            Ok("no-op: no placement changes needed".to_string())
        }
        PlacementParseResult::Ok(ref cmd @ PlacementCommand::Edit { .. }) => {
            apply_placement_command(cmd, workgraph_dir)?;
            Ok(format!("applied placement for '{}'", source_task_id))
        }
        PlacementParseResult::Unparseable(line) => {
            anyhow::bail!("unparseable placement output: {}", line)
        }
        PlacementParseResult::Empty => {
            anyhow::bail!("placement agent produced no text output")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_noop() {
        let result = parse_placement_command("Some reasoning here\n\nno-op", "my-task");
        assert!(matches!(
            result,
            PlacementParseResult::Ok(PlacementCommand::NoOp)
        ));
    }

    #[test]
    fn test_parse_noop_case_insensitive() {
        let result = parse_placement_command("No-Op", "my-task");
        assert!(matches!(
            result,
            PlacementParseResult::Ok(PlacementCommand::NoOp)
        ));
    }

    #[test]
    fn test_parse_edit_after() {
        let result =
            parse_placement_command("reasoning...\n\nwg edit my-task --after dep-a", "my-task");
        match result {
            PlacementParseResult::Ok(PlacementCommand::Edit {
                task_id,
                after,
                before,
            }) => {
                assert_eq!(task_id, "my-task");
                assert_eq!(after, vec!["dep-a"]);
                assert!(before.is_empty());
            }
            other => panic!("Expected Edit, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_edit_before() {
        let result =
            parse_placement_command("reasoning...\n\nwg edit my-task --before dep-b", "my-task");
        match result {
            PlacementParseResult::Ok(PlacementCommand::Edit {
                task_id,
                after,
                before,
            }) => {
                assert_eq!(task_id, "my-task");
                assert!(after.is_empty());
                assert_eq!(before, vec!["dep-b"]);
            }
            other => panic!("Expected Edit, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_edit_both() {
        let result = parse_placement_command(
            "wg edit my-task --after dep-a --before dep-b",
            "my-task",
        );
        match result {
            PlacementParseResult::Ok(PlacementCommand::Edit {
                task_id,
                after,
                before,
            }) => {
                assert_eq!(task_id, "my-task");
                assert_eq!(after, vec!["dep-a"]);
                assert_eq!(before, vec!["dep-b"]);
            }
            other => panic!("Expected Edit, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_edit_comma_separated_deps() {
        let result = parse_placement_command(
            "wg edit my-task --after dep-a,dep-b,dep-c",
            "my-task",
        );
        match result {
            PlacementParseResult::Ok(PlacementCommand::Edit { after, .. }) => {
                assert_eq!(after, vec!["dep-a", "dep-b", "dep-c"]);
            }
            other => panic!("Expected Edit, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_edit_backtick_wrapped() {
        let result = parse_placement_command(
            "reasoning...\n\n`wg edit my-task --after dep-a`",
            "my-task",
        );
        match result {
            PlacementParseResult::Ok(PlacementCommand::Edit { after, .. }) => {
                assert_eq!(after, vec!["dep-a"]);
            }
            other => panic!("Expected Edit, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_wrong_task_id() {
        let result =
            parse_placement_command("wg edit wrong-task --after dep-a", "my-task");
        assert!(matches!(result, PlacementParseResult::Unparseable(_)));
    }

    #[test]
    fn test_parse_empty() {
        let result = parse_placement_command("", "my-task");
        assert!(matches!(result, PlacementParseResult::Empty));
    }

    #[test]
    fn test_parse_unparseable() {
        let result = parse_placement_command("just some random text", "my-task");
        assert!(matches!(result, PlacementParseResult::Unparseable(_)));
    }

    #[test]
    fn test_parse_edit_no_edges() {
        // wg edit with no --after or --before is invalid
        let result = parse_placement_command("wg edit my-task", "my-task");
        assert!(matches!(result, PlacementParseResult::Unparseable(_)));
    }

    #[test]
    fn test_extract_text_from_stream() {
        let mut file = NamedTempFile::new().unwrap();
        // Write some JSONL events
        writeln!(file, r#"{{"type":"system","system":"You are a placement agent"}}"#).unwrap();
        writeln!(file, r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"I'll analyze the task.\n\nwg edit my-task --after dep-a"}}]}}}}"#).unwrap();
        writeln!(file, r#"{{"type":"result","usage":{{"input_tokens":100,"output_tokens":50}}}}"#)
            .unwrap();
        file.flush().unwrap();

        let text = extract_text_from_stream(file.path()).unwrap();
        assert!(text.contains("wg edit my-task --after dep-a"));
    }

    #[test]
    fn test_extract_text_empty_stream() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"type":"result","usage":{{"input_tokens":0,"output_tokens":0}}}}"#)
            .unwrap();
        file.flush().unwrap();

        let text = extract_text_from_stream(file.path()).unwrap();
        assert!(text.is_empty());
    }

    #[test]
    fn test_parse_edit_with_blocked_by_alias() {
        let result = parse_placement_command(
            "wg edit my-task --blocked-by dep-a",
            "my-task",
        );
        match result {
            PlacementParseResult::Ok(PlacementCommand::Edit { after, .. }) => {
                assert_eq!(after, vec!["dep-a"]);
            }
            other => panic!("Expected Edit, got {:?}", other),
        }
    }

    #[test]
    fn test_trailing_whitespace_ignored() {
        let result = parse_placement_command(
            "reasoning...\n\nwg edit my-task --after dep-a\n\n  \n",
            "my-task",
        );
        match result {
            PlacementParseResult::Ok(PlacementCommand::Edit { after, .. }) => {
                assert_eq!(after, vec!["dep-a"]);
            }
            other => panic!("Expected Edit, got {:?}", other),
        }
    }
}
