//! The Inbox list, the new-spec flow that starts from it, and the first-use and scan screens.

use super::*;

#[test]
fn inbox_list() -> Result<()> {
    let mut fixture = Fixture::new("list")?;
    fixture.check("list")?;
    fixture.focus("Settlement reconciliation")?;
    fixture.check("list-long-title-selected")?;
    fixture.press(KeyCode::Char('a'))?;
    fixture.check("list-archive-confirm")?;
    fixture.press(KeyCode::Esc)?;
    fixture.check("list-archive-cancelled")?;
    fixture.press(KeyCode::Char('a'))?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("list-item-archived")?;
    fixture.app.notice.clear();
    fixture.app.list.tree.focused = 0;
    fixture.press(KeyCode::Enter)?;
    fixture.check("list-folder-collapsed")?;
    fixture.press(KeyCode::Enter)?;
    fixture.set_jira(false)?;
    fixture.check("list-jira-off")?;
    fixture.finish()
}

#[test]
fn deep_folders_and_long_names() -> Result<()> {
    let mut fixture = Fixture::new("list-deep")?;
    let deep = fixture.root.join(
        "specs/missions/zeller/settlement-and-reconciliation-programme/\
         acquirer-integrations/regional-scheme-adapters",
    );
    fs::create_dir_all(&deep)?;
    let title = "Chargeback evidence handling for regional card schemes";
    fs::write(deep.join("chargeback-evidence.md"), format!("# {title}\n"))?;
    fixture.app.store.scan()?;
    fixture.app.refresh()?;
    fixture.focus("Chargeback evidence")?;
    fixture.check("list-deep-path-selected")?;
    fixture.finish()
}

#[test]
fn new_spec_flow() -> Result<()> {
    let mut fixture = Fixture::new("new-spec")?;
    let clients = || vec![Profile::Opus, Profile::Codex];
    fixture.app.choose_client(ChoicePurpose::NewSpec, clients());
    fixture.check("new-spec-client-choice")?;
    fixture.press(KeyCode::Char('j'))?;
    fixture.check("new-spec-second-client")?;
    fixture.press(KeyCode::Enter)?;
    fixture.check("new-spec-workspace-prompt")?;
    fixture.press(KeyCode::Esc)?;
    fixture
        .app
        .choose_client(ChoicePurpose::NewSpec, Vec::new());
    fixture.check("new-spec-no-client")?;
    fixture.finish()
}

#[test]
fn first_use_and_scan_issues() -> Result<()> {
    let mut fixture = Fixture::empty("first-use")?;
    fixture.check("settings-first-use")?;
    crate::ui::picker::set_test_result(Err("Could not open the macOS selector".into()));
    fixture.press(KeyCode::Enter)?;
    fixture.check("settings-picker-error")?;
    fixture.finish()?;

    let mut fixture = Fixture::new("scan-issues")?;
    let report = ScanReport {
        imported: 2,
        known: 3,
        archived: 1,
        dropped: 1,
        issues: vec![
            "Cannot read spec /tmp/herdr-inbox-golden-scan-issues/specs/locked.md: Permission denied"
                .into(),
            "Spec outside selected folder: /tmp/elsewhere/linked.md".into(),
        ],
    };
    scan::show(&mut fixture.app, report);
    fixture.check("scan-issues")?;
    fixture.finish()
}
