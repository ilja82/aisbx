//! The shared skills store (`K`), one sandbox's skills (`i`) and the deprecated Repo skills
//! picker. Core decides the rows and every decision drawn; nothing here reads a file or calls
//! sbx.

use aisbx_core::app::{skills_shared_line, App, Hit, SandboxSkillRow, Screen, SkillRow};
use aisbx_core::compat;
use aisbx_core::input::{KeyCode, KeyEvent};
use aisbx_core::model;
use aisbx_core::repo_skills_ui::{RepoScope, RepoSkillRow, RepoSkillsForm};
use eframe::egui;

use crate::form::{self, Look, Rows};
use crate::page::{self, Item};

/// What a row of the store's page can do, as its buttons: a skill is removed (`d`), each of the
/// store's commands and the repo skills picker run by ↵, as the TUI's rows do. A skill's and the
/// daily update's switch is Space ([`App::skill_switch`]).
pub fn row_actions(row: &SkillRow) -> Vec<(&'static str, KeyCode, Look)> {
    match row {
        SkillRow::Skill(_) => vec![("Remove", KeyCode::Char('d'), Look::Danger)],
        SkillRow::Import => vec![("Import…", KeyCode::Enter, Look::Plain)],
        SkillRow::AddRepo => vec![("Add…", KeyCode::Enter, Look::Plain)],
        SkillRow::Update => vec![("Update…", KeyCode::Enter, Look::Plain)],
        SkillRow::RepoSkills => vec![("Open", KeyCode::Enter, Look::Plain)],
        SkillRow::AutoUpdate | SkillRow::Close => Vec::new(),
    }
}

/// What a row of a sandbox's skills page can do, as its buttons: *Apply now*, the reset and the
/// repo skills picker run by ↵. A skill's switch is Space ([`App::sandbox_skill_switch`]).
pub fn sandbox_row_actions(row: &SandboxSkillRow) -> Vec<(&'static str, KeyCode, Look)> {
    match row {
        SandboxSkillRow::ApplyNow => vec![("Apply", KeyCode::Enter, Look::Plain)],
        SandboxSkillRow::Reset => vec![("Reset", KeyCode::Enter, Look::Plain)],
        SandboxSkillRow::RepoSkills => vec![("Open", KeyCode::Enter, Look::Plain)],
        SandboxSkillRow::Skill(_) | SandboxSkillRow::Close => Vec::new(),
    }
}

/// The key that turns a row's switch the other way, as core's skills screens take it.
const SWITCH_KEY: KeyCode = KeyCode::Char(' ');

/// The rows by `heading`, each run of rows under one heading a group, in order. A row without a
/// heading (the button) is in none.
fn groups(headings: Vec<Option<(String, String)>>) -> Vec<((String, String), Vec<usize>)> {
    let mut groups: Vec<((String, String), Vec<usize>)> = Vec::new();
    for (i, heading) in headings.into_iter().enumerate() {
        let Some(heading) = heading else {
            continue;
        };
        match groups.last_mut() {
            Some((h, items)) if *h == heading => items.push(i),
            _ => groups.push((heading, vec![i])),
        }
    }
    groups
}

/// What a click on a row of a skills page asked of core: only to select the row, or to press a
/// key on it (its switch's Space, a button's key).
#[derive(Clone, Copy)]
enum Asked {
    Select(usize),
    Press(usize, KeyCode),
}

/// One row of a skills page as an item of its group: `title` and `meta`, the row's buttons and,
/// where it has one, its switch at the right edge. `i` is the row's place in core's rows, `n`
/// its place in the group; `notes` shows under it while selected. Returns what a click on it
/// asked.
fn row_item(
    ui: &mut egui::Ui,
    list: &Rows,
    (i, n): (usize, usize),
    (title, meta): (&str, &str),
    switch: Option<bool>,
    actions: &[(&'static str, KeyCode, Look)],
    notes: &[String],
) -> Option<Asked> {
    let labels: Vec<(&str, Look)> = actions.iter().map(|(l, _, look)| (*l, *look)).collect();
    let item = Item {
        title,
        mark: None,
        meta,
        state: None,
        actions: &labels,
        switch,
        reserve: 0.0,
        selected: list.is_focused(i),
        rule: n > 0,
    };
    let got = page::item(ui, item, list.follows(i), |ui| {
        page::notes(ui, &notes.join(" "))
    });
    match (got.switch, got.action, got.item) {
        (true, _, _) => Some(Asked::Press(i, SWITCH_KEY)),
        (false, Some(a), _) => Some(Asked::Press(i, actions[a].1)),
        (false, None, true) => Some(Asked::Select(i)),
        (false, None, false) => None,
    }
}

/// Hand this frame's click to core: a key on its row after core's cursor moved there, so it acts
/// on that row; a click on the page's bar as it is.
fn hand(app: &mut App, asked: Option<Asked>, bar: Option<Hit>) {
    match asked {
        Some(Asked::Press(i, code)) => {
            app.click(Hit::FormFocus(i));
            app.on_key(KeyEvent::from(code));
        }
        Some(Asked::Select(i)) => app.click(Hit::FormFocus(i)),
        None => {
            if let Some(hit) = bar {
                app.click(hit);
            }
        }
    }
}

/// Draw the skills store: its skills under where they came from, each with *Remove* and its
/// switch for all sandboxes at the right edge, then the commands that change the store, the
/// daily update's switch and the repo skills picker; a click on a row selects it and says what
/// it does.
pub fn show_store(ui: &mut egui::Ui, app: &mut App) {
    let Screen::Skills(skills) = &app.screen else {
        return;
    };
    let view: &App = app;
    let rows = view.skill_rows();
    let focus = skills.focus.min(rows.len() - 1);
    let mut list = Rows::new(ui, "skills", focus);
    let known = view.skills.is_some();
    let close = form::index_of(&rows, SkillRow::Close);
    let groups = groups(rows.iter().map(|r| view.skill_heading(r)).collect());
    let store = match view.skills_store_path() {
        Some(store) => format!("The store: {}.", model::tilde(&store)),
        None => "sbx does not report where the store is.".to_string(),
    };
    let facts = [store, skills_shared_line(view.skills_sharers())]
        .into_iter()
        .chain(view.skills_trust_lines())
        .collect::<Vec<_>>()
        .join(" ");
    let subtitle = format!("{facts}\n{}", view.skills_intro().join(" "));
    let mut asked = None;

    page::bar(ui, "skills", |ui| {
        list.buttons(ui, &[("Close", close, false)]);
    });
    page::body(ui, "skills", "Shared agent skills", &subtitle, |ui| {
        let weak = ui.visuals().weak_text_color();
        if !known {
            page::paragraph(ui, "Reading `sbx skills ls`…", weak);
        }
        if known && !rows.iter().any(|r| matches!(r, SkillRow::Skill(_))) {
            page::heading(ui, "Skills in the store", "");
            page::paragraph(
                ui,
                "Nothing in the store yet — import this host's, or add from a repository.",
                weak,
            );
        }
        for ((title, gloss), items) in &groups {
            page::heading(ui, title, gloss);
            page::group(ui, |ui| {
                for (n, i) in items.iter().enumerate() {
                    let row = &rows[*i];
                    let (name, meta) = view.skill_view(row);
                    let got = row_item(
                        ui,
                        &list,
                        (*i, n),
                        (&name, &meta),
                        view.skill_switch(row),
                        &row_actions(row),
                        &view.skill_notes(row),
                    );
                    asked = asked.or(got);
                }
            });
        }
    });
    hand(app, asked, list.clicked);
}

/// Draw one sandbox's skills: the store's skills under where they came from, each with this
/// sandbox's switch at the right edge, then *Apply now*, the reset and the repo skills picker,
/// each with the button that runs it.
pub fn show_sandbox(ui: &mut egui::Ui, app: &mut App) {
    let Screen::SandboxSkills(form) = &app.screen else {
        return;
    };
    let view: &App = app;
    let sandbox = form.sandbox.as_str();
    let rows = view.sandbox_skill_rows(sandbox);
    let focus = form.focus.min(rows.len() - 1);
    let mut list = Rows::new(ui, "sandbox-skills", focus);
    let close = form::index_of(&rows, SandboxSkillRow::Close);
    let groups = groups(rows.iter().map(|r| view.sandbox_skill_heading(r)).collect());
    let title = format!("Skills · {}", view.sandbox_label(sandbox));
    let subtitle = view.sandbox_skills_intro(sandbox).join(" ");
    let mut asked = None;

    page::bar(ui, "sandbox-skills", |ui| {
        list.buttons(ui, &[("Close", close, false)]);
    });
    page::body(ui, "sandbox-skills", &title, &subtitle, |ui| {
        for ((title, gloss), items) in &groups {
            page::heading(ui, title, gloss);
            page::group(ui, |ui| {
                for (n, i) in items.iter().enumerate() {
                    let row = &rows[*i];
                    let (name, meta) = view.sandbox_skill_view(sandbox, row);
                    let got = row_item(
                        ui,
                        &list,
                        (*i, n),
                        (&name, &meta),
                        view.sandbox_skill_switch(sandbox, row),
                        &sandbox_row_actions(row),
                        &view.sandbox_skill_notes(sandbox, row),
                    );
                    asked = asked.or(got);
                }
            });
        }
    });
    hand(app, asked, list.clicked);
}

/// Draw the Repo skills picker under its deprecation line and pass a click on a row or a button
/// to core.
pub fn show_repo(ui: &mut egui::Ui, app: &mut App) {
    let Screen::RepoSkills(repo) = &app.screen else {
        return;
    };
    let view: &App = app;
    let rows = repo.rows();
    let focus = repo.focus.min(rows.len() - 1);
    let mut list = Rows::new(ui, "repo-skills", focus);
    let index = |row: RepoSkillRow| form::index_of(&rows, row);
    let dark = ui.visuals().dark_mode;

    page::bar(ui, "repo-skills", |ui| {
        if repo.dirty {
            form::warn(ui, "Not saved yet.");
        }
        list.buttons(
            ui,
            &[
                ("Save", index(RepoSkillRow::Save), false),
                ("Close", index(RepoSkillRow::Close), false),
            ],
        );
    });
    let title = match &repo.scope {
        RepoScope::Global => "Repo skills · all sandboxes".to_string(),
        RepoScope::Sandbox(name) => format!("Repo skills · {}", view.sandbox_label(name)),
    };
    page::body(ui, "repo-skills", &title, repo.intro(), |ui| {
        page::paragraph(ui, compat::SKILL_REPOS_SCREEN, crate::theme::warn(dark));
        if repo.error.is_some() {
            for line in repo.error_lines() {
                form::warn(ui, &line);
            }
        } else if repo.repos.is_empty() {
            form::para(ui, aisbx_core::repo_skills_ui::NO_REPOS);
        }
        let mut shown: Option<&str> = None;
        for (i, row) in rows.iter().enumerate() {
            let id = match row {
                RepoSkillRow::All(id) | RepoSkillRow::Skill { repo: id, .. } => Some(id.as_str()),
                RepoSkillRow::SyncNow | RepoSkillRow::Save | RepoSkillRow::Close => None,
            };
            if id != shown {
                footer(ui, repo, shown);
                if let Some(r) = id.and_then(|id| repo.repos.iter().find(|r| r.id == id)) {
                    let (source, meta) = r.heading();
                    form::section(ui, &source, &meta);
                }
                shown = id;
            }
            let job = match row {
                RepoSkillRow::All(id) => {
                    tick_row(ui, repo, row, "all skills", repo.locked(id, None))
                }
                RepoSkillRow::Skill { name, locked, .. } => tick_row(ui, repo, row, name, *locked),
                RepoSkillRow::SyncNow => form::with_hint(
                    ui,
                    "▸ Sync now",
                    Some(crate::theme::accent(dark)),
                    "otherwise at the next start",
                ),
                // In the bottom panel.
                RepoSkillRow::Save | RepoSkillRow::Close => continue,
            };
            list.row(ui, Some(i), job);
            if list.is_focused(i) {
                let notes = repo.notes(row);
                if !notes.is_empty() {
                    ui.indent(("repo-notes", i), |ui| form::para(ui, &notes.join(" ")));
                }
            }
        }
        footer(ui, repo, shown);
    });
    if let Some(hit) = list.clicked {
        app.click(hit);
    }
}

/// One tick row: the box, the name, and `global` — greyed — where a global pick locks it.
fn tick_row(
    ui: &egui::Ui,
    repo: &RepoSkillsForm,
    row: &RepoSkillRow,
    name: &str,
    locked: bool,
) -> egui::text::LayoutJob {
    let tick = if repo.ticked(row) { "☑" } else { "☐" };
    let color = locked.then(|| ui.visuals().weak_text_color());
    form::with_hint(
        ui,
        &format!("{tick} {name}"),
        color,
        if locked { "global" } else { "" },
    )
}

/// Under a repo's rows: why it has no readable copy, and the folders it does not offer.
fn footer(ui: &mut egui::Ui, repo: &RepoSkillsForm, id: Option<&str>) {
    let Some(r) = id.and_then(|id| repo.repos.iter().find(|r| r.id == id)) else {
        return;
    };
    for line in r.footer() {
        form::para(ui, &line);
    }
}
