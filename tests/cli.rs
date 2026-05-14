use assert_cmd::Command;
use predicates::str::contains;

#[test]
fn help_mentions_core_subcommands() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.arg("--help")
        .assert()
        .success()
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
fn text_search_prints_results() {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.args(["text", "form factors", "--limit", "3"])
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
    assert_eq!(value[0]["mcid"], 211);
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
