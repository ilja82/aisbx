#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::time::Duration;

const FAKE_SBX: &str = r#"#!/bin/sh
[ "$1" = "--no-input" ] && shift
printf '%s\n' "$*" >> "$FAKE_SBX_LOG"
case "$1" in
  ls) cat "$FAKE_SBX_LS" ;;
  inspect) cat "$FAKE_SBX_INSPECT" ;;
  exec)
    shift
    while :; do
      case "$1" in
        -i|-t|-it) shift ;;
        -w) shift 2 ;;
        *) break ;;
      esac
    done
    shift
    run=
    launch=
    for a in "$@"; do
      shift
      case "$a" in
        /tmp/aisbx-run-*) a="$FAKE_ROOT$a"; run=1 ;;
        @@AISBX_PROMPT@@) launch=1 ;;
      esac
      set -- "$@" "$a"
    done
    if [ -n "$launch" ] && [ -n "$FAKE_CLIENT_DIES_ON_INT" ]; then
      "$@" &
      trap 'exit 1' INT
      wait $!
      exit
    fi
    [ -n "$run" ] && exec "$@" ;;
  *) ;;
esac
exit 0
"#;

const FAKE_CLAUDE: &str = r#"#!/bin/sh
if [ -n "$FAKE_AGENT_PIDFILE" ]; then
  echo $$ > "$FAKE_AGENT_PIDFILE"
  exec sleep 30 < /dev/null > /dev/null 2>&1
fi
for a in "$@"; do
  printf '%s' "$a" | od -An -v -tx1 | tr -d ' \n'
  printf '\n'
done
"#;

const NOTE: &str = "docs/deprecations.md#mcp-default";

/// Each `up` writes the one test folder kit of `demo` (`folders::kit_dir`): tests take turns.
static ONE_RUN: std::sync::Mutex<()> = std::sync::Mutex::new(());

struct Rig {
    base: PathBuf,
    work: PathBuf,
}

impl Rig {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!("aisbx-tests-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let rig = Rig {
            work: base.join("work"),
            base,
        };
        for dir in ["home", "state", "work", "bin", "fakeroot/tmp"] {
            std::fs::create_dir_all(rig.base.join(dir)).unwrap();
        }
        for (name, body) in [("sbx", FAKE_SBX), ("claude", FAKE_CLAUDE)] {
            let fake = rig.base.join("bin").join(name);
            std::fs::write(&fake, body).unwrap();
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        rig.sbx_state("[]", "{}");
        rig
    }

    fn sbx_state(&self, ls: &str, inspect: &str) {
        std::fs::write(self.base.join("ls.json"), ls).unwrap();
        std::fs::write(self.base.join("inspect.json"), inspect).unwrap();
    }

    fn existing_demo(&self) {
        let ls = format!(
            r#"[{{"name":"demo","agent":"claude-anthropic","status":"stopped","workspaces":["{}"]}}]"#,
            self.work.display()
        );
        let inspect = r#"{"name":"demo","agent":"claude-anthropic","kits":["/nowhere/kits/claude-anthropic"]}"#;
        self.sbx_state(&ls, inspect);
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.base.join("log")).unwrap_or_default()
    }

    fn create_line(&self) -> Option<String> {
        (self.log().lines())
            .find(|l| l.starts_with("create"))
            .map(String::from)
    }

    fn aisbx(&self, args: &[&str]) -> Output {
        let _turn = ONE_RUN.lock().unwrap_or_else(|e| e.into_inner());
        self.command(args).output().unwrap()
    }

    fn command(&self, args: &[&str]) -> Command {
        let path = format!(
            "{}:{}",
            self.base.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_aisbx"));
        cmd.args(args)
            .current_dir(&self.work)
            .env("PATH", path)
            .env("HOME", self.base.join("home"))
            .env("XDG_STATE_HOME", self.base.join("state"))
            .env("AISBX_HOME", env!("CARGO_MANIFEST_DIR").to_string() + "/..")
            .env("FAKE_SBX_LOG", self.base.join("log"))
            .env("FAKE_SBX_LS", self.base.join("ls.json"))
            .env("FAKE_SBX_INSPECT", self.base.join("inspect.json"))
            .env("FAKE_ROOT", self.base.join("fakeroot"));
        cmd
    }

    fn up(&self, extra: &[&str]) -> Output {
        let mut args = vec!["up", "--backend", "claude-anthropic", "--name", "demo"];
        args.extend_from_slice(extra);
        self.aisbx(&args)
    }

    fn run_dirs_left(&self) -> Vec<String> {
        (std::fs::read_dir(self.base.join("fakeroot/tmp")).unwrap())
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

/// The fake `claude`'s output: one argument per line, as hex.
fn argv_of(stdout: &str) -> Vec<String> {
    (stdout.lines())
        .map(|hex| {
            let bytes: Vec<u8> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();
            String::from_utf8(bytes).unwrap()
        })
        .collect()
}

#[test]
fn a_new_up_starts_without_mcp_unless_asked_and_notes_it_once_on_stderr() {
    for (flags, note, none) in [
        (vec![], true, true),
        (vec!["--no-mcp"], false, true),
        (vec!["--mcp"], false, false),
    ] {
        let rig = Rig::new("up-mcp");
        let out = rig.up(&flags);
        let err = text(&out.stderr);
        assert_eq!(out.status.code(), Some(0), "{flags:?}: {err}");
        assert_eq!(
            text(&out.stdout),
            "demo\n",
            "{flags:?}: stdout is the name alone"
        );
        assert_eq!(
            err.matches(NOTE).count(),
            usize::from(note),
            "{flags:?}: {err}"
        );
        let create = rig.create_line().expect("a create");
        assert_eq!(
            create.matches("--static-mcp").count(),
            usize::from(none),
            "{create}"
        );
        assert_eq!(create.contains("--static-mcp none"), none, "{create}");
    }
}

#[test]
fn up_on_an_existing_sandbox_creates_nothing_and_notes_no_mcp_default() {
    for flags in [vec![], vec!["--mcp"], vec!["--no-mcp"]] {
        let rig = Rig::new("up-reuse");
        rig.existing_demo();
        let out = rig.up(&flags);
        let err = text(&out.stderr);
        assert_eq!(out.status.code(), Some(0), "{flags:?}: {err}");
        assert_eq!(text(&out.stdout), "demo\n");
        assert_eq!(err.matches(NOTE).count(), 0, "{flags:?}: {err}");
        assert!(rig.create_line().is_none(), "{}", rig.log());
        assert_eq!(
            err.contains("only applies when a sandbox is created"),
            !flags.is_empty(),
            "{flags:?}: {err}"
        );
    }
}

#[test]
fn up_with_the_deprecated_host_skills_kit_still_creates_and_warns_once_on_stderr() {
    const DEPRECATED: &str = "docs/deprecations.md#host-skills";
    for (kit, warned) in [("host-skills", true), ("rtk", false)] {
        let rig = Rig::new("up-host-skills");
        let out = rig.up(&["--kit", kit, "--mcp"]);
        let err = text(&out.stderr);
        assert_eq!(out.status.code(), Some(0), "{kit}: {err}");
        assert_eq!(
            text(&out.stdout),
            "demo\n",
            "{kit}: stdout is the name alone"
        );
        assert_eq!(
            err.matches(DEPRECATED).count(),
            usize::from(warned),
            "{kit}: {err}"
        );
        let create = rig.create_line().expect("a create");
        assert!(create.contains(&format!("kits/{kit}")), "{create}");
    }
}

#[test]
fn up_warns_on_stderr_before_it_creates_a_read_write_sandbox() {
    for (mode, warned) in [("readwrite", true), ("readonly", false), ("off", false)] {
        let rig = Rig::new("up-readwrite");
        let out = rig.up(&["--skills", mode, "--mcp"]);
        let err = text(&out.stderr);
        assert_eq!(out.status.code(), Some(0), "{mode}: {err}");
        assert_eq!(
            text(&out.stdout),
            "demo\n",
            "{mode}: stdout is the name alone"
        );
        let warning = "the shared skills are read-write in this sandbox";
        assert_eq!(
            err.matches(warning).count(),
            usize::from(warned),
            "{mode}: {err}"
        );
        let create = rig.create_line().expect("a create");
        assert!(create.contains(&format!("--skills {mode}")), "{create}");
    }
}

#[test]
fn both_mcp_flags_exit_2_before_any_sbx_call() {
    let rig = Rig::new("up-mcp-both");
    let out = rig.up(&["--mcp", "--no-mcp"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    assert!(rig.log().is_empty(), "{}", rig.log());
}

#[test]
fn run_hands_the_lean_flags_to_claude_code_intact() {
    let rig = Rig::new("run-lean");
    rig.existing_demo();
    let system = "You are an engine.\nAnswer in \"JSON\" with $HOME, `id` and 'single'\n\n";
    let out = rig.aisbx(&[
        "run",
        "demo",
        "--prompt",
        "- the task\nline 2\n",
        "--no-update",
        "--no-tools",
        "--system-prompt",
        system,
        "--no-slash-commands",
        "--no-session",
        "--",
        "--own",
    ]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let want = [
        "-p",
        "--output-format",
        "json",
        "--system-prompt",
        system,
        "--tools",
        "",
        "--strict-mcp-config",
        "--disable-slash-commands",
        "--no-session-persistence",
        "--dangerously-skip-permissions",
        "--own",
        "--",
        "- the task\nline 2",
    ];
    assert_eq!(argv_of(&text(&out.stdout)), want);
    assert!(rig.run_dirs_left().is_empty(), "{:?}", rig.run_dirs_left());
}

#[test]
fn run_refuses_a_system_prompt_with_a_nul_byte_before_it_uploads_anything() {
    let rig = Rig::new("run-nul");
    rig.existing_demo();
    let file = rig.base.join("system.md");
    std::fs::write(&file, "be brief\0and lean\n").unwrap();
    let out = rig.aisbx(&[
        "run",
        "demo",
        "--prompt",
        "task",
        "--no-update",
        "--system-prompt-file",
        file.to_str().unwrap(),
    ]);
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(err.contains("NUL byte"), "{err}");
    assert!(!rig.log().contains("exec"), "{}", rig.log());
    assert!(rig.run_dirs_left().is_empty(), "{:?}", rig.run_dirs_left());
}

fn alive(pid: &str) -> bool {
    Command::new("kill")
        .args(["-0", pid])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

#[test]
fn ctrl_c_to_the_process_group_stops_the_agent_although_the_client_ends_first() {
    let rig = Rig::new("run-ctrl-c");
    rig.existing_demo();
    let pidfile = rig.base.join("agent.pid");
    let _turn = ONE_RUN.lock().unwrap_or_else(|e| e.into_inner());
    let run = rig
        .command(&["run", "demo", "--prompt", "task", "--no-update"])
        .env("FAKE_CLIENT_DIES_ON_INT", "1")
        .env("FAKE_AGENT_PIDFILE", &pidfile)
        .process_group(0)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut agent = String::new();
    for _ in 0..200 {
        agent = std::fs::read_to_string(&pidfile).unwrap_or_default();
        agent = agent.trim().to_string();
        if !agent.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(!agent.is_empty(), "the agent never started: {}", rig.log());
    let group = format!("-{}", run.id());
    let sent = Command::new("kill").args(["-INT", "--", &group]).status();
    assert!(sent.is_ok_and(|s| s.success()), "kill -INT {group}");
    let out = run.wait_with_output().unwrap();
    let mut left = alive(&agent);
    for _ in 0..50 {
        if !left {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
        left = alive(&agent);
    }
    if left {
        let _ = Command::new("kill").args(["-KILL", &agent]).status();
    }
    let err = text(&out.stderr);
    assert_eq!(out.status.code(), Some(130), "{err}");
    assert!(err.contains("interrupted"), "{err}");
    assert!(!left, "the agent outlived the run: {}", rig.log());
}
