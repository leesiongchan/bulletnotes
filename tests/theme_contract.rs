use bulletnotes::theme::{
    contrast::check_contrast,
    format::{decode_theme, encode_theme},
    palette::{ThemeEntry, builtin_themes},
    *,
};
use serde_json::{Value, json};
const VALID: &[u8] = include_bytes!("fixtures/themes/valid.json");
fn modified(field: &str, value: Value) -> Vec<u8> {
    let mut theme: Value = serde_json::from_slice(VALID).unwrap();
    theme[field] = value;
    serde_json::to_vec(&theme).unwrap()
}

#[test]
fn size_boundary_requires_valid_json() {
    let mut valid = VALID.to_vec();
    valid.resize(32768, b' ');
    assert!(decode_theme(&valid).is_ok());
    valid.push(b' ');
    assert!(matches!(decode_theme(&valid), Err(ThemeError::TooLarge)));
    assert!(decode_theme(&vec![b' '; 32768]).is_err());
}
#[test]
fn names_count_trimmed_unicode_scalars() {
    let input = modified("name", json!(format!("  {}  ", "é".repeat(32))));
    assert_eq!(decode_theme(&input).unwrap().name, "é".repeat(32));
    for name in [" ".to_owned(), "é".repeat(33), "👩🏽‍💻".repeat(9)] {
        assert!(decode_theme(&modified("name", json!(name))).is_err());
    }
}
#[test]
fn duplicate_keys_are_rejected_recursively_even_when_unknown() {
    for extra in [
        r#", "name":"another""#,
        r#", "unknown":1, "unknown":2"#,
        r#", "extra":{"a":1,"a":2}"#,
        r#", "extra":[{"a":1,"a":2}]"#,
        r#", "na\u006de":"duplicate escaped key""#,
    ] {
        let text = format!(
            "{}{}{}",
            std::str::from_utf8(VALID).unwrap().trim_end_matches('}'),
            extra,
            "}"
        );
        assert!(decode_theme(text.as_bytes()).is_err(), "accepted duplicate");
    }
}
#[test]
fn unknown_fields_are_ignored_and_not_exported() {
    let decoded = decode_theme(&modified("future", json!({"nested":[1,2,{"safe":true}]}))).unwrap();
    assert_eq!(decoded, decode_theme(VALID).unwrap());
    let exported: Value = serde_json::from_slice(&encode_theme(&decoded).unwrap()).unwrap();
    assert!(exported.get("future").is_none());
    assert_eq!(exported.as_object().unwrap().len(), 10);
}
#[test]
fn invalid_version_types_missing_fields_and_colors_are_rejected() {
    for (field, value) in [
        ("schemaVersion", json!(2)),
        ("schemaVersion", json!(1.0)),
        ("dark", json!("false")),
        ("paper", json!("#fff")),
        ("paper", json!("#ffffff00")),
        ("paper", json!("#GG0000")),
        ("name", Value::Null),
    ] {
        assert!(decode_theme(&modified(field, value)).is_err());
    }
    let mut missing: Value = serde_json::from_slice(VALID).unwrap();
    missing.as_object_mut().unwrap().remove("ink");
    assert!(decode_theme(&serde_json::to_vec(&missing).unwrap()).is_err());
    assert!(decode_theme(&[0xff, 0xfe]).is_err());
    assert!(decode_theme(b"{} {}").is_err());
}
#[test]
fn export_roundtrips_version_one() -> Result<(), ThemeError> {
    let theme = decode_theme(VALID)?;
    assert_eq!(decode_theme(&encode_theme(&theme)?)?, theme);
    Ok(())
}
#[test]
fn optional_colors_survive_export_and_settings_roundtrips() {
    let mut input: Value = serde_json::from_slice(VALID).unwrap();
    input["cursor"] = json!("#19A2E6");
    input["scrollbar"] = json!("#E69A19");
    input["grid"] = json!("#DEDAD0");
    input["swatches"] = json!([
        "#FF7828", "#C85A1E", "#FFA050", "#4BA64B", "#6E8CA0", "#42230F", "#422323", "#F6BF9F"
    ]);
    input["markdown"] = json!({
        "heading1": "#FF8D33", "heading2": "#D46F27", "heading3": "#FFB062",
        "link": "#9F5910", "syntax": "#949494"
    });
    let theme = decode_theme(&serde_json::to_vec(&input).unwrap()).unwrap();
    let exported: Value = serde_json::from_slice(&encode_theme(&theme).unwrap()).unwrap();
    assert_eq!(exported["cursor"], "#19A2E6");
    assert_eq!(exported["scrollbar"], "#E69A19");
    assert_eq!(exported["grid"], input["grid"]);
    assert_eq!(exported["swatches"], input["swatches"]);
    assert_eq!(exported["markdown"], input["markdown"]);
    let settings = bulletnotes::store::settings::AppSettings {
        imported_themes: vec![ThemeEntry::import(theme)],
        ..Default::default()
    };
    let restored: bulletnotes::store::settings::AppSettings =
        serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
    assert_eq!(restored, settings);
}
#[test]
fn cursor_and_scrollbar_reject_invalid_colors() {
    for value in [
        json!(vec!["#FFFFFF"; 7]),
        json!(vec!["invalid"; 8]),
        json!(vec![Value::Null; 8]),
    ] {
        assert!(decode_theme(&modified("swatches", value)).is_err());
    }
    for field in ["cursor", "scrollbar", "grid"] {
        for value in [json!("#fff"), json!("#FFFFFF00"), json!(17)] {
            assert!(decode_theme(&modified(field, value)).is_err(), "{field}");
        }
    }
}
#[test]
fn system_mode_preserves_both_assignments() {
    let themes = builtin_themes().unwrap();
    let settings = AppearanceSettings {
        mode: AppearanceMode::System,
        paper: PaperStyle::Grid,
        font_size: 19.0,
        ..Default::default()
    };
    let saved = settings.clone();
    for system in [
        SystemAppearance::Dark,
        SystemAppearance::Light,
        SystemAppearance::Dark,
    ] {
        let resolved = resolve_theme(&settings, system, &themes).unwrap();
        assert_eq!(
            resolved.id,
            if system == SystemAppearance::Light {
                settings.light_theme.clone()
            } else {
                settings.dark_theme.clone()
            }
        );
        assert_eq!(resolved.paper, PaperStyle::Grid);
        assert_eq!(resolved.font_size, 19.0);
    }
    assert_eq!(settings, saved);
}
#[test]
fn imported_names_cannot_replace_builtin_identity() {
    let theme = decode_theme(VALID).unwrap();
    let a = ThemeEntry::import(theme.clone());
    let b = ThemeEntry::import(theme);
    assert_ne!(a.id, b.id);
    assert!(!a.id.is_builtin());
    assert!(
        builtin_themes()
            .unwrap()
            .iter()
            .all(|entry| entry.id != a.id)
    );
}
#[test]
fn low_contrast_requires_override_and_keeps_previous_settings() {
    let low = decode_theme(&modified("ink", json!("#FCFAF4"))).unwrap();
    assert!(!check_contrast(&low).passes());
    let entry = ThemeEntry::import(low);
    let previous = AppearanceSettings::default();
    let saved = previous.clone();
    assert!(matches!(
        select_theme(&previous, SystemAppearance::Light, &entry, false),
        Err(ThemeError::LowContrast)
    ));
    assert_eq!(previous, saved);
    let allowed = select_theme(&previous, SystemAppearance::Light, &entry, true).unwrap();
    assert_eq!(allowed.light_theme, entry.id);
    assert_eq!(allowed.dark_theme, previous.dark_theme);
}
#[test]
fn invalid_settings_never_resolve_silently() {
    let themes = builtin_themes().unwrap();
    for font in [f32::NAN, f32::INFINITY, 10.0, 25.0] {
        let settings = AppearanceSettings {
            font_size: font,
            ..Default::default()
        };
        assert!(resolve_theme(&settings, SystemAppearance::Light, &themes).is_err());
    }
    let settings = AppearanceSettings {
        light_theme: ThemeId("missing".into()),
        ..Default::default()
    };
    assert!(resolve_theme(&settings, SystemAppearance::Light, &themes).is_err());
}
