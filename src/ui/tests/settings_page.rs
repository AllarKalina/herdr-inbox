use super::proximity::{Fixture, press};
use super::support::lines;
use super::*;

fn open_settings_row(app: &mut App, row: usize) -> Result<()> {
    press(app, KeyCode::Char('s'))?;
    assert_eq!(app.screen, Screen::Settings);
    for _ in 0..row {
        press(app, KeyCode::Char('j'))?;
    }
    app.refresh()
}

#[test]
fn settings_rows_show_values_and_the_selected_row_names_its_action() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    let app = &mut fixture.app;
    press(app, KeyCode::Esc)?;
    for (row, verb, value) in [
        (0, "change", "/specs"),
        (1, "toggle", "on"),
        (2, "open", "empty"),
    ] {
        open_settings_row(app, row)?;
        for (width, height) in [(40, 18), (100, 35)] {
            let text = lines(app, width, height)?;
            assert!(text[3].starts_with("  Specs folder   "), "{text:#?}");
            assert!(text[4].starts_with("  Jira           on"), "{text:#?}");
            assert!(text[5].starts_with("  Archive        empty"), "{text:#?}");
            assert!(text[3 + row].contains(value), "{text:#?}");
            assert!(
                text[usize::from(height) - 2]
                    .starts_with(&format!("  j/k select · Enter {verb} · Esc back")),
                "{text:#?}"
            );
        }
        press(app, KeyCode::Esc)?;
        assert_eq!(app.screen, Screen::List);
    }
    Ok(())
}

#[test]
fn turning_jira_off_removes_its_column_and_milestone_and_unlocks_dev() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    let app = &mut fixture.app;
    assert_eq!(app.milestone_selected, Milestone::Jira);
    press(app, KeyCode::Esc)?;
    assert!(lines(app, 100, 35)?[3].contains("Jira"));
    assert!(lines(app, 40, 18)?[3].contains("S J D P"));

    open_settings_row(app, 1)?;
    press(app, KeyCode::Enter)?;
    assert!(!app.store.settings()?.jira);
    assert!(lines(app, 100, 35)?[4].starts_with("  Jira           off"));
    press(app, KeyCode::Esc)?;

    let wide = lines(app, 100, 35)?;
    assert!(wide[3].contains("Spec") && wide[3].contains("Dev") && wide[3].contains("PR"));
    assert!(!wide.join("\n").contains("Jira"));
    let narrow = lines(app, 40, 18)?;
    assert!(narrow[3].trim_end().ends_with("  S D P"), "{narrow:#?}");

    press(app, KeyCode::Enter)?;
    assert_eq!(app.screen, Screen::Detail);
    assert_eq!(app.milestone_selected, Milestone::Dev);
    assert_eq!(app.actions(), vec![DetailAction::Implement]);
    for (width, height) in [(40, 18), (60, 24), (100, 35)] {
        let text = lines(app, width, height)?.join("\n");
        assert!(!text.to_lowercase().contains("jira"), "{text}");
        for stage in ["SPEC", "DEV", "PR"] {
            assert!(text.contains(stage), "{stage} missing at {width}x{height}");
        }
        assert!(text.contains("Log dev quest"));
    }
    press(app, KeyCode::Char('k'))?;
    assert_eq!(app.milestone_selected, Milestone::Spec);
    press(app, KeyCode::Char('j'))?;
    assert_eq!(app.milestone_selected, Milestone::Dev);
    press(app, KeyCode::Char('j'))?;
    press(app, KeyCode::Char('j'))?;
    assert_eq!(app.milestone_selected, Milestone::Pr);

    // Turning it back on restores the stage and makes the ticket the next step again.
    press(app, KeyCode::Esc)?;
    open_settings_row(app, 1)?;
    press(app, KeyCode::Enter)?;
    assert!(app.store.settings()?.jira);
    press(app, KeyCode::Esc)?;
    press(app, KeyCode::Enter)?;
    assert_eq!(app.milestone_selected, Milestone::Jira);
    assert!(lines(app, 100, 35)?.join("\n").contains("Bind Jira ticket"));
    Ok(())
}

#[test]
fn archive_list_restores_with_progress_and_deletes_only_after_confirmation() -> Result<()> {
    let mut fixture = Fixture::new(2)?;
    let app = &mut fixture.app;
    let record = app.current().unwrap().clone();
    let archive_selected = |app: &mut App| -> Result<()> {
        press(app, KeyCode::Char('a'))?;
        press(app, KeyCode::Enter)?;
        assert!(app.records.is_empty());
        open_settings_row(app, 2)?;
        assert!(lines(app, 100, 35)?[5].starts_with("  Archive        1 spec"));
        press(app, KeyCode::Enter)?;
        assert_eq!(app.screen, Screen::Archive);
        Ok(())
    };
    press(app, KeyCode::Esc)?;
    archive_selected(app)?;
    for (width, height) in [(40, 18), (100, 35)] {
        let text = lines(app, width, height)?;
        assert!(
            text[1].starts_with("  Inbox / Settings / Archive"),
            "{text:#?}"
        );
        assert!(text[3].starts_with("  Payment retries"), "{text:#?}");
        let footer = &text[usize::from(height) - 2];
        assert!(
            footer.contains("r restore · d delete · Esc back"),
            "{footer}"
        );
    }

    press(app, KeyCode::Char('d'))?;
    let confirm = lines(app, 100, 35)?.join("\n");
    assert!(confirm.contains("Confirm delete"));
    assert!(confirm.contains("The file moves to the macOS Trash."));
    assert!(confirm.contains("Enter delete this spec · Esc cancel"));
    press(app, KeyCode::Esc)?;
    assert_eq!(app.store.trash_list()?.len(), 1);
    assert!(record.spec_path.is_file());

    press(app, KeyCode::Char('r'))?;
    assert_eq!(app.message, "Restored Payment retries");
    assert_eq!(app.records.len(), 1);
    assert_eq!(app.records[0].id, record.id);
    assert_eq!(app.records[0].jira.key.as_deref(), Some("PAY-123"));
    assert!(lines(app, 100, 35)?[3].starts_with("  No archived specs."));
    press(app, KeyCode::Esc)?;
    assert_eq!(app.screen, Screen::Settings);
    assert_eq!(app.settings.selected, super::super::settings::Row::Archive);
    press(app, KeyCode::Esc)?;
    assert_eq!(app.screen, Screen::List);

    archive_selected(app)?;
    press(app, KeyCode::Char('d'))?;
    press(app, KeyCode::Enter)?;
    assert_eq!(
        app.message,
        "Deleted Payment retries; file moved to the macOS Trash"
    );
    assert!(!record.spec_path.exists());
    assert!(app.store.path().join("test-macos-trash").is_dir());
    assert!(app.store.trash_list()?.is_empty());
    assert!(app.store.get(&record.id).is_err());
    assert_eq!(app.store.scan()?.imported, 0);
    Ok(())
}

#[test]
fn a_prompt_is_dropped_only_when_its_own_item_leaves_the_inbox() -> Result<()> {
    let mut fixture = Fixture::new(1)?;
    let app = &mut fixture.app;
    press(app, KeyCode::Esc)?;
    let first = app.current().unwrap().id.clone();
    let folder = app.settings.config.sources[0].path.clone();
    fs::write(folder.join("second.md"), "# Second spec\n")?;
    app.store.scan()?;
    app.refresh()?;
    let second = app
        .records
        .iter()
        .find(|record| record.id != first)
        .unwrap()
        .id
        .clone();

    // Typing a new spec's details must survive some other item being archived elsewhere.
    app.begin(Prompt::LaunchWorkspace {
        profile: Profile::Opus,
    });
    app.input = "my-workspace".into();
    app.store.archive(&first)?;
    app.refresh()?;
    assert!(matches!(app.prompt, Some(Prompt::LaunchWorkspace { .. })));
    assert_eq!(app.input, "my-workspace");

    // A prompt about an item goes away with that item.
    app.begin(Prompt::Jira { id: second.clone() });
    app.input = "PAY-".into();
    app.store.archive(&second)?;
    app.refresh()?;
    assert!(app.prompt.is_none());
    assert!(app.input.is_empty());
    Ok(())
}
