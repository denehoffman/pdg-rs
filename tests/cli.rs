use assert_cmd::Command;
use predicates::{prelude::PredicateBooleanExt, str::contains};

const TEST_DB_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/data/pdgall-2025-v0.2.2.sqlite"
);

fn pdg_command() -> Command {
    let mut cmd = Command::cargo_bin("pdg").unwrap();
    cmd.env("PDG_RS_DB_PATH", TEST_DB_PATH);
    cmd
}

#[test]
fn help_mentions_only_canonical_subcommands() {
    let mut cmd = pdg_command();
    cmd.arg("--help")
        .assert()
        .success()
        .stdout(contains("\n  find "))
        .stdout(contains("show"))
        .stdout(contains("db"))
        .stdout(contains("tui"))
        .stdout(contains("completions"))
        .stdout(contains("pdg find --text 'form factors'"))
        .stdout(contains("\n  search ").not())
        .stdout(contains("  particle").not())
        .stdout(contains("  pdgid").not())
        .stdout(contains("  text").not());
}

#[test]
fn shell_completion_scripts_are_available_without_a_database() {
    let cache = tempfile::tempdir().unwrap();
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        pdg_command()
            .env_remove("PDG_RS_DB_PATH")
            .env("PDG_RS_CACHE_DIR", cache.path())
            .args(["--offline", "completions", shell])
            .assert()
            .success()
            .stdout(contains("pdg"))
            .stdout(contains("completions"))
            .stderr("");
    }
    pdg_command()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(contains("--db-path"))
        .stdout(contains("gauge-boson"))
        .stdout(contains("self-conjugate"))
        .stdout(contains("0-or-1").not());
}

#[test]
fn find_includes_the_photon_in_both_integer_isospin_searches() {
    for value in ["0", "1"] {
        pdg_command()
            .args([
                "find",
                "--mcid",
                "22",
                "--isospin",
                value,
                "--format",
                "json",
            ])
            .assert()
            .success()
            .stdout(contains("\"name\": \"gamma\""))
            .stdout(contains("\"isospin\": \"0 or 1\""));
    }
    pdg_command()
        .args([
            "find",
            "--mcid",
            "22",
            "--isospin",
            "1/2",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout("[]\n");
    pdg_command()
        .args(["find", "--isospin", "0-or-1"])
        .assert()
        .code(2)
        .stderr(contains("expected 0, 1/2, 1, 3/2, unknown, or missing"));
}

#[test]
fn completion_metadata_keeps_filter_aliases_and_negative_parity() {
    pdg_command()
        .args([
            "find",
            "--class",
            "gauge_higgs_boson",
            "--type",
            "self",
            "--limit",
            "1",
        ])
        .assert()
        .success()
        .stdout(contains("Self-Conjugate"));
    pdg_command()
        .args(["find", "pi+", "--parity", "-", "--limit", "1"])
        .assert()
        .success()
        .stdout(contains("S008"));
}

#[test]
fn find_without_criteria_prints_its_help_without_opening_a_database() {
    let cache = tempfile::tempdir().unwrap();
    pdg_command()
        .env_remove("PDG_RS_DB_PATH")
        .env("PDG_RS_CACHE_DIR", cache.path())
        .args(["find", "--offline"])
        .assert()
        .success()
        .stdout(contains("Usage: pdg find"))
        .stdout(contains("--offline"))
        .stdout(contains("pdg find --class meson --mass 900..1100"));
}

#[test]
fn find_can_combine_aliases_and_filters() {
    let result = pdg_command()
        .args([
            "find",
            "X(3872)",
            "--charge",
            "0",
            "--offline",
            "--format",
            "json",
            "--limit",
            "1",
        ])
        .assert()
        .success();
    let hits: Vec<serde_json::Value> = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0]["name"], "chi_c1(3872)");
}

#[test]
fn nested_search_commands_are_unrecognized() {
    for subcommand in ["particles", "text"] {
        pdg_command()
            .args(["search", subcommand, "pi+"])
            .assert()
            .code(2)
            .stderr(contains("unrecognized subcommand 'search'"));
    }
}

#[test]
fn find_validates_text_options() {
    pdg_command()
        .args(["find", "--text"])
        .assert()
        .code(2)
        .stderr(contains("[QUERY]").or(contains("<QUERY>")));
    pdg_command()
        .args(["find", "--text", "form factors", "--class", "lepton"])
        .assert()
        .code(2)
        .stderr(contains("--text"))
        .stderr(contains("--class"));
    pdg_command()
        .args(["find", "pi+", "--show-full-text"])
        .assert()
        .code(2)
        .stderr(contains("--text"));
}

#[test]
fn db_path_prints_cache_path() {
    let mut cmd = pdg_command();
    cmd.env_remove("PDG_RS_DB_PATH")
        .args(["db", "path"])
        .assert()
        .success()
        .stdout(contains("pdgall-2026.0.sqlite"));
}

#[test]
fn db_path_prints_the_effective_override() {
    let expected = format!("{TEST_DB_PATH}\n");
    pdg_command()
        .args(["db", "path"])
        .assert()
        .success()
        .stdout(expected.clone());
    pdg_command()
        .env("PDG_RS_DB_PATH", "unused.sqlite")
        .args(["db", "path", "--db-path", TEST_DB_PATH])
        .assert()
        .success()
        .stdout(expected);
}

#[test]
fn db_path_flag_overrides_environment_for_queries_and_status() {
    pdg_command()
        .env("PDG_RS_DB_PATH", "unused.sqlite")
        .args(["--offline", "--db-path", TEST_DB_PATH, "show", "pi+"])
        .assert()
        .success()
        .stdout(contains("PDG edition 2025"));
    pdg_command()
        .env("PDG_RS_DB_PATH", "unused.sqlite")
        .args(["db", "status", "--db-path", TEST_DB_PATH])
        .assert()
        .success()
        .stdout(contains("database override (--db-path)"))
        .stdout(contains(TEST_DB_PATH))
        .stdout(contains("database edition: 2025"));
}

#[test]
fn missing_database_paths_report_an_error_without_creating_a_file() {
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.sqlite");
    for args in [["show", "S008"], ["db", "fetch"]] {
        pdg_command()
            .args(args)
            .arg("--db-path")
            .arg(&missing)
            .assert()
            .code(1)
            .stdout("")
            .stderr(contains("PDG database file does not exist"))
            .stderr(contains("missing.sqlite"));
        assert!(!missing.exists());
    }
}

#[test]
fn db_status_identifies_an_unsupported_sqlite_file() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unrelated.sqlite");
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute_batch("CREATE TABLE unrelated (value TEXT)")
        .unwrap();
    drop(conn);
    pdg_command()
        .args(["db", "status", "--db-path"])
        .arg(&path)
        .assert()
        .code(1)
        .stderr(contains("invalid or unsupported PDG database schema"))
        .stderr(contains("unrelated.sqlite"))
        .stderr(contains("pdgparticle"));
}

#[test]
fn db_fetch_honors_offline_for_missing_and_corrupt_caches() {
    let cache = tempfile::tempdir().unwrap();
    let cached_file = cache.path().join("pdgall-2026.0.sqlite");
    pdg_command()
        .env_remove("PDG_RS_DB_PATH")
        .env("PDG_RS_CACHE_DIR", cache.path())
        .env("PDG_RS_OFFLINE", "0")
        .args(["db", "fetch", "--offline"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(contains("downloads are disabled"));
    assert!(!cached_file.exists());

    std::fs::write(&cached_file, b"corrupt").unwrap();
    for (environment, flags) in [("0", &["--offline"][..]), ("1", &[][..])] {
        pdg_command()
            .env_remove("PDG_RS_DB_PATH")
            .env("PDG_RS_CACHE_DIR", cache.path())
            .env("PDG_RS_OFFLINE", environment)
            .args(["db", "fetch"])
            .args(flags)
            .assert()
            .code(1)
            .stdout("")
            .stderr(contains("PDG database size mismatch"));
        assert_eq!(std::fs::read(&cached_file).unwrap(), b"corrupt");
    }
}

#[test]
fn db_clear_rejects_an_explicit_database_path() {
    let cache = tempfile::tempdir().unwrap();
    let local = cache.path().join("local.sqlite");
    let cached = cache.path().join("pdgall-2026.0.sqlite");
    std::fs::write(&local, b"local").unwrap();
    std::fs::write(&cached, b"cache").unwrap();
    pdg_command()
        .env("PDG_RS_CACHE_DIR", cache.path())
        .args(["db", "clear", "--db-path"])
        .arg(&local)
        .assert()
        .code(1)
        .stderr(contains("db clear operates on the default cache"));
    assert!(local.exists());
    assert!(cached.exists());
}

#[test]
fn db_status_reports_database_override() {
    let mut cmd = pdg_command();
    cmd.args(["db", "status"])
        .assert()
        .success()
        .stdout(contains("database override:"))
        .stdout(contains("pdgall-2025-v0.2.2.sqlite"))
        .stdout(contains("database edition: 2025"))
        .stdout(contains("data release timestamp:"))
        .stdout(contains("available numeric editions: 2025, 2024"));
}

#[test]
fn find_can_choose_an_available_numeric_edition() {
    let result = pdg_command()
        .args(["find", "pi+", "--edition", "2024", "--format", "json"])
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert_eq!(value[0]["mass"]["edition"], "2024");

    pdg_command()
        .args(["find", "pi+", "--edition", "1900"])
        .assert()
        .code(1)
        .stderr(contains("PDG edition \"1900\" is unavailable"))
        .stderr(contains("2025, 2024"));
}

#[test]
fn show_reports_selected_edition_and_database_release() {
    let result = pdg_command()
        .args(["show", "S008M", "--edition", "2024", "--format", "json"])
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert_eq!(value["edition"], "2024");
    assert_eq!(value["database"]["edition"], "2025");
    assert!(value["database"]["data_release_timestamp"].is_string());
    assert_eq!(value["data"][0]["edition"], "2024");
}

#[test]
fn find_particles_prints_summary() {
    let mut cmd = pdg_command();
    cmd.args(["find", "K", "--limit", "3"])
        .assert()
        .success()
        .stdout(contains("Mass"))
        .stdout(contains("Quantum"))
        .stdout(contains("Charge").not())
        .stdout(contains("Decays").not());
}

#[test]
fn find_particles_accepts_filters() {
    let mut cmd = pdg_command();
    cmd.args(["find", "--class", "lepton", "--limit", "3"])
        .assert()
        .success()
        .stdout(contains("Lepton"));
}

#[test]
fn find_particles_accepts_mcid_without_query() {
    let mut cmd = pdg_command();
    cmd.args(["find", "--mcid", "22"])
        .assert()
        .success()
        .stdout(contains("S000"))
        .stdout(contains("gamma"))
        .stdout(contains("Usage:").not());
}

#[test]
fn find_particles_accepts_negative_mcid_without_equals() {
    let mut cmd = pdg_command();
    cmd.args(["find", "--mcid", "-2212"])
        .assert()
        .success()
        .stdout(contains("S016"))
        .stdout(contains("pbar"))
        .stdout(contains("-2212"))
        .stdout(contains("Usage:").not());
}

#[test]
fn find_mcid_composes_with_name_and_charge_filters() {
    for (name, charge, should_match) in [
        ("pi-", "-1", true),
        ("pi+", "-1", false),
        ("pi-", "0", false),
    ] {
        let result = pdg_command()
            .args(["find", "--mcid", "-211", "--format", "json"])
            .args([name, "--charge", charge])
            .assert()
            .success();
        let hits: Vec<serde_json::Value> =
            serde_json::from_slice(&result.get_output().stdout).unwrap();
        if should_match {
            assert_eq!(hits.len(), 1);
            assert_eq!(hits[0]["name"], "pi-");
        } else {
            assert!(hits.is_empty());
        }
    }
}

#[test]
fn find_can_opt_in_to_unknown_property_values() {
    let args = ["find", "pi+", "--width", "0..1", "--format", "json"];
    pdg_command().args(args).assert().success().stdout("[]\n");
    pdg_command()
        .args(args)
        .arg("--include-unknown-properties")
        .assert()
        .success()
        .stdout(contains("\"name\": \"pi+\""));
}

#[test]
fn find_rejects_invalid_property_ranges() {
    for (range, error) in [("NaN..1", "cannot be NaN"), ("2..1", "must be <=")] {
        pdg_command()
            .args(["find", "--mass", range])
            .assert()
            .code(2)
            .stderr(contains(error));
    }
}

#[test]
fn find_decay_contains_does_not_reuse_one_product() {
    pdg_command()
        .args([
            "find",
            "pi+",
            "--decay-contains",
            "mu+,mu+",
            "--decay-expansion",
            "literal",
            "--format",
            "json",
        ])
        .assert()
        .success()
        .stdout("[]\n");
}

#[test]
fn find_particles_matches_common_aliases() {
    let mut cmd = pdg_command();
    cmd.args(["find", "proton"])
        .assert()
        .success()
        .stdout(contains("S016"))
        .stdout(contains("p"))
        .stdout(contains("No particles found").not());
}

#[test]
fn find_particles_accepts_database_aliases_and_fuzzy_queries() {
    for (query, name) in [("X(3872)", "chi_c1(3872)"), ("π⁺", "pi+"), ("prtoon", "p")] {
        let mut cmd = pdg_command();
        let result = cmd
            .args(["find", query, "--limit", "1", "--format", "json"])
            .assert()
            .success();
        let hits: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
        assert_eq!(hits[0]["name"], name, "{query}");
    }
}

#[test]
fn find_particles_ranks_exact_short_names_first() {
    let mut cmd = pdg_command();
    let stdout = String::from_utf8(
        cmd.args(["find", "p", "--limit", "5"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    let proton = stdout.find("│ S016").unwrap();
    let pentaquark = stdout.find("│ B171").unwrap_or(usize::MAX);
    assert!(proton < pentaquark);
}

#[test]
fn find_particles_filter_only_defaults_to_all_results() {
    let mut cmd = pdg_command();
    let output = cmd
        .args(["find", "--charge", "+2", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(value.as_array().unwrap().len() > 20);
}

#[test]
fn find_particles_accepts_decay_filters() {
    let mut cmd = pdg_command();
    cmd.args([
        "find",
        "f_0(980)0",
        "--decays-to",
        "KS,KS",
        "--charge",
        "0",
        "--limit",
        "1",
    ])
    .assert()
    .success()
    .stdout(contains("Meson"))
    .stdout(contains("Matching PDG decay channels"))
    .stdout(contains("M003.1"))
    .stdout(contains("f_0(980) --> K Kbar"));
}

#[test]
fn find_decay_filters_accept_neutral_kaon_aliases() {
    let query = |final_state: &str| {
        pdg_command()
            .args([
                "find",
                "--class",
                "meson",
                "--charge",
                "0",
                "--mass",
                "900..2000",
                "--decays-to",
                final_state,
                "--all",
                "--format",
                "json",
            ])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone()
    };
    let canonical = query("K(S)0,K(S)0");
    let results: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
    let particle = results
        .as_array()
        .unwrap()
        .iter()
        .find(|particle| particle["name"] == "f_0(980)0")
        .unwrap();
    let channel = &particle["matching_decays"][0];
    assert_eq!(channel["pdgid"], "M003.1");
    assert_eq!(channel["description"], "f_0(980) --> K Kbar");
    assert_eq!(channel["matched_initial_states"], false);
    assert_eq!(channel["matched_final_states"], true);
    let products = channel["products"].as_array().unwrap();
    assert!(products.iter().any(|product| {
        product["name"] == "Kbar" && product["is_outgoing"] == true && product["multiplier"] == 1
    }));
    assert_eq!(query("KS,KS"), canonical);
    assert_eq!(query("Kₛ⁰, KS"), canonical);
}

#[test]
fn find_particles_uses_section_derived_properties() {
    let mut cmd = pdg_command();
    cmd.args([
        "find",
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
fn find_particles_filters_section_derived_properties() {
    let mut cmd = pdg_command();
    cmd.args(["find", "a_0(", "--mass", "1500..2000", "--limit", "20"])
        .assert()
        .success()
        .stdout(contains("a_0(1710)0"))
        .stdout(contains("a_0(980)0").not());
}

#[test]
fn find_text_prints_results() {
    let mut cmd = pdg_command();
    cmd.args(["find", "--text", "form factors", "--limit", "3"])
        .assert()
        .success()
        .stdout(contains("PDG ID"))
        .stdout(contains("Title"))
        .stdout(contains("Match"))
        .stdout(contains("Score").not())
        .stdout(contains("Source"))
        .stdout(contains("Description"))
        .stdout(contains("Parent S008"))
        .stdout(contains("pi+- FORM FACTORS"));
}

#[test]
fn find_text_includes_footnotes() {
    let mut cmd = pdg_command();
    cmd.args(["find", "--text", "normalisation decay", "--limit", "2"])
        .assert()
        .success()
        .stdout(contains("S042P86"))
        .stdout(contains("normalisation"))
        .stdout(contains("B0 --> K*(892)0 tau+ mu-"));
}

#[test]
fn show_resolves_particle_names_aliases_and_signed_mcids() {
    for (selector, name, mcid) in [
        (&["π⁺"][..], "pi+", Some(211)),
        (&["X(3872)"][..], "chi_c1(3872)", None),
        (&["--mcid", "-211"][..], "pi-", Some(-211)),
    ] {
        let result = pdg_command()
            .arg("show")
            .args(selector)
            .args(["--format", "json"])
            .assert()
            .success();
        let value: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
        assert_eq!(value["particle"]["name"], name, "{selector:?}");
        assert_eq!(value["particle"]["mcid"].as_i64(), mcid, "{selector:?}");
    }
}

#[test]
fn show_lists_variants_for_shared_pdg_ids() {
    let names = ["rho(770)+", "rho(770)-", "rho(770)0"];
    let pretty = pdg_command()
        .args(["show", "M009"])
        .assert()
        .success()
        .stdout(contains("Particle variants"));
    let stdout = std::str::from_utf8(&pretty.get_output().stdout).unwrap();
    for name in names {
        assert!(stdout.contains(name));
    }

    let result = pdg_command()
        .args(["show", "M009", "--format", "json"])
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert!(value["particle"].is_null());
    let variants = value["variants"].as_array().unwrap();
    let actual_names = variants
        .iter()
        .map(|variant| variant["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(actual_names, names);
}

#[test]
fn show_reports_ambiguous_and_close_matches_on_stderr() {
    pdg_command()
        .args(["show", "P_c_cbar(4312)+", "--format", "json"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(contains("matches multiple particles"))
        .stderr(contains("B185"))
        .stderr(contains("B205"));

    pdg_command()
        .args(["show", "prtoon", "--format", "json"])
        .assert()
        .code(1)
        .stdout("")
        .stderr(contains("Close matches"))
        .stderr(contains("p (PDG ID S016, MCID 2212)"));
}

#[test]
fn show_requires_exactly_one_lookup_selector() {
    pdg_command()
        .arg("show")
        .assert()
        .code(2)
        .stderr(contains("<TARGET>"));
    pdg_command()
        .args(["show", "S008", "--mcid", "211"])
        .assert()
        .code(2)
        .stderr(contains("cannot be used with"));
}

#[test]
fn show_finds_generic_pdgid_rows_with_labels() {
    let mut cmd = pdg_command();
    cmd.args(["show", "S008245"])
        .assert()
        .success()
        .stdout(contains("Section"))
        .stdout(contains("pi+- FORM FACTORS"));
}

#[test]
fn show_particle_promotes_section_properties_with_sources() {
    let mut cmd = pdg_command();
    cmd.args(["show", "M036"])
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
    let mut cmd = pdg_command();
    cmd.args(["show", "M036", "--view", "full"])
        .assert()
        .success()
        .stdout(contains("Related PDG IDs"))
        .stdout(contains("Measurements for M036R2").not());
}

#[test]
fn show_measurements_use_reference_blocks() {
    let mut cmd = pdg_command();
    let stdout = String::from_utf8(
        cmd.args(["show", "M036R2", "--view", "measurements"])
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
    let mut cmd = pdg_command();
    cmd.args(["show", "S008"])
        .assert()
        .success()
        .stdout(contains("Properties"))
        .stdout(contains("Form factor"))
        .stdout(contains("S008FV"))
        .stdout(contains("Section S008245"));
}

#[test]
fn show_overview_is_the_default_and_omits_full_sections() {
    let mut cmd = pdg_command();
    cmd.args(["show", "M036"])
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
    let mut cmd = pdg_command();
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
    let mut cmd = pdg_command();
    cmd.args(["show", "M036", "--related-details"])
        .assert()
        .success()
        .stdout(contains("Related details"))
        .stdout(contains("Measurements for M036R2"));

    let result = pdg_command()
        .args(["show", "M036", "--related-details", "--format", "json"])
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert_eq!(value["view"], "full");
    let related = value["related_details"].as_array().unwrap();
    let ratio = related
        .iter()
        .find(|row| row["entry"]["pdgid"] == "M036R2")
        .unwrap();
    assert_eq!(ratio["measurements"][0]["technique"], "BES3");
}

#[test]
fn show_mass_view_keeps_only_mass_and_its_source() {
    let mut cmd = pdg_command();
    cmd.args(["show", "M036", "--view", "mass"])
        .assert()
        .success()
        .stdout(contains("M036MX"))
        .stdout(contains("M036W1").not())
        .stdout(contains("\x1b[").not());
    let result = pdg_command()
        .args(["show", "M036", "--view", "mass", "--format", "json"])
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert_eq!(value["view"], "mass");
    let properties = value["properties"].as_array().unwrap();
    assert_eq!(properties.len(), 1);
    assert_eq!(properties[0]["value"]["pdgid"], "M036MX");
    assert_eq!(properties[0]["source"]["kind"], "section");
    assert!(value.get("measurements").is_none());
}

#[test]
fn show_decay_view_includes_recorded_channels() {
    pdg_command()
        .args(["show", "pi+", "--view", "decays"])
        .assert()
        .success()
        .stdout(contains("Branching fractions"))
        .stdout(contains("S008.1"))
        .stdout(contains("Measurements for").not());
    let result = pdg_command()
        .args(["show", "pi+", "--view", "decays", "--format", "json"])
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    let channels = value["branching_fractions"].as_array().unwrap();
    let channel = channels
        .iter()
        .find(|row| row["pdgid"] == "S008.1")
        .unwrap();
    assert!(
        channel["products"]
            .as_array()
            .unwrap()
            .iter()
            .any(|product| product["name"] == "mu+")
    );
    assert!(value.get("properties").is_none());
}

#[test]
fn show_evidence_views_find_property_measurements_and_unique_references() {
    pdg_command()
        .args(["show", "pi+", "--view", "measurements"])
        .assert()
        .success()
        .stdout(contains("Measurements for S008M"))
        .stdout(contains("\x1b[").not());
    pdg_command()
        .args(["show", "M036R2", "--view", "references"])
        .assert()
        .success()
        .stdout(contains("ABLIKIM 2022AH"))
        .stdout(contains("Values:").not());
    let result = pdg_command()
        .args(["show", "pi+", "--view", "references", "--format", "json"])
        .assert()
        .success();
    let value: serde_json::Value = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert_eq!(value["view"], "references");
    let references = value["references"].as_array().unwrap();
    let mut document_ids = std::collections::HashSet::new();
    assert!(!references.is_empty());
    assert!(
        references
            .iter()
            .all(|reference| { document_ids.insert(reference["document_id"].as_str().unwrap()) })
    );
    assert!(value.get("measurements").is_none());
}

#[test]
fn show_json_is_valid() {
    let mut cmd = pdg_command();
    let output = cmd
        .args(["show", "S009", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["view"], "overview");
    assert_eq!(value["entry"]["pdgid"], "S009");
    assert_eq!(value["particle"]["mcid"], 111);
}

#[test]
fn show_json_includes_measurement_details() {
    let mut cmd = pdg_command();
    let output = cmd
        .args([
            "show",
            "M036R2",
            "--view",
            "measurements",
            "--format",
            "json",
        ])
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
fn find_particles_json_is_valid() {
    let mut cmd = pdg_command();
    let output = cmd
        .args(["find", "pi+", "--limit", "1", "--format", "json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value[0]["pdgid"], "S008");
    assert!(value[0].get("matching_decays").is_none());
}

#[test]
fn find_text_json_is_valid() {
    let mut cmd = pdg_command();
    let output = cmd
        .args([
            "find",
            "--text",
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
fn find_text_reports_the_correction_and_parent_entry_in_json() {
    let result = pdg_command()
        .args([
            "find",
            "--text",
            "form factros",
            "--limit",
            "1",
            "--format",
            "json",
        ])
        .assert()
        .success();
    let hits: Vec<serde_json::Value> = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert_eq!(hits[0]["pdgid"], "S008245");
    assert_eq!(hits[0]["title"], "pi+- FORM FACTORS");
    assert_eq!(hits[0]["parent"]["pdgid"], "S008");
    assert_eq!(hits[0]["correction"]["original"], "factros");
    assert_eq!(hits[0]["correction"]["replacement"], "factors");
}

#[test]
fn find_text_json_includes_footnote_source() {
    let mut cmd = pdg_command();
    let output = cmd
        .args([
            "find",
            "--text",
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
fn find_text_all_overrides_an_explicit_limit() {
    let args = [
        "find",
        "--text",
        "form factors",
        "--limit",
        "0",
        "--format",
        "json",
    ];
    pdg_command().args(args).assert().success().stdout("[]\n");

    let result = pdg_command().args(args).arg("--all").assert().success();
    let hits: Vec<serde_json::Value> = serde_json::from_slice(&result.get_output().stdout).unwrap();
    assert!(hits.len() > 1);
}

#[test]
fn tui_stub_is_reserved() {
    let mut cmd = pdg_command();
    cmd.arg("tui")
        .assert()
        .failure()
        .stderr(contains("TUI is not implemented yet"));
}
