#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

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
    for a in "$@"; do
      shift
      case "$a" in /tmp/aisbx-run-*) a="$FAKE_ROOT$a"; run=1 ;; esac
      set -- "$@" "$a"
    done
    [ -n "$run" ] && exec "$@" ;;
  *) ;;
esac
exit 0
"#;

const FAKE_CLAUDE: &str = r#"#!/bin/sh
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
        let path = format!(
            "{}:{}",
            self.base.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        Command::new(env!("CARGO_BIN_EXE_aisbx"))
            .args(args)
            .current_dir(&self.work)
            .env("PATH", path)
            .env("HOME", self.base.join("home"))
            .env("XDG_STATE_HOME", self.base.join("state"))
            .env("AISBX_HOME", env!("CARGO_MANIFEST_DIR").to_string() + "/..")
            .env("FAKE_SBX_LOG", self.base.join("log"))
            .env("FAKE_SBX_LS", self.base.join("ls.json"))
            .env("FAKE_SBX_INSPECT", self.base.join("inspect.json"))
            .env("FAKE_ROOT", self.base.join("fakeroot"))
            .output()
            .unwrap()
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
