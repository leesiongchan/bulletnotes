use bulletnotes::domain::{
    connection::ConnectionConfig,
    page::{PageKey, PagePath, new_note_path},
};
use uuid::Uuid;

#[test]
fn paths_preserve_unicode_and_literal_percent() {
    for input in [
        "Projects/50% café.md",
        "%2e%2e/日本語.md",
        "a # b?.md",
        "  note .md",
    ] {
        assert_eq!(PagePath::parse(input).unwrap().as_str(), input);
    }
}

#[test]
fn paths_reject_traversal() {
    for input in [
        "",
        "/absolute.md",
        "../outside.md",
        "a/./b.md",
        "a/../b.md",
        "a\\b.md",
        "a\0b.md",
        "a//b.md",
        "a/",
        "C:/outside.md",
    ] {
        assert!(PagePath::parse(input).is_err(), "accepted {input:?}");
    }
}

#[test]
fn connection_identity_keeps_prefix() {
    let a = ConnectionConfig::parse("https://NOTES.example:443/a").unwrap();
    let same = ConnectionConfig::parse("https://notes.example/a/").unwrap();
    let b = ConnectionConfig::parse("https://notes.example/b/").unwrap();
    assert_eq!(a.id(), same.id());
    assert_ne!(a.id(), b.id());
    assert_eq!(a.id().as_str(), "https://notes.example/a/");
}

#[test]
fn connection_rejects_secrets_and_ambiguous_inputs() {
    for input in [
        "http://notes.example",
        "https://u:p@notes.example",
        "https://u@notes.example",
        "https://notes.example?token=secret",
        "https://notes.example#fragment",
        "https://notes.example/\\other",
        " https://notes.example",
        "https://notes.example/\n",
    ] {
        assert!(ConnectionConfig::parse(input).is_err());
    }
}

#[test]
fn connection_errors_do_not_echo_secret_input() {
    let error = ConnectionConfig::parse("https://user:VERY_SECRET@notes.example").unwrap_err();
    assert!(!format!("{error} {error:?}").contains("VERY_SECRET"));
}

#[test]
fn new_note_path_is_stable() {
    let now: jiff::Zoned = "2026-10-17T09:08:07+08:00[Asia/Kuala_Lumpur]"
        .parse()
        .unwrap();
    let id = Uuid::parse_str("b6e26fe3-9e69-4a8f-942c-8ddae58c3cc0").unwrap();
    let a = new_note_path(&now, id);
    assert_eq!(
        a.as_str(),
        "Quick Notes/2026-10-17/090807-b6e26fe3-9e69-4a8f-942c-8ddae58c3cc0.md"
    );
    assert_eq!(a, new_note_path(&now, id));
}

#[test]
fn changing_display_title_does_not_change_page_identity() {
    let key = PageKey {
        connection: ConnectionConfig::parse("https://notes.example")
            .unwrap()
            .id()
            .clone(),
        path: PagePath::parse("a.md").unwrap(),
    };
    let original = key.clone();
    assert_eq!(key.title_from("# First title\nbody"), "First title");
    assert_eq!(key.title_from("# Changed title\nbody"), "Changed title");
    assert_eq!(key, original);
}
