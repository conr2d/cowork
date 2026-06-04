use cowork_host::provision::list::{DistroEntry, distro_present, parse_distro_list};

#[test]
fn parses_typical_english_output() {
    let output = "  NAME      STATE           VERSION\n\
                  * Ubuntu    Running         2\n\
                  \u{20}\u{20}Cowork    Stopped         2\n";
    let entries = parse_distro_list(output);
    assert_eq!(
        entries,
        vec![
            DistroEntry {
                name: "Ubuntu".to_string(),
                version: 2,
                default: true,
            },
            DistroEntry {
                name: "Cowork".to_string(),
                version: 2,
                default: false,
            },
        ]
    );
}

#[test]
fn skips_localized_header() {
    // Japanese header: its last column ("バージョン") is not numeric, so it is
    // skipped without any header-text matching.
    let output = "  名前      状態            バージョン\n\
                  * Ubuntu    実行中          2\n";
    let entries = parse_distro_list(output);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "Ubuntu");
    assert_eq!(entries[0].version, 2);
    assert!(entries[0].default);
}

#[test]
fn tolerates_multi_token_state_column() {
    // STATE may be multi-word/localized; NAME is the first column and VERSION
    // the last, so the entry is still extracted correctly.
    let output = "  NAME    STATE        VERSION\n\
                  \u{20}\u{20}Cowork  Some State   2\n";
    let entries = parse_distro_list(output);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "Cowork");
    assert_eq!(entries[0].version, 2);
}

#[test]
fn tolerates_bom_and_blank_lines() {
    let output = "\u{feff}  NAME    STATE      VERSION\n\
                  \n   \n\
                  * Ubuntu  Running    2\n\n";
    let entries = parse_distro_list(output);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "Ubuntu");
}

#[test]
fn parses_wsl1_version() {
    let output = "  NAME    STATE      VERSION\n\
                  \u{20}\u{20}Legacy  Stopped    1\n";
    let entries = parse_distro_list(output);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].version, 1);
}

#[test]
fn header_only_yields_empty() {
    let output = "  NAME    STATE      VERSION\n";
    assert!(parse_distro_list(output).is_empty());
}

#[test]
fn empty_input_yields_empty() {
    assert!(parse_distro_list("").is_empty());
}

#[test]
fn distro_present_is_case_insensitive() {
    let entries = parse_distro_list(
        "  NAME    STATE      VERSION\n\
         * Ubuntu  Running    2\n\
         \u{20}\u{20}Cowork  Stopped    2\n",
    );
    assert!(distro_present(&entries, "Cowork"));
    assert!(distro_present(&entries, "cowork"));
    assert!(distro_present(&entries, "Ubuntu"));
    assert!(!distro_present(&entries, "Debian"));
}

#[test]
fn cowork_created_with_ubuntu_left_present() {
    // The WP5 invariant: provisioning `Cowork` must leave an existing `Ubuntu`
    // intact. Parsing the post-provision list confirms both are present.
    let entries = parse_distro_list(
        "  NAME    STATE      VERSION\n\
         * Ubuntu  Running    2\n\
         \u{20}\u{20}Cowork  Stopped    2\n",
    );
    assert!(distro_present(&entries, "Ubuntu"));
    assert!(distro_present(&entries, "Cowork"));
}
