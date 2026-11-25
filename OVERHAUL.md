# pdg-rs overhaul roadmap

Shared particle matching, the flatter `pdg find` CLI, physics filters, typed lookup, edition-aware queries, local database access, lazy text indexing, shell completions, and text search recovery are complete. `pdg tui` uses the same ranked results and detail records as the CLI.

## Commands and detail views

```sh
pdg find 'π⁺'
pdg find 'X(3872)'
pdg find --class meson --mass 900..1100
pdg find --decays-to KS,KS --charge 0
pdg find --text 'form factors'
pdg find --text 'form fact*'
pdg show 'pi+'                    # Particle by canonical name or unambiguous alias
pdg show --mcid -211              # Particle by signed Monte Carlo ID
pdg show M009                     # PDG entry and its particle variants
pdg show S008245                  # A specific data, text, or section entry
pdg show S008M --edition 2024      # Historical numeric data
pdg show f0980 --view mass         # Shell-safe resonance shorthand
pdg show a1320+                   # A specific charge variant
pdg show 'pi+' --view decays
pdg show S008M --view measurements
pdg show S008M --view references
pdg db status
pdg completions zsh
pdg tui f980                      # Interactive particle browser
pdg tui --text 'form factors'    # Interactive text search
```

Use `find` for discovery and `show` for resolving a specific record. `show` accepts a unique close match and lists candidates when the name is ambiguous. JSON for a shared PDG ID lists its variants and sets `particle` to `null`. Give any changes to default output, JSON, and filter semantics a migration note.

The Rust interface supports ranked discovery through `find_particles` and typed resolution through `lookup`.

```rust,ignore
let pdg = Pdg::open_cached()?;
let hits = pdg.find_particles("π⁺", ParticleSearchQuery::new())?;
let pion = pdg.lookup(LookupSelector::Mcid(211))?;
```

`lookup` reports `Found`, `Ambiguous`, or `Missing`. Search hits can be detached with `into_owned()`. A resolved particle supports `details(ParticleDetailSelection::Properties(&[DataType::Mass]))` for an owned property record. The CLI uses `Pdg::entry_details` and `EntryDetailView` for its named views. The TUI uses the same loader.

## Ranked backlog

Rank reflects usefulness and dependencies. **Small** means a localized change. **Medium** means an interface change or work across a few modules. **Large** means several pieces of new behavior that need to be designed and tested together.

| Rank | Improvement | Usefulness | Difficulty | Scope |
| --- | --- | --- | --- | --- |
| 1 | **Complete in `d686c72`.** Share ranked particle search across interfaces | Very high | Medium | The CLI uses `Pdg::find_particles` alongside the existing physics filters. Search recognizes canonical names, PDG aliases and redirects, and common names and notation. Exact and partial matches rank ahead of bounded typo matches from `strsim`. Hits include the canonical identity, matched spelling, and match reason. [Shared matcher](src/particle_search.rs#L57) |
| 2 | **Complete in `524dd46`.** Flatten the CLI | Very high | Medium | Use `pdg find QUERY` for particles, `pdg find --text QUERY` for PDG prose, and `pdg find --class lepton` for filter-only searches. Root help includes runnable examples. `find` replaces the nested `search` commands. [CLI commands](src/main.rs#L232) |
| 3 | **Complete in `eecbec4`.** Resolve names, MCIDs, and PDG IDs in `show` | Very high | Medium | `show` uses typed lookup to resolve exact names and aliases, signed MCIDs, and PDG database IDs. Shared IDs list all variants. Ambiguous names and close matches produce diagnostics with candidate IDs. [Typed lookup](src/lookup.rs#L35) |
| 4 | **Complete in `fb9096c`.** Make physics filters trustworthy | Very high | Medium | MCID combines with name and property filters. Ranges require a comparable overlapping interval, with `--include-unknown-properties` for exploratory searches. Rust and CLI reject reversed bounds and NaN. Both outgoing decay filters match distinct product occurrences within one mode. [Particle search](src/lib.rs#L418) |
| 5 | **Complete in `1a7fc65`.** Add a small high-level Rust interface | High | Medium | Ranked and filter hits support `into_owned()`. `PdgParticle::details` returns an owned overview, selected properties, or selected properties with measurements and references. The records are serializable and preserve units, uncertainties, PDG IDs, edition, and provenance. [Particle details](src/particle_details.rs) |
| 6 | **Complete in `5da493a`.** Honor the opened database's edition | High | Medium | Numeric queries default to the opened file's declared edition. The default download is pinned to `pdgall-2026.0.sqlite`. `--edition` and `Pdg::with_edition` select a historical edition with numeric data. Properties, range filters, and branching values use that selection. `db status` shows the file release and available editions, and `show` JSON includes the selected edition and release metadata. [Database metadata](src/database.rs#L24) |
| 7 | **Complete in `4b54231`.** Make local and offline database access predictable | High | Medium | `--db-path` takes precedence over `PDG_RS_DB_PATH`, and `db path` and `db status` report the selected file. Databases open read-only with path-specific file and schema errors. `db fetch` honors offline mode, and `db clear` targets the pinned cache. [Read-only open](src/database.rs#L85) |
| 8 | **Complete in `89aa5a3`.** Improve help, completion, and the quickstart | High | Small | Root help shows particle, text, and decay examples. `pdg completions` generates scripts from the CLI definition, including canonical filter values. The README covers loading completions and using typed lookup and ranked search offline in Rust. [Shell completion](README.md#shell-completion), [Rust quickstart](README.md#rust-quickstart) |
| 9 | **Complete in `a3a1de5`.** Make detail views and output formats agree | High | Medium | `pdg show` defaults to an overview and offers mass, decays, measurements, references, and full views. Terminal and JSON render the same library-loaded sections. Both formats support `--related-details`, and redirected output has no ANSI styles. [Entry details](src/entry_details.rs) |
| 10 | **Complete in `d5f0fcb`.** Improve discovery of PDG prose | Medium–high | Medium | Text search supports explicit prefixes and tries a last-word prefix or a single-word correction when an exact query finds nothing. Terminal and JSON output show the hit's source, parent entry, and any spelling correction. Named views for mass and decays are part of the detail-view work. [Text search](src/text_search.rs#L11) |
| 11 | **Complete in `1651d17`.** Make search bounded and startup lighter | Medium–high | Medium | Build the temporary text index on the first text query and reuse it for the connection. SQLite applies text limits after ranking. Particle details load after result truncation. [Bounded text search](src/text_search.rs#L39) |
| 12 | **Complete in `b1d4fdc` and `fe0284b`.** Make decay queries easier to write and interpret | Medium–high | Medium | Inclusive decay filters resolve exact aliases and normalized particle notation. Rust search hits, terminal output, and JSON expose the matching PDG channel and its recorded description, including qualifiers on family-expanded matches. [Decay search](src/lib.rs) |
| 13 | **Initial browser complete in `8ef95da`.** Build an optional TUI for browsing | Medium | Large | `pdg tui` provides live particle and text search, a result list with a preview, entry detail views, back navigation, and clipboard shortcuts. The browser shares CLI ranking, lookup, edition selection, and offline behavior. It is enabled by default through the optional `tui` feature. [Terminal browser](src/tui.rs) |
| 14 | **Complete in `5f75cc3`.** Add physics filters to the TUI | High | Medium | Advanced search offers selectable class, type, charge, spin, and decay mode, plus editable mass and decay products. The browser shares CLI filter values and query construction, reuses physics matches while ranking name searches, and opens the recorded PDG channels. [Filter panel](src/tui/filters.rs) |
| 15 | **Complete in `a66700d`.** Copy a TUI search as a CLI command | Medium–high | Small | F3 copies the active query, filters, database options, and result limit as a shell-quoted `pdg find` command. [Command export](src/tui/command.rs) |
| 16 | **Complete in `73966d1`.** Follow linked PDG entries in the TUI | Medium–high | Small | Links lists the entry's parent, children, and related IDs. Enter follows a link, and Esc restores each prior view, including its selection and scroll position. [Entry details](src/tui/detail.rs) |
| 17 | **Complete in `a44dfbb`.** Add tabs to TUI detail views | Medium–high | Small | Available views are numbered consecutively for each entry. Clicking a tab, pressing its number, or cycling with Tab and Shift+Tab switches views directly. Direct mass and decay records show their recorded values in Overview. [Entry details](src/tui/detail.rs) |
| 18 | **Complete in `659da3c`.** Add TUI help and search shortcuts | High | Small | `?` shows keyboard and mouse controls, Ctrl+F opens advanced particle search, and `/` returns to the query from detail views. [Help screen](src/tui/help.rs) |
| 19 | **Complete in `28e80cc`.** Sort particle search results by mass | High | Medium | `pdg find --sort mass` and Ctrl+O in the TUI order matches by mass before applying the result limit. Entries without a comparable mass sort last. The copied search command includes the selected sort. [Shared ordering](src/lib.rs) |
| 20 | **Complete in `ad0f3e3`.** Support all particle filters in the TUI | High | Medium | Advanced filters include parity, C-parity, G-parity, isospin, signed MCID, width, lifetime, incoming decay states, unknown numeric data, and decay expansion. The form and copied commands share the CLI filter parser. [Filter panel](src/tui/filters.rs) |
| 21 | **Complete in `7ba966d`.** Add decay-state autocomplete to the TUI | High | Medium | Products and Decays from suggest particle names and PDG groups as you type. Down and Enter replace the current comma-separated entry. Literal mode suggests spellings recorded on the corresponding side of the decay. [Filter panel](src/tui/filters.rs) |
| 22 | **Proposed.** Jump directly to an entry in the TUI | High | Small–medium | Add a Go to prompt for PDG IDs and signed MCIDs. Reuse typed lookup, open the resolved detail view, and let Esc return to the previous entry or search. |
| 23 | **Complete in `bc7e334`.** Enter advanced filters in the search bar | High | Medium | Particle search accepts `field:value` tags with optional name text. Tab completes tags and values, Ctrl+F edits those same filters, and F3 exports the equivalent CLI command. [Inline queries](src/tui/query.rs) |

Particle lookup and inclusive decay filters accept shell-safe resonance names and unique close matches (`5c4b696`). A shorthand naming one PDG listing can select its charge variants. Distinct candidate listings remain ambiguous.

## Verified findings

The original audit used the bundled 2025 SQLite database. The all-features run passed 101 tests and the no-default-features run passed 99. The probes below record behavior at the time of that audit.

| Probe | Current behavior |
| --- | --- |
| `pdg show 'pi+'` | Exits with `NotFound("pi+")`. |
| `pdg show M009 --summary --format json` | Shows only `rho(770)0`. The database also has `rho(770)+` and `rho(770)-` under `M009`. The bundled database contains 290 particle PDG IDs shared by multiple rows. |
| Exact name `P_c_cbar(4312)+` in the database | Appears under both `B185` and `B205`, both without an MCID. The current `Pdg::particle` returns one row. |
| `pdg search particles 'X(3872)'` and `pdg search particles KS0` | No results even though the database maps those names to `chi_c1(3872)` and `K(S)0`. The search also misses `π⁺`, `K_S^0`, and `prtoon`. |
| `pdg search text 'form factros'` | No results. `form factors` finds the pion form-factor section. |
| `pdg search particles --mcid 211 --charge 0` | Returns the positively charged `pi+`. The MCID path skips the filter builder. |
| `pdg search particles 'pi+' --width 999999..1000000` | Returns `pi+` with a blank width. Missing numeric data currently passes the check, and an existing test expects a particle with missing width to appear in a width search. Changing the default needs release notes. [Width test](src/models/pdgparticle.rs#L1605) |
| `pdg search particles 'pi+' --decay-contains 'mu+,mu+' --decay-expansion literal` | Returns `pi+`. In the bundled listing for `S008`, every outgoing `mu+` row has multiplier one and each belongs to a different mode. |
| `pdg show S008 --related-details --format json` | Output is byte-for-byte identical with and without `--related-details`. |
| `pdg show S008M --summary --format json` with a 2024-only fixture | Returns no numeric rows. A temporary copy with 2024 metadata and 2025 data removed still contained two 2024 `S008M` rows. |
| `pdg --offline show S008` with `PDG_RS_DB_PATH` set to a nonexistent file | Creates an empty SQLite file at that path and reports `no such table: pdgid`. The probe used a temporary directory. |

Source inspection also showed that `--offline` reaches `show` and `search`, while `db fetch` consults `PDG_RS_OFFLINE` through the cache code and bypasses the CLI flag. I did not run a live database download or measure startup times. [CLI dispatch](src/main.rs#L420), [cache policy](src/database.rs#L22)

## CLI timing

Median elapsed times from nine paired runs per command using debug builds and the bundled 2025 database on this machine. Each invocation opened a new connection with the file already in the OS cache.

| Command | Before | After |
| --- | ---: | ---: |
| `pdg show 'pi+' --summary --format json` | 476 ms | 99 ms |
| `pdg find p --limit 1 --format json` | 440 ms | 66 ms |
| `pdg find --text decay --limit 5 --format json` | 362 ms | 346 ms |
| `pdg find --text decay --all --format json` | 412 ms | 416 ms |

## Release sequence

1. **Correctness and identity.** Fix MCID composition, numeric and decay filter semantics, shared IDs, edition handling, and explicit database opening. Add targeted tests using the bundled data and a small older-edition fixture.
2. **Search and CLI refresh.** Ship the shared matcher, `find`, and name-aware `show` together. Cover exact names and aliases, real typos, Unicode notation, charge and antiparticle preservation, and ambiguity with a small query corpus. Use the same cases for Rust and CLI tests. Document command and output migrations.
3. **Detail and browsing.** Share selected detail records and renderers' inputs, improve prose search and query responsiveness, and then build `pdg tui` around those interfaces.

Keep Python bindings, full table-by-table Rust models, and automatic edition updates for a later decision. The current typed physics filters, measurement details, FTS text sources, and verified database cache already provide the capabilities needed for the first releases.
