//! One sandbox's MCP servers (`t`): which of this computer's servers its gateway has loaded.
//! Core decides the rows and every decision drawn; nothing here calls sbx.

use aisbx_core::app::{App, SandboxMcpRow, Screen};
use eframe::egui;

use crate::form::{self, Rows};
use crate::page;
use crate::skills::{groups, hand, row_item};

/// Draw one sandbox's MCP servers under where they are set up, each with whether the sandbox
/// has it loaded as the switch at the right edge.
pub fn show(ui: &mut egui::Ui, app: &mut App) {
    let Screen::SandboxMcp(form) = &app.screen else {
        return;
    };
    let view: &App = app;
    let sandbox = form.sandbox.as_str();
    let rows = view.sandbox_mcp_rows(sandbox);
    let focus = form.focus.min(rows.len() - 1);
    let mut list = Rows::new(ui, "sandbox-mcp", focus);
    let close = form::index_of(&rows, SandboxMcpRow::Close);
    let groups = groups(rows.iter().map(|r| view.sandbox_mcp_heading(r)).collect());
    let title = format!("MCP servers · {}", view.sandbox_label(sandbox));
    let subtitle = view.sandbox_mcp_intro(sandbox).join(" ");
    let mut asked = None;

    page::bar(ui, "sandbox-mcp", |ui| {
        list.buttons(ui, &[("Close", close, false)]);
    });
    page::body(ui, "sandbox-mcp", &title, &subtitle, |ui| {
        for ((title, gloss), items) in &groups {
            page::heading(ui, title, gloss);
            page::group(ui, |ui| {
                for (n, i) in items.iter().enumerate() {
                    let row = &rows[*i];
                    let (name, meta) = view.sandbox_mcp_view(row);
                    let got = row_item(
                        ui,
                        &list,
                        (*i, n),
                        (&name, &meta),
                        view.sandbox_mcp_switch(sandbox, row),
                        &[],
                        &view.sandbox_mcp_notes(sandbox, row),
                    );
                    asked = asked.or(got);
                }
            });
        }
    });
    hand(app, asked, list.clicked);
}
