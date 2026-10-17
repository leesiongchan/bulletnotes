use bulletnotes::app::find::NoteFind;
#[test]
fn find_uses_utf8_source_offsets_and_preserves_crlf() {
    let body = "café\r\n👩🏽‍💻 日本語\r\ncafé";
    let mut find = NoteFind::default();
    find.set_query(body, "café".into());
    assert_eq!(find.total(), 2);
    assert_eq!(find.step(body, 1), Some(0..5));
    let start = body.rfind("café").unwrap();
    assert_eq!(find.step(body, 1), Some(start..body.len()));
    assert_eq!(find.current(), Some(2));
    find.set_query(body, "👩🏽‍💻 日本語\r\n".into());
    let range = find.step(body, 1).unwrap();
    assert_eq!(&body[range], "👩🏽‍💻 日本語\r\n");
}
#[test]
fn find_wraps_in_both_directions_and_matches_literal_case() {
    let body = "a.* B a.* A.*";
    let mut find = NoteFind::default();
    find.set_query(body, "a.*".into());
    assert_eq!(find.total(), 2);
    assert_eq!(find.step(body, -1), Some(6..9));
    assert_eq!(find.step(body, 1), Some(0..3));
    assert_eq!(find.step(body, -1), Some(6..9));
}
#[test]
fn empty_queries_and_missing_matches_do_not_select_or_modify_text() {
    let body = "é 👩🏽‍💻";
    let mut find = NoteFind::default();
    for query in ["", "absent"] {
        find.set_query(body, query.into());
        assert_eq!(find.total(), 0);
        assert_eq!(find.step(body, 1), None);
        assert_eq!(find.current(), None);
    }
    assert_eq!(body, "é 👩🏽‍💻");
}
#[test]
fn edits_invalidate_match_selection_without_moving_the_editor() {
    let mut find = NoteFind::default();
    find.set_query("a a", "a".into());
    find.step("a a", 1);
    find.refresh("a a a");
    assert_eq!(find.total(), 3);
    assert_eq!(find.current(), None);
}
