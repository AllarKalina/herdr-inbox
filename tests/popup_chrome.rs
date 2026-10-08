use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::{Block, Borders, Widget},
};

#[test]
fn manifest_title_is_valid_and_leaves_the_popup_border_unlabeled() {
    let manifest: toml::Value = toml::from_str(include_str!("../herdr-plugin.toml")).unwrap();
    let title = manifest["panes"][0]["title"].as_str().unwrap();
    // Herdr 0.9.1 validates titles by trimming and rejecting empty strings.
    assert!(!title.trim().is_empty());
    assert_eq!(title, "\u{200b}");
    for width in [12, 40, 100] {
        let area = Rect::new(0, 0, width, 8);
        let mut plain = Buffer::empty(area);
        let mut titled = Buffer::empty(area);
        Block::default()
            .borders(Borders::ALL)
            .render(area, &mut plain);
        Block::default()
            .borders(Borders::ALL)
            .title(title)
            .render(area, &mut titled);
        assert_eq!(titled, plain, "title altered the border at width {width}");
    }
}
