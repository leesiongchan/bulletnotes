use bulletnotes::sync::text_merge::{MergeDisposition, SourceEdit, TextMergeResult, merge_text};

fn check_edits(local: &str, result: &TextMergeResult) {
    let mut rebuilt = String::new();
    let mut previous = 0;
    for edit in &result.edits {
        assert!(previous <= edit.range.start);
        assert!(edit.range.start <= edit.range.end);
        assert!(local.is_char_boundary(edit.range.start));
        assert!(local.is_char_boundary(edit.range.end));
        rebuilt.push_str(&local[previous..edit.range.start]);
        rebuilt.push_str(&edit.replacement);
        previous = edit.range.end;
    }
    rebuilt.push_str(&local[previous..]);
    assert_eq!(rebuilt, result.body);
}

fn check(base: &str, local: &str, remote: &str, body: &str, disposition: MergeDisposition) {
    let result = merge_text(base, local, remote);
    assert_eq!(result.body, body);
    assert_eq!(result.disposition, disposition);
    check_edits(local, &result);
}

#[test]
fn compatible_words_survive_on_one_line() {
    let result = merge_text("red fox jumps\n", "blue fox jumps\n", "red fox runs\n");
    assert_eq!(result.body, "blue fox runs\n");
    assert_eq!(result.disposition, MergeDisposition::Clean);
    assert_eq!(
        result.edits,
        vec![SourceEdit {
            range: 9..14,
            replacement: "runs".into()
        }]
    );
    check_edits("blue fox jumps\n", &result);
}

#[test]
fn overlap_keeps_remote_and_independent_local_words() {
    check(
        "red fox jumps\n",
        "blue fox leaps\n",
        "green fox jumps\n",
        "green fox leaps\n",
        MergeDisposition::PreservedOverlap,
    );
    check(
        "",
        "local",
        "server",
        "server",
        MergeDisposition::PreservedOverlap,
    );
    check(
        "hello red fox",
        "hello fox",
        "hello green fox",
        "hello green fox",
        MergeDisposition::PreservedOverlap,
    );
    check(
        "hello red fox",
        "hello green fox",
        "hello fox",
        "hello fox",
        MergeDisposition::PreservedOverlap,
    );
}

#[test]
fn literal_markers_are_not_control_data() {
    check(
        "<<<<<<< ours\nred fox\n=======\n>>>>>>> theirs",
        "<<<<<<< ours\nblue fox\n=======\n>>>>>>> theirs",
        "<<<<<<< ours\nred cat\n=======\n>>>>>>> theirs",
        "<<<<<<< ours\nblue cat\n=======\n>>>>>>> theirs",
        MergeDisposition::Clean,
    );
    check(
        "0\n1\nred fox",
        "0\n1\nblue fox",
        "0\n1\nred cat",
        "0\n1\nblue cat",
        MergeDisposition::Clean,
    );
}

#[test]
fn source_bytes_survive_crlf_unicode_and_missing_newline() {
    check(
        "---\r\ntitle: café\r\n---\r\n```\r\n色は赤\r\n```\r\ncafe\u{301} 👩🏽‍💻 red fox",
        "---\r\ntitle: café\r\n---\r\n```\r\n色は青\r\n```\r\ncafe\u{301} 👩🏽‍💻 red fox",
        "---\r\ntitle: café\r\n---\r\n```\r\n色は赤\r\n```\r\ncafe\u{301} 👩🏽‍💻 red cat",
        "---\r\ntitle: café\r\n---\r\n```\r\n色は青\r\n```\r\ncafe\u{301} 👩🏽‍💻 red cat",
        MergeDisposition::Clean,
    );
    check(
        "cafe\u{301} 👩🏽‍💻 red fox",
        "cafe\u{301} 👩🏽‍💻 blue fox",
        "cafe\u{301} 👩🏽‍💻 red cat",
        "cafe\u{301} 👩🏽‍💻 blue cat",
        MergeDisposition::Clean,
    );
    check(
        "a\0 red fox",
        "a\0 blue fox",
        "a\0 red cat",
        "a\0 blue cat",
        MergeDisposition::Clean,
    );
}

#[test]
fn budget_fallback_preserves_the_remote_document() {
    let local = "local";
    let remote = "r".repeat(1_048_577);
    let result = merge_text("base", local, &remote);
    assert_eq!(result.body, remote);
    assert_eq!(result.disposition, MergeDisposition::BudgetFallback);
    assert_eq!(
        result.edits,
        vec![SourceEdit {
            range: 0..5,
            replacement: remote
        }]
    );
    check_edits(local, &result);
    let lines = "x\n".repeat(6_667);
    let words = "x ".repeat(3_400);
    for base in [lines, words] {
        let local = format!("local {base}");
        let remote = format!("remote {base}");
        check(
            &base,
            &local,
            &remote,
            &remote,
            MergeDisposition::BudgetFallback,
        );
    }
}

#[test]
fn exact_byte_limit_is_admitted() {
    let base = "a".repeat(1_048_576);
    let local = "b".repeat(1_048_576);
    let remote = "c".repeat(1_048_576);
    check(
        &base,
        &local,
        &remote,
        &remote,
        MergeDisposition::PreservedOverlap,
    );
}

#[test]
fn line_and_word_passes_admit_the_token_boundary_and_reject_the_next_token() {
    let base = "x\n".repeat(6_666);
    let local = format!("local\n{base}");
    let remote = format!("{base}remote\n");
    check(
        &base,
        &local,
        &remote,
        &format!("local\n{base}remote\n"),
        MergeDisposition::Clean,
    );
    let remote = format!("{base}remote\nextra\n");
    check(
        &base,
        &local,
        &remote,
        &remote,
        MergeDisposition::BudgetFallback,
    );

    let prefix = "x ".repeat(3_332);
    let base = format!("{prefix}red fox");
    let local = format!("{prefix}blue fox");
    let remote = format!("{prefix}redcat!");
    check(
        &base,
        &local,
        &remote,
        &remote,
        MergeDisposition::PreservedOverlap,
    );
    let remote = format!("{prefix}redcat!!");
    check(
        &base,
        &local,
        &remote,
        &remote,
        MergeDisposition::BudgetFallback,
    );
}

#[test]
fn clean_line_merge_does_not_require_word_refinement() {
    let long_line = "word ".repeat(10_001);
    let base = format!("first\n{long_line}\nlast\n");
    let local = format!("local\n{long_line}\nlast\n");
    let remote = format!("first\n{long_line}\nremote\n");
    let expected = format!("local\n{long_line}\nremote\n");
    check(&base, &local, &remote, &expected, MergeDisposition::Clean);
}

#[test]
fn unambiguous_fast_paths_do_not_discard_large_local_edits() {
    for large in ["🦊".repeat(262_145), "word\n".repeat(20_001)] {
        check("base", &large, "base", &large, MergeDisposition::Clean);
        check("base", &large, &large, &large, MergeDisposition::Clean);
        check("base", "base", &large, &large, MergeDisposition::Clean);
    }
}

#[test]
fn generated_identity_and_one_sided_cases_reconstruct_exact_sources() {
    let fragments = [
        "",
        "a",
        "\r\n",
        "cafe\u{301}",
        "🦊",
        "日本語",
        "<<<<<<<",
        "  \t",
        "---\nkey: value\n---",
        "```rust\nfn main() {}\n```",
    ];
    for a in fragments {
        for b in fragments {
            for c in fragments {
                let base = format!("{a}{b}{c}");
                let edited = format!("{a} edited {b}{c}");
                check(&base, &base, &base, &base, MergeDisposition::Clean);
                check(&base, &base, &edited, &edited, MergeDisposition::Clean);
                check(&base, &edited, &base, &edited, MergeDisposition::Clean);
                check(&base, &edited, &edited, &edited, MergeDisposition::Clean);
            }
        }
    }
}

#[test]
fn generated_concurrent_edits_preserve_compatible_and_overlapping_changes() {
    for middle in [
        "fox",
        "日本語",
        "cafe\u{301}",
        "👩🏽‍💻",
        "`code`",
        "\t",
        "<<<<<<<",
    ] {
        for end in ["", "\n", "\r\n"] {
            let base = format!("red {middle} jumps{end}");
            let local = format!("blue {middle} jumps{end}");
            let remote = format!("red {middle} runs{end}");
            check(
                &base,
                &local,
                &remote,
                &format!("blue {middle} runs{end}"),
                MergeDisposition::Clean,
            );
            let local = format!("blue {middle} leaps{end}");
            let remote = format!("green {middle} jumps{end}");
            check(
                &base,
                &local,
                &remote,
                &format!("green {middle} leaps{end}"),
                MergeDisposition::PreservedOverlap,
            );
        }
    }
}
