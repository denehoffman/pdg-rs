use std::{
    collections::HashSet,
    env,
    ffi::OsStr,
    fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
    sync::OnceLock,
};

use clap::{
    CommandFactory, Parser, Subcommand, ValueEnum, ValueHint,
    builder::{PossibleValue, TypedValueParser},
};
use clap_complete::{Shell, generate};
use comfy_table::{ColumnConstraint, ContentArrangement, Width};
use owo_colors::{OwoColorize, Style};
use pdg_rs::{
    AngularMomentum, BranchingFraction, BranchingRatio, Charge, DataEntry, DecayStateExpansion,
    EntryContent, EntryDetailView, EntryDetails, Isospin, Lookup, LookupSelector, Parity,
    ParticleClass, ParticleFilterHit, ParticlePropertyDetail, ParticleSearchQuery, ParticleType,
    Pdg, PdgError, PdgFootnote, PdgIdEntry, PdgMeasurement, PdgMeasurementValue, PdgParticle,
    PdgReference, PdgText, TextSearchResult, TextSearchSource,
};
use thiserror::Error;

pub(crate) mod cli;
use cli::{ParticleDto, ShowDto, TextSearchDto, particle_table, table};

#[derive(Debug, Error)]
enum CliError {
    #[error(transparent)]
    Pdg(#[from] PdgError),
    #[error("{0}")]
    Lookup(String),
    #[error("{0}")]
    InvalidArgument(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

type CliResult<T> = Result<T, CliError>;

trait PrettyString {
    fn pretty_string(&self) -> String;
}

trait PrettyMeasurementValue: PrettyString {
    fn pretty_detail_lines(&self) -> Vec<String>;
}

trait CliStringExt {
    fn to_pdgid_string(&self) -> String;
    fn to_value_string(&self) -> String;
    fn to_link_string(&self) -> String;
    fn to_field_string(&self) -> String;
    fn to_title_string(&self) -> String;
}

fn styled(text: &str, style: Style) -> String {
    static COLOR: OnceLock<bool> = OnceLock::new();
    if *COLOR.get_or_init(|| io::stdout().is_terminal() && env::var_os("NO_COLOR").is_none()) {
        text.style(style).to_string()
    } else {
        text.to_owned()
    }
}

impl CliStringExt for str {
    fn to_pdgid_string(&self) -> String {
        styled(self, Style::new().magenta().bold())
    }

    fn to_value_string(&self) -> String {
        styled(self, Style::new().yellow())
    }

    fn to_link_string(&self) -> String {
        styled(self, Style::new().blue().underline())
    }

    fn to_field_string(&self) -> String {
        styled(self, Style::new().cyan())
    }

    fn to_title_string(&self) -> String {
        styled(self, Style::new().bold())
    }
}

impl PrettyString for DataEntry<'_> {
    fn pretty_string(&self) -> String {
        self.to_string().to_value_string()
    }
}

impl PrettyString for PdgMeasurementValue {
    fn pretty_string(&self) -> String {
        let mut parts = vec![self.to_string().to_value_string()];
        if let Some(column_name) = self
            .column_name
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            parts.push(column_name.to_field_string());
        }
        if let Some(limit_type) = self.limit_type {
            parts.push(limit_type.to_string().to_field_string());
        }
        if self.used_in_average {
            parts.push(styled("used in average", Style::new().green()));
        }
        if self.used_in_fit {
            parts.push(styled("used in fit", Style::new().green()));
        }
        parts.join(" | ")
    }
}

impl PrettyMeasurementValue for PdgMeasurementValue {
    fn pretty_detail_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        push_optional_line(
            &mut lines,
            "Value",
            self.value.map(|value| value.to_string()),
        );
        push_optional_line(
            &mut lines,
            "Error +",
            self.error_positive.map(|value| value.to_string()),
        );
        push_optional_line(
            &mut lines,
            "Error -",
            self.error_negative.map(|value| value.to_string()),
        );
        push_optional_line(
            &mut lines,
            "Stat error +",
            self.stat_error_positive.map(|value| value.to_string()),
        );
        push_optional_line(
            &mut lines,
            "Stat error -",
            self.stat_error_negative.map(|value| value.to_string()),
        );
        push_optional_line(
            &mut lines,
            "Syst error +",
            self.syst_error_positive.map(|value| value.to_string()),
        );
        push_optional_line(
            &mut lines,
            "Syst error -",
            self.syst_error_negative.map(|value| value.to_string()),
        );
        push_optional_line(&mut lines, "Unit", self.unit_text.as_deref());
        lines
    }
}

fn reference_lines(reference: &PdgReference) -> Vec<String> {
    let mut lines = vec![reference.document_id.trim().to_title_string()];
    push_optional_line(&mut lines, "Title", reference.title.as_deref());
    push_optional_line(
        &mut lines,
        "Year",
        reference.publication_year.map(|year| year.to_string()),
    );
    push_optional_line(
        &mut lines,
        "Publication",
        reference.publication_name.as_deref(),
    );
    if let Some(doi) = &reference.doi {
        push_optional_line(
            &mut lines,
            "DOI",
            Some(format!("https://doi.org/{doi}").to_link_string()),
        );
    }
    if let Some(inspire_id) = &reference.inspire_id {
        push_optional_line(
            &mut lines,
            "INSPIRE",
            Some(format!("https://inspirehep.net/literature/{inspire_id}").to_link_string()),
        );
    }
    lines
}

impl PrettyString for PdgMeasurement {
    fn pretty_string(&self) -> String {
        let mut lines = reference_lines(&self.reference);
        push_optional_line(&mut lines, "Technique", self.technique.as_deref());
        push_optional_line(&mut lines, "Event count", self.event_count.as_deref());
        push_optional_line(
            &mut lines,
            "Confidence level",
            self.confidence_level.map(|value| value.to_string()),
        );
        push_optional_line(&mut lines, "Charge", self.charge.as_deref());
        push_optional_line(&mut lines, "Comment", self.comment.as_deref());

        if !self.values.is_empty() {
            lines.push(format!("  {}", "Values:".to_field_string()));
            for value in &self.values {
                lines.push(format!("    {}", value.pretty_string()));
                for detail in value.pretty_detail_lines() {
                    lines.push(format!("      {detail}"));
                }
            }
        }

        if !self.footnotes.is_empty() {
            lines.push(format!("  {}", "Footnotes:".to_field_string()));
            for footnote in &self.footnotes {
                let index = footnote
                    .index
                    .map(|index| index.to_string())
                    .unwrap_or_default();
                lines.push(format!(
                    "    [{}] {}",
                    index,
                    footnote.text.clone().unwrap_or_default()
                ));
            }
        }

        lines.join("\n")
    }
}

fn push_optional_line<T: ToString>(lines: &mut Vec<String>, label: &str, value: Option<T>) {
    if let Some(value) = value {
        let value = value.to_string();
        if !value.is_empty() {
            lines.push(format!("  {}: {}", label.to_field_string(), value));
        }
    }
}

#[derive(Parser)]
#[command(
    name = "pdg",
    version,
    about = "Search the Particle Data Group database",
    after_help = "Examples:\n  pdg find 'π⁺'\n  pdg find --class lepton --limit 5\n  pdg find --decays-to KS,KS --charge 0\n  pdg find --text 'form factors'\n  pdg show 'pi+'\n  pdg completions zsh"
)]
struct Cli {
    #[command(flatten)]
    database: DatabaseOptions,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Parser)]
struct DatabaseOptions {
    /// Disable database downloads.
    #[arg(long, global = true)]
    offline: bool,
    /// Open this local SQLite file in place of the default database.
    #[arg(long, global = true, value_name = "FILE", value_hint = ValueHint::FilePath)]
    db_path: Option<PathBuf>,
}

impl DatabaseOptions {
    fn override_path(&self) -> Option<PathBuf> {
        self.db_path
            .clone()
            .or_else(|| env::var_os("PDG_RS_DB_PATH").map(PathBuf::from))
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Find particles by name and properties, or search PDG text.
    Find(Box<FindCommand>),
    /// Look up a particle or PDG database entry.
    Show(ShowCommand),
    /// Manage the cached PDG database.
    Db(DbCommand),
    /// Generate a shell completion script.
    Completions {
        /// Shell to generate the script for.
        #[arg(value_enum)]
        shell: Shell,
    },
    /// Reserved for the future terminal UI.
    Tui,
}

#[derive(Parser)]
struct DbCommand {
    #[command(subcommand)]
    command: DbCommands,
}

#[derive(Subcommand)]
enum DbCommands {
    /// Show the selected database source, path, and edition.
    Status,
    /// Make the selected database available locally and print its path.
    Fetch,
    /// Print the effective database path.
    Path,
    /// Remove the default cached database.
    Clear,
}

#[derive(Parser)]
#[command(
    after_help = "Examples:\n  pdg find 'X(3872)'\n  pdg find --class meson --mass 900..1100\n  pdg find --text 'form factors'"
)]
struct FindCommand {
    /// Search PDG descriptions, text, and footnotes using the query.
    #[arg(long, requires = "query", conflicts_with = "particle_filters")]
    text: bool,
    /// Print the full matching text below each snippet.
    #[arg(long, requires = "text")]
    show_full_text: bool,
    /// Use an edition available in this file for numeric properties, such as 2024.
    #[arg(long, conflicts_with = "text")]
    edition: Option<String>,
    /// Search query. Particle names support aliases, common notation, and typos.
    query: Option<String>,
    #[command(flatten, next_help_heading = "Output options")]
    output: SearchOutput,
    #[command(flatten, next_help_heading = "Particle filters")]
    filters: ParticleFilters,
}

#[derive(Parser)]
#[command(
    after_help = "Examples:\n  pdg show 'pi+'\n  pdg show --mcid -211 --view mass\n  pdg show M009 --view decays\n  pdg show S008M --view measurements"
)]
struct ShowCommand {
    /// PDG database ID, particle name, or exact alias.
    #[arg(required_unless_present = "mcid")]
    target: Option<String>,
    /// Look up a particle by signed Monte Carlo ID.
    #[arg(long, allow_hyphen_values = true, conflicts_with = "target")]
    mcid: Option<isize>,
    /// Use an edition available in this file for numeric data, such as 2024.
    #[arg(long)]
    edition: Option<String>,
    #[command(flatten)]
    output: ShowOutput,
}

#[allow(clippy::doc_markdown)]
#[derive(Parser, Clone)]
#[group(id = "particle_filters")]
struct ParticleFilters {
    /// Filter by particle class.
    #[arg(long, value_parser = SuggestedValues(parse_class, &["gauge-boson", "lepton", "quark", "meson", "baryon"]))]
    class: Option<ParticleClass>,
    /// Filter by particle/antiparticle/self-conjugate type.
    #[arg(long = "type", value_parser = SuggestedValues(parse_particle_type, &["particle", "antiparticle", "self-conjugate"]))]
    particle_type: Option<ParticleType>,
    /// Filter by electric charge.
    #[arg(long, allow_hyphen_values = true, value_parser = SuggestedValues(parse_charge, &["-2", "-1", "-2/3", "-1/3", "0", "+1/3", "+2/3", "+1", "+2"]))]
    charge: Option<Charge>,
    /// Filter by signed Monte Carlo ID.
    #[arg(long, allow_hyphen_values = true)]
    mcid: Option<isize>,
    /// Filter by mass range in MeV, formatted as MIN..MAX.
    #[arg(long, value_parser = parse_range)]
    mass: Option<(f64, f64)>,
    /// Filter by isospin. The photon matches both 0 and 1.
    #[arg(long, value_parser = SuggestedValues(parse_isospin_filter, &["0", "1/2", "1", "3/2", "unknown", "missing"]))]
    isospin: Option<QuantumArg<Isospin>>,
    /// Filter by G parity.
    #[arg(long, value_parser = SuggestedValues(parse_parity_filter, PARITY_VALUES))]
    g_parity: Option<QuantumArg<Parity>>,
    /// Filter by spin/angular momentum. Custom PDG spin strings are also accepted.
    #[arg(long, hide_possible_values = true, value_parser = SuggestedValues(parse_spin_filter, SPIN_VALUES))]
    spin: Option<QuantumArg<AngularMomentum>>,
    /// Filter by parity.
    #[arg(long, value_parser = SuggestedValues(parse_parity_filter, PARITY_VALUES))]
    parity: Option<QuantumArg<Parity>>,
    /// Filter by charge conjugation parity.
    #[arg(long, value_parser = SuggestedValues(parse_parity_filter, PARITY_VALUES))]
    c_parity: Option<QuantumArg<Parity>>,
    /// Filter by width range in MeV, formatted as MIN..MAX.
    #[arg(long, value_parser = parse_range)]
    width: Option<(f64, f64)>,
    /// Filter by lifetime range in seconds, formatted as MIN..MAX.
    #[arg(long, value_parser = parse_range)]
    lifetime: Option<(f64, f64)>,
    /// Include particles with no comparable value for a requested property range.
    #[arg(long)]
    include_unknown_properties: bool,
    /// Filter particles with an exact decay to comma-separated final states, such as KS,KS.
    #[arg(long, value_delimiter = ',')]
    decays_to: Vec<String>,
    /// Filter particles with a decay containing comma-separated final states.
    #[arg(long, value_delimiter = ',')]
    decay_contains: Vec<String>,
    /// Filter particles which can be produced from comma-separated initial states.
    #[arg(long, value_delimiter = ',')]
    decays_from: Vec<String>,
    /// Choose alias and PDG family matching or exact recorded decay-product spellings.
    #[arg(long, value_enum, default_value_t = DecayExpansionArg::Inclusive)]
    decay_expansion: DecayExpansionArg,
}

#[derive(Parser, Clone)]
struct SearchOutput {
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Pretty)]
    format: OutputFormat,
    /// Maximum number of results to print. Searches with a query default to 20.
    #[arg(long)]
    limit: Option<usize>,
    /// Print all matching results.
    #[arg(long)]
    all: bool,
}

impl SearchOutput {
    fn result_limit(&self, default: Option<usize>) -> Option<usize> {
        if self.all {
            None
        } else {
            self.limit.or(default)
        }
    }
}

#[derive(Parser, Clone, Copy)]
struct ShowOutput {
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Pretty)]
    format: OutputFormat,
    /// Select which details to show. Defaults to overview.
    #[arg(long, value_enum)]
    view: Option<ShowView>,
    /// Expand linked and child entries. Implies --view full.
    #[arg(long)]
    related_details: bool,
}

#[derive(Clone, Copy, ValueEnum, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
enum ShowView {
    /// Headline properties and direct data.
    Overview,
    /// Mass and its PDG source.
    Mass,
    /// Branching fractions and ratios.
    Decays,
    /// Measurements for the entry and its properties or section data.
    Measurements,
    /// Unique publications supporting those measurements.
    References,
    /// Text, data, decay listings, measurements, and linked entry IDs.
    Full,
}

impl ShowOutput {
    fn selection(self) -> CliResult<(ShowView, EntryDetailView)> {
        let view = self.view.unwrap_or(if self.related_details {
            ShowView::Full
        } else {
            ShowView::Overview
        });
        let selection = match view {
            ShowView::Full => EntryDetailView::Full {
                related: self.related_details,
            },
            _ if self.related_details => {
                return Err(CliError::InvalidArgument(
                    "--related-details is only available with --view full".into(),
                ));
            }
            ShowView::Overview => EntryDetailView::Overview,
            ShowView::Mass => EntryDetailView::Mass,
            ShowView::Decays => EntryDetailView::Decays,
            ShowView::Measurements => EntryDetailView::Measurements,
            ShowView::References => EntryDetailView::References,
        };
        Ok((view, selection))
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Pretty,
    Json,
}

#[derive(Clone, Copy, ValueEnum)]
enum DecayExpansionArg {
    /// Resolve exact aliases and include related PDG decay states.
    Inclusive,
    /// Match the product spellings recorded in the database.
    Literal,
}

impl From<DecayExpansionArg> for DecayStateExpansion {
    fn from(value: DecayExpansionArg) -> Self {
        match value {
            DecayExpansionArg::Inclusive => Self::Inclusive,
            DecayExpansionArg::Literal => Self::Literal,
        }
    }
}

#[derive(Clone)]
enum QuantumArg<T> {
    Missing,
    Value(T),
}

const PARITY_VALUES: &[&str] = &["+", "-", "unknown", "missing"];
const SPIN_VALUES: &[&str] = &[
    "0", "1/2", "1", "3/2", "2", "5/2", "3", "7/2", "4", "9/2", "5", "11/2", "6", "13/2", "7",
    "15/2", "unknown", "missing",
];

// Give Clap and completion generators canonical spellings while preserving the parser's aliases.
#[derive(Clone)]
struct SuggestedValues<P>(P, &'static [&'static str]);

impl<P: TypedValueParser> TypedValueParser for SuggestedValues<P> {
    type Value = P::Value;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &OsStr,
    ) -> Result<Self::Value, clap::Error> {
        self.0.parse_ref(cmd, arg, value)
    }

    fn possible_values(&self) -> Option<Box<dyn Iterator<Item = PossibleValue> + '_>> {
        Some(Box::new(self.1.iter().copied().map(PossibleValue::new)))
    }
}

fn main() -> ExitCode {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn run() -> CliResult<()> {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        return print_help(None);
    };

    match command {
        Commands::Find(command) => run_find(&command, &cli.database),
        Commands::Show(command) => run_show(&command, &cli.database),
        Commands::Db(command) => run_db(&command, &cli.database),
        Commands::Completions { shell } => {
            let mut script = Vec::new();
            generate(shell, &mut Cli::command(), "pdg", &mut script);
            io::stdout().lock().write_all(&script)?;
            Ok(())
        }
        Commands::Tui => Err(CliError::InvalidArgument(
            "TUI is not implemented yet. Use `pdg find`, `pdg show`, or `pdg db`.".into(),
        )),
    }
}

fn print_help(subcommand: Option<&str>) -> CliResult<()> {
    let mut root = Cli::command();
    root.build();
    let mut command = &mut root;
    if let Some(name) = subcommand {
        command = command
            .find_subcommand_mut(name)
            .expect("help must name a CLI subcommand");
    }
    command.print_help()?;
    println!();
    Ok(())
}

fn open_pdg(database: &DatabaseOptions, edition: Option<&str>) -> CliResult<Pdg> {
    let db = if let Some(path) = database.override_path() {
        Pdg::open_path(path)?
    } else if database.offline {
        Pdg::open_cached()?
    } else {
        Pdg::open()?
    };
    Ok(match edition {
        Some(edition) => db.with_edition(edition)?,
        None => db,
    })
}

fn run_db(command: &DbCommand, database: &DatabaseOptions) -> CliResult<()> {
    match command.command {
        DbCommands::Status => {
            if let Some(path) = database.override_path() {
                let db = Pdg::open_path(&path)?;
                let source = if database.db_path.is_some() {
                    "database override (--db-path)"
                } else {
                    "database override"
                };
                return print_database_status(source, &path, &db);
            }

            let path = Pdg::cached_database_path()?;
            match Pdg::open_cached() {
                Ok(db) => print_database_status("cached database", &path, &db),
                Err(PdgError::OfflineDatabaseMissing(_)) => {
                    println!("database not cached: {}", path.display());
                    Ok(())
                }
                Err(error) => Err(error.into()),
            }
        }
        DbCommands::Fetch => {
            let path = if let Some(path) = database.override_path() {
                Pdg::open_path(&path)?;
                path
            } else if database.offline {
                Pdg::open_cached()?;
                Pdg::cached_database_path()?
            } else {
                Pdg::ensure_database()?
            };
            println!("{}", path.display());
            Ok(())
        }
        DbCommands::Path => {
            let path = database
                .override_path()
                .map_or_else(Pdg::cached_database_path, Ok)?;
            println!("{}", path.display());
            Ok(())
        }
        DbCommands::Clear => {
            if database.db_path.is_some() {
                return Err(CliError::InvalidArgument(
                    "db clear operates on the default cache. Omit --db-path.".into(),
                ));
            }
            let path = Pdg::cached_database_path()?;
            match fs::remove_file(&path) {
                Ok(()) => println!("removed {}", path.display()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    println!("database not cached: {}", path.display());
                }
                Err(error) => return Err(error.into()),
            }
            Ok(())
        }
    }
}

fn print_database_status(source: &str, path: &Path, db: &Pdg) -> CliResult<()> {
    let info = db.database_info();
    let editions = db.available_editions()?;
    println!("{source}: {}", path.display());
    println!("database edition: {}", info.edition);
    if let Some(schema_version) = &info.schema_version {
        println!("schema version: {schema_version}");
    }
    if let Some(release) = &info.data_release_timestamp {
        println!("data release timestamp: {release}");
    } else if let Some(release) = &info.data_release {
        println!("data release: {release}");
    }
    println!("available numeric editions: {}", editions.join(", "));
    Ok(())
}

const DEFAULT_SEARCH_LIMIT: usize = 20;

fn run_find(command: &FindCommand, database: &DatabaseOptions) -> CliResult<()> {
    let query = command.query.as_deref();
    if let (true, Some(query)) = (command.text, query) {
        return run_text(query, &command.output, command.show_full_text, database);
    }
    if query.is_none() && !has_particle_filters(&command.filters) {
        return print_help(Some("find"));
    }

    let filters = build_query(&command.filters)?;
    let db = open_pdg(database, command.edition.as_deref())?;
    let mut hits = if let Some(query) = query {
        db.find_particles(query, filters)?
            .into_iter()
            .map(|hit| ParticleFilterHit {
                particle: hit.particle,
                matching_decays: hit.matching_decays,
            })
            .collect()
    } else {
        db.search_particle_hits(filters)?
    };

    let default_limit = query.is_some().then_some(DEFAULT_SEARCH_LIMIT);
    if let Some(limit) = command.output.result_limit(default_limit) {
        hits.truncate(limit);
    }
    output_particles(&hits, &command.output)
}

fn run_text(
    query: &str,
    output: &SearchOutput,
    show_full_text: bool,
    database: &DatabaseOptions,
) -> CliResult<()> {
    let db = open_pdg(database, None)?;
    let results =
        db.search_text_with_limit(query, output.result_limit(Some(DEFAULT_SEARCH_LIMIT)))?;
    match output.format {
        OutputFormat::Pretty => {
            print_text_results(&results, show_full_text);
            Ok(())
        }
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &results.iter().map(TextSearchDto::from).collect::<Vec<_>>()
                )?
            );
            Ok(())
        }
    }
}

fn run_show(command: &ShowCommand, database: &DatabaseOptions) -> CliResult<()> {
    let (view, selection) = command.output.selection()?;
    let db = open_pdg(database, command.edition.as_deref())?;
    let (target, lookup) = if let Some(mcid) = command.mcid {
        (
            format!("MCID {mcid}"),
            db.lookup(LookupSelector::Mcid(mcid))?,
        )
    } else {
        let target = command
            .target
            .as_deref()
            .expect("the CLI requires a lookup selector");
        let lookup = match db.lookup(LookupSelector::PdgId(target))? {
            Lookup::Missing { .. } => db.lookup(LookupSelector::Name(target))?,
            found => found,
        };
        (format!("{target:?}"), lookup)
    };

    let (entry, particles) = match lookup {
        Lookup::Found { entry, particles } => (entry, particles),
        Lookup::Ambiguous(particles) => {
            let choices = particles
                .iter()
                .map(particle_choice)
                .collect::<Vec<_>>()
                .join("\n");
            return Err(CliError::Lookup(format!(
                "{target} matches multiple particles\n{choices}"
            )));
        }
        Lookup::Missing { suggestions } => {
            let mut message = format!("no PDG entry or particle matched {target}");
            if !suggestions.is_empty() {
                message.push_str("\nClose matches\n");
                let choices = suggestions
                    .iter()
                    .map(|hit| particle_choice(&hit.particle))
                    .collect::<Vec<_>>()
                    .join("\n");
                message.push_str(&choices);
            }
            return Err(CliError::Lookup(message));
        }
    };
    let mut details = db.entry_details(&entry, selection)?;
    details.related_particles.retain(|related| {
        !particles
            .iter()
            .any(|particle| related.pdgid == particle.pdgid && related.name == particle.name)
    });
    match command.output.format {
        OutputFormat::Pretty => print_show(&db, &entry, &particles, view, &details),
        OutputFormat::Json => {
            let dto = ShowDto::new(&db, &entry, &particles, view, &details);
            println!("{}", serde_json::to_string_pretty(&dto)?);
            Ok(())
        }
    }
}

fn particle_choice(particle: &PdgParticle<'_>) -> String {
    let mcid = particle
        .mcid
        .map_or_else(String::new, |mcid| format!(", MCID {mcid}"));
    format!("  {} (PDG ID {}{mcid})", particle.name, particle.pdgid)
}

fn output_particles(hits: &[ParticleFilterHit<'_>], output: &SearchOutput) -> CliResult<()> {
    match output.format {
        OutputFormat::Pretty => print_particle_hits(hits),
        OutputFormat::Json => {
            let dtos = hits
                .iter()
                .map(ParticleDto::try_from)
                .collect::<CliResult<Vec<_>>>()?;
            println!("{}", serde_json::to_string_pretty(&dtos)?);
            Ok(())
        }
    }
}

fn print_show(
    db: &Pdg,
    entry: &PdgIdEntry,
    particles: &[PdgParticle<'_>],
    view: ShowView,
    details: &EntryDetails<'_>,
) -> CliResult<()> {
    print_title(
        &format!("{} {}", entry.pdgid, entry.description),
        &details.content.texts,
    );
    let database_edition = &db.database_info().edition;
    if db.edition() == database_edition {
        println!("PDG edition {database_edition}");
    } else {
        println!(
            "Numeric edition {} from PDG database edition {database_edition}",
            db.edition(),
        );
    }
    print_entry_summary(entry);

    if !particles.is_empty() {
        let heading = if particles.len() == 1 {
            "Particle"
        } else {
            "Particle variants"
        };
        println!("{}", heading.to_field_string());
        println!("{}", particle_table(particles, false)?);
    }
    print_headline_properties(&details.properties);
    if !details.related_particles.is_empty() {
        println!("{}", "Related particles".to_field_string());
        println!("{}", particle_table(&details.related_particles, false)?);
    }
    print_branching_fractions(&details.branching_fractions);
    print_branching_ratios(&details.branching_ratios);

    if !details.children.is_empty() {
        println!("{}", "Child PDG IDs".to_field_string());
        print_pdgid_entries(&details.children);
    }
    if !details.related_entries.is_empty() {
        println!("{}", "Related PDG IDs".to_field_string());
        print_pdgid_entries(&details.related_entries);
    }
    print_entry_content(&details.content);
    if !details.references.is_empty() {
        println!("{}", "References".to_field_string());
        for reference in &details.references {
            println!("{}", reference_lines(reference).join("\n"));
        }
    }
    if !details.related_details.is_empty() {
        println!("{}", "Related details".to_field_string());
        for related in &details.related_details {
            let entry = &related.entry;
            print_title(
                &format!("{} {}", entry.pdgid, entry.description),
                &related.content.texts,
            );
            print_entry_summary(entry);
            print_entry_content(&related.content);
        }
    }
    print_empty_view(view, details);
    Ok(())
}

fn print_empty_view(view: ShowView, details: &EntryDetails<'_>) {
    let empty = match view {
        ShowView::Mass if details.properties.is_empty() && details.content.data.is_empty() => {
            "mass data"
        }
        ShowView::Decays
            if details.branching_fractions.is_empty()
                && details.branching_ratios.is_empty()
                && details.content.data.is_empty() =>
        {
            "decay listings"
        }
        ShowView::Measurements if details.content.measurements.is_empty() => "measurements",
        ShowView::References if details.references.is_empty() => "references",
        _ => return,
    };
    println!("No {empty} for this entry.");
}

fn print_entry_content(content: &EntryContent<'_>) {
    if !content.data.is_empty() {
        println!("{}", "Data".to_field_string());
        print_data_entries(&content.data);
    }
    for measurements in content.measurements.chunk_by(|a, b| a.pdgid == b.pdgid) {
        print_measurements_for(&measurements[0].pdgid, measurements);
    }
    print_unattached_footnotes(&content.footnotes, &content.measurements);
}

fn print_title(title: &str, texts: &[PdgText]) {
    let mut table = table();
    table
        .set_content_arrangement(ContentArrangement::Dynamic)
        .set_width(100)
        .set_constraints([ColumnConstraint::Boundaries {
            lower: Width::Fixed(40),
            upper: Width::Fixed(100),
        }]);
    table.add_row([title.to_title_string()]);
    for text in texts {
        if let Some(text) = &text.text
            && !text.is_empty()
        {
            table.add_row([text.clone()]);
        }
    }
    println!("{table}");
}

fn print_entry_summary(entry: &PdgIdEntry) {
    let mut headers = vec!["PDG ID".to_string(), "Type".to_string()];
    let mut values = vec![entry.pdgid.to_pdgid_string(), entry.data_type.to_string()];
    if let Some(parent) = &entry.parent_pdgid
        && !parent.is_empty()
    {
        headers.push("Parent".to_string());
        values.push(parent.to_pdgid_string());
    }
    if let Some(mode) = entry.mode_number {
        headers.push("Mode".to_string());
        values.push(mode.to_string());
    }

    let mut meta = table();
    meta.set_header(headers);
    meta.add_row(values);
    println!("{meta}");
}

fn print_headline_properties(properties: &[ParticlePropertyDetail]) {
    if properties.is_empty() {
        return;
    }

    println!("{}", "Properties".to_field_string());
    let mut table = table();
    table.set_header(["Property", "Value", "Source PDG ID", "Source"]);
    for property in properties {
        table.add_row([
            property.data_type.to_string(),
            property.value.display.to_value_string(),
            property.value.pdgid.to_pdgid_string(),
            property.source.to_string(),
        ]);
    }
    println!("{table}");
}

fn print_data_entries(entries: &[DataEntry<'_>]) {
    let mut table = table();
    table.set_header(["PDG ID", "Value", "Type", "Comment"]);
    table.set_constraints([
        ColumnConstraint::ContentWidth,
        ColumnConstraint::ContentWidth,
        ColumnConstraint::ContentWidth,
        ColumnConstraint::Boundaries {
            lower: Width::Fixed(30),
            upper: Width::Fixed(72),
        },
        ColumnConstraint::Boundaries {
            lower: Width::Fixed(16),
            upper: Width::Fixed(32),
        },
    ]);
    for entry in entries {
        table.add_row([
            entry.pdgid.to_pdgid_string(),
            entry.pretty_string(),
            entry.value_type.to_string(),
            entry.comment.clone().unwrap_or_default(),
        ]);
    }
    println!("{table}");
}

fn print_pdgid_entries(entries: &[PdgIdEntry]) {
    let mut table = table();
    table.set_header(["PDG ID", "Type", "Description"]);
    table.set_constraints([
        ColumnConstraint::ContentWidth,
        ColumnConstraint::ContentWidth,
        ColumnConstraint::Boundaries {
            lower: Width::Fixed(35),
            upper: Width::Fixed(72),
        },
    ]);
    for entry in entries {
        table.add_row([
            entry.pdgid.to_pdgid_string(),
            entry.data_type.to_string(),
            entry.description.clone(),
        ]);
    }
    println!("{table}");
}

fn build_query(filters: &ParticleFilters) -> CliResult<ParticleSearchQuery> {
    let mut search =
        ParticleSearchQuery::new().include_unknown_properties(filters.include_unknown_properties);
    if let Some(mcid) = filters.mcid {
        search = search.mcid(mcid);
    }
    if let Some(class) = filters.class {
        search = search.class(class);
    }
    if let Some(particle_type) = filters.particle_type {
        search = search.particle_type(particle_type);
    }
    if let Some(charge) = filters.charge {
        search = search.charge(charge);
    }
    if let Some(isospin) = &filters.isospin {
        search = match isospin {
            QuantumArg::Missing => search.isospin(None),
            QuantumArg::Value(value) => search.isospin(*value),
        };
    }
    if let Some(g_parity) = &filters.g_parity {
        search = match g_parity {
            QuantumArg::Missing => search.g_parity(None),
            QuantumArg::Value(value) => search.g_parity(*value),
        };
    }
    if let Some(spin) = &filters.spin {
        search = match spin {
            QuantumArg::Missing => search.angular_momentum(None),
            QuantumArg::Value(value) => search.angular_momentum(value.clone()),
        };
    }
    if let Some(parity) = &filters.parity {
        search = match parity {
            QuantumArg::Missing => search.parity(None),
            QuantumArg::Value(value) => search.parity(*value),
        };
    }
    if let Some(c_parity) = &filters.c_parity {
        search = match c_parity {
            QuantumArg::Missing => search.charge_conjugation(None),
            QuantumArg::Value(value) => search.charge_conjugation(*value),
        };
    }
    if let Some((min, max)) = filters.mass {
        search = search.mass_range_mev(min, max);
    }
    if let Some((min, max)) = filters.width {
        search = search.width_range_mev(min, max);
    }
    if let Some((min, max)) = filters.lifetime {
        search = search.lifetime_range_seconds(min, max);
    }
    if !filters.decays_to.is_empty() && !filters.decay_contains.is_empty() {
        return Err(CliError::InvalidArgument(
            "use only one of --decays-to or --decay-contains".into(),
        ));
    }
    if !filters.decays_to.is_empty() {
        search = search.decays_to(filters.decays_to.clone());
    }
    if !filters.decay_contains.is_empty() {
        search = search.decay_contains(filters.decay_contains.clone());
    }
    if !filters.decays_from.is_empty() {
        search = search.decays_from(filters.decays_from.clone());
    }
    Ok(search.decay_state_expansion(filters.decay_expansion.into()))
}

const fn has_particle_filters(filters: &ParticleFilters) -> bool {
    filters.class.is_some()
        || filters.particle_type.is_some()
        || filters.charge.is_some()
        || filters.isospin.is_some()
        || filters.g_parity.is_some()
        || filters.spin.is_some()
        || filters.parity.is_some()
        || filters.c_parity.is_some()
        || filters.mass.is_some()
        || filters.width.is_some()
        || filters.lifetime.is_some()
        || !filters.decays_to.is_empty()
        || !filters.decay_contains.is_empty()
        || !filters.decays_from.is_empty()
        || filters.mcid.is_some()
}

fn print_particle_hits(hits: &[ParticleFilterHit<'_>]) -> CliResult<()> {
    if hits.is_empty() {
        println!("{}", "No particles found.".to_value_string());
        return Ok(());
    }

    println!(
        "{}",
        particle_table(hits.iter().map(|hit| &hit.particle), true)?
    );
    if hits.iter().any(|hit| !hit.matching_decays.is_empty()) {
        println!("Matching PDG decay channels");
        let mut channels = table();
        channels.set_header(["Particle", "PDG ID", "Matched states", "Recorded channel"]);
        for hit in hits {
            for channel in &hit.matching_decays {
                let states = match (channel.matched_initial_states, channel.matched_final_states) {
                    (true, true) => "Initial and final",
                    (true, false) => "Initial",
                    _ => "Final",
                };
                channels.add_row([
                    hit.particle.name.as_str(),
                    channel.pdgid.as_str(),
                    states,
                    channel.description.as_str(),
                ]);
            }
        }
        println!("{channels}");
    }
    Ok(())
}

fn print_branching_fractions(decays: &[BranchingFraction<'_>]) {
    if decays.is_empty() {
        return;
    }
    println!("{}", "Branching fractions".to_field_string());
    let mut table = table();
    table.set_header(["PDG ID", "Kind", "Value", "Description", "Related PDG IDs"]);
    table.set_constraints([
        ColumnConstraint::ContentWidth,
        ColumnConstraint::ContentWidth,
        ColumnConstraint::ContentWidth,
        ColumnConstraint::Boundaries {
            lower: Width::Fixed(30),
            upper: Width::Fixed(72),
        },
    ]);
    for decay in decays {
        let related_ids = decay
            .related_data
            .iter()
            .map(|entry| entry.pdgid.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        table.add_row([
            decay.pdgid.to_pdgid_string(),
            decay.kind.to_string(),
            decay.value.to_string().to_value_string(),
            decay.description.clone(),
            related_ids
                .split(", ")
                .filter(|pdgid| !pdgid.is_empty())
                .map(CliStringExt::to_pdgid_string)
                .collect::<Vec<_>>()
                .join(", "),
        ]);
    }
    println!("{table}");
}

fn print_branching_ratios(ratios: &[BranchingRatio<'_>]) {
    if ratios.is_empty() {
        return;
    }
    println!("{}", "Branching ratios".to_field_string());
    let mut table = table();
    table.set_header(["PDG ID", "Value", "Description"]);
    table.set_constraints([
        ColumnConstraint::ContentWidth,
        ColumnConstraint::ContentWidth,
        ColumnConstraint::Boundaries {
            lower: Width::Fixed(30),
            upper: Width::Fixed(72),
        },
    ]);
    for ratio in ratios {
        table.add_row([
            ratio.pdgid.to_pdgid_string(),
            ratio.value.to_string().to_value_string(),
            ratio.description.clone(),
        ]);
    }
    println!("{table}");
}

fn print_text_results(results: &[TextSearchResult], show_full_text: bool) {
    let Some(first) = results.first() else {
        println!("{}", "No text results found.".to_value_string());
        return;
    };
    if let Some(correction) = &first.correction {
        println!(
            "Showing matches for {:?} in place of {:?}",
            correction.replacement, correction.original
        );
    }
    let mut table = table();
    table.set_header(["PDG ID", "Title", "Source", "Match"]);
    table.set_constraints([
        ColumnConstraint::ContentWidth,
        ColumnConstraint::Boundaries {
            lower: Width::Fixed(24),
            upper: Width::Fixed(46),
        },
        ColumnConstraint::ContentWidth,
        ColumnConstraint::Boundaries {
            lower: Width::Fixed(35),
            upper: Width::Fixed(72),
        },
    ]);
    for result in results {
        let mut title = result.title.clone().unwrap_or_default();
        if let Some(parent) = &result.parent {
            title.push_str(&format!("\nParent {} {}", parent.pdgid, parent.title));
        }
        let source = match &result.source {
            TextSearchSource::Description => "Description".to_owned(),
            TextSearchSource::Text { .. } => "Text".to_owned(),
            TextSearchSource::Footnote { index } => format!("Footnote {index}"),
        };
        table.add_row([
            result.pdgid.to_pdgid_string(),
            title,
            source,
            if show_full_text {
                result.text.clone()
            } else {
                result.snippet.clone()
            },
        ]);
    }
    println!("{table}");
}

fn print_footnotes(footnotes: &[PdgFootnote]) {
    if footnotes.is_empty() {
        return;
    }
    println!("{}", "Footnotes".to_field_string());
    for footnote in footnotes {
        let index = footnote
            .index
            .map(|index| index.to_string())
            .unwrap_or_default();
        println!("[{}] {}", index, footnote.text.clone().unwrap_or_default());
    }
}

fn print_unattached_footnotes(footnotes: &[PdgFootnote], measurements: &[PdgMeasurement]) {
    let attached = measurements
        .iter()
        .flat_map(|measurement| measurement.footnotes.iter())
        .map(footnote_key)
        .collect::<HashSet<_>>();
    let unattached = footnotes
        .iter()
        .filter(|footnote| !attached.contains(&footnote_key(footnote)))
        .cloned()
        .collect::<Vec<_>>();
    print_footnotes(&unattached);
}

fn footnote_key(footnote: &PdgFootnote) -> (Option<isize>, Option<String>) {
    (footnote.index, footnote.text.clone())
}

fn print_measurements(measurements: &[PdgMeasurement]) {
    if measurements.is_empty() {
        return;
    }
    for (index, measurement) in measurements.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("{}", measurement.pretty_string());
    }
}

fn print_measurements_for(pdgid: &str, measurements: &[PdgMeasurement]) {
    if measurements.is_empty() {
        return;
    }
    println!("{}", format!("Measurements for {pdgid}").to_field_string());
    print_measurements(measurements);
}

fn parse_class(value: &str) -> Result<ParticleClass, String> {
    match normalize(value).as_str() {
        "gaugeboson" | "gaugehiggsboson" | "boson" => Ok(ParticleClass::GaugeBoson),
        "lepton" => Ok(ParticleClass::Lepton),
        "quark" => Ok(ParticleClass::Quark),
        "meson" => Ok(ParticleClass::Meson),
        "baryon" => Ok(ParticleClass::Baryon),
        _ => Err("expected gauge-boson, lepton, quark, meson, or baryon".into()),
    }
}

fn parse_particle_type(value: &str) -> Result<ParticleType, String> {
    match normalize(value).as_str() {
        "particle" => Ok(ParticleType::Particle),
        "antiparticle" | "anti" => Ok(ParticleType::Antiparticle),
        "selfconjugate" | "self" => Ok(ParticleType::SelfConjugate),
        _ => Err("expected particle, antiparticle, or self-conjugate".into()),
    }
}

fn parse_charge(value: &str) -> Result<Charge, String> {
    match value.trim() {
        "+2" | "2" => Ok(Charge::PlusPlus),
        "+1" | "1" => Ok(Charge::Plus),
        "0" => Ok(Charge::Neutral),
        "-1" => Ok(Charge::Minus),
        "-2" => Ok(Charge::MinusMinus),
        "+1/3" | "1/3" => Ok(Charge::PlusOneThird),
        "+2/3" | "2/3" => Ok(Charge::PlusTwoThirds),
        "-1/3" => Ok(Charge::MinusOneThird),
        "-2/3" => Ok(Charge::MinusTwoThirds),
        _ => Err("expected -2, -1, -2/3, -1/3, 0, 1/3, 2/3, 1, or 2".into()),
    }
}

fn parse_isospin_filter(value: &str) -> Result<QuantumArg<Isospin>, String> {
    if is_missing(value) {
        return Ok(QuantumArg::Missing);
    }
    Ok(QuantumArg::Value(match normalize(value).as_str() {
        "0" => Isospin::I0,
        "1/2" | "half" => Isospin::I1,
        "1" => Isospin::I2,
        "3/2" => Isospin::I3,
        "unknown" | "?" => Isospin::Unknown,
        _ => return Err("expected 0, 1/2, 1, 3/2, unknown, or missing".into()),
    }))
}

fn parse_parity_filter(value: &str) -> Result<QuantumArg<Parity>, String> {
    if is_missing(value) {
        return Ok(QuantumArg::Missing);
    }
    if value.trim() == "-" {
        return Ok(QuantumArg::Value(Parity::Minus));
    }
    Ok(QuantumArg::Value(match normalize(value).as_str() {
        "+" | "plus" => Parity::Plus,
        "minus" => Parity::Minus,
        "unknown" | "?" => Parity::Unknown,
        _ => return Err("expected +, -, unknown, or missing".into()),
    }))
}

#[allow(clippy::unnecessary_wraps)]
fn parse_spin_filter(value: &str) -> Result<QuantumArg<AngularMomentum>, String> {
    if is_missing(value) {
        return Ok(QuantumArg::Missing);
    }
    Ok(QuantumArg::Value(match normalize(value).as_str() {
        "0" => AngularMomentum::J0,
        "1/2" => AngularMomentum::J1,
        "1" => AngularMomentum::J2,
        "3/2" => AngularMomentum::J3,
        "2" => AngularMomentum::J4,
        "5/2" => AngularMomentum::J5,
        "3" => AngularMomentum::J6,
        "7/2" => AngularMomentum::J7,
        "4" => AngularMomentum::J8,
        "9/2" => AngularMomentum::J9,
        "5" => AngularMomentum::J10,
        "11/2" => AngularMomentum::J11,
        "6" => AngularMomentum::J12,
        "13/2" => AngularMomentum::J13,
        "7" => AngularMomentum::J14,
        "15/2" => AngularMomentum::J15,
        "unknown" | "?" => AngularMomentum::Unknown,
        _ => AngularMomentum::Custom(value.to_string()),
    }))
}

fn parse_range(value: &str) -> Result<(f64, f64), String> {
    let (min, max) = value
        .split_once("..")
        .or_else(|| value.split_once(':'))
        .ok_or_else(|| "expected range formatted as MIN..MAX".to_string())?;
    let min = f64::from_str(min).map_err(|_| "invalid range minimum".to_string())?;
    let max = f64::from_str(max).map_err(|_| "invalid range maximum".to_string())?;
    if min.is_nan() || max.is_nan() {
        return Err("range bounds cannot be NaN".into());
    }
    if min > max {
        return Err("range minimum must be <= maximum".into());
    }
    Ok((min, max))
}

fn normalize(value: &str) -> String {
    value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|ch| !matches!(ch, '-' | '_' | ' '))
        .collect()
}

fn is_missing(value: &str) -> bool {
    matches!(normalize(value).as_str(), "missing" | "none" | "null")
}
