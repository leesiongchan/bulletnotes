use bulletnotes::{
    store::settings::AppSettings,
    theme::{
        AppearanceMode, SystemAppearance, ThemeId,
        format::{decode_theme, encode_theme},
        palette::builtin_themes,
        resolve_theme, select_theme,
    },
};

#[test]
fn all_bundled_palettes_resolve_with_cursor_and_scrollbar_colors() {
    let expected = [
        (
            "antinote-vancouver",
            "Vancouver",
            false,
            "#FFFFFF",
            "#0B486B",
            "#FFEBCF",
        ),
        (
            "antinote-tokyo-drift",
            "Tokyo Drift",
            true,
            "#2A2626",
            "#58C2FF",
            "#746550",
        ),
        (
            "antinote-gundam-wing",
            "Gundam Wing",
            false,
            "#F5F5F5",
            "#0064B4",
            "#FFDF5A",
        ),
        (
            "antinote-vendetta",
            "Vendetta",
            true,
            "#141414",
            "#BA0B0B",
            "#781414",
        ),
        (
            "antinote-totoro",
            "Totoro",
            false,
            "#FAF5EB",
            "#557D69",
            "#FFE6A0",
        ),
        (
            "antinote-mononoke",
            "Mononoke",
            true,
            "#03180C",
            "#289A28",
            "#F06446",
        ),
        ("antinote-a24", "A24", true, "#0F1612", "#4D7967", "#493B59"),
        (
            "antinote-agrabah",
            "Agrabah",
            true,
            "#0A0A28",
            "#F5DC00",
            "#993055",
        ),
        (
            "antinote-knight",
            "Knight",
            true,
            "#0A0A0F",
            "#3C5082",
            "#1C4B63",
        ),
        (
            "antinote-shadow-moses",
            "Shadow Moses",
            true,
            "#292D2D",
            "#9BC6D3",
            "#7BC290",
        ),
        (
            "antinote-piccolo",
            "Piccolo",
            false,
            "#F5FFF5",
            "#A032B4",
            "#B3F92C",
        ),
        (
            "antinote-tartan",
            "Tartan",
            false,
            "#F3F1EB",
            "#FF7828",
            "#F6BF9F",
        ),
        (
            "antinote-muad-dib",
            "Muad'Dib",
            false,
            "#E1D2B4",
            "#99501A",
            "#EFD978",
        ),
        (
            "antinote-sanrio",
            "Sanrio",
            false,
            "#FFFFFF",
            "#84B0DD",
            "#CCE1D1",
        ),
        (
            "antinote-community-flexoki-dark",
            "Flexoki Dark",
            true,
            "#100F0F",
            "#3AA99F",
            "#282726",
        ),
        (
            "antinote-community-flexoki-light",
            "Flexoki Light",
            false,
            "#FFFCF0",
            "#24837B",
            "#E6E4D9",
        ),
        (
            "antinote-community-north-star",
            "North Star",
            false,
            "#FFFFFF",
            "#4F46E5",
            "#F2F2F2",
        ),
        (
            "antinote-community-ayu-mirage",
            "Ayu Mirage",
            true,
            "#202734",
            "#FFB454",
            "#1F2430",
        ),
        (
            "antinote-community-catppuccin-frappe",
            "Catppuccin Frappé",
            true,
            "#303446",
            "#8CAAEE",
            "#414559",
        ),
        (
            "antinote-community-catppuccin-latte",
            "Catppuccin Latte",
            false,
            "#EFF1F5",
            "#1E66F5",
            "#CCD0DA",
        ),
        (
            "antinote-community-catppuccin-macchiato",
            "Catppuccin Macchiato",
            true,
            "#24273A",
            "#8AADF4",
            "#363A4F",
        ),
        (
            "antinote-community-catppuccin-mocha",
            "Catppuccin Mocha",
            true,
            "#1E1E2E",
            "#89B4FA",
            "#313244",
        ),
        (
            "antinote-community-catpuccinmocha",
            "Catppuccin Mocha",
            true,
            "#1E1E2E",
            "#CBA6F7",
            "#45475A",
        ),
        (
            "antinote-community-dotmatrix",
            "Dot Matrix",
            true,
            "#9BBC0F",
            "#306230",
            "#BCC787",
        ),
        (
            "antinote-community-dracula",
            "Dracula",
            true,
            "#282A36",
            "#BD93F9",
            "#44475A",
        ),
        (
            "antinote-community-gruvbox",
            "Gruvbox",
            true,
            "#282828",
            "#D65D0E",
            "#746550",
        ),
        (
            "antinote-community-horizon",
            "Horizon (Warm)",
            true,
            "#1C1E26",
            "#E95678",
            "#2E303E",
        ),
        (
            "antinote-community-icebergvim",
            "Iceberg",
            true,
            "#161821",
            "#E27878",
            "#2E313F",
        ),
        (
            "antinote-community-material-deep-ocean",
            "Material Deep Ocean",
            true,
            "#0D101A",
            "#88DDFF",
            "#1F2233",
        ),
        (
            "antinote-community-monokai",
            "Monokai",
            true,
            "#272822",
            "#F92672",
            "#49483E",
        ),
        (
            "antinote-community-neo",
            "Neo",
            true,
            "#19002E",
            "#556B2F",
            "#746550",
        ),
        (
            "antinote-community-nord",
            "Nord",
            true,
            "#2E3440",
            "#8FBCBB",
            "#434C5E",
        ),
        (
            "antinote-community-nordaurora",
            "Nord Aurora",
            true,
            "#2E3440",
            "#BF616A",
            "#434C5E",
        ),
        (
            "antinote-community-oceanicnext",
            "Oceanic Next",
            true,
            "#1B2B34",
            "#EC5F67",
            "#4F5B66",
        ),
        (
            "antinote-community-onedarkpro",
            "One Dark Pro",
            true,
            "#282C34",
            "#C678DD",
            "#3E4451",
        ),
        (
            "antinote-community-solarized",
            "Solarized Dark",
            true,
            "#002B36",
            "#B58900",
            "#073642",
        ),
        (
            "antinote-community-wumpus",
            "Wumpus",
            true,
            "#323339",
            "#5865F2",
            "#585864",
        ),
    ];
    let themes = builtin_themes().unwrap();
    assert_eq!(themes.len(), 41);
    let mut ids = std::collections::HashSet::new();
    for entry in &themes {
        assert!(ids.insert(&entry.id));
        assert!(entry.id.is_builtin());
    }
    for (id, name, dark, paper, cursor, scrollbar) in expected {
        let entry = themes.iter().find(|entry| entry.id.0 == id).unwrap();
        let settings = AppSettings::default();
        let appearance =
            select_theme(&settings.appearance, SystemAppearance::Light, entry, true).unwrap();
        let settings = AppSettings {
            appearance,
            ..settings
        };
        settings.validate().unwrap();
        let restored: AppSettings =
            serde_json::from_slice(&serde_json::to_vec(&settings).unwrap()).unwrap();
        let resolved = resolve_theme(
            &restored.appearance,
            SystemAppearance::Light,
            &restored.themes().unwrap(),
        )
        .unwrap();
        assert_eq!(resolved.palette.name, name);
        assert_eq!(resolved.palette.dark, dark);
        assert_eq!(resolved.palette.paper.hex(), paper, "{id}");
        assert_eq!(resolved.palette.cursor.unwrap().hex(), cursor, "{id}");
        assert_eq!(resolved.palette.scrollbar.unwrap().hex(), scrollbar, "{id}");
        assert_eq!(
            decode_theme(&encode_theme(&resolved.palette).unwrap()).unwrap(),
            resolved.palette
        );
        assert_eq!(restored.appearance.dark_theme, ThemeId("graphite".into()));
        let mut dark_settings = restored.appearance.clone();
        dark_settings.mode = AppearanceMode::Dark;
        dark_settings.dark_theme = ThemeId(id.into());
        assert_eq!(
            resolve_theme(&dark_settings, SystemAppearance::Light, &themes)
                .unwrap()
                .palette,
            entry.theme
        );
    }
}
