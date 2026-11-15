use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use aisbx_core::compat;

/// An empty state folder for one test: the binary reads and writes there, never in the real one.
fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aisbx-tests-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn aisbx(state: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_aisbx"))
        .args(args)
        .env("XDG_STATE_HOME", state)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[test]
fn the_skill_repo_commands_warn_once_on_stderr_and_keep_their_stdout() {
    let state = scratch("deprecations");

    let out = aisbx(&state, &["skill-repo", "auto-update"]);
    assert!(out.status.success());
    assert_eq!(
        text(&out.stdout),
        "on\n",
        "stdout as before the deprecation"
    );
    let stderr = text(&out.stderr);
    let warning = compat::skill_repos_warning("skill-repo");
    assert_eq!(stderr.lines().collect::<Vec<_>>(), [warning.as_str()]);

    let out = aisbx(&state, &["skill", "ls", "--json"]);
    assert!(out.status.success());
    let stdout = text(&out.stdout);
    assert!(
        stdout.starts_with('{') && stdout.ends_with("}\n") && !stdout.contains("deprecated"),
        "one JSON document: {stdout}"
    );
    let stderr = text(&out.stderr);
    let warning = compat::skill_repos_warning("skill");
    assert_eq!(
        stderr.lines().filter(|l| *l == warning).count(),
        1,
        "{stderr}"
    );

    let out = aisbx(&state, &["skill", "sync", "--start"]);
    assert_eq!(out.status.code(), Some(2), "no sandbox name: refused");
    assert!(out.stdout.is_empty());
    let stderr = text(&out.stderr);
    assert!(!stderr.contains("deprecated"), "aisbx's own sync: {stderr}");

    let _ = std::fs::remove_dir_all(&state);
}

#[test]
fn aisbx_skills_keeps_its_switches_in_the_state_folder() {
    let state = scratch("skills");

    let out = aisbx(&state, &["skills", "auto-update"]);
    assert!(out.status.success());
    assert_eq!(text(&out.stdout), "on\n");
    assert!(out.stderr.is_empty(), "{}", text(&out.stderr));

    let out = aisbx(&state, &["skills", "auto-update", "off"]);
    assert!(out.status.success());
    assert!(out.stdout.is_empty());
    assert_eq!(text(&out.stderr), "auto-update: off\n");
    let file = state.join("aisbx").join("store-skills.json");
    let saved = std::fs::read_to_string(&file).unwrap();
    assert!(saved.contains("\"auto_update\": false"), "{saved}");

    let out = aisbx(&state, &["skills", "on", "--global", "a b"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(out.stdout.is_empty());

    let _ = std::fs::remove_dir_all(&state);
}

#[test]
fn the_help_names_aisbx_skills_and_marks_the_skill_repos_deprecated() {
    let out = Command::new(env!("CARGO_BIN_EXE_aisbx"))
        .arg("--help")
        .output()
        .unwrap();
    assert!(out.status.success());
    let help = text(&out.stdout);
    let usage: Vec<&str> = help
        .lines()
        .skip_while(|l| *l != "USAGE:")
        .take_while(|l| !l.is_empty())
        .collect();
    assert!(
        usage
            .iter()
            .any(|l| l.starts_with("    aisbx skills ls|on|off|default ")),
        "{usage:#?}"
    );
    for command in ["aisbx skill-repo ", "aisbx skill "] {
        let line = usage.iter().find(|l| l.trim_start().starts_with(command));
        assert!(
            line.is_some_and(|l| l.contains("deprecated: ")),
            "{command}: {usage:#?}"
        );
    }
    assert!(
        help.contains("\n    aisbx skills ls [SANDBOX] [--json]\n"),
        "{help}"
    );
}
