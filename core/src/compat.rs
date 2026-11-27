//! The deprecated ways that still work, one function per item, named after its id in
//! `docs/deprecations.md`. Removing an item is: delete its functions, their calls, their tests
//! and its row there.

/// `skill-repos`: the line `aisbx skill-repo` and `aisbx skill` print on stderr, once per run.
pub fn skill_repos_warning(command: &str) -> String {
    format!(
        "aisbx: `aisbx {command}` is deprecated: add the repository on Skills (K) with a, or \
         with `sbx skills add REPO` and `aisbx skills update --track REPO`, then switch its skills \
         on with `aisbx skills on` (or K); see docs/deprecations.md#skill-repos"
    )
}

/// `skill-repos`: the line `aisbx up --skill` prints on stderr, once per run.
pub fn skill_repos_up_warning() -> String {
    "aisbx: `aisbx up --skill` is deprecated: use `--skill-on SKILL` for a skill of the shared \
     store (`sbx skills add REPO` puts it there); see docs/deprecations.md#skill-repos"
        .to_string()
}

/// `skill-repos`: the line the dashboard's Repo skills screens show under their title.
pub const SKILL_REPOS_SCREEN: &str = "Deprecated: skill repositories make way for the shared \
     skills store. Add the skills there (K, then Add from a Git repository) and switch them on.";

/// `skill-repos`: the title words of the rows that open those screens.
pub const SKILL_REPOS_ROW: &str = "Repo skills (deprecated)";

/// `skill-repos`: whether a `skill` command line is aisbx's own pre-start sync, which warns
/// nothing: the dashboard and `aisbx run` call it, not a person.
pub fn skill_repos_quiet(args: &[String]) -> bool {
    args.first().map(String::as_str) == Some("sync") && args.iter().any(|a| a == "--start")
}

/// `host-skills`: the line `aisbx up --kit host-skills` prints on stderr, once per run; `None` for
/// kits without it.
pub fn host_skills_warning(addons: &[String]) -> Option<String> {
    addons.iter().any(|a| a == crate::kits::HOST_SKILLS).then(|| {
        "aisbx: the host-skills kit is deprecated: import your skills into the shared store with \
         `sbx skills import` (or Skills, K, in the dashboard) and switch them on with \
         `aisbx skills on --global SKILL`; see docs/deprecations.md#host-skills"
            .to_string()
    })
}

/// `host-skills`: what the details pane says of a sandbox whose kits (`addons`) hold it, beside
/// the action that removes it (the kits editor); `None` for one without it.
pub fn host_skills_note(addons: &[String]) -> Option<&'static str> {
    addons
        .iter()
        .any(|a| a == crate::kits::HOST_SKILLS)
        .then_some(
            "host-skills is deprecated: import your skills on Skills and switch them on, then \
             remove the kit.",
        )
}

/// `host-skills`: what the status line says when the create wizard opens on the answers of a
/// create that had the kit, which it no longer ticks; `None` when they did not have it.
pub fn host_skills_prefill(saved: &[String]) -> Option<&'static str> {
    saved
        .iter()
        .any(|a| a == crate::kits::HOST_SKILLS)
        .then_some(
            "prefilled with the settings of your last sandbox, without host-skills: it is \
             deprecated — import your skills on Skills (K) instead",
        )
}

pub fn mcp_default_warning() -> String {
    "aisbx: note: a new sandbox now starts without MCP servers; pass `--mcp` to let the agent find \
     the host's servers as before, or `--no-mcp` to keep them off without this note; see \
     docs/deprecations.md#mcp-default"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_default_warning_names_the_flag_and_the_row() {
        let line = mcp_default_warning();
        assert!(line.starts_with("aisbx: "), "{line}");
        assert!(line.contains("`--mcp`"), "{line}");
        assert!(
            line.contains("`--no-mcp` to keep them off without this note"),
            "{line}"
        );
        assert!(!line.contains("  "), "{line}");
        assert!(line.ends_with("docs/deprecations.md#mcp-default"), "{line}");
    }

    #[test]
    fn host_skills_lines_name_the_replacement_and_the_row() {
        let words = |w: &[&str]| w.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let with = words(&["rtk", "host-skills"]);
        let line = host_skills_warning(&with).expect("a warning");
        assert!(
            line.starts_with("aisbx: the host-skills kit is deprecated"),
            "{line}"
        );
        assert!(line.contains("`sbx skills import`"), "{line}");
        assert!(line.contains("`aisbx skills on --global SKILL`"), "{line}");
        assert!(line.ends_with("docs/deprecations.md#host-skills"), "{line}");
        assert!(!line.contains("  "), "{line}");
        for other in [words(&["rtk"]), words(&["host-skills-old"]), Vec::new()] {
            assert_eq!(host_skills_warning(&other), None, "{other:?}");
            assert_eq!(host_skills_note(&other), None, "{other:?}");
            assert_eq!(host_skills_prefill(&other), None, "{other:?}");
        }
        let note = host_skills_note(&with).expect("a note");
        assert!(note.starts_with("host-skills is deprecated"), "{note}");
        assert!(!note.contains("  "), "{note}");
        let prefill = host_skills_prefill(&with).expect("a status line");
        assert!(prefill.starts_with(crate::create::PREFILLED), "{prefill}");
        assert!(prefill.ends_with("on Skills (K) instead"), "{prefill}");
        assert!(!prefill.contains("  "), "{prefill}");
    }

    #[test]
    fn skill_repos_warnings_name_the_replacement_and_the_row() {
        for line in [skill_repos_warning("skill-repo"), skill_repos_up_warning()] {
            assert!(line.starts_with("aisbx: `aisbx "), "{line}");
            assert!(line.contains("sbx skills add REPO"), "{line}");
            assert!(line.ends_with("docs/deprecations.md#skill-repos"), "{line}");
        }
        assert!(skill_repos_warning("skill").contains("`aisbx skill` is deprecated"));
        assert!(skill_repos_up_warning().contains("--skill-on SKILL"));
        assert!(SKILL_REPOS_SCREEN.starts_with("Deprecated: "));
        assert!(SKILL_REPOS_ROW.ends_with("(deprecated)"));
        let words = |w: &[&str]| w.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(skill_repos_quiet(&words(&["sync", "--start", "box"])));
        assert!(!skill_repos_quiet(&words(&["sync", "box"])));
        assert!(!skill_repos_quiet(&words(&["add", "box", "--start"])));
        assert!(!skill_repos_quiet(&[]));
    }
}
