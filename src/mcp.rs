//! `dogechain mcp`: registers the Dogechain MCP server (hosted at
//! dogechain.com; nothing runs locally) with AI agents, using each agent's own
//! command-line tool. The CLI never edits another program's config files; for
//! agents without such a tool, `--print` shows what to add by hand.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::Serialize;
use serde_json::{Value, json};

use crate::error::{CliError, Result};

pub const SERVER_NAME: &str = "dogechain";
pub const SERVER_URL: &str = "https://dogechain.com/mcp";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Agent {
    Claude,
    Codex,
}

const AGENTS: [Agent; 2] = [Agent::Claude, Agent::Codex];

impl Agent {
    fn id(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
        }
    }

    fn program(self) -> &'static str {
        self.id()
    }

    /// Codex has no per-project MCP configuration.
    fn supports_project(self) -> bool {
        matches!(self, Agent::Claude)
    }

    fn add_args(self, project: bool) -> Vec<&'static str> {
        match self {
            Agent::Claude => vec![
                "mcp",
                "add",
                "--transport",
                "http",
                "--scope",
                if project { "project" } else { "user" },
                SERVER_NAME,
                SERVER_URL,
            ],
            Agent::Codex => vec!["mcp", "add", SERVER_NAME, "--url", SERVER_URL],
        }
    }

    fn remove_args(self, project: bool) -> Vec<&'static str> {
        match self {
            Agent::Claude => vec![
                "mcp",
                "remove",
                "--scope",
                if project { "project" } else { "user" },
                SERVER_NAME,
            ],
            Agent::Codex => vec!["mcp", "remove", SERVER_NAME],
        }
    }

    /// The URL of the agent's `dogechain` server, if it has one.
    fn configured_url(self, program: &Path) -> Result<Option<String>> {
        match self {
            Agent::Claude => {
                let out = run(program, &["mcp", "get", SERVER_NAME])?;
                if !out.status.success() {
                    return Ok(None);
                }
                let text = String::from_utf8_lossy(&out.stdout);
                if text.contains(SERVER_URL) {
                    return Ok(Some(SERVER_URL.to_owned()));
                }
                Ok(Some(
                    text.lines()
                        .find_map(|l| l.trim().strip_prefix("URL:"))
                        .map(|u| u.trim().to_owned())
                        .unwrap_or_default(),
                ))
            }
            Agent::Codex => {
                let out = run(program, &["mcp", "get", SERVER_NAME, "--json"])?;
                if !out.status.success() {
                    return Ok(None);
                }
                if String::from_utf8_lossy(&out.stdout).contains(SERVER_URL) {
                    return Ok(Some(SERVER_URL.to_owned()));
                }
                let v: Value = serde_json::from_slice(&out.stdout).unwrap_or(Value::Null);
                let url = v["transport"]["url"].as_str().or(v["url"].as_str());
                Ok(Some(url.unwrap_or_default().to_owned()))
            }
        }
    }
}

/// Finds a program on PATH, including Windows `.exe` and `.cmd` shims.
fn find_program(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let names: Vec<String> = if cfg!(windows) {
        vec![
            format!("{name}.exe"),
            format!("{name}.cmd"),
            name.to_owned(),
        ]
    } else {
        vec![name.to_owned()]
    };
    std::env::split_paths(&path)
        .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
        .find(|p| p.is_file())
}

fn run(program: &Path, args: &[&str]) -> Result<Output> {
    Command::new(program)
        .args(args)
        .output()
        .map_err(|e| CliError::Other(format!("could not run {}: {e}", program.display())))
}

#[derive(Serialize)]
struct Outcome {
    agent: &'static str,
    result: String,
}

fn report(outcomes: &[Outcome], json: bool, out: &mut dyn Write) -> Result<()> {
    if json {
        writeln!(
            out,
            "{:#}",
            json!({ "status": "success", "data": { "server": SERVER_URL, "agents": outcomes } })
        )?;
    } else {
        for o in outcomes {
            let name = AGENTS
                .iter()
                .find(|a| a.id() == o.agent)
                .map(|a| a.name())
                .unwrap_or(o.agent);
            writeln!(out, "{name}: {}", o.result)?;
        }
    }
    Ok(())
}

fn found_agents() -> Result<Vec<(Agent, PathBuf)>> {
    let found: Vec<_> = AGENTS
        .iter()
        .filter_map(|&a| find_program(a.program()).map(|p| (a, p)))
        .collect();
    if found.is_empty() {
        return Err(CliError::NotFound(
            "neither `claude` nor `codex` was found on PATH; \
             run `dogechain mcp install --print` for other apps"
                .into(),
        ));
    }
    Ok(found)
}

fn command_failed(agent: Agent, out: &Output) -> CliError {
    let stderr = String::from_utf8_lossy(&out.stderr);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let detail = stderr
        .lines()
        .chain(stdout.lines())
        .find(|l| !l.trim().is_empty());
    CliError::Other(format!(
        "{} could not update its MCP servers: {}",
        agent.name(),
        detail.unwrap_or("no details")
    ))
}

pub fn install(project: bool, force: bool, json: bool, out: &mut dyn Write) -> Result<()> {
    let mut outcomes = Vec::new();
    let mut skipped = false;
    for (agent, program) in found_agents()? {
        if project && !agent.supports_project() {
            outcomes.push(Outcome {
                agent: agent.id(),
                result: "skipped: no per-project setting; run without --project".into(),
            });
            continue;
        }
        let result = match agent.configured_url(&program)? {
            Some(url) if url == SERVER_URL => "already added".to_owned(),
            Some(url) if !force => {
                skipped = true;
                format!(
                    "skipped: a server named {SERVER_NAME} already points to {} (use --force to replace)",
                    if url.is_empty() {
                        "something else"
                    } else {
                        &url
                    }
                )
            }
            existing => {
                if existing.is_some() {
                    let removed = run(&program, &agent.remove_args(project))?;
                    if !removed.status.success() {
                        return Err(command_failed(agent, &removed));
                    }
                }
                let added = run(&program, &agent.add_args(project))?;
                if !added.status.success() {
                    return Err(command_failed(agent, &added));
                }
                if existing.is_some() {
                    "replaced"
                } else {
                    "added"
                }
                .to_owned()
            }
        };
        outcomes.push(Outcome {
            agent: agent.id(),
            result,
        });
    }
    report(&outcomes, json, out)?;
    if !json {
        writeln!(
            out,
            "Other apps (Cursor, VS Code, Gemini CLI, Claude.ai, ChatGPT): dogechain mcp install --print"
        )?;
    }
    if skipped {
        return Err(CliError::Other(
            "some agents were not changed because they already had a different dogechain server"
                .into(),
        ));
    }
    Ok(())
}

pub fn status(json: bool, out: &mut dyn Write) -> Result<()> {
    let mut outcomes = Vec::new();
    for agent in AGENTS {
        let result = match find_program(agent.program()) {
            None => "not found on PATH".to_owned(),
            Some(program) => match agent.configured_url(&program)? {
                None => "not added".to_owned(),
                Some(url) if url == SERVER_URL => format!("added ({SERVER_URL})"),
                Some(url) => format!("a server named {SERVER_NAME} points to {url}"),
            },
        };
        outcomes.push(Outcome {
            agent: agent.id(),
            result,
        });
    }
    report(&outcomes, json, out)
}

pub fn uninstall(project: bool, force: bool, json: bool, out: &mut dyn Write) -> Result<()> {
    let mut outcomes = Vec::new();
    let mut skipped = false;
    for (agent, program) in found_agents()? {
        if project && !agent.supports_project() {
            outcomes.push(Outcome {
                agent: agent.id(),
                result: "skipped: no per-project setting".into(),
            });
            continue;
        }
        let result = match agent.configured_url(&program)? {
            None => "not added".to_owned(),
            Some(url) if url != SERVER_URL && !force => {
                skipped = true;
                format!("skipped: {SERVER_NAME} points to {url}, not Dogechain (use --force)")
            }
            Some(_) => {
                let removed = run(&program, &agent.remove_args(project))?;
                if !removed.status.success() {
                    return Err(command_failed(agent, &removed));
                }
                "removed".to_owned()
            }
        };
        outcomes.push(Outcome {
            agent: agent.id(),
            result,
        });
    }
    report(&outcomes, json, out)?;
    if skipped {
        return Err(CliError::Other(
            "some agents were not changed because their dogechain server is not Dogechain's".into(),
        ));
    }
    Ok(())
}

/// Setup for every supported app, for copying by hand. Changes nothing.
pub fn print(project: bool, json: bool, out: &mut dyn Write) -> Result<()> {
    let claude = format!("claude {}", Agent::Claude.add_args(project).join(" "));
    let codex = format!("codex {}", Agent::Codex.add_args(false).join(" "));
    let cursor = json!({ "mcpServers": { SERVER_NAME: { "url": SERVER_URL } } });
    let vscode = json!({ "servers": { SERVER_NAME: { "type": "http", "url": SERVER_URL } } });
    let gemini = json!({ "mcpServers": { SERVER_NAME: { "httpUrl": SERVER_URL } } });
    if json {
        let data = json!({
            "server": SERVER_URL,
            "claude_code": { "command": claude },
            "codex": { "command": codex },
            "cursor": { "file": if project { ".cursor/mcp.json" } else { "~/.cursor/mcp.json" }, "add": cursor },
            "vscode": { "file": ".vscode/mcp.json", "add": vscode },
            "gemini_cli": { "file": "~/.gemini/settings.json", "add": gemini },
            "claude_app": "Settings → Connectors → Add custom connector → the server URL",
            "chatgpt": "Settings → Apps & Connectors → Advanced settings → turn on Developer mode, then Create → the server URL, No authentication",
        });
        writeln!(out, "{:#}", json!({ "status": "success", "data": data }))?;
        return Ok(());
    }
    let cursor_file = if project {
        ".cursor/mcp.json"
    } else {
        "~/.cursor/mcp.json"
    };
    writeln!(
        out,
        "Dogechain MCP server: {SERVER_URL} (Streamable HTTP, no sign-in)\n"
    )?;
    writeln!(out, "Claude Code:\n  {claude}\n")?;
    writeln!(out, "Codex:\n  {codex}\n")?;
    writeln!(out, "Cursor: add to {cursor_file}\n  {cursor}\n")?;
    writeln!(out, "VS Code: add to .vscode/mcp.json\n  {vscode}\n")?;
    writeln!(
        out,
        "Gemini CLI: add to ~/.gemini/settings.json\n  {gemini}\n"
    )?;
    writeln!(
        out,
        "Claude.ai and Claude Desktop: Settings → Connectors → Add custom connector → {SERVER_URL}\n"
    )?;
    writeln!(
        out,
        "ChatGPT: Settings → Apps & Connectors → Advanced settings → turn on Developer mode, \
         then Create → {SERVER_URL}, No authentication"
    )?;
    writeln!(
        out,
        "\nMerge these into existing files rather than replacing them."
    )?;
    Ok(())
}
