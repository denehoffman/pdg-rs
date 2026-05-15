use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

#[test]
fn help_mentions_only_canonical_subcommands() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(contains("show"))
        .stdout(contains("search"))
        .stdout(contains("tui"))
        .stdout(contains("  particle").not())
        .stdout(contains("  pdgid").not())
        .stdout(contains("  text").not());
}

#[test]
fn search_particles_requires_query_or_filter() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["search", "particles"])
        .assert()
        .success()
        .stdout(contains("Usage:"));
}

#[test]
fn search_particles_prints_summary() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["search", "particles", "K", "--limit", "3"])
        .assert()
        .success()
        .stdout(contains("Mass"))
        .stdout(contains("Quantum"))
        .stdout(contains("Charge").not())
        .stdout(contains("Decays").not());
}

#[test]
fn search_particles_accepts_filters() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["search", "particles", "--class", "lepton", "--limit", "3"])
        .assert()
        .success()
        .stdout(contains("Lepton"));
}

#[test]
fn search_particles_accepts_mcid_without_query() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["search", "particles", "--mcid", "22"])
        .assert()
        .success()
        .stdout(contains("S000"))
        .stdout(contains("gamma"))
        .stdout(contains("Usage:").not());
}

#[test]
fn search_particles_accepts_decay_filters() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args([
        "search",
        "particles",
        "--decays-to",
        "K(S)0,K(S)0",
        "--charge",
        "0",
        "--limit",
        "3",
    ])
    .assert()
    .success()
    .stdout(contains("Meson"));
}

#[test]
fn search_particles_uses_section_derived_properties() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args([
        "search",
        "particles",
        "--decays-to",
        "K(S)0,K(S)0",
        "--parity",
        "+",
        "--c-parity",
        "+",
        "--limit",
        "20",
    ])
    .assert()
    .success()
    .stdout(contains("a_0(980)0"))
    .stdout(contains("980+-20 MeV"))
    .stdout(contains("50 to 100 MeV"));
}

#[test]
fn search_particles_filters_section_derived_properties() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args([
        "search",
        "particles",
        "a_0(",
        "--mass",
        "1500..2000",
        "--limit",
        "20",
    ])
    .assert()
    .success()
    .stdout(contains("a_0(1710)0"))
    .stdout(contains("a_0(980)0").not());
}

#[test]
fn search_text_prints_results() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["search", "text", "form factors", "--limit", "3"])
        .assert()
        .success()
        .stdout(contains("PDG ID"))
        .stdout(contains("Title"))
        .stdout(contains("Match"))
        .stdout(contains("Score").not())
        .stdout(contains("Source").not())
        .stdout(contains("pi+- FORM FACTORS"));
}

#[test]
fn search_text_includes_footnotes() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["search", "text", "normalisation decay", "--limit", "2"])
        .assert()
        .success()
        .stdout(contains("S042P86"))
        .stdout(contains("normalisation"))
        .stdout(contains("B0 --> K*(892)0 tau+ mu-"));
}

#[test]
fn show_finds_generic_pdgid_rows_with_labels() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["show", "S008245"])
        .assert()
        .success()
        .stdout(contains("Section"))
        .stdout(contains("pi+- FORM FACTORS"));
}

#[test]
fn show_particle_promotes_section_properties_with_sources() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["show", "M036", "--summary"])
        .assert()
        .success()
        .stdout(contains("Particle"))
        .stdout(contains("Name"))
        .stdout(contains("a_0(980)0"))
        .stdout(contains("M036MX"))
        .stdout(contains("980+-20 MeV"))
        .stdout(contains("M036W1"))
        .stdout(contains("50 to 100 MeV"))
        .stdout(contains("Child PDG IDs").not());
}

#[test]
fn show_particle_does_not_expand_related_branching_measurements() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["show", "M036"])
        .assert()
        .success()
        .stdout(contains("Related PDG IDs"))
        .stdout(contains("Measurements for M036R2").not());
}

#[test]
fn show_measurements_use_reference_blocks() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    let stdout = String::from_utf8(
        cmd.args(["show", "M036R2"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert!(stdout.contains("Measurements for M036R2"));
    assert!(stdout.contains("ABLIKIM 2022AH"));
    assert!(stdout.contains("https://doi.org/"));
    assert!(stdout.contains("https://inspirehep.net/literature/"));
    assert!(stdout.contains("Title"));
    assert!(stdout.contains("Technique"));
    assert!(stdout.contains("Values"));
    assert!(stdout.contains("Footnotes"));
    assert!(stdout.contains("[1] Using D_s()+"));
    assert!(!stdout.contains("Index"));
    let first_reference = stdout.find("ABLIKIM 2022AH").unwrap();
    let first_footnote = stdout.find("[1] Using D_s()+").unwrap();
    let next_reference = stdout.find("ABELE 1998").unwrap();
    assert!(first_reference < first_footnote);
    assert!(first_footnote < next_reference);
}

#[test]
fn show_particle_headline_includes_other_section_properties() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["show", "S008", "--summary"])
        .assert()
        .success()
        .stdout(contains("Properties"))
        .stdout(contains("Form factor"))
        .stdout(contains("S008FV"))
        .stdout(contains("Section S008245"));
}

#[test]
fn show_summary_omits_full_sections() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["show", "M036", "--summary"])
        .assert()
        .success()
        .stdout(contains("Branching fractions").not())
        .stdout(contains("Branching ratios").not())
        .stdout(contains("Child PDG IDs").not())
        .stdout(contains("Related PDG IDs").not())
        .stdout(contains("Measurements for").not());
}

#[test]
fn show_text_is_attached_to_title() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["show", "S071DGF"])
        .assert()
        .success()
        .stdout(contains("Text").not())
        .stdout(contains("Type"))
        .stdout(contains("PDG ID  ┆ Type ┆ Text").not())
        .stdout(contains("h:").not())
        .stdout(contains("This section includes"));
}

#[test]
fn show_related_details_expands_related_measurements() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["show", "M036", "--related-details"])
        .assert()
        .success()
        .stdout(contains("Related details"))
        .stdout(contains("Measurements for M036R2"));
}

#[test]
fn show_json_is_valid() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    let output = cmd
        .args(["show", "S008", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["entry"]["pdgid"], "S008");
    assert_eq!(value["particle"]["mcid"], 211);
}

#[test]
fn show_json_includes_measurement_details() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    let output = cmd
        .args(["show", "M036R2", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    let measurement = &value["measurements"][0];
    let measurement_value = &measurement["values"][0];

    assert_eq!(measurement["technique"], "BES3");
    assert_eq!(measurement["place"], "U");
    assert_eq!(measurement["changebar"], false);
    assert_eq!(measurement_value["column_name"], "VALUE");
    assert_eq!(measurement_value["value_text"], "0.137 +-0.036 +-0.042");
    assert_eq!(measurement_value["used_in_average"], true);
    assert_eq!(measurement_value["used_in_fit"], true);
    assert!(measurement_value["stat_error_positive"].is_number());
    assert!(measurement_value["syst_error_positive"].is_number());
}

#[test]
fn search_particles_json_is_valid() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    let output = cmd
        .args([
            "search",
            "particles",
            "pi+",
            "--limit",
            "1",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value[0]["pdgid"], "S008");
}

#[test]
fn search_text_json_is_valid() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    let output = cmd
        .args([
            "search",
            "text",
            "form factors",
            "--limit",
            "3",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(value.as_array().unwrap().len() <= 3);
}

#[test]
fn search_text_json_includes_footnote_source() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    let output = cmd
        .args([
            "search",
            "text",
            "normalisation decay",
            "--limit",
            "1",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value[0]["source"], "footnote");
    assert_eq!(value[0]["pdgid"], "S042P86");
}

#[test]
fn tui_stub_is_reserved() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.arg("tui")
        .assert()
        .failure()
        .stderr(contains("TUI is not implemented yet"));
}
