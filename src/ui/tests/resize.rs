//! Every screen must survive any popup size, including ones too small to be useful.

use super::proximity::{Fixture, press};
use super::*;

/// Renders one screen at every size and reports the first size that panics.
fn sweep(app: &mut App, label: &str, failures: &mut Vec<String>) {
    // Every tiny size, then the breakpoints the layouts switch on.
    let widths = (0..=12).chain([16, 20, 25, 39, 40, 63, 64, 77, 78, 100, 200]);
    let heights = (0..=10).chain([12, 15, 17, 18, 19, 20, 25, 26, 35, 60]);
    let sizes = widths.flat_map(|width| heights.clone().map(move |height| (width, height)));
    for (width, height) in sizes {
        let drawn = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| draw::draw(frame, app)).unwrap();
        }));
        if drawn.is_err() {
            failures.push(format!("{label} at {width}x{height}"));
            return;
        }
    }
}

#[test]
fn no_screen_panics_at_any_size() -> Result<()> {
    let mut failures = Vec::new();
    let failures = &mut failures;
    for jira in [true, false] {
        let mut fixture = Fixture::new(2)?;
        let app = &mut fixture.app;
        let mut settings = app.store.settings()?;
        settings.jira = jira;
        app.store.save_settings(&settings)?;
        app.refresh()?;
        let id = app.current().unwrap().id.clone();

        for stage in Milestone::visible(jira) {
            app.select_milestone(*stage);
            sweep(app, &format!("detail {stage:?} jira={jira}"), failures);
        }
        app.begin(Prompt::Pr { id: id.clone() });
        app.input = "https://github.example/org/repo/pull/123456789".into();
        sweep(app, "detail prompt", failures);
        app.prompt = None;
        if !jira {
            // Only the detail view depends on the Jira setting.
            continue;
        }
        app.screen = Screen::Reader;
        sweep(app, "reader", failures);
        app.screen = Screen::Detail;
        app.choose_client(
            ChoicePurpose::Refine { id: id.clone() },
            vec![Profile::Opus, Profile::Codex],
        );
        sweep(app, "refine chooser", failures);
        app.choice_selected = None;

        app.screen = Screen::List;
        sweep(app, "list", failures);
        app.begin(Prompt::Archive { id: id.clone() });
        sweep(app, "list archive confirm", failures);
        app.prompt = None;
        app.choose_client(ChoicePurpose::NewSpec, vec![Profile::Opus, Profile::Codex]);
        sweep(app, "list chooser", failures);
        app.choice_selected = None;
        app.begin(Prompt::LaunchWorkspace {
            profile: Profile::Opus,
        });
        sweep(app, "list launch prompt", failures);
        app.prompt = None;

        app.screen = Screen::Settings;
        sweep(app, "settings", failures);
        app.screen = Screen::Archive;
        sweep(app, "archive empty", failures);
        app.store.archive(&id)?;
        app.refresh()?;
        sweep(app, "archive list", failures);
        press(app, KeyCode::Char('d'))?;
        sweep(app, "archive delete confirm", failures);
        press(app, KeyCode::Esc)?;
        app.scan_issues = vec!["Cannot read spec /tmp/x.md: Permission denied".into(); 12];
        app.screen = Screen::ScanResult;
        sweep(app, "scan results", failures);
    }
    assert!(failures.is_empty(), "screens panicked: {failures:#?}");
    Ok(())
}
