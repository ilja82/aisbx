#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Command, Output};

const FAKE_SBX: &str = r#"#!/bin/sh
[ "$1" = "--no-input" ] && shift
echo "$*" >> "$FAKE_SBX_LOG"
case "$1" in
  ls) cat "$FAKE_SBX_LS" ;;
  inspect)
    for a in "$@"; do
      [ -f "$FAKE_SBX_INSPECT.$a" ] && { cat "$FAKE_SBX_INSPECT.$a"; exit 0; }
    done
    cat "$FAKE_SBX_INSPECT" ;;
  *) ;;
esac
exit 0
"#;

struct Rig {
    base: PathBuf,
    home: PathBuf,
    state: PathBuf,
    work: PathBuf,
}

impl Rig {
    fn new(tag: &str) -> Self {
        let base = std::env::temp_dir().join(format!("aisbx-tests-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let rig = Rig {
            home: base.join("home"),
            state: base.join("root/state"),
            work: base.join("work"),
            base,
        };
        for dir in [
            rig.home.join(".gradle"),
            rig.state.clone(),
            rig.work.clone(),
            rig.base.join("bin"),
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        let fake = rig.base.join("bin/sbx");
        std::fs::write(&fake, FAKE_SBX).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        rig
    }

    fn stage(&self) -> PathBuf {
        self.state.join("aisbx").join("aisbx-gradle-properties")
    }

    fn host_file(&self) -> PathBuf {
        self.home.join(".gradle/gradle.properties")
    }

    fn sbx_state(&self, ls: &str, inspect: &str) {
        std::fs::write(self.base.join("ls.json"), ls).unwrap();
        std::fs::write(self.base.join("inspect.json"), inspect).unwrap();
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.base.join("log")).unwrap_or_default()
    }

    fn up(&self, extra: &[&str]) -> Output {
        let path = format!(
            "{}:{}",
            self.base.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        Command::new(env!("CARGO_BIN_EXE_aisbx"))
            .args(["up", "--backend", "claude-anthropic", "--name", "demo"])
            .args(extra)
            .current_dir(&self.work)
            .env("PATH", path)
            .env("HOME", &self.home)
            .env("XDG_STATE_HOME", &self.state)
            .env("AISBX_HOME", env!("CARGO_MANIFEST_DIR").to_string() + "/..")
            .env("FAKE_SBX_LOG", self.base.join("log"))
            .env("FAKE_SBX_LS", self.base.join("ls.json"))
            .env("FAKE_SBX_INSPECT", self.base.join("inspect.json"))
            .output()
            .unwrap()
    }

    fn existing(&self, kits: &[&str]) {
        if kits.contains(&"host-gradle-properties") {
            std::fs::create_dir_all(self.stage()).unwrap();
        }
        let kit_paths: Vec<String> = kits.iter().map(|k| format!("\"/repo/kits/{k}\"")).collect();
        let ls = format!(
            r#"[{{"name":"demo","agent":"claude-anthropic","status":"stopped","workspaces":["{}","{}:ro"]}}]"#,
            self.work.display(),
            self.stage().display()
        );
        let inspect = format!(
            r#"{{"name":"demo","agent":"claude-anthropic","kits":[{}],"host_mounts":[{{"host_path":"{}","read_only":true}}]}}"#,
            kit_paths.join(","),
            self.stage().display()
        );
        self.sbx_state(&ls, &inspect);
    }

    fn with_opted_out_sandbox(&self, mount_key: &str, host: &str) {
        let ls = format!(
            r#"[{{"name":"demo","agent":"claude-anthropic","status":"stopped","workspaces":["{}","{}:ro"]}},{{"name":"other","agent":"claude-anthropic","status":"stopped","workspaces":["{}"]}}]"#,
            self.work.display(),
            self.stage().display(),
            self.work.display()
        );
        let other = format!(
            r#"{{"name":"other","agent":"claude-anthropic","kits":["/repo/kits/host-gradle"],"{mount_key}":[{{"host_path":"{host}"}}]}}"#
        );
        std::fs::write(self.base.join("ls.json"), ls).unwrap();
        std::fs::write(self.base.join("inspect.json.other"), other).unwrap();
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

fn staged(rig: &Rig) -> Option<String> {
    std::fs::read_to_string(rig.stage().join("gradle.properties")).ok()
}

const WITH: [&str; 3] = ["claude-anthropic", "host-gradle", "host-gradle-properties"];
const WITHOUT: [&str; 2] = ["claude-anthropic", "host-gradle"];

fn assert_ok(out: &Output) {
    assert!(out.status.success(), "{}", text(&out.stderr));
}

#[test]
fn plain_up_on_a_sandbox_with_the_addon_stages_its_edits_and_deletion() {
    let rig = Rig::new("up-props-on");
    std::fs::write(rig.host_file(), "a=1\n").unwrap();
    rig.existing(&WITH);

    assert_ok(&rig.up(&[]));
    assert_eq!(staged(&rig).as_deref(), Some("a=1\n"));
    std::fs::write(rig.host_file(), "a=2\n").unwrap();
    assert_ok(&rig.up(&[]));
    assert_eq!(staged(&rig).as_deref(), Some("a=2\n"));
    std::fs::remove_file(rig.host_file()).unwrap();
    assert_ok(&rig.up(&[]));
    assert_eq!(staged(&rig), None, "a deleted host file loses its copy");
    assert!(!rig.log().contains("create"), "{}", rig.log());
}

#[test]
fn plain_up_follows_the_installed_kits_not_this_calls_flags() {
    let rig = Rig::new("up-props-flags");
    std::fs::write(rig.host_file(), "a=1\n").unwrap();
    rig.existing(&WITHOUT);
    let out = rig.up(&["--kit", "host-gradle-properties"]);
    assert!(!out.status.success(), "the flag cannot add a kit to it");
    assert_eq!(staged(&rig), None, "the flag does not install the addon");
    assert!(!rig.stage().exists());

    rig.existing(&WITH);
    assert_ok(&rig.up(&["--kit", "host-gradle"]));
    assert_eq!(
        staged(&rig).as_deref(),
        Some("a=1\n"),
        "without the flag the installed addon still counts"
    );
}

#[test]
fn up_refuses_to_stage_where_an_opted_out_sandbox_mounts_a_folder_holding_the_copy() {
    let parent = |rig: &Rig| rig.base.join("root").display().to_string();
    for (tag, key, direct) in [
        ("up-props-runtime", "runtime_mounts", false),
        ("up-props-bound", "host_mounts", false),
        ("up-props-direct", "host_mounts", true),
    ] {
        let rig = Rig::new(tag);
        std::fs::write(rig.host_file(), "a=1\n").unwrap();
        rig.existing(&WITH);
        std::fs::create_dir_all(rig.stage()).unwrap();
        let host = match direct {
            true => rig.stage().display().to_string(),
            false => parent(&rig),
        };
        rig.with_opted_out_sandbox(key, &host);
        let out = rig.up(&[]);
        assert_ok(&out);
        assert_eq!(staged(&rig), None, "{tag}");
        assert!(
            text(&out.stderr).contains("overlaps") && text(&out.stderr).contains("other"),
            "{tag}: {}",
            text(&out.stderr)
        );
    }
}

#[test]
fn up_stages_when_the_only_mount_of_the_folder_is_the_addons_own() {
    let rig = Rig::new("up-props-own");
    std::fs::write(rig.host_file(), "a=1\n").unwrap();
    rig.existing(&WITH);
    assert_ok(&rig.up(&[]));
    assert_eq!(staged(&rig).as_deref(), Some("a=1\n"));
}

#[test]
fn a_new_up_with_invalid_mounts_json_stages_nothing_and_creates_nothing() {
    let rig = Rig::new("up-props-new");
    std::fs::write(rig.host_file(), "a=1\n").unwrap();
    rig.sbx_state("[]", "{}");
    std::fs::create_dir_all(rig.state.join("aisbx")).unwrap();
    std::fs::write(rig.state.join("aisbx/mounts.json"), "{ not json").unwrap();
    let out = rig.up(&["--kit", "host-gradle-properties"]);
    assert!(!out.status.success(), "{}", text(&out.stderr));
    assert!(!rig.stage().exists(), "nothing staged");
    assert!(!rig.log().contains("create"), "{}", rig.log());
}

#[test]
fn a_new_up_with_the_addon_stages_and_mounts_it_and_without_it_stages_nothing() {
    let rig = Rig::new("up-props-create");
    std::fs::write(rig.host_file(), "a=1\n").unwrap();
    rig.sbx_state("[]", "{}");
    let off = rig.up(&["--kit", "host-gradle"]);
    assert_ok(&off);
    assert!(!rig.stage().exists());
    assert!(rig.log().contains("create"), "{}", rig.log());

    let _ = std::fs::remove_file(rig.base.join("log"));
    assert_ok(&rig.up(&["--kit", "host-gradle-properties"]));
    assert_eq!(staged(&rig).as_deref(), Some("a=1\n"));
    let create = rig
        .log()
        .lines()
        .find(|l| l.starts_with("create"))
        .unwrap()
        .to_string();
    assert!(
        create.ends_with(&format!("{}:ro", rig.stage().display())),
        "{create}"
    );
}

#[test]
fn an_opted_out_create_refuses_a_workspace_that_holds_an_earlier_copy() {
    let rig = Rig::new("up-props-holds");
    std::fs::write(rig.host_file(), "a=1\n").unwrap();
    rig.sbx_state("[]", "{}");
    assert_ok(&rig.up(&["--kit", "host-gradle-properties"]));
    assert!(rig.stage().exists());
    let _ = std::fs::remove_file(rig.base.join("log"));
    let mount = format!("{}:ro", rig.base.join("root").display());
    let out = rig.up(&["--kit", "host-gradle", "--mount", &mount]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("earlier sandbox staged"),
        "{}",
        text(&out.stderr)
    );
    assert!(!rig.log().contains("create"), "{}", rig.log());
}

#[test]
fn an_opted_out_create_refuses_the_staging_folder_as_an_extra_mount() {
    let rig = Rig::new("up-props-exact");
    std::fs::write(rig.host_file(), "a=1\n").unwrap();
    rig.sbx_state("[]", "{}");
    assert_ok(&rig.up(&["--kit", "host-gradle-properties"]));
    assert!(rig.stage().exists());
    let _ = std::fs::remove_file(rig.base.join("log"));
    let mount = format!("{}:ro", rig.stage().display());
    let out = rig.up(&["--kit", "host-gradle", "--mount", &mount]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("earlier sandbox staged"),
        "{}",
        text(&out.stderr)
    );
    assert!(!rig.log().contains("create"), "{}", rig.log());
}

#[test]
fn an_opted_out_create_refuses_the_staging_folder_as_the_primary_workspace() {
    let rig = Rig::new("up-props-primary");
    std::fs::write(rig.host_file(), "a=1\n").unwrap();
    rig.sbx_state("[]", "{}");
    assert_ok(&rig.up(&["--kit", "host-gradle-properties"]));
    assert!(rig.stage().exists());
    let _ = std::fs::remove_file(rig.base.join("log"));
    let stage = rig.stage().display().to_string();
    let out = rig.up(&["--kit", "host-gradle", &stage]);
    assert!(!out.status.success());
    assert!(
        text(&out.stderr).contains("earlier sandbox staged"),
        "{}",
        text(&out.stderr)
    );
    assert!(!rig.log().contains("create"), "{}", rig.log());
}

#[test]
fn staging_stops_when_a_mount_of_another_sandbox_cannot_be_read() {
    let rig = Rig::new("up-props-malformed");
    std::fs::write(rig.host_file(), "a=1\n").unwrap();
    let ls =
        r#"[{"name":"other","agent":"claude-anthropic","status":"stopped","workspaces":["/tmp"]}]"#;
    rig.sbx_state(ls, "{}");
    let other = r#"{"name":"other","agent":"claude-anthropic","kits":["/repo/kits/host-gradle"],"host_mounts":[{"host_path":5}]}"#;
    std::fs::write(rig.base.join("inspect.json.other"), other).unwrap();
    let out = rig.up(&["--kit", "host-gradle-properties"]);
    assert_eq!(out.status.code(), Some(125), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("other"), "{}", text(&out.stderr));
    assert!(staged(&rig).is_none());
    assert!(!rig.log().contains("create"), "{}", rig.log());
}

#[test]
fn staging_stops_on_a_relative_source_or_a_missing_field_of_another_sandbox() {
    for (tag, mount) in [
        ("up-props-relative", r#"{"host_path":"relative/dir"}"#),
        ("up-props-nopath", r#"{"read_only":true}"#),
    ] {
        let rig = Rig::new(tag);
        std::fs::write(rig.host_file(), "a=1\n").unwrap();
        let ls = r#"[{"name":"other","agent":"claude-anthropic","status":"stopped","workspaces":["/tmp"]}]"#;
        rig.sbx_state(ls, "{}");
        let other = format!(
            r#"{{"name":"other","agent":"claude-anthropic","kits":["/repo/kits/host-gradle"],"host_mounts":[{mount}]}}"#
        );
        std::fs::write(rig.base.join("inspect.json.other"), other).unwrap();
        let out = rig.up(&["--kit", "host-gradle-properties"]);
        assert_eq!(out.status.code(), Some(125), "{tag}: {}", text(&out.stderr));
        assert!(
            text(&out.stderr).contains("other"),
            "{tag}: {}",
            text(&out.stderr)
        );
        assert!(staged(&rig).is_none(), "{tag}");
        assert!(!rig.stage().exists(), "{tag}");
        assert!(!rig.log().contains("create"), "{tag}: {}", rig.log());
    }
}

#[test]
fn an_opted_out_create_refuses_the_staging_folder_configured_in_mounts_json() {
    let rig = Rig::new("up-props-configured");
    std::fs::write(rig.host_file(), "a=1\n").unwrap();
    rig.sbx_state("[]", "{}");
    assert_ok(&rig.up(&["--kit", "host-gradle-properties"]));
    let _ = std::fs::remove_file(rig.base.join("log"));
    let config = format!(
        r#"{{"version":1,"folders":[{{"path":"{}","access":"read"}}]}}"#,
        rig.stage().display()
    );
    std::fs::write(rig.state.join("aisbx/mounts.json"), config).unwrap();
    let out = rig.up(&["--kit", "host-gradle"]);
    assert!(!out.status.success(), "{}", text(&out.stderr));
    assert!(
        text(&out.stderr).contains("earlier sandbox staged"),
        "{}",
        text(&out.stderr)
    );
    assert!(!rig.log().contains("create"), "{}", rig.log());
}

#[test]
fn staging_stops_on_a_source_that_loops_or_does_not_resolve() {
    for tag in ["up-props-loop", "up-props-gone"] {
        let rig = Rig::new(tag);
        std::fs::write(rig.host_file(), "a=1\n").unwrap();
        let source = rig.base.join("source");
        if tag == "up-props-loop" {
            std::os::unix::fs::symlink(&source, &source).unwrap();
        }
        let ls = format!(
            r#"[{{"name":"other","agent":"claude-anthropic","status":"stopped","workspaces":["{}"]}}]"#,
            rig.work.display()
        );
        rig.sbx_state(&ls, "{}");
        let other = format!(
            r#"{{"name":"other","agent":"claude-anthropic","kits":["/repo/kits/host-gradle"],"host_mounts":[{{"host_path":"{}","container_target":"/x","read_only":true}}]}}"#,
            source.display()
        );
        std::fs::write(rig.base.join("inspect.json.other"), other).unwrap();
        let out = rig.up(&["--kit", "host-gradle-properties"]);
        assert!(!out.status.success(), "{tag}");
        assert!(
            text(&out.stderr).contains("other"),
            "{tag}: {}",
            text(&out.stderr)
        );
        assert!(staged(&rig).is_none(), "{tag}");
        assert!(!rig.stage().exists(), "{tag}");
        assert!(!rig.log().contains("create"), "{tag}: {}", rig.log());
    }
}

#[test]
fn up_stages_nothing_when_the_addons_own_mount_folder_is_gone() {
    let rig = Rig::new("up-props-nostage");
    std::fs::write(rig.host_file(), "a=1\n").unwrap();
    rig.existing(&WITH);
    std::fs::remove_dir_all(rig.stage()).unwrap();
    let out = rig.up(&[]);
    assert!(
        text(&out.stderr).contains("demo") && text(&out.stderr).contains("rebuild"),
        "{}",
        text(&out.stderr)
    );
    assert!(!rig.stage().exists(), "nothing was staged");
}
