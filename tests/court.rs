#[test]
fn the_court_page_embeds_the_snapshot_whole() {
    let data = serde_json::json!({ "marker": "the-whole-snapshot-7193" });
    let page = arbiter::court::page(&data);
    assert!(page.starts_with("<!doctype html"));
    assert!(page.contains("the-whole-snapshot-7193"));
    assert!(
        !page.contains("/*ARBITER_DATA*/"),
        "the placeholder is replaced"
    );
}
