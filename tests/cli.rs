use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::str::contains;

#[test]
fn help_mentions_core_subcommands() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(contains("show"))
        .stdout(contains("search"))
        .stdout(contains("particle"));
}

#[test]
fn particle_search_prints_compact_table() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["particle", "pi+", "--limit", "1"])
        .assert()
        .success()
        .stdout(contains("S008"))
        .stdout(contains("pi+"));
}

#[test]
fn particle_search_accepts_filters() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["particle", "--class", "lepton", "--limit", "3"])
        .assert()
        .success()
        .stdout(contains("Lepton"));
}

#[test]
fn particle_search_accepts_decay_filters() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args([
        "particle",
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
fn pdgid_search_uses_string_ids() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["pdgid", "S008"])
        .assert()
        .success()
        .stdout(contains("pi+"))
        .stdout(contains("211"));
}

#[test]
fn particle_lookup_finds_literal_kaon_names() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["particle", "K(S)0"])
        .assert()
        .success()
        .stdout(contains("S012"))
        .stdout(contains("K(S)0"));

    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["particle", "K(L)0"])
        .assert()
        .success()
        .stdout(contains("S013"))
        .stdout(contains("K(L)0"));
}

#[test]
fn show_finds_generic_pdgid_rows() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["show", "S008245"])
        .assert()
        .success()
        .stdout(contains("SEC"))
        .stdout(contains("pi+- FORM FACTORS"));
}

#[test]
fn pdgid_alias_finds_generic_pdgid_rows() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["pdgid", "S008245"])
        .assert()
        .success()
        .stdout(contains("SEC"))
        .stdout(contains("S008FV"));
}

#[test]
fn search_particles_uses_fast_summary_by_default() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["search", "particles", "K", "--limit", "3"])
        .assert()
        .success()
        .stdout(contains("Mass"))
        .stdout(contains("Quantum"))
        .stdout(predicates::str::contains("Decays").not());
}

#[test]
fn text_search_prints_results() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["text", "form factors", "--limit", "3"])
        .assert()
        .success()
        .stdout(contains("PDG ID"));

    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["search", "text", "form factors", "--limit", "3"])
        .assert()
        .success()
        .stdout(contains("PDG ID"));
}

#[test]
fn particle_json_is_valid() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    let output = cmd
        .args(["particle", "pi+", "--limit", "1", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value[0]["pdg_id"], "S008");
}

#[test]
fn pdgid_json_is_valid() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    let output = cmd
        .args(["pdgid", "S008", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["entry"]["pdg_id"], "S008");
    assert_eq!(value["particle"]["mcid"], 211);
}

#[test]
fn text_json_is_valid() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    let output = cmd
        .args(["text", "form factors", "--limit", "3", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(value.as_array().unwrap().len() <= 3);
}

#[test]
fn tui_stub_is_reserved() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.arg("tui")
        .assert()
        .failure()
        .stderr(contains("TUI is not implemented yet"));
}

#[test]
fn show_measurements_groups_related_entries() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["show", "S008.1", "--with-measurements"])
        .assert()
        .success()
        .stdout(contains("Related PDG IDs"))
        .stdout(contains("Measurements for S008R10"));
}
