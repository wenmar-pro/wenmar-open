//! `setup claude|codex`: install the skill file, and register the MCP
//! server when asked to.
//!
//! The one file this writes is the skill file, in the directory the agent
//! reads skills from or the one given with `--dir`. Registering the MCP
//! server means running the agent's own command, which is only printed
//! unless `--yes` is given.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::ValueEnum;
use serde::Serialize;

use crate::env::Env;
use crate::error::{CliError, IO};

/// The skill file, as it is installed.
pub const SKILL: &str = include_str!("../skill/SKILL.md");

/// The line that marks an installed skill file as this tool's own, to
/// replace without asking.
pub const MARKER: &str = "<!-- wenmar-open-skill: managed.";

/// The directory the skill is installed under, inside the skills directory.
const SKILL_DIRECTORY: &str = "wenmar-open";

/// A coding agent this tool can set itself up for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    /// Claude Code.
    Claude,
    /// Codex.
    Codex,
}

impl Agent {
    pub const ALL: [Agent; 2] = [Agent::Claude, Agent::Codex];

    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    /// Where the agent reads a person's own skills from, under their home
    /// directory.
    fn skills_under_home(self) -> [&'static str; 2] {
        match self {
            Agent::Claude => [".claude", "skills"],
            Agent::Codex => [".agents", "skills"],
        }
    }

    /// The command that registers `wenmar-open mcp` with the agent.
    pub fn register_command(self) -> Vec<&'static str> {
        match self {
            Agent::Claude => vec![
                "claude",
                "mcp",
                "add",
                "--scope",
                "user",
                "wenmar-open",
                "--",
                "wenmar-open",
                "mcp",
            ],
            Agent::Codex => vec![
                "codex",
                "mcp",
                "add",
                "wenmar-open",
                "--",
                "wenmar-open",
                "mcp",
            ],
        }
    }

    /// The skills directory: `--dir`, or the agent's own under the home
    /// directory.
    pub fn skills_directory(self, env: &Env, flag: Option<&Path>) -> Result<PathBuf, CliError> {
        if let Some(directory) = flag.filter(|path| !path.as_os_str().is_empty()) {
            return Ok(directory.to_path_buf());
        }
        match &env.home {
            Some(home) => Ok(self
                .skills_under_home()
                .iter()
                .fold(home.clone(), |path, part| path.join(part))),
            None => Err(
                CliError::new(IO, "There is no home directory to install the skill in.")
                    .with_hint("Pass --dir with the directory your agent reads skills from."),
            ),
        }
    }

    /// The path of the skill file inside a skills directory.
    pub fn skill_path(directory: &Path) -> PathBuf {
        directory.join(SKILL_DIRECTORY).join("SKILL.md")
    }
}

/// What `setup` did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Setup {
    pub agent: Agent,
    /// The path of the skill file.
    pub skill: String,
    /// Whether the file was written. `false` when it was already current.
    pub skill_written: bool,
    /// The command that registers the MCP server.
    pub mcp_command: String,
    /// Whether that command was run. It is run only with `--yes`.
    pub mcp_registered: bool,
}

fn failed(path: &Path, what: &str, error: &std::io::Error) -> CliError {
    CliError::new(
        IO,
        format!("{} could not be {what}: {error}.", path.display()),
    )
    .with_hint("Pass --dir with a directory you can write to.")
}

/// Writes the skill file into `directory`. Returns its path and whether
/// anything was written. A file there that someone has edited, so that it
/// no longer carries the marker, is only replaced with `force`.
pub fn install(directory: &Path, force: bool) -> Result<(PathBuf, bool), CliError> {
    let path = Agent::skill_path(directory);
    match std::fs::read(&path) {
        Ok(existing) if existing == SKILL.as_bytes() => return Ok((path, false)),
        Ok(existing) => {
            let ours = String::from_utf8_lossy(&existing).contains(MARKER);
            if !ours && !force {
                return Err(CliError::new(
                    "skill_edited",
                    format!(
                        "{} has been changed by hand and was left as it is.",
                        path.display()
                    ),
                )
                .with_hint("Pass --force to replace it."));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(failed(&path, "read", &error)),
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| failed(&path, "created", &error))?;
    }
    std::fs::write(&path, SKILL).map_err(|error| failed(&path, "written", &error))?;
    Ok((path, true))
}

/// The file a program name means, looking through `PATH` as a shell does.
fn find_program(name: &str, path: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    let extensions: &[&str] = if cfg!(windows) {
        &["", ".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    std::env::split_paths(path?)
        .flat_map(|directory| {
            extensions
                .iter()
                .map(move |extension| directory.join(format!("{name}{extension}")))
        })
        .find(|candidate| candidate.is_file())
}

/// Runs the agent's own command to register the MCP server. It is given no
/// input, so it cannot wait for an answer.
pub fn register(agent: Agent, env: &Env) -> Result<(), CliError> {
    let command = agent.register_command();
    let by_hand = format!("Run it yourself: {}", command.join(" "));
    let (program, arguments) = match command.split_first() {
        Some(parts) => parts,
        None => return Ok(()),
    };
    let Some(found) = find_program(program, env.path.as_deref()) else {
        return Err(CliError::new(
            "register_failed",
            format!("`{program}` was not found on PATH, so the MCP server was not registered."),
        )
        .with_hint(by_hand));
    };
    let output = Command::new(found)
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| {
            CliError::new(
                "register_failed",
                format!("`{program}` could not be run: {error}."),
            )
            .with_hint(by_hand.clone())
        })?;
    if output.status.success() {
        return Ok(());
    }
    let said: String = String::from_utf8_lossy(&output.stderr)
        .trim()
        .chars()
        .take(300)
        .collect();
    Err(CliError::new(
        "register_failed",
        format!("`{program}` did not register the MCP server: {said}"),
    )
    .with_hint(by_hand))
}

/// Installs the skill for `agent`, and registers the MCP server if `yes`.
pub fn run(
    agent: Agent,
    env: &Env,
    directory: Option<&Path>,
    force: bool,
    yes: bool,
) -> Result<Setup, CliError> {
    let directory = agent.skills_directory(env, directory)?;
    let (path, written) = install(&directory, force)?;
    if yes {
        register(agent, env)?;
    }
    Ok(Setup {
        agent,
        skill: path.display().to_string(),
        skill_written: written,
        mcp_command: agent.register_command().join(" "),
        mcp_registered: yes,
    })
}

#[cfg(test)]
mod tests {
    use clap::{CommandFactory, Parser};

    use super::*;
    use crate::cli::Cli;

    /// Splits a command line the way a shell does, for the plain cases a
    /// skill file shows: words, and text in single or double quotes.
    fn words(line: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut word = String::new();
        let mut started = false;
        let mut quote: Option<char> = None;
        for character in line.chars() {
            match quote {
                Some(open) if character == open => quote = None,
                Some(_) => word.push(character),
                None if character == '\'' || character == '"' => {
                    quote = Some(character);
                    started = true;
                }
                None if character.is_whitespace() => {
                    if started {
                        words.push(std::mem::take(&mut word));
                        started = false;
                    }
                }
                None => {
                    word.push(character);
                    started = true;
                }
            }
        }
        assert_eq!(quote, None, "a quote is left open in: {line}");
        if started {
            words.push(word);
        }
        words
    }

    /// The command lines the skill file shows, inside its code blocks.
    fn shown() -> Vec<&'static str> {
        let mut inside = false;
        let mut lines = Vec::new();
        for line in SKILL.lines() {
            if line.starts_with("```") {
                inside = !inside;
            } else if inside && line.starts_with("wenmar-open") {
                lines.push(line);
            }
        }
        lines
    }

    #[test]
    fn every_command_line_in_the_skill_file_is_one_the_tool_accepts() {
        let lines = shown();
        assert!(lines.len() >= 15, "{lines:#?}");
        for line in lines {
            if let Err(error) = Cli::try_parse_from(words(line)) {
                panic!("the skill file shows a command the tool refuses:\n{line}\n{error}");
            }
        }
    }

    /// The names of every command that has no commands under it.
    fn leaves(command: &clap::Command, prefix: &str, found: &mut Vec<String>) {
        let name = format!("{prefix}{}", command.get_name());
        let mut under = command
            .get_subcommands()
            .filter(|sub| sub.get_name() != "help")
            .peekable();
        if under.peek().is_none() {
            found.push(name);
            return;
        }
        for sub in under {
            leaves(sub, &format!("{name} "), found);
        }
    }

    #[test]
    fn every_command_the_tool_has_is_in_the_skill_file() {
        let mut found = Vec::new();
        leaves(&Cli::command(), "", &mut found);
        assert!(
            found.contains(&"wenmar-open vin decode".to_owned()),
            "{found:?}"
        );
        for command in found {
            assert!(
                SKILL.contains(&command),
                "the skill file does not mention `{command}`"
            );
        }
    }

    #[test]
    fn the_skill_file_has_its_front_matter_and_its_marker() {
        assert!(SKILL.starts_with("---\nname: wenmar-open\ndescription: "));
        assert!(SKILL.contains(MARKER));
        assert!(SKILL.is_ascii(), "plain text survives every terminal");
    }

    #[test]
    fn a_quoted_word_is_one_word() {
        assert_eq!(
            words("wenmar-open vehicles search f150 --jq '.[0].id'"),
            [
                "wenmar-open",
                "vehicles",
                "search",
                "f150",
                "--jq",
                ".[0].id"
            ]
        );
        assert_eq!(words(r#"a "b c"  '' d"#), ["a", "b c", "", "d"]);
    }

    #[test]
    fn each_agent_has_its_skills_directory_and_its_command() {
        let env = Env {
            home: Some(PathBuf::from("/home/pat")),
            ..Env::default()
        };
        assert_eq!(
            Agent::Claude.skills_directory(&env, None).unwrap(),
            Path::new("/home/pat/.claude/skills")
        );
        assert_eq!(
            Agent::Codex.skills_directory(&env, None).unwrap(),
            Path::new("/home/pat/.agents/skills")
        );
        assert_eq!(
            Agent::Claude
                .skills_directory(&env, Some(Path::new("/elsewhere")))
                .unwrap(),
            Path::new("/elsewhere")
        );
        assert_eq!(
            Agent::skill_path(Path::new("/s")),
            Path::new("/s/wenmar-open/SKILL.md")
        );
        assert_eq!(
            Agent::Claude.register_command().join(" "),
            "claude mcp add --scope user wenmar-open -- wenmar-open mcp"
        );
        assert_eq!(
            Agent::Codex.register_command().join(" "),
            "codex mcp add wenmar-open -- wenmar-open mcp"
        );
        // The command it registers is one the tool accepts.
        for agent in Agent::ALL {
            let command = agent.register_command();
            let after = command.iter().position(|word| *word == "--").unwrap();
            assert!(Cli::try_parse_from(command.iter().skip(after + 1).copied()).is_ok());
        }
    }

    #[test]
    fn with_no_home_directory_the_error_says_to_pass_a_directory() {
        let error = Agent::Claude
            .skills_directory(&Env::default(), None)
            .unwrap_err();
        assert_eq!(error.code, "io");
        assert_eq!(
            error.hint(),
            Some("Pass --dir with the directory your agent reads skills from.")
        );
    }
}
