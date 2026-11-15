//! Which skills of sbx's shared skills store each sandbox gets. Every skill has a switch for all
//! sandboxes, off until switched on; a sandbox can turn single skills the other way. Beside the
//! switches, `store-skills.json` keeps where aisbx saw each skill come from and when the store's
//! Git skills were last updated.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// The file in the state folder.
pub const CONFIG_FILE: &str = "store-skills.json";

const VERSION: u32 = 1;

/// How long the store's Git skills stay as they are before the dashboard updates them.
pub const UPDATE_EVERY: u64 = 24 * 60 * 60;

/// The kit hook that links the store into an agent's skills folder, as the kits run it.
pub const HOOK: &str = "/usr/local/lib/aisbx/skills/skills-store.sh";

/// The same hook's text: the pre-start step runs this copy, so a sandbox built from an older kit
/// gets the switches too.
pub const SCRIPT: &str = include_str!("../../kits/claude-litellm/files/skills/skills-store.sh");

/// Where a store skill came from, as aisbx saw it arrive.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Origin {
    /// Imported from this host's skills folder, written `~/<folder>`.
    Folder(String),
    /// Added from this Git repository, as it was typed: `owner/repo` or a Git URL.
    Repo(String),
}

/// A sandbox's own switches: the skills it turns on or off against the switch for all sandboxes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Own {
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub on: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub off: BTreeSet<String>,
}

impl Own {
    pub fn is_empty(&self) -> bool {
        self.on.is_empty() && self.off.is_empty()
    }

    /// This sandbox's switch for `skill`, where it has its own.
    pub fn get(&self, skill: &str) -> Option<bool> {
        match (self.on.contains(skill), self.off.contains(skill)) {
            (true, _) => Some(true),
            (_, true) => Some(false),
            _ => None,
        }
    }

    /// Make `skill` `on` here. `global` is the switch for all sandboxes: matching it drops this
    /// sandbox's own switch, so the sandbox follows the switch for all again.
    pub fn set(&mut self, skill: &str, on: bool, global: bool) {
        self.on.remove(skill);
        self.off.remove(skill);
        match (on, on == global) {
            (_, true) => {}
            (true, false) => {
                self.on.insert(skill.to_string());
            }
            (false, false) => {
                self.off.insert(skill.to_string());
            }
        }
    }
}

/// The whole file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Switches {
    pub version: u32,
    /// The skills every sandbox gets. A skill not here is off.
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub on: BTreeSet<String>,
    /// Each sandbox's own switches, by sbx name.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sandboxes: BTreeMap<String, Own>,
    /// Where each skill came from, recorded when the dashboard imported or added it.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub origins: BTreeMap<String, Origin>,
    /// The daily update of the store's Git skills; absent is on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auto_update: Option<bool>,
    /// When that update last started, in seconds since the epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<u64>,
    /// The Git repositories the dashboard added skills from, by the source as typed: the daily
    /// update keeps their skills in step with them.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub repos: BTreeMap<String, Tracked>,
    /// What the last update of those repositories did, one line per repository.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub last_report: Vec<String>,
}

/// A repository whose skills the update keeps in step: every skill of it, also skills it gains.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tracked {
    /// Each of its skills as the last update saw it: the git tree id of the skill's folder. Empty
    /// until the first update.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub known: BTreeMap<String, String>,
}

impl Default for Switches {
    fn default() -> Self {
        Self {
            version: VERSION,
            on: BTreeSet::new(),
            sandboxes: BTreeMap::new(),
            origins: BTreeMap::new(),
            auto_update: None,
            updated_at: None,
            repos: BTreeMap::new(),
            last_report: Vec::new(),
        }
    }
}

/// Who a switch is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope<'a> {
    All,
    Sandbox(&'a str),
}

impl Switches {
    /// The switch for all sandboxes.
    pub fn global(&self, skill: &str) -> bool {
        self.on.contains(skill)
    }

    /// The sandbox's own switch for `skill`, where it has one.
    pub fn own(&self, sandbox: &str, skill: &str) -> Option<bool> {
        self.sandboxes.get(sandbox)?.get(skill)
    }

    /// Whether the sandbox gets `skill`: its own switch, else the switch for all sandboxes.
    pub fn is_on(&self, sandbox: &str, skill: &str) -> bool {
        self.own(sandbox, skill)
            .unwrap_or_else(|| self.global(skill))
    }

    /// The switch `scope` sees for `skill`.
    pub fn shown(&self, scope: Scope, skill: &str) -> bool {
        match scope {
            Scope::All => self.global(skill),
            Scope::Sandbox(name) => self.is_on(name, skill),
        }
    }

    /// Turn `skill` `on` for `scope`. A sandbox's switch that matches the switch for all is
    /// dropped. `false` when nothing changed.
    pub fn set(&mut self, scope: Scope, skill: &str, on: bool) -> bool {
        let before = self.clone();
        match scope {
            Scope::All => {
                match on {
                    true => self.on.insert(skill.to_string()),
                    false => self.on.remove(skill),
                };
            }
            Scope::Sandbox(name) => {
                let global = self.global(skill);
                let own = self.sandboxes.entry(name.to_string()).or_default();
                own.set(skill, on, global);
                if own.is_empty() {
                    self.sandboxes.remove(name);
                }
            }
        }
        *self != before
    }

    /// Drop a sandbox's own switches for `skills`, or all of them when `skills` is empty.
    pub fn reset(&mut self, sandbox: &str, skills: &[String]) -> bool {
        let Some(own) = self.sandboxes.get_mut(sandbox) else {
            return false;
        };
        let before = own.clone();
        if skills.is_empty() {
            *own = Own::default();
        } else {
            for skill in skills {
                own.on.remove(skill);
                own.off.remove(skill);
            }
        }
        let changed = *own != before;
        if own.is_empty() {
            self.sandboxes.remove(sandbox);
        }
        changed
    }

    /// A new sandbox of this name starts on `own`: never on the switches of an earlier one.
    pub fn replace_own(&mut self, sandbox: &str, own: Own) -> bool {
        let before = self.sandboxes.get(sandbox).cloned();
        match own.is_empty() {
            true => self.sandboxes.remove(sandbox),
            false => self.sandboxes.insert(sandbox.to_string(), own.clone()),
        };
        before.unwrap_or_default() != own
    }

    /// The sandbox is gone: drop its switches.
    pub fn forget_sandbox(&mut self, sandbox: &str) -> bool {
        self.sandboxes.remove(sandbox).is_some()
    }

    /// The skill left the store: drop every switch and the origin of it, so a skill of that name
    /// added later starts off.
    pub fn forget_skill(&mut self, skill: &str) -> bool {
        let before = self.clone();
        self.on.remove(skill);
        self.origins.remove(skill);
        for own in self.sandboxes.values_mut() {
            own.on.remove(skill);
            own.off.remove(skill);
        }
        self.sandboxes.retain(|_, own| !own.is_empty());
        *self != before
    }

    /// The store skills a sandbox links, sorted: the ones on for all, then its own switches.
    pub fn linked(&self, sandbox: &str) -> Vec<String> {
        self.linked_with(self.sandboxes.get(sandbox).unwrap_or(&Own::default()))
    }

    /// [`Switches::linked`] for a sandbox whose own switches are `own`, as a form holds them.
    pub fn linked_with(&self, own: &Own) -> Vec<String> {
        let mut out: BTreeSet<String> = self.on.iter().cloned().collect();
        out.extend(own.on.iter().cloned());
        for skill in &own.off {
            out.remove(skill);
        }
        out.into_iter().filter(|s| valid_name(s)).collect()
    }

    /// Record where these skills came from.
    pub fn record(&mut self, origins: impl IntoIterator<Item = (String, Origin)>) -> bool {
        let before = self.origins.clone();
        for (skill, origin) in origins {
            if valid_name(&skill) {
                self.origins.insert(skill, origin);
            }
        }
        self.origins != before
    }

    /// The daily update of the store's Git skills is on.
    pub fn auto_update(&self) -> bool {
        self.auto_update.unwrap_or(true)
    }

    /// Whether the daily update is due at `now`: switched on, a repository to keep in step, and no
    /// update started in the last [`UPDATE_EVERY`].
    pub fn update_due(&self, now: u64) -> bool {
        self.auto_update()
            && !self.repos.is_empty()
            && self
                .updated_at
                .is_none_or(|at| now.saturating_sub(at) >= UPDATE_EVERY)
    }
}

/// A store skill's name as aisbx passes it on: letters, digits, `.`, `_` and `-`, not starting
/// with `.` or `-`, at most 128 characters.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    let first = chars.next();
    first.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        && name.len() <= 128
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

pub fn config_path(state: &Path) -> PathBuf {
    state.join(CONFIG_FILE)
}

/// The file. A missing file is every skill off; a file that does not parse, has another
/// `version` or a name this build refuses is an error naming the file, never a default the next
/// write would put over it.
pub fn load(path: &Path) -> Result<Switches> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Switches::default()),
        Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
    };
    let switches: Switches = serde_json::from_str(&text).with_context(|| {
        format!(
            "{} does not parse; fix or remove it (it is left as it is)",
            path.display()
        )
    })?;
    if switches.version != VERSION {
        bail!(
            "{} has version {}, this aisbx reads version {VERSION}; it is left as it is",
            path.display(),
            switches.version
        );
    }
    let names = switches
        .on
        .iter()
        .chain(switches.origins.keys())
        .chain(
            switches
                .sandboxes
                .values()
                .flat_map(|o| o.on.iter().chain(&o.off)),
        )
        .chain(switches.repos.values().flat_map(|t| t.known.keys()));
    for name in names {
        if !valid_name(name) {
            bail!(
                "{}: {:?} is not a skill name aisbx accepts; it is left as it is",
                path.display(),
                name.escape_debug().to_string()
            );
        }
    }
    Ok(switches)
}

/// Change the file under an exclusive lock on `<path>.lock`: load, apply `f`, and write through a
/// temp file and a rename only when `f` changed something. A file that does not load is not
/// written. Hands back the switches as they are now.
pub fn update(path: &Path, f: impl FnOnce(&mut Switches) -> Result<()>) -> Result<Switches> {
    let dir = path
        .parent()
        .context("the store-skills path has no parent directory")?;
    fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
    let _lock = crate::skill_repos::lock_file(&path.with_extension("json.lock"))?;
    let mut switches = load(path)?;
    let before = switches.clone();
    f(&mut switches)?;
    if switches == before {
        return Ok(switches);
    }
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let tmp = path.with_extension(format!(
        "json.{}.{}.tmp",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let written = fs::write(&tmp, serde_json::to_vec_pretty(&switches)?)
        .with_context(|| format!("writing {}", tmp.display()))
        .and_then(|()| {
            fs::rename(&tmp, path).with_context(|| format!("replacing {}", path.display()))
        });
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written.map(|()| switches)
}

/// The pre-start step that links exactly `skills` of the store into the agent's `dirs`, and
/// keeps that choice for the kit hook's later starts: `sbx exec <sandbox> sh -c <hook> …`.
pub fn link_argv(sandbox: &str, dirs: &[String], skills: &[String]) -> Vec<String> {
    let mut argv: Vec<String> = [
        "sbx",
        "exec",
        sandbox,
        "sh",
        "-c",
        SCRIPT,
        "skills-store.sh",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    argv.push("--only".into());
    argv.extend(skills.iter().filter(|s| valid_name(s)).cloned());
    argv.push("--".into());
    argv.extend(dirs.iter().cloned());
    argv
}

/// One of this host's skills folders and the skill folders in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostFolder {
    /// `~/<folder>`, as `sbx skills import` names it.
    pub dir: String,
    pub skills: BTreeSet<String>,
}

/// The skills folders `sbx skills import` reads ([`crate::sbx::SKILL_SOURCES`]) under `home`, in
/// its order, each with the folders in it. A folder that is missing is left out.
pub fn host_folders(home: &Path) -> Vec<HostFolder> {
    crate::sbx::SKILL_SOURCES
        .iter()
        .filter_map(|rel| {
            let entries = fs::read_dir(home.join(rel)).ok()?;
            let skills = entries
                .filter_map(|e| {
                    let e = e.ok()?;
                    let name = e.file_name().into_string().ok()?;
                    (e.path().is_dir() && valid_name(&name)).then_some(name)
                })
                .collect();
            Some(HostFolder {
                dir: format!("~/{rel}"),
                skills,
            })
        })
        .collect()
}

/// What `sbx skills import` copied: each of `store`'s skills that a host folder holds, from the
/// first folder that holds it, as sbx takes the first copy.
pub fn imported(store: &[String], host: &[HostFolder]) -> Vec<(String, Origin)> {
    store
        .iter()
        .filter_map(|skill| {
            let from = host.iter().find(|f| f.skills.contains(skill))?;
            Some((skill.clone(), Origin::Folder(from.dir.clone())))
        })
        .collect()
}

/// What `sbx skills add <repo>` brought: every skill the store holds now and did not before.
pub fn added(repo: &str, before: &[String], after: &[String]) -> Vec<(String, Origin)> {
    after
        .iter()
        .filter(|s| !before.contains(s))
        .map(|s| (s.clone(), Origin::Repo(repo.to_string())))
        .collect()
}

/// A heading of the skills list: where its skills came from.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Source {
    /// A host folder, recorded at import or found holding a skill of the name.
    Folder(String),
    /// A Git repository the dashboard added the skills from.
    Repo(String),
    /// Neither: added outside the dashboard, or from a folder no longer on this host.
    Unknown,
}

/// The store's skills under their sources: the host folders in `sbx skills import`'s order, then
/// the repositories by name, then the rest. A recorded origin wins over a host folder that holds
/// a skill of the name.
pub fn groups(
    store: &[String],
    origins: &BTreeMap<String, Origin>,
    host: &[HostFolder],
) -> Vec<(Source, Vec<String>)> {
    let source_of = |skill: &String| match origins.get(skill) {
        Some(Origin::Repo(repo)) => Source::Repo(repo.clone()),
        Some(Origin::Folder(dir)) => Source::Folder(dir.clone()),
        None => host
            .iter()
            .find(|f| f.skills.contains(skill))
            .map_or(Source::Unknown, |f| Source::Folder(f.dir.clone())),
    };
    let rank = |source: &Source| match source {
        Source::Folder(dir) => (
            0,
            crate::sbx::SKILL_SOURCES
                .iter()
                .position(|rel| *dir == format!("~/{rel}"))
                .unwrap_or(usize::MAX),
            dir.clone(),
        ),
        Source::Repo(repo) => (1, 0, repo.clone()),
        Source::Unknown => (2, 0, String::new()),
    };
    let mut out: Vec<(Source, Vec<String>)> = Vec::new();
    for skill in store {
        let source = source_of(skill);
        match out.iter_mut().find(|(s, _)| *s == source) {
            Some((_, skills)) => skills.push(skill.clone()),
            None => out.push((source, vec![skill.clone()])),
        }
    }
    out.sort_by_key(|(source, _)| rank(source));
    for (_, skills) in &mut out {
        skills.sort();
    }
    out
}

impl Switches {
    /// Keep `repo` in step from now on, every skill of it.
    pub fn track(&mut self, repo: &str) -> bool {
        if self.repos.contains_key(repo) {
            return false;
        }
        self.repos.insert(repo.to_string(), Tracked::default());
        true
    }

    /// Stop keeping `repo` in step when the store holds none of its skills any more.
    pub fn untrack_if_gone(&mut self, repo: &str, store: &[String]) -> bool {
        let held = (self.origins.iter())
            .any(|(skill, o)| *o == Origin::Repo(repo.to_string()) && store.contains(skill));
        !held && self.repos.remove(repo).is_some()
    }
}

/// What one repository's update changes in the store.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RepoPlan {
    /// Skills the repository gained, which the store does not hold yet.
    pub new: Vec<String>,
    /// Skills from this repository whose folder changed since the last update.
    pub changed: Vec<String>,
    /// Skills from this repository that it no longer has.
    pub deleted: Vec<String>,
    /// Every skill of the repository the update keeps in step, as it is now.
    pub known: BTreeMap<String, String>,
    /// Skills in the store now recorded as this repository's: on the first update, those it holds
    /// that came from nowhere aisbx recorded, or, right after an add, every one it holds.
    pub claimed: Vec<String>,
}

impl RepoPlan {
    /// The skills `sbx skills add <repo> --skill …` puts into the store.
    pub fn to_add(&self) -> Vec<String> {
        self.new.iter().chain(&self.changed).cloned().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.new.is_empty() && self.changed.is_empty() && self.deleted.is_empty()
    }
}

/// One repository's update: `found` is each skill of its fetched commit with its content id,
/// `store` the store's skills now. Only skills that came from this repository change: one the
/// user imported or added from elsewhere under the same name is left alone, and one the user
/// removed from the store does not come back. The first update only learns the content ids and
/// claims the store's skills of the repository: `just_added` (the add replaced every one of
/// them) claims them all, else only those of no recorded origin.
pub fn plan_repo(
    repo: &str,
    tracked: &Tracked,
    found: &[(String, String)],
    store: &[String],
    origins: &BTreeMap<String, Origin>,
    just_added: bool,
) -> RepoPlan {
    let ours = |skill: &str| origins.get(skill) == Some(&Origin::Repo(repo.to_string()));
    let wanted: BTreeMap<String, String> = found
        .iter()
        .filter(|(name, _)| valid_name(name))
        .cloned()
        .collect();
    let mut plan = RepoPlan {
        known: wanted.clone(),
        ..RepoPlan::default()
    };
    if tracked.known.is_empty() {
        plan.claimed = (wanted.keys())
            .filter(|name| store.contains(name))
            .filter(|name| just_added || !origins.contains_key(*name))
            .cloned()
            .collect();
        return plan;
    }
    for (name, content) in &wanted {
        match tracked.known.get(name) {
            None if !store.contains(name) => plan.new.push(name.clone()),
            Some(before) if before != content && store.contains(name) && ours(name) => {
                plan.changed.push(name.clone())
            }
            _ => {}
        }
    }
    for name in tracked.known.keys() {
        if !wanted.contains_key(name) && store.contains(name) && ours(name) {
            plan.deleted.push(name.clone());
        }
    }
    plan
}

/// The report line of one repository's update; `None` when it changed nothing.
pub fn report_line(repo: &str, plan: &RepoPlan) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    if !plan.new.is_empty() {
        parts.push(format!(
            "new {} (off until you switch it on)",
            plan.new.join(", ")
        ));
    }
    if !plan.changed.is_empty() {
        parts.push(format!("changed {}", plan.changed.join(", ")));
    }
    if !plan.deleted.is_empty() {
        parts.push(format!("deleted {}", plan.deleted.join(", ")));
    }
    (!parts.is_empty()).then(|| format!("{repo}: {}", parts.join("; ")))
}

/// Where the update keeps its mirrors of the repositories: apart from the deprecated skill
/// repositories' `skill-cache/`.
pub fn cache_root(state: &Path) -> PathBuf {
    state.join("store-skill-cache")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("aisbx-store-skills-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn every_skill_starts_off_and_a_sandbox_follows_the_switch_for_all() {
        let mut s = Switches::default();
        assert!(!s.global("pdf"));
        assert!(!s.is_on("box", "pdf"));
        assert!(s.linked("box").is_empty());
        assert!(s.set(Scope::All, "pdf", true));
        assert!(!s.set(Scope::All, "pdf", true), "no change, no write");
        assert!(s.is_on("box", "pdf"));
        assert_eq!(s.linked("box"), ["pdf"]);
        assert!(s.set(Scope::All, "pdf", false));
        assert!(!s.is_on("box", "pdf"));
    }

    #[test]
    fn a_sandbox_switch_overrides_and_one_that_matches_is_dropped() {
        let mut s = Switches::default();
        s.set(Scope::All, "pdf", true);
        assert!(s.set(Scope::Sandbox("box"), "pdf", false));
        assert_eq!(s.own("box", "pdf"), Some(false));
        assert!(!s.is_on("box", "pdf"));
        assert!(s.is_on("other", "pdf"));
        assert!(s.set(Scope::Sandbox("box"), "pdf", true));
        assert_eq!(s.own("box", "pdf"), None, "back on the switch for all");
        assert!(s.sandboxes.is_empty(), "an empty entry is not kept");

        assert!(s.set(Scope::Sandbox("box"), "docx", true));
        assert_eq!(s.linked("box"), ["docx", "pdf"]);
        assert_eq!(s.linked("other"), ["pdf"]);
        assert!(s.set(Scope::All, "docx", true));
        assert_eq!(
            s.own("box", "docx"),
            Some(true),
            "a change for all keeps own switches"
        );
        assert!(s.set(Scope::All, "docx", false));
        assert!(s.is_on("box", "docx"));
        assert!(!s.is_on("other", "docx"));
    }

    #[test]
    fn reset_replace_and_forget() {
        let mut s = Switches::default();
        s.set(Scope::Sandbox("box"), "a", true);
        s.set(Scope::Sandbox("box"), "b", true);
        assert!(s.reset("box", &strings(&["a"])));
        assert_eq!(s.linked("box"), ["b"]);
        assert!(s.reset("box", &[]));
        assert!(!s.reset("box", &[]));
        assert!(s.sandboxes.is_empty());

        let own = Own {
            on: ["x".to_string()].into(),
            off: BTreeSet::new(),
        };
        assert!(s.replace_own("box", own.clone()));
        assert!(!s.replace_own("box", own));
        assert!(s.replace_own("box", Own::default()));
        assert!(s.sandboxes.is_empty());

        s.set(Scope::All, "pdf", true);
        s.set(Scope::Sandbox("box"), "pdf", false);
        s.set(Scope::Sandbox("two"), "pdf", false);
        s.record([("pdf".to_string(), Origin::Repo("o/r".into()))]);
        assert!(s.forget_sandbox("two"));
        assert!(s.forget_skill("pdf"));
        assert_eq!(
            s,
            Switches::default(),
            "a removed skill leaves nothing behind"
        );
    }

    #[test]
    fn the_file_round_trips_and_a_bad_one_is_left_alone() {
        let dir = scratch("file");
        let path = config_path(&dir);
        assert_eq!(
            load(&path).unwrap(),
            Switches::default(),
            "no file, every skill off"
        );
        let saved = update(&path, |s| {
            s.set(Scope::All, "pdf", true);
            s.set(Scope::Sandbox("box"), "docx", true);
            s.record([("pdf".to_string(), Origin::Folder("~/.claude/skills".into()))]);
            Ok(())
        })
        .unwrap();
        assert_eq!(load(&path).unwrap(), saved);
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("\"folder\": \"~/.claude/skills\""), "{text}");
        assert!(!text.contains("auto_update"), "absent while on: {text}");

        let modified = fs::metadata(&path).unwrap().modified().unwrap();
        update(&path, |s| {
            s.set(Scope::All, "pdf", true);
            Ok(())
        })
        .unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            modified,
            "no change, no write"
        );

        for bad in [
            "{",
            r#"{"version": 2}"#,
            r#"{"version": 1, "on": ["../x"]}"#,
            r#"{"version": 1, "sandboxes": {"box": {"off": ["a b"]}}}"#,
        ] {
            fs::write(&path, bad).unwrap();
            assert!(load(&path).is_err(), "{bad}");
            assert!(update(&path, |s| {
                s.set(Scope::All, "x", true);
                Ok(())
            })
            .is_err());
            assert_eq!(fs::read_to_string(&path).unwrap(), bad, "left as it was");
        }
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn names_that_reach_the_sandbox_are_plain() {
        for good in ["pdf", "frontend-design", "a.b_c", "_x", "A1"] {
            assert!(valid_name(good), "{good}");
        }
        for bad in [
            "",
            ".hidden",
            "-x",
            "a/b",
            "..",
            "a b",
            "a\nb",
            &"x".repeat(129),
        ] {
            assert!(!valid_name(bad), "{bad:?}");
        }
        let mut s = Switches::default();
        s.on.insert("../x".into());
        s.on.insert("ok".into());
        assert_eq!(s.linked("box"), ["ok"]);
    }

    #[test]
    fn the_link_step_runs_the_kits_hook_with_the_names_and_folders() {
        let argv = link_argv(
            "box",
            &strings(&["/home/agent/.claude/skills"]),
            &strings(&["pdf", "a b", "docx"]),
        );
        assert_eq!(argv[..5], ["sbx", "exec", "box", "sh", "-c"]);
        assert_eq!(argv[5], SCRIPT);
        assert_eq!(
            argv[6..],
            [
                "skills-store.sh",
                "--only",
                "pdf",
                "docx",
                "--",
                "/home/agent/.claude/skills"
            ]
        );
        assert!(SCRIPT.starts_with("#!/bin/sh\n") && SCRIPT.ends_with("exit 0\n"));
        assert!(SCRIPT.contains("--only"), "the hook takes the step's names");
        let none = link_argv("box", &strings(&["/d"]), &[]);
        assert_eq!(none[6..], ["skills-store.sh", "--only", "--", "/d"]);
    }

    #[test]
    fn the_daily_update_needs_a_tracked_repository_and_a_day() {
        let mut s = Switches::default();
        assert!(s.auto_update());
        assert!(!s.update_due(10_000_000), "no repository to keep in step");
        s.record([("pdf".to_string(), Origin::Folder("~/.agents/skills".into()))]);
        assert!(
            !s.update_due(10_000_000),
            "an imported skill is no repository"
        );
        s.track("anthropics/skills");
        assert!(s.update_due(10_000_000), "never updated");
        s.updated_at = Some(10_000_000);
        assert!(!s.update_due(10_000_000 + UPDATE_EVERY - 1));
        assert!(s.update_due(10_000_000 + UPDATE_EVERY));
        s.auto_update = Some(false);
        assert!(!s.update_due(10_000_000 + 2 * UPDATE_EVERY));
    }

    #[test]
    fn a_repository_is_tracked_whole_and_untracked_once_its_skills_are_gone() {
        let mut s = Switches::default();
        assert!(s.track("o/a"));
        assert!(!s.track("o/a"), "tracked once");
        assert_eq!(s.repos["o/a"], Tracked::default());

        s.record([("pdf".to_string(), Origin::Repo("o/a".into()))]);
        assert!(
            !s.untrack_if_gone("o/a", &strings(&["pdf"])),
            "pdf still in the store"
        );
        assert!(s.untrack_if_gone("o/a", &strings(&["web"])));
        assert!(s.repos.is_empty());
    }

    fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect()
    }

    #[test]
    fn the_update_adds_new_skills_changes_changed_ones_and_deletes_gone_ones() {
        let repo = "o/a";
        let from = |names: &[&str]| -> BTreeMap<String, Origin> {
            names
                .iter()
                .map(|n| (n.to_string(), Origin::Repo(repo.into())))
                .collect()
        };
        let first = Tracked::default();
        let found = pairs(&[("pdf", "t1"), ("docx", "t2"), ("xlsx", "t3")]);
        let store = strings(&["pdf", "docx"]);
        let plan = plan_repo(repo, &first, &found, &store, &from(&["pdf", "docx"]), false);
        assert!(plan.is_empty(), "the first update only learns: {plan:?}");
        assert_eq!(plan.known.len(), 3);
        assert!(
            plan.claimed.is_empty(),
            "both already recorded as this repo's"
        );
        let mut mixed = from(&["pdf"]);
        mixed.insert("docx".into(), Origin::Folder("~/.claude/skills".into()));
        let unknown = strings(&["pdf", "docx", "xlsx"]);
        let daily = plan_repo(repo, &first, &found, &unknown, &mixed, false);
        assert_eq!(
            daily.claimed,
            ["xlsx"],
            "only a skill of no recorded origin"
        );
        let added = plan_repo(repo, &first, &found, &unknown, &mixed, true);
        assert_eq!(
            added.claimed,
            ["docx", "pdf", "xlsx"],
            "the add replaced them all"
        );

        let tracked = Tracked {
            known: plan.known.clone(),
        };
        let mut origins = from(&["pdf", "docx", "old"]);
        origins.insert("web".into(), Origin::Folder("~/.claude/skills".into()));
        let tracked = Tracked {
            known: [("old", "t0"), ("web", "w0")]
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .chain(tracked.known)
                .collect(),
        };
        let found = pairs(&[
            ("pdf", "t1"),
            ("docx", "t9"),
            ("xlsx", "t3"),
            ("new", "n1"),
            ("web", "w1"),
            ("bad/x", "z"),
        ]);
        let store = strings(&["pdf", "docx", "old", "web"]);
        let plan = plan_repo(repo, &tracked, &found, &store, &origins, false);
        assert_eq!(
            plan.new,
            ["new"],
            "xlsx was known and is not in the store: removed by the user"
        );
        assert_eq!(
            plan.changed,
            ["docx"],
            "web changed but was imported: left alone"
        );
        assert_eq!(plan.deleted, ["old"]);
        assert_eq!(plan.to_add(), ["new", "docx"]);
        assert!(!plan.known.contains_key("bad/x"));
        assert_eq!(
            report_line(repo, &plan).unwrap(),
            "o/a: new new (off until you switch it on); changed docx; deleted old"
        );

        assert_eq!(report_line(repo, &RepoPlan::default()), None);
    }

    #[test]
    fn import_and_add_record_where_skills_came_from() {
        let host = vec![
            HostFolder {
                dir: "~/.agents/skills".into(),
                skills: ["a".to_string()].into(),
            },
            HostFolder {
                dir: "~/.claude/skills".into(),
                skills: ["a".to_string(), "b".to_string()].into(),
            },
        ];
        assert_eq!(
            imported(&strings(&["a", "b", "c"]), &host),
            [
                ("a".to_string(), Origin::Folder("~/.agents/skills".into())),
                ("b".to_string(), Origin::Folder("~/.claude/skills".into())),
            ]
        );
        assert_eq!(
            added("o/r", &strings(&["a", "b"]), &strings(&["a", "b", "c"])),
            [("c".to_string(), Origin::Repo("o/r".into()))]
        );
    }

    #[test]
    fn skills_are_grouped_by_where_they_came_from() {
        let host = vec![
            HostFolder {
                dir: "~/.agents/skills".into(),
                skills: ["web".to_string()].into(),
            },
            HostFolder {
                dir: "~/.claude/skills".into(),
                skills: ["slides".to_string(), "web".to_string(), "pdf".to_string()].into(),
            },
        ];
        let origins: BTreeMap<String, Origin> = [
            ("pdf".to_string(), Origin::Repo("anthropics/skills".into())),
            ("docx".to_string(), Origin::Repo("anthropics/skills".into())),
            ("old".to_string(), Origin::Folder("~/.cursor/skills".into())),
            ("z".to_string(), Origin::Repo("acme/skills".into())),
        ]
        .into();
        let store = strings(&["web", "slides", "pdf", "docx", "old", "z", "lost"]);
        assert_eq!(
            groups(&store, &origins, &host),
            [
                (Source::Folder("~/.agents/skills".into()), strings(&["web"])),
                (
                    Source::Folder("~/.claude/skills".into()),
                    strings(&["slides"])
                ),
                (Source::Folder("~/.cursor/skills".into()), strings(&["old"])),
                (Source::Repo("acme/skills".into()), strings(&["z"])),
                (
                    Source::Repo("anthropics/skills".into()),
                    strings(&["docx", "pdf"])
                ),
                (Source::Unknown, strings(&["lost"])),
            ]
        );
    }

    #[test]
    #[cfg(unix)]
    fn host_folders_list_the_skill_folders_in_sbx_order() {
        let home = scratch("host");
        fs::create_dir_all(home.join(".claude/skills/b")).unwrap();
        fs::create_dir_all(home.join(".agents/skills/a")).unwrap();
        fs::write(home.join(".agents/skills/loose.md"), "x").unwrap();
        fs::create_dir_all(home.join("elsewhere/linked")).unwrap();
        std::os::unix::fs::symlink(home.join("elsewhere/linked"), home.join(".agents/skills/l"))
            .unwrap();
        let found = host_folders(&home);
        assert_eq!(
            found,
            [
                HostFolder {
                    dir: "~/.agents/skills".into(),
                    skills: ["a".to_string(), "l".to_string()].into(),
                },
                HostFolder {
                    dir: "~/.claude/skills".into(),
                    skills: ["b".to_string()].into(),
                },
            ]
        );
        let _ = fs::remove_dir_all(&home);
    }
}
