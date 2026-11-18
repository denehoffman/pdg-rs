# pdg-rs

This crate enables programmatic access to the [Particle Data Group](https://pdg.lbl.gov)'s database of particle physics measurements. This project is independently developed and not affiliated with the PDG. Please use the citation at the end of this README as you would if you were using the PDG website!

## Installation

### For programmatic usage inside another Rust crate
```bash
cargo add pdg-rs
```
### For general CLI usage
```bash
cargo install pdg-rs
```

## Usage

### CLI
This crate provides a `pdg` binary for searching the PDG database. The first command that needs the default database downloads and verifies `pdgall-2026.0.sqlite` in your OS-specific cache directory. [Official PDG database downloads](https://pdg.lbl.gov/2026/api/index.html)

```bash
pdg find 'K_S^0' --limit 5
pdg find 'X(3872)'
pdg find prtoon
pdg find --class lepton --limit 5
pdg find --decays-to 'KS,KS' --charge 0
pdg find --text 'form factors'
pdg show 'pi+'
pdg show f0980
pdg show a1320+
pdg show --mcid -211 --view mass
pdg show M009 --view decays
pdg show S008245
```
`pdg find` searches particles by default. Use `--text` to search PDG descriptions, text, and footnotes, and add `--show-full-text` to expand the snippets.

Use `pdg find --text 'form fact*'` to match a partial term. When a whole-word query finds nothing, `pdg` also tries the last word as a prefix and checks for a likely spelling correction. Results show the source and parent entry, with the correction displayed when one was used.

The first text search builds a temporary index for that database connection. `pdg find --text` returns 20 ranked hits by default. Use `--limit` to request a different count or `--all` for every match. Rust callers can use `Pdg::search_text_with_limit`.

`pdg show` accepts particle names and aliases, including a unique partial match or typo correction, as well as PDG database IDs such as `S008M` and `S008245`. Pass `--mcid` for a signed Monte Carlo ID. A shared PDG ID such as `M009` displays all particle variants. JSON reports them in `variants` with `particle` set to `null`. If several candidates are still possible, `show` lists the choices on stderr. In Rust, `Pdg::lookup` takes a `LookupSelector` and returns a `Lookup` outcome.

Shells interpret unquoted parentheses before running `pdg`. Quote the full name, as in `pdg show 'f0(980)'`, or use the shorthand `pdg show f0980` or `pdg show f980`. The resonance number must match. `pdg show a21320` and `pdg show a1320` open the shared `a_2(1320)` listing. Add a charge, as in `a1320+` or `a13200`, to select one variant. If a shorthand spans distinct listings, include the index or use a PDG ID. For example, `f2300` matches both `f_2(2300)0` and `f_4(2300)0`.

Use `pdg show TARGET --view VIEW` to select details in either terminal or JSON output.

| View | Shows |
| --- | --- |
| `overview` (default) | Identity, headline properties with their PDG sources, direct data, and text for non-particle entries |
| `mass` | Mass values and their PDG sources |
| `decays` | Branching fractions and ratios, or direct data for a channel or ratio ID |
| `measurements` | Measurements for the entry and its headline properties or immediate section entries |
| `references` | Unique publications for the same scope as the measurements view |
| `full` | Text, data, decay listings, direct measurements, and linked entry IDs |

`--related-details` selects the full view and expands descendant and linked entries in both formats. Rust callers can load the same sections with `Pdg::entry_details` and an `EntryDetailView`.

For scripts, replace `--summary` with the default invocation or `--view overview`. Use `--view full` for the previous expanded default. JSON includes a top-level `view` and uses top-level `properties` for mass, lifetime, width, and other selected properties. Empty and unselected sections are omitted. Expanded linked entries appear in `related_details`. The CLI exits with 0 on success, 1 for lookup or runtime errors, and 2 for argument parsing errors. Redirected output contains no ANSI styling.

Particle filters combine with AND, including `--mcid`. Mass, width, and lifetime filters require an overlapping property interval by default. Use `--include-unknown-properties` to also include particles without a comparable value for a requested property. `--decay-contains` counts repeated products within one decay mode. For example, `--decay-contains 'pi+,pi+'` requires two `pi+` products in the same mode.

Decay filters accept aliases, resonance shorthand, and unique close matches in their default inclusive mode. For example, `pdg find --decay-contains f980` searches for the recorded `f_0(980)` group. `KS`, `Kₛ⁰`, and `K(S)0` resolve to the same neutral kaon before PDG family expansion. `--decay-expansion literal` compares the product spellings as recorded in the database.

Decay-filtered results list the matching PDG channels with their IDs and recorded descriptions. A `KS,KS` search can match a channel listed as `K Kbar` through family expansion. Use `pdg show M003.1` to inspect that listing. JSON includes a `matching_decays` array with the channel description, product names, multiplicities, and which state filters matched. In Rust, `Pdg::search_particle_hits` and `Pdg::find_particles` expose the same matches.

Particle name search recognizes PDG aliases, common names, and notation such as `π⁺`. Exact names and aliases rank first, followed by resonance shorthand, partial matches, and typo corrections. In Rust, call `pdg.find_particles("X(3872)", ParticleSearchQuery::new())?` to get the same ranked results with the matched spelling and reason.

Pass `--db-path FILE` to open an existing local SQLite database read-only. The flag takes precedence over `PDG_RS_DB_PATH`. With neither override, `pdg` uses the pinned database in the cache, which you can relocate with `PDG_RS_CACHE_DIR`.

`pdg db path` prints the effective database path, and `pdg db status` reports the source and release. `pdg db fetch` prepares the selected database for local use. `--offline` or `PDG_RS_OFFLINE=1` disables downloads, including for `db fetch`. `pdg db clear` removes the pinned cache file and rejects `--db-path`.

Numeric data and property filters default to the opened file's edition. Run `pdg db status` to see its release and available numeric editions. Use `pdg show 'pi+' --edition 2024` or `pdg find --mass 100..150 --edition 2024` for historical values. Rust callers can use `Pdg::open()?.with_edition("2024")?`. The particle catalogue, decay listings, text, and measurements come from the opened file's release. JSON from `show` reports the selected `edition` and the release metadata in `database`.

### Terminal browser

```sh
pdg tui
pdg tui f980
pdg tui --text 'form factors'
```

Type to search, press Tab to switch between particle names and PDG text, and use the arrow keys to select a result. Enter opens it. In the detail pane, keys 1 through 5 select overview, mass, decays, measurements, and references. Key 0 returns to the matched passage for a text result. Use the arrow or Page Up and Page Down keys to scroll. Esc returns to the results or exits from the search screen, and Ctrl+C exits from either screen.

Press F2 from particle search to edit the filters. Class, particle type, charge, and spin start at **Any**. Use Tab or Shift+Tab to move between fields. On a selectable field, use Left or Right to cycle, or press Space to open the list and Enter to choose an option. Spin also has a **Custom…** option for PDG spin strings.

Enter applies the filters together and Esc cancels the edits. Mass ranges in `MeV` and decay products are editable, and blank fields accept any value. You can apply filters with an empty search box, then type a name to narrow the results. The browser remembers the particle filters when you switch to text search.

For example, enter `meson`, `0`, `900..2000`, and `KS,KS` with **Decays to** selected. Opening a result shows its matching channels with the recorded PDG descriptions. Select a channel and press Enter to open that listing. Key 6 returns to the matching channels from the particle's other detail views.

Press `c` in the detail pane to copy the canonical name when a single particle is selected. Press `i` to copy the PDG ID. Clipboard delivery uses OSC 52 and depends on terminal support. The browser uses the same `--db-path`, `--offline`, and `--edition` options as the CLI queries. The `tui` Cargo feature is enabled by default. To build the binary without the browser, use `--no-default-features --features ratio`.

### Shell completion

`pdg completions --help` lists the supported shells. Load the generated script in the current shell with one of these commands.

```bash
source <(pdg completions bash)
```

```zsh
autoload -Uz compinit
compinit
source <(pdg completions zsh)
```

```fish
pdg completions fish | source
```

### Rust quickstart

After caching the database with `pdg db fetch` or `Pdg::open`, use `Pdg::open_cached` for offline access.

```rust,no_run
use pdg_rs::{Charge, Lookup, LookupSelector, ParticleSearchQuery, Pdg, PdgResult};

fn main() -> PdgResult<()> {
    let pdg = Pdg::open_cached()?;
    if let Lookup::Found { entry, .. } = pdg.lookup(LookupSelector::Mcid(211))? {
        println!("{} {}", entry.pdgid, entry.description);
    }
    let filters = ParticleSearchQuery::new().charge(Charge::Plus);
    for hit in pdg.find_particles("pion", filters)? {
        println!("{} ({:?})", hit.particle.name, hit.kind);
    }
    Ok(())
}
```

Call `into_owned()` on a ranked or filter search hit to keep its identity, match information, and decay channels after closing the database. For details on a particle, choose which properties to load. The returned record can also outlive the database and serialize with Serde.

```rust,no_run
use pdg_rs::{DataType, Lookup, LookupSelector, ParticleDetailSelection, Pdg, PdgResult};

fn main() -> PdgResult<()> {
    let pdg = Pdg::open_cached()?;
    if let Lookup::Found { particles, .. } = pdg.lookup(LookupSelector::Mcid(211))? {
        for particle in particles {
            let details =
                particle.details(ParticleDetailSelection::Properties(&[DataType::Mass]))?;
            if let Some(mass) = details.property(DataType::Mass) {
                println!("{} from {}", mass.value.display, mass.value.pdgid);
            }
        }
    }
    Ok(())
}
```

Use `ParticleDetailSelection::Measurements(&[DataType::Mass])` to also load the measurements and references for that property. `ParticleDetailSelection::Overview` loads the headline properties shown by `pdg show`.

### Get information about a specific particle:
```rust,no_run
use pdg_rs::{Pdg, PdgResult};

fn main() -> PdgResult<()> {
    let pdg = Pdg::open()?;
    let pi_plus = pdg.particle("pi+")?.unwrap();
    // alternatively `pdg.mcid(211)?.unwrap()`
    println!("{pi_plus}");
    let m_pi_plus = pi_plus.mass()?.unwrap();
    println!("{m_pi_plus}");
    Ok(())
}

// pi+ (S008, Meson, Particle, charge +1), MCID 211, I=1, G=-, J=0, P=-
// 139.57039+-0.00018 MeV
```
Most queries return `Option`s because we can't guarantee the search term will yield results (for instance, the "p-" doesn't exist, so asking for it should return `None`). The `PdgError` type is mostly reserved for errors encountered while parsing data from the sqlite database. Particle information is either directly available (charge, quantum numbers) or queryable (mass, lifetime). Most of these methods will produce unique data types (like a `Mass` struct) which contain additional information:

```rust,no_run
use pdg_rs::{Pdg, PdgResult};

fn main() -> PdgResult<()> {
    let pdg = Pdg::open()?;
    let pi_plus = pdg.particle("pi+")?.unwrap();
    let m_pi_plus = pi_plus.mass()?.unwrap();

    let mass_texts = pdg.texts_for(&m_pi_plus.pdgid)?;
    for text in mass_texts {
        if let Some(text) = text.text {
            println!("{}", text);
        }
    }
    let mass_measurements = pdg.measurements_for(&m_pi_plus.pdgid)?;
    for measurement in mass_measurements {
        println!(
            "{:<20} {:<50}: {}",
            measurement.reference.document_id,
            measurement.reference.doi.unwrap_or_default(),
            measurement
                .values
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(())
}
```
This will output:
```text
The most accurate charged pion mass measurements are based upon x-ray wavelength measurements for transitions in pi--mesonic atoms.  The observed line is the blend of three components, corresponding to different K-shell occupancies.  JECKELMANN 1994 revisits the occupancy question, with the conclusion that two sets of occupancy ratios, resulting in two different pion masses (Solutions A and B), are equally probable. We choose the higher Solution B since only this solution is consistent with a positive mass-squared for the muon neutrino, given the precise muon momentum measurements now available (DAUM 1991, ASSAMAGAN 1994, and ASSAMAGAN 1996) for the decay of pions at rest.  Earlier mass determinations with pi-mesonic atoms may have used incorrect K-shell screening corrections.      Measurements with an error of >0.005 MeV have been omitted from this Listing.
DAUM 2019            10.1016/j.physletb.2019.07.027                    : 139.57021 +-0.00014 MeV
TRASSINELLI 2016     10.1016/j.physletb.2016.06.025                    : 139.57077 +-0.00018 MeV
LENZ 1998            10.1016/S0370-2693(97)01337-3                     : 139.57071 +-0.00053 MeV
JECKELMANN 1994      10.1016/0370-2693(94)90358-1                      : 139.56995 +-0.00035 MeV
ASSAMAGAN 1996       10.1103/PhysRevD.53.6065                          : 139.57022 +-0.00014 MeV
JECKELMANN 1994      10.1016/0370-2693(94)90358-1                      : 139.56782 +-0.00037 MeV
DAUM 1991            10.1016/0370-2693(91)90078-5                      : 139.56996 +-0.00067 MeV
JECKELMANN 1986B     10.1016/0375-9474(86)90476-8                      : 139.56752 +-0.00037 MeV
ABELA 1984           10.1016/0370-2693(84)90155-2                      : 139.5704 +-0.0011 MeV
LU 1980              10.1103/PhysRevLett.45.1066                       : 139.5664 +-0.0009 MeV
CARTER 1976          10.1103/PhysRevLett.37.1380                       : 139.5686 +-0.0020 MeV
MARUSHENKO 1976                                                        : 139.5660 +-0.0024 MeV
```
We are sometimes limited by the type schema defined by the PDG, but this is just a matter of safety. As you can see, the last reference here didn't come with a DOI. However, switching `doi` for `inspire_id` does give us a result for every entry here. These measurement entries also include additional information like systematic uncertainties, event counts, and experimental details, all found in the PDG database.

### Decays
Branching ratios/fractions are also available in the database, and `pdg-rs` provides an interface for exploring them:
```rust,no_run
use pdg_rs::{Pdg, PdgResult};

fn main() -> PdgResult<()> {
    let pdg = Pdg::open()?;
    let pi_plus = pdg.particle("pi+")?.unwrap();

    let decays = pi_plus.branching_fractions()?;
    for decay in decays {
        println!("{:<20} {}", decay.value.to_string(), decay.description)
    }
    Ok(())
}
```
```text
99.98770+-0.00004%   pi+ --> mu+ nu_mu
(1.230+-0.004)E-4    pi+ --> e+ nu_e
(1.036+-0.006)E-8    pi+ --> e+ nu_e pi0
(3.2+-0.5)E-9        pi+ --> e+ nu_e e+ e-
<9E-6                pi+ --> mu+ nu_mu nu nubar
<1.6E-7              pi+ --> e+ nu_e nu nubar
<1.5E-3              pi+ --> mu+ nubar_e
<8.0E-3              pi+ --> mu+ nu_e
<1.6E-6              pi+ --> mu- e+ e+ nu
```
Of course, we might like to see how these measurements were obtained. Sometimes we may be able to find a raw branching fraction measurement, but more often these are associated to measurements of branching ratios. We can query branching ratios, but we can also find related data entries):
```rust,no_run
use pdg_rs::{Pdg, PdgResult};

fn main() -> PdgResult<()> {
    let pdg = Pdg::open()?;
    let pi_plus = pdg.particle("pi+")?.unwrap();

    let decays = pi_plus.branching_fractions()?;
    for decay in decays {
        println!("{:<20} {}", decay.value.to_string(), decay.description);
        for data in &decay.related_data {
            println!("    {:<20} {}", data.value.to_string(), data.description);
            for measurement in data.measurements()? {
                println!(
                    "        {:<25}  {}",
                    measurement.reference.document_id,
                    measurement
                        .reference
                        .doi
                        .clone()
                        .or(measurement.reference.inspire_id.clone())
                        .or(measurement.reference.title.clone())
                        .unwrap_or_default()
                );
            }
        }
    }
    Ok(())
}
```
```text
99.98770+-0.00004%   pi+ --> mu+ nu_mu
    1.2327+-0.0023E-4    [G(pi+ --> e+ nu_e)+ G(pi+ --> e+ nu(e) gamma)]/[G(pi+ --> mu+ nu_mu)+ G(pi+ --> mu+ nu_mu gamma)]
        AGUILAR-AREVALO 2015       10.1103/PhysRevLett.115.071801
        CZAPEK 1993                10.1103/PhysRevLett.70.17
        BRITTON 1992               10.1103/PhysRevLett.68.3000
        BRYMAN 1986                10.1103/PhysRevD.33.1211
        DICAPUA 1964               10.1103/PhysRev.133.B1333
        ANDERSON 1960              10.1103/PhysRev.119.2050
    3.2+-0.5E-9          G(pi+ --> e+ nu(e) e+ e-)/G(pi+ --> mu+ nu_mu)
        EGLI 1989                  10.1016/0370-2693(89)90358-4
        BARANOV 1992
        KORENCHENKO 1976B
        KORENCHENKO 1971
    <8.6E-6              G(pi+ --> mu+ nu_mu nu nubar)/G(pi+ --> mu+ nu_mu)
        AGUILAR-AREVALO 2020A      10.1103/PhysRevD.102.012001
(1.230+-0.004)E-4    pi+ --> e+ nu_e
    (123.0+-0.4)E-2E-4   G(pi+ --> e+ nu_e)/G(total)
    1.2327+-0.0023E-4    [G(pi+ --> e+ nu_e)+ G(pi+ --> e+ nu(e) gamma)]/[G(pi+ --> mu+ nu_mu)+ G(pi+ --> mu+ nu_mu gamma)]
        AGUILAR-AREVALO 2015       10.1103/PhysRevLett.115.071801
        CZAPEK 1993                10.1103/PhysRevLett.70.17
        BRITTON 1992               10.1103/PhysRevLett.68.3000
        BRYMAN 1986                10.1103/PhysRevD.33.1211
        DICAPUA 1964               10.1103/PhysRev.133.B1333
        ANDERSON 1960              10.1103/PhysRev.119.2050
(1.036+-0.006)E-8    pi+ --> e+ nu_e pi0
    1.036+-0.006E-8      G(pi+ --> e+ nu(e) pi0)/G(total)
        POCANIC 2004               10.1103/PhysRevLett.93.181803
        MCFARLANE 1985             10.1103/PhysRevD.32.547
        DEPOMMIER 1968             10.1016/0550-3213(68)90305-2
        BACASTOW 1965              10.1103/PhysRev.139.B407
        BERTRAM 1965               10.1103/PhysRev.139.B617
        DUNAITSEV 1965
        BARTLETT 1964              10.1103/PhysRev.136.B1452
        DEPOMMIER 1963             10.1016/S0375-9601(63)80030-4
(3.2+-0.5)E-9        pi+ --> e+ nu_e e+ e-
    3.2+-0.5E-9          G(pi+ --> e+ nu(e) e+ e-)/G(pi+ --> mu+ nu_mu)
        EGLI 1989                  10.1016/0370-2693(89)90358-4
        BARANOV 1992
        KORENCHENKO 1976B
        KORENCHENKO 1971
<9E-6                pi+ --> mu+ nu_mu nu nubar
    <8.6E-6              G(pi+ --> mu+ nu_mu nu nubar)/G(pi+ --> mu+ nu_mu)
        AGUILAR-AREVALO 2020A      10.1103/PhysRevD.102.012001
<1.6E-7              pi+ --> e+ nu_e nu nubar
    <1.6E-7              G(pi+ --> e+ nu(e) nu nubar)/G(total)
        AGUILAR-AREVALO 2020A      10.1103/PhysRevD.102.012001
        PICCIOTTO 1988             10.1103/PhysRevD.37.1131
<1.5E-3              pi+ --> mu+ nubar_e
    <1.5E-3              G(pi+ --> mu+ nubar_e)/G(total)
        COOPER 1982                10.1016/0370-2693(82)90914-5
<8.0E-3              pi+ --> mu+ nu_e
    <8.0E-3              G(pi+ --> mu+ nu_e)/G(total)
        COOPER 1982                10.1016/0370-2693(82)90914-5
<1.6E-6              pi+ --> mu- e+ e+ nu
    <1.6E-6              G(pi+ --> mu- e+ e+ nu)/G(total)
        BARANOV 1991B
        KORENCHENKO 1987
```

### Particle searches
It is often useful to search for all particles which satisfy some set of properties. This is currently sort of difficult to do with just the PDG website or other programmatic interfaces, but generally enough information exists in the database itself to allow for complex searches. For example, let's search for all particles which may decay to $`K_S^0K_S^0`$. By default, searches are inclusive, so $`K_S^0`$ will be mapped to other entries like $`K^0`$ and $`\bar{K}^0`$, which is useful since these are often the states listed in the database. Of course, not everything that can decay to $`K\bar{K}`$ can also decay to $`K_S^0K_S^0`$, and the PDG sometimes includes forbidden decays simply because there are published results, so it's often nice to narrow the search with additional information, such as $`P`$, $`C`$, and charge:
```rust,no_run
use pdg_rs::{Charge, Parity, ParticleSearchQuery, Pdg, PdgResult};

fn main() -> PdgResult<()> {
    let pdg = Pdg::open()?;
    let particles = pdg.search_particles(
        ParticleSearchQuery::default()
            .decays_to(["K(S)0", "K(S)0"])
            .charge(Charge::Neutral)
            .parity(Parity::Plus)
            .charge_conjugation(Parity::Plus),
    )?;
    for particle in particles {
        println!("{particle}");
    }
    Ok(())
}
```
```text
f_0(980)0 (M003, Meson, Self-Conjugate, charge 0), MCID 9010221, I=0, G=+, J=0, P=+, C=+
f_2(1270)0 (M005, Meson, Self-Conjugate, charge 0), MCID 225, I=0, G=+, J=2, P=+, C=+
a_2(1320)0 (M012, Meson, Self-Conjugate, charge 0), MCID 115, I=1, G=-, J=2, P=+, C=+
f_2^'(1525)0 (M013, Meson, Self-Conjugate, charge 0), MCID 335, I=0, G=+, J=2, P=+, C=+
f_4(2050)0 (M016, Meson, Self-Conjugate, charge 0), MCID 229, I=0, G=+, J=4, P=+, C=+
a_4(1970)0 (M017, Meson, Self-Conjugate, charge 0), MCID 119, I=1, G=-, J=4, P=+, C=+
a_0(980)0 (M036, Meson, Self-Conjugate, charge 0), MCID 9000111, I=1, G=-, J=0, P=+, C=+
f_4(2300)0 (M041, Meson, Self-Conjugate, charge 0), MCID 9010229, I=0, G=+, J=4, P=+, C=+
f_2(2150)0 (M042, Meson, Self-Conjugate, charge 0), MCID 9070225, I=0, G=+, J=2, P=+, C=+
chi_c2(3930) (M050, Meson, Self-Conjugate, charge 0), MCID 100445, I=0, G=+, J=2, P=+, C=+
chi_c1(1P) (M055, Meson, Self-Conjugate, charge 0), MCID 20443, I=0, G=+, J=1, P=+, C=+
chi_c0(1P) (M056, Meson, Self-Conjugate, charge 0), MCID 10441, I=0, G=+, J=0, P=+, C=+
chi_c2(1P) (M057, Meson, Self-Conjugate, charge 0), MCID 445, I=0, G=+, J=2, P=+, C=+
f_2(1430)0 (M066, Meson, Self-Conjugate, charge 0), MCID 9000225, I=0, G=+, J=2, P=+, C=+
f_0(1710)0 (M068, Meson, Self-Conjugate, charge 0), MCID 10331, I=0, G=+, J=0, P=+, C=+
f_J(2220)0 (M082, Meson, Self-Conjugate, charge 0), MCID 9000229, I=0, G=+, J=2++ or 4, P=+, C=+
f_2(2010)0 (M106, Meson, Self-Conjugate, charge 0), MCID 9060225, I=0, G=+, J=2, P=+, C=+
f_2(2300)0 (M107, Meson, Self-Conjugate, charge 0), MCID 9080225, I=0, G=+, J=2, P=+, C=+
f_2(1640)0 (M117, Meson, Self-Conjugate, charge 0), MCID 9020225, I=0, G=+, J=2, P=+, C=+
f_2(1565)0 (M123, Meson, Self-Conjugate, charge 0), MCID 9010225, I=0, G=+, J=2, P=+, C=+
f_2(1950)0 (M135, Meson, Self-Conjugate, charge 0), MCID 9050225, I=0, G=+, J=2, P=+, C=+
f_2(1910)0 (M142, Meson, Self-Conjugate, charge 0), MCID 9040225, I=0, G=+, J=2, P=+, C=+
f_0(1370)0 (M147, Meson, Self-Conjugate, charge 0), MCID 10221, I=0, G=+, J=0, P=+, C=+
a_0(1450)0 (M149, Meson, Self-Conjugate, charge 0), MCID 10111, I=1, G=-, J=0, P=+, C=+
f_0(1500)0 (M152, Meson, Self-Conjugate, charge 0), MCID 9030221, I=0, G=+, J=0, P=+, C=+
chi_c0(3915) (M159, Meson, Self-Conjugate, charge 0), I=0, G=+, J=0, P=+, C=+
a_2(1700)0 (M162, Meson, Self-Conjugate, charge 0), MCID 9000115, I=1, G=-, J=2, P=+, C=+
a_0(1950)0 (M227, Meson, Self-Conjugate, charge 0), I=1, G=-, J=0, P=+, C=+
a_0(1710)0 (M263, Meson, Self-Conjugate, charge 0), I=1, G=-, J=0, P=+, C=+
f_0(1770)0 (M264, Meson, Self-Conjugate, charge 0), I=0, G=+, J=0, P=+, C=+
```

There are lots of interesting ways to filter particle searches, such as selecting particles in a certain mass range.

The database itself contains data beyond individual particle and decay measurements. A lot of this data is hard to classify, so it's often useful to use search queries. For example, if we wanted to learn about constraints on extra dimensions, we might do the following:
```rust,no_run
use pdg_rs::{Pdg, PdgResult};

fn main() -> PdgResult<()> {
    let pdg = Pdg::open()?;

    let Some(result) = pdg
        .search_text("extra dimensions Newtonian")?
        .into_iter()
        .find(|result| result.text.starts_with("This section includes"))
    else {
        return Ok(());
    };
    println!("{}\n", result.text);
    let measurements = pdg.measurements_for(result.pdgid)?;
    for measurement in &measurements {
        println!(
            "{:<25} ({})",
            measurement.reference.document_id,
            measurement.comment.clone().unwrap_or_default()
        );
        for value in &measurement.values {
            if value.value.is_some() {
                println!("Value: {}", value);
            }
            for footnote in &measurement.footnotes {
                println!("{}", footnote.text.clone().unwrap_or_default());
            }
        }
        println!();
    }
    println!("\nReferences:\n");
    for measurement in measurements {
        println!(
            "{:<25}  {}",
            measurement.reference.document_id,
            measurement
                .reference
                .doi
                .clone()
                .or(measurement.reference.inspire_id.clone())
                .or(measurement.reference.title.clone())
                .unwrap_or_default()
        );
    }
    Ok(())
}
```
```text
This section includes limits on the size of extra dimensions from deviations in the Newtonian (1/r**2) gravitational force law at short distances. Deviations are parametrized by a gravitational potential of the form V = -(G m m'/r) [1 + alpha exp(-r/R)]. For delta toroidal extra dimensions of equal size, alpha = 8delta/3. Quoted bounds are for delta = 2 unless otherwise noted.

BLAKEMORE 2021            (Optical levitation)
BLAKEMORE 2021 obtain constraints on non-Newtonian forces with strengths |alpha| ~> E8 and length scales R > 10 mum. See their Fig. 4 for more details including comparison with previous searches.

HEACOCK 2021              (Neutron scattering)
HEACOCK 2021 obtain constraints on non-Newtonian forces with strengths E18~< |alpha|~< E25 and length scales R ~= 0.02 -- 10 nm. See their Figure 3 for more details. This improves the results of HADDOCK 2018. These constraints do not place limits on the size of extra flat dimensions.

LEE 2020                  (Torsion pendulum)
LEE 2020 search for new forces probing a range of |alpha| ~=  and length scales R ~= 7 -- 90 mum. For delta = 1 the bound on R is 30 mum. See their Fig. 5 for details on the bound.

TAN 2020A                 (Torsion pendulum)
Value: <37 micrometers
TAN 2020A search for new forces probing a range of |alpha| ~= 4E-3 -- 1E2 and length scales R ~= 40 -- 350 mum. See their Fig. 6 for details on the bound.

BERGE 2018                (Space accelerometer)
BERGE 2018 uses results from the MICROSCOPE experiment to obtain constraints on non-Newtonian forces with strengths E-11~< |alpha|~< E-7 and length scales R ~>E5 m. See their Figure 1 for more details. These constraints do not place limits on the size of extra flat dimensions.

FAYET 2018A               (Space accelerometer)
FAYET 2018A uses results from the MICROSCOPE experiment to obtain constraints on an EP-violating force possibly arising from a new U(1) gauge boson. For R ~>E7 m the limits are |alpha| ~< a few E-13 to a few E-11 depending on the coupling, corresponding to |epsilon| ~< E-24 for the coupling of the new spin-1 or spin-0 mediator. These constraints do not place limits on the size of extra flat dimensions. This extends the results of FAYET 2018.

KLIMCHITSKAYA 2017A       (Torsion oscillator)
KLIMCHITSKAYA 2017A uses an experiment that measures the difference of Casimir forces to obtain bounds on non-Newtonian forces with strengths |alpha| ~= E5 -- E17 and length scales R = 0.03 -- 10 mum. See their Fig. 3. These constraints do not place limits on the size of extra flat dimensions.

XU 2013                   (Nuclei properties)
XU 2013 obtain constraints on non-Newtonian forces with strengths |alpha| ~= 10**34 -- 10**36 and length scales R ~= 1 -- 10 fm. See their Fig. 4 for more details. These constraints do not place limits on the size of extra flat dimensions.

BEZERRA 2011              (Torsion oscillator)
BEZERRA 2011 obtain constraints on non-Newtonian forces with strengths E11~< |alpha|~< E18 and length scales R = 30 -- 1260 nm. See their Fig. 2 for more details. These constraints do not place limits on the size of extra flat dimensions.

SUSHKOV 2011              (Torsion pendulum)
SUSHKOV 2011 obtain improved limits on non-Newtonian forces with strengths E7~< |alpha| ~< E11 and length scales 0.4 mum < R < 4 mum (95% CL). See their Fig. 2. These bounds do not place limits on the size of extra flat dimensions. However, a model dependent bound of M_{{*}} > 70 TeV is obtained assuming gauge bosons that couple to baryon number also propagate in (4 + delta) dimensions.

BEZERRA 2010              (Microcantilever)
BEZERRA 2010 obtain improved constraints on non-Newtonian forces with strengths E19~< |alpha|~< E29 and length scales R = 1.6 -- 14 nm (95% CL). See their Fig. 1. This bound does not place limits on the size of extra flat dimensions.

MASUDA 2009               (Torsion pendulum)
MASUDA 2009 obtain improved constraints on non-Newtonian forces with strengths E9~<|alpha|~<E11 and length scales R = 1.0 -- 2.9 mum (95% CL). See their Fig. 3. This bound does not place limits on the size of extra flat dimensions.

GERACI 2008               (Microcantilever)
GERACI 2008 obtain improved constraints on non-Newtonian forces with strengths |alpha| > 14,000 and length scales R = 5 -- 15 micrometers. See their Fig. 9. This bound does not place limits on the size of extra flat dimensions.

TRENKEL 2008              (Newton's constant)
TRENKEL 2008 uses two independent measurements of Newton's constant G to constrain new forces with strength |alpha|~=E-4 and length scales R = 0.02 -- 1 m. See their Fig. 1.
This bound does not place limits on the size of extra flat dimensions.

DECCA 2007A               (Torsion oscillator)
DECCA 2007A search for new forces and obtain bounds in the region with strengths |alpha| ~= E13 -- E18 and length scales R = 20 -- 86 nm. See their Fig. 6. This bound does not place limits on the size of extra flat dimensions.

KAPNER 2007               (Torsion pendulum)
Value: <37 micrometers
KAPNER 2007 search for new forces, probing a range of |alpha| ~= 10**-3 -- 10**5 and length scales R ~= 10 -- 1000 mum. For delta = 1 the bound on R is 44 mum. For delta = 2, the bound is expressed in terms of M_{{*}}, here translated to a bound on the radius. See their Fig. 6 for details on the bound.

TU 2007                   (Torsion pendulum)
Value: <47 micrometers
TU 2007 search for new forces probing a range of |alpha| ~= E-1 -- E5 and length scales R ~= 20 -- 1000 mum. For delta = 1 the bound on R is 53 mum. See their Fig. 3 for details on the bound.

SMULLIN 2005              (Microcantilever)
SMULLIN 2005 search for new forces, and obtain bounds in the region with strengths alpha ~= 10**3 -- 10**8 and length scales R = 6 -- 20 mum. See their Figs. 1 and 16 for details on the bound. This work does not place limits on the size of extra flat dimensions.

HOYLE 2004                (Torsion pendulum)
Value: <130 micrometers
HOYLE 2004 search for new forces, probing alpha down to 10**-2 and distances down to 10mum. Quoted bound on R is for delta = 2. For delta = 1, bound goes to 160 mum. See their Fig. 34 for details on the bound.

CHIAVERINI 2003           (Microcantilever)
CHIAVERINI 2003 search for new forces, probing alpha above 10**4 and lambda down to 3mum, finding no signal. See their Fig. 4 for details on the bound. This bound does not place limits on the size of extra flat dimensions.

LONG 2003                 (Microcantilever)
LONG 2003 search for new forces, probing alpha down to 3, and distances down to about 10mum. See their Fig. 4 for details on the bound.

HOYLE 2001                (Torsion pendulum)
Value: <190 micrometers
HOYLE 2001 search for new forces, probing alpha down to 10**-2 and distances down to 20mum. See their Fig. 4 for details on the bound. The quoted bound is for alpha >= 3.

HOSKINS 1985              (Torsion pendulum)
HOSKINS 1985 search for new forces, probing distances down to 4 mm. See their Fig. 13 for details on the bound. This bound does not place limits on the size of extra flat dimensions.


References:

BLAKEMORE 2021             10.1103/PhysRevD.104.L061101
HEACOCK 2021               10.1126/science.abc2794
LEE 2020                   10.1103/PhysRevLett.124.101101
TAN 2020A                  10.1103/PhysRevLett.124.051301
BERGE 2018                 10.1103/PhysRevLett.120.141101
FAYET 2018A                10.1103/PhysRevD.99.055043
KLIMCHITSKAYA 2017A        10.1103/PhysRevD.95.123013
XU 2013                    10.1088/0954-3899/40/3/035107
BEZERRA 2011               10.1103/PhysRevD.83.075004
SUSHKOV 2011               10.1103/PhysRevLett.107.171101
BEZERRA 2010               10.1103/PhysRevD.81.055003
MASUDA 2009                10.1103/PhysRevLett.102.171101
GERACI 2008                10.1103/PhysRevD.78.022002
TRENKEL 2008               10.1103/PhysRevD.77.122001
DECCA 2007A                10.1140/epjc/s10052-007-0346-z
KAPNER 2007                10.1103/PhysRevLett.98.021101
TU 2007                    10.1103/PhysRevLett.98.201101
SMULLIN 2005               10.1103/PhysRevD.72.122001
HOYLE 2004                 10.1103/PhysRevD.70.042004
CHIAVERINI 2003            10.1103/PhysRevLett.90.151101
LONG 2003                  10.1038/nature01432
HOYLE 2001                 10.1103/PhysRevLett.86.1418
HOSKINS 1985               10.1103/PhysRevD.32.3084
```

Data is not always in a consistent form, although that is mostly due to the difficulty of having to account for so many different kinds of data entries. The search feature is intended to be more of a backend to a future TUI which will allow for these sorts of resources to be discovered in a more interactive manner rather than poking around at each `pdgid` entry to see what references/footnotes/values it may relate to.

## Project Status
Currently this is just a working example. I have some plans to expand the interface, but I'm open to outside contributions and ideas, please raise issues or make pull requests if interested. Some things I'm currently considering/working on:

- A TUI interface for handling particle queries and raw data searches with better organization of footnotes and references.
- Rust structs mirroring every table to provide a consistent backend
- Incorporating the JSON data from [here](https://pdg.lbl.gov/current/PDGIdentifiers.json) and [here](https://pdg.lbl.gov/current/PDGIdentifiers-references.json) to add a bit of additional context to the database.
- Automatically downloading and/or dynamically updating the database when a new version of the PDG is released.
- Possibly a Python interface, although I'd have to ensure there is some actual value to it beyond a clone of the [`particle`](https://pypi.org/project/particle/) library. I think right now it could be justified given that `particle` is mainly a curated selection raw particle information while `pdg-rs` incorporates the entire database of measurements and data not specific to any single particle.

## Citations
When using data from `pdg-rs`, cite the PDG release you opened. The default download uses the [2026 Review of Particle Physics](https://pdg.lbl.gov/2026/list_of_RPP_publications.html).

```bibtex
@article{pdg2026,
    author = "Takahashi, F. and others",
    collaboration = "Particle Data Group",
    title = "{Review of particle physics}",
    doi = "10.1142/S0217751X26300115",
    journal = "Int. J. Mod. Phys. A",
    volume = "41",
    number = "22",
    pages = "2630011",
    year = "2026"
}
```


## See also:
The [`particle`](https://pypi.org/project/particle/) library (Python)
