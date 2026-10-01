use serde_json::json;
use wenmar_open_cli::env::Env;
use wenmar_open_cli::setup::SKILL;

use crate::common::{self, Fixture};
use crate::server::{self, Reply};

/// Surroundings with a home directory inside the fixture.
fn home(fixture: &Fixture) -> Env {
    Env {
        home: Some(fixture.directory().join("home")),
        ..common::env(fixture)
    }
}

fn files_under(path: &std::path::Path) -> Vec<String> {
    let mut found = Vec::new();
    let mut pending = vec![path.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).unwrap() {
            let entry = entry.unwrap().path();
            if entry.is_dir() {
                pending.push(entry);
            } else {
                found.push(
                    entry
                        .strip_prefix(path)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                );
            }
        }
    }
    found.sort();
    found
}

#[test]
fn setup_writes_the_skill_file_and_prints_the_command_without_running_it() {
    let fixture = common::empty_dir();
    let env = home(&fixture);
    let skill = fixture
        .directory()
        .join("home/.claude/skills/wenmar-open/SKILL.md");

    let run = common::run(&env, &["setup", "claude"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(
        run.json(),
        json!({
            "agent": "claude",
            "skill": skill.display().to_string(),
            "skill_written": true,
            "mcp_command": "claude mcp add --scope user wenmar-open -- wenmar-open mcp",
            "mcp_registered": false
        })
    );
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), SKILL);
    // The skill file is the only thing written, anywhere.
    assert_eq!(
        files_under(fixture.directory()),
        ["home/.claude/skills/wenmar-open/SKILL.md"]
    );

    // Again: nothing to write.
    let run = common::run(&env, &["setup", "claude"]);
    assert_eq!(run.json()["skill_written"], false);

    let run = common::run(&env, &["setup", "codex"]);
    assert_eq!(
        run.json()["mcp_command"],
        "codex mcp add wenmar-open -- wenmar-open mcp"
    );
    assert_eq!(
        files_under(fixture.directory()),
        [
            "home/.agents/skills/wenmar-open/SKILL.md",
            "home/.claude/skills/wenmar-open/SKILL.md"
        ]
    );
}

#[test]
fn an_older_copy_is_replaced_and_a_hand_edited_one_is_not() {
    let fixture = common::empty_dir();
    let directory = fixture.directory().join("skills");
    let skill = directory.join("wenmar-open/SKILL.md");
    let args = ["setup", "claude", "--dir", directory.to_str().unwrap()];
    std::fs::create_dir_all(skill.parent().unwrap()).unwrap();

    // What an earlier version of this tool wrote: it carries the marker.
    std::fs::write(
        &skill,
        "# old\n<!-- wenmar-open-skill: managed. Written by an earlier version. -->\n",
    )
    .unwrap();
    let run = common::run(&common::env(&fixture), &args);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.json()["skill_written"], true);
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), SKILL);

    // What a person wrote.
    std::fs::write(&skill, "# my own notes\n").unwrap();
    let run = common::run(&common::env(&fixture), &args);
    assert_eq!(run.code, 1);
    assert_eq!(run.error()["error"]["code"], "skill_edited");
    assert_eq!(
        run.error()["error"]["details"]["hint"],
        "Pass --force to replace it."
    );
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), "# my own notes\n");

    let mut forced = args.to_vec();
    forced.push("--force");
    let run = common::run(&common::env(&fixture), &forced);
    assert_eq!(run.code, 0);
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), SKILL);
}

#[test]
fn setup_with_no_home_directory_or_an_unknown_agent_is_an_error() {
    let fixture = common::empty_dir();
    let run = common::run(&common::env(&fixture), &["setup", "claude"]);
    assert_eq!(run.code, 1);
    assert_eq!(
        run.error()["error"]["details"]["hint"],
        "Pass --dir with the directory your agent reads skills from."
    );
    for args in [vec!["setup"], vec!["setup", "cursor"]] {
        let run = common::run(&home(&fixture), &args);
        assert_eq!(run.code, 2, "{args:?}");
    }
    assert!(fixture.files().is_empty());
}

#[cfg(unix)]
#[test]
fn with_yes_the_agents_own_command_is_run_and_given_no_input() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = common::empty_dir();
    let bin = fixture.directory().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let log = fixture.directory().join("ran.txt");
    // A stand-in for `claude` that records how it was called, and would
    // hang on `read` if it were given a terminal to read from.
    let script = format!(
        "#!/bin/sh\nread answer\necho \"$@\" > '{}'\n",
        log.display()
    );
    let program = bin.join("claude");
    std::fs::write(&program, script).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();

    let env = Env {
        path: Some(bin.clone().into_os_string()),
        ..home(&fixture)
    };
    let run = common::run(&env, &["setup", "claude", "--yes"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    assert_eq!(run.json()["mcp_registered"], true);
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "mcp add --scope user wenmar-open -- wenmar-open mcp\n"
    );

    // An agent that is not installed: the skill is written, and the error
    // says what to run by hand.
    let run = common::run(&env, &["setup", "codex", "--yes"]);
    assert_eq!(run.code, 1);
    let error = run.error();
    assert_eq!(error["error"]["code"], "register_failed");
    assert_eq!(
        error["error"]["details"]["hint"],
        "Run it yourself: codex mcp add wenmar-open -- wenmar-open mcp"
    );
    assert!(
        fixture
            .directory()
            .join("home/.agents/skills/wenmar-open/SKILL.md")
            .is_file()
    );
}

#[test]
fn doctor_says_which_source_answers() {
    let server = server::serve(|request| {
        if request.target == "/v1/meta" {
            Reply::json(
                200,
                &json!({ "data_version": "2026.10", "vpic_release": "vPICList_lite_2026_10", "built_at": "2026-11-01 04:00:00", "server_version": "0.1.0" }),
            )
        } else {
            Reply::html(404, "no")
        }
    });
    let fixture = common::data_dir();
    let online = Env {
        api: Some(server.url().to_owned()),
        ..home(&fixture)
    };

    let run = common::run(&online, &["doctor"]);
    assert_eq!(run.code, 0, "{}", run.stderr);
    let report = run.json();
    assert_eq!(report["ok"], true);
    assert_eq!(report["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(report["answers_from"], "data file");
    assert_eq!(report["data"]["data_version"], "2026.09");
    assert_eq!(
        report["api"],
        json!({ "url": server.url(), "checked": true, "reachable": true, "data_version": "2026.10" })
    );
    assert_eq!(report["skills"]["claude"]["installed"], false);

    // With --online the data file is not what answers.
    let run = common::run(&online, &["doctor", "--online"]);
    assert_eq!(run.json()["answers_from"], "api");

    // The API down, the data file there: still fine.
    let run = common::run(&home(&fixture), &["doctor"]);
    assert_eq!(run.code, 0);
    assert_eq!(run.json()["api"]["reachable"], false);
    assert_eq!(run.json()["answers_from"], "data file");

    // --offline does not touch the network.
    let before = server.seen().len();
    let run = common::run(&online, &["doctor", "--offline"]);
    assert_eq!(run.json()["api"]["checked"], false);
    assert_eq!(server.seen().len(), before);

    // After setup, the skill is reported.
    common::run(&online, &["setup", "claude"]);
    let run = common::run(&online, &["doctor", "--offline"]);
    assert_eq!(run.json()["skills"]["claude"]["installed"], true);
    assert_eq!(run.json()["skills"]["claude"]["current"], true);
}

#[test]
fn doctor_fails_when_nothing_can_answer_and_still_prints_its_report() {
    let fixture = common::empty_dir();
    let run = common::run(&common::env(&fixture), &["doctor"]);
    assert_eq!(run.code, 1);
    let report = run.json();
    assert_eq!(report["ok"], false);
    assert_eq!(report["answers_from"], "nothing");
    assert_eq!(report["data"]["installed"], false);
    assert_eq!(run.error()["error"]["code"], "unhealthy");
    // An API that answers with a web page is not reachable either.
    let server = server::serve(|_| Reply::html(200, "<html>Sign in</html>"));
    let env = Env {
        api: Some(server.url().to_owned()),
        ..common::env(&fixture)
    };
    let run = common::run(&env, &["doctor"]);
    assert_eq!(run.code, 1);
    assert_eq!(run.json()["api"]["reachable"], false);
    // With no home directory at all, the report still comes.
    let run = common::run(
        &Env {
            api: Some(common::NO_API.to_owned()),
            ..Env::default()
        },
        &["doctor"],
    );
    assert_eq!(run.code, 1);
    assert_eq!(run.json()["data"]["usable"], false);
}

#[test]
fn with_no_command_and_no_terminal_the_tool_says_what_it_is_and_stops() {
    let fixture = common::data_dir();
    // Standard input is never read: it is given something that would be a
    // question to answer.
    let run = common::run_with_input(&common::env(&fixture), &[], b"y\n");
    assert_eq!(run.code, 0, "{}", run.stderr);
    let overview = run.json();
    assert_eq!(overview["name"], "wenmar-open");
    assert_eq!(overview["answers_from"], "data file");
    assert_eq!(overview["data"]["data_version"], "2026.09");
    assert_eq!(overview["api"], common::NO_API);
    let commands: Vec<&str> = overview["commands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|command| command["command"].as_str().unwrap())
        .collect();
    assert_eq!(
        commands,
        [
            "wenmar-open vin decode",
            "wenmar-open vehicles years",
            "wenmar-open vehicles makes",
            "wenmar-open vehicles models",
            "wenmar-open vehicles submodels",
            "wenmar-open vehicles engines",
            "wenmar-open vehicles search",
            "wenmar-open vehicles entry",
            "wenmar-open data pull",
            "wenmar-open data status",
            "wenmar-open mcp",
            "wenmar-open setup",
            "wenmar-open doctor"
        ]
    );
    let about = overview["commands"][0]["about"].as_str().unwrap();
    assert!(about.starts_with("Decode one VIN"), "{about}");
    // No data file and no home directory: it still answers, from nothing
    // but itself.
    let run = common::run(&Env::default(), &[]);
    assert_eq!(run.code, 0);
    assert_eq!(run.json()["answers_from"], "api");
    let run = common::run(&common::env(&fixture), &["--jq", ".version"]);
    assert_eq!(run.stdout, format!("{}\n", env!("CARGO_PKG_VERSION")));
}
