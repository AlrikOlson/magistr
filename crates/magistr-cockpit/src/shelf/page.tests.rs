use super::*;

#[test]
fn the_page_is_the_chapters_html_at_its_own_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    let chapter = Chapter {
        chunk: "tui2-5b".to_owned(),
        title: "The shelf, opened".to_owned(),
        prose: "One record, three routes.".to_owned(),
        ..Chapter::default()
    };
    let shelf = Shelf::new("tui2-5b", &["think:7".to_owned()]);
    let out = dir.path().join("deliverables").join("tui2-5b");
    let page = write_page(&shelf, &chapter, &out).expect("the page writes");
    assert_eq!(page, out.join("chapter.html"));
    let html = std::fs::read_to_string(&page).expect("the page");
    assert_eq!(html, chapter.html(&shelf));
    assert!(html.contains("<h1>The shelf, opened</h1>"));
    assert!(html.contains("<li>Trace: think 7</li>"), "{html}");
}
