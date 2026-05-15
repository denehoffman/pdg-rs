use std::{collections::HashSet, env, fs, path::PathBuf, str::FromStr};

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use comfy_table::{ColumnConstraint, ContentArrangement, Width};
use owo_colors::OwoColorize;
use pdg_rs::{
    AngularMomentum, Charge, DataEntry, DecayStateExpansion, Isospin, Parity, ParticleClass,
    ParticleSearchQuery, ParticleType, Pdg, PdgError, PdgFootnote, PdgIdEntry, PdgMeasurement,
    PdgMeasurementValue, PdgParticle, PdgText, TextSearchResult,
};
use thiserror::Error;

pub(crate) mod cli;
use cli::{
    DataEntryDto, FootnoteDto, MeasurementDto, ParticleDto, PdgIdEntryDto, ShowDto, TextDto,
    TextSearchDto, headline_property_rows, particle_table, table,
};

#[derive(Debug, Error)]
enum CliError {
    #[error(transparent)]
    Pdg(#[from] PdgError),
    #[error("no particle found for {0}")]
    NotFound(String),
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
}

impl CliStringExt for str {
    fn to_pdgid_string(&self) -> String {
        self.magenta().bold().to_string()
    }

    fn to_value_string(&self) -> String {
        self.yellow().to_string()
    }

    fn to_link_string(&self) -> String {
        self.blue().underline().to_string()
    }

    fn to_field_string(&self) -> String {
        self.cyan().to_string()
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
            parts.push("used in average".green().to_string());
        }
        if self.used_in_fit {
            parts.push("used in fit".green().to_string());
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

impl PrettyString for PdgMeasurement {
    fn pretty_string(&self) -> String {
        let mut lines = vec![self.reference.document_id.trim().bold().to_string()];
        push_optional_line(&mut lines, "Title", self.reference.title.as_deref());
        push_optional_line(
            &mut lines,
            "Year",
            self.reference.publication_year.map(|year| year.to_string()),
        );
        push_optional_line(
            &mut lines,
            "Publication",
            self.reference.publication_name.as_deref(),
        );
        if let Some(doi) = &self.reference.doi {
            push_optional_line(
                &mut lines,
                "DOI",
                Some(format!("https://doi.org/{doi}").to_link_string()),
            );
        }
        if let Some(inspire_id) = &self.reference.inspire_id {
            push_optional_line(
                &mut lines,
                "INSPIRE",
                Some(format!("https://inspirehep.net/literature/{inspire_id}").to_link_string()),
            );
        }
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
    about = "Search the Particle Data Group database"
)]
struct Cli {
    /// Require an already cached database instead of downloading it.
    #[arg(long, global = true)]
    offline: bool,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Look up any PDG database ID, such as S008 or S008245.
    Show(ShowCommand),
    /// Search particles or text.
    Search(SearchCommand),
    /// Manage the cached PDG database.
    Db(DbCommand),
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
    /// Show whether the default database is available locally.
    Status,
    /// Download and verify the default database.
    Fetch,
    /// Print the default cache path.
    Path,
    /// Remove the default cached database.
    Clear,
}

#[derive(Parser)]
struct ParticleCommand {
    /// Substring to match against particle names.
    query: Option<String>,
    #[command(flatten)]
    filters: ParticleFilters,
    #[command(flatten)]
    output: ParticleOutput,
}

#[derive(Parser)]
struct ShowCommand {
    /// String PDG ID, such as S008 or S008245.
    pdgid: String,
    #[command(flatten)]
    output: ShowOutput,
}

#[derive(Parser)]
struct SearchCommand {
    #[command(subcommand)]
    command: SearchCommands,
}

#[derive(Subcommand)]
enum SearchCommands {
    /// Search particles by name and particle properties.
    Particles(ParticleCommand),
    /// Search PDG descriptions and text entries.
    Text(TextCommand),
}

#[derive(Parser)]
struct TextCommand {
    /// Full-text search query.
    query: String,
    #[arg(long, value_enum, default_value_t = OutputFormat::Pretty)]
    format: OutputFormat,
    #[arg(long, default_value_t = 20)]
    limit: usize,
    #[arg(long)]
    all: bool,
    #[arg(long)]
    show_full_text: bool,
}

#[allow(clippy::doc_markdown)]
#[derive(Parser, Clone)]
struct ParticleFilters {
    /// Filter by particle class.
    #[arg(long, value_parser = parse_class)]
    class: Option<ParticleClass>,
    /// Filter by particle/antiparticle/self-conjugate type.
    #[arg(long = "type", value_parser = parse_particle_type)]
    particle_type: Option<ParticleType>,
    /// Filter by electric charge, e.g. 0, +1, -1, +2/3.
    #[arg(long, allow_hyphen_values = true, value_parser = parse_charge)]
    charge: Option<Charge>,
    /// Filter by isospin; accepts values like 0, 1/2, 1, 3/2, unknown, or missing.
    #[arg(long, value_parser = parse_isospin_filter)]
    isospin: Option<QuantumArg<Isospin>>,
    /// Filter by G parity; accepts +, -, unknown, or missing.
    #[arg(long, value_parser = parse_parity_filter)]
    g_parity: Option<QuantumArg<Parity>>,
    /// Filter by spin/angular momentum; accepts values like 0, 1/2, 1, 3/2, unknown, or missing.
    #[arg(long, value_parser = parse_spin_filter)]
    spin: Option<QuantumArg<AngularMomentum>>,
    /// Filter by parity; accepts +, -, unknown, or missing.
    #[arg(long, value_parser = parse_parity_filter)]
    parity: Option<QuantumArg<Parity>>,
    /// Filter by charge conjugation parity; accepts +, -, unknown, or missing.
    #[arg(long, value_parser = parse_parity_filter)]
    c_parity: Option<QuantumArg<Parity>>,
    /// Filter by mass range in MeV, formatted as MIN..MAX.
    #[arg(long, value_parser = parse_range)]
    mass: Option<(f64, f64)>,
    /// Filter by width range in MeV, formatted as MIN..MAX.
    #[arg(long, value_parser = parse_range)]
    width: Option<(f64, f64)>,
    /// Filter by lifetime range in seconds, formatted as MIN..MAX.
    #[arg(long, value_parser = parse_range)]
    lifetime: Option<(f64, f64)>,
    /// Filter particles with an exact decay to comma-separated final states.
    #[arg(long, value_delimiter = ',')]
    decays_to: Vec<String>,
    /// Filter particles with a decay containing comma-separated final states.
    #[arg(long, value_delimiter = ',')]
    decay_contains: Vec<String>,
    /// Filter particles which can be produced from comma-separated initial states.
    #[arg(long, value_delimiter = ',')]
    decays_from: Vec<String>,
    /// Control whether decay state names are expanded through particle families.
    #[arg(long, value_enum, default_value_t = DecayExpansionArg::Inclusive)]
    decay_expansion: DecayExpansionArg,
    /// Look up a particle by Monte Carlo ID instead of name/search filters.
    #[arg(long)]
    mcid: Option<isize>,
}

#[derive(Parser, Clone)]
struct ParticleOutput {
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Pretty)]
    format: OutputFormat,
    /// Maximum number of results to print.
    #[arg(long, default_value_t = 20)]
    limit: usize,
    /// Print all matching results.
    #[arg(long)]
    all: bool,
}

#[derive(Parser, Clone, Copy)]
struct ShowOutput {
    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Pretty)]
    format: OutputFormat,
    /// Show only metadata, particle identity, headline properties, and direct data.
    #[arg(long)]
    summary: bool,
    /// Include data, text, measurements, references, and footnotes for related PDG IDs.
    #[arg(long)]
    related_details: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum OutputFormat {
    Pretty,
    Json,
}

#[derive(Clone, Copy, ValueEnum)]
enum DecayExpansionArg {
    Inclusive,
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

fn main() -> CliResult<()> {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        println!();
        return Ok(());
    };

    match command {
        Commands::Show(command) => run_show(&command.pdgid, command.output, cli.offline),
        Commands::Search(command) => match command.command {
            SearchCommands::Particles(command) => run_particle(command, cli.offline),
            SearchCommands::Text(command) => run_text(command, cli.offline),
        },
        Commands::Db(command) => run_db(&command),
        Commands::Tui => Err(CliError::InvalidArgument(
            "TUI is not implemented yet; use `pdg show`, `pdg search particles`, `pdg search text`, or `pdg db`"
                .into(),
        )),
    }
}

fn open_pdg(offline: bool) -> CliResult<Pdg> {
    Ok(if offline {
        Pdg::open_cached()?
    } else {
        Pdg::open()?
    })
}

fn run_db(command: &DbCommand) -> CliResult<()> {
    match command.command {
        DbCommands::Status => {
            if let Some(path) = env::var_os("PDG_RS_DB_PATH").map(PathBuf::from) {
                Pdg::open_cached()?;
                println!("database override: {}", path.display());
                return Ok(());
            }

            let path = Pdg::cached_database_path()?;
            if path.exists() {
                Pdg::open_cached()?;
                println!("cached database: {}", path.display());
            } else {
                println!("database not cached: {}", path.display());
            }
            Ok(())
        }
        DbCommands::Fetch => {
            let path = Pdg::ensure_database()?;
            println!("{}", path.display());
            Ok(())
        }
        DbCommands::Path => {
            println!("{}", Pdg::cached_database_path()?.display());
            Ok(())
        }
        DbCommands::Clear => {
            let path = Pdg::cached_database_path()?;
            if path.exists() {
                fs::remove_file(&path)?;
                println!("removed {}", path.display());
            } else {
                println!("database not cached: {}", path.display());
            }
            Ok(())
        }
    }
}

fn run_particle(command: ParticleCommand, offline: bool) -> CliResult<()> {
    if command.query.is_none() && !has_particle_filters(&command.filters) {
        ParticleCommand::command().print_help()?;
        println!();
        return Ok(());
    }

    let db = open_pdg(offline)?;
    let mut particles: Vec<_> = if let Some(mcid) = command.filters.mcid {
        db.mcid(mcid)?.into_iter().collect()
    } else {
        db.search_particles(build_query(command.query, &command.filters)?)?
    };

    particles = apply_limit(particles, command.output.limit, command.output.all);
    output_particles(&particles, &command.output)
}

fn run_text(command: TextCommand, offline: bool) -> CliResult<()> {
    let db = open_pdg(offline)?;
    let results = apply_limit(db.search_text(command.query)?, command.limit, command.all);
    match command.format {
        OutputFormat::Pretty => print_text_results(&db, &results, command.show_full_text),
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

fn run_show(pdgid: &str, output: ShowOutput, offline: bool) -> CliResult<()> {
    let db = open_pdg(offline)?;
    let entry = db
        .pdgid(pdgid)?
        .ok_or_else(|| CliError::NotFound(pdgid.to_string()))?;
    let particle = db.particle_by_pdgid(&entry.pdgid)?;

    match output.format {
        OutputFormat::Pretty => print_show(&db, &entry, particle.as_ref(), output),
        OutputFormat::Json => {
            let dto = show_dto(&db, &entry, particle.as_ref(), output)?;
            println!("{}", serde_json::to_string_pretty(&dto)?);
            Ok(())
        }
    }
}

fn output_particles(particles: &[PdgParticle<'_>], output: &ParticleOutput) -> CliResult<()> {
    match output.format {
        OutputFormat::Pretty => print_particles(particles),
        OutputFormat::Json => {
            let dtos = particles
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
    particle: Option<&PdgParticle<'_>>,
    output: ShowOutput,
) -> CliResult<()> {
    let texts = if output.summary {
        Vec::new()
    } else {
        db.texts_for(&entry.pdgid)?
    };
    print_title(&format!("{} {}", entry.pdgid, entry.description), &texts);
    print_entry_summary(entry);

    if let Some(particle) = particle {
        print_particle_identity(particle)?;
        print_headline_properties(db, particle)?;
        if !output.summary {
            print_related_particles(particle)?;
            print_branching_fractions(particle)?;
            print_branching_ratios(particle)?;
        }
    }

    let data = db.data_for(&entry.pdgid)?;
    if !data.is_empty() {
        println!("{}", "Data".cyan());
        print_data_entries(&data);
    }

    let children = db.children_for_pdgid(&entry.pdgid)?;
    if !output.summary && !children.is_empty() {
        println!("{}", "Child PDG IDs".cyan());
        print_pdgid_entries(&children);
    }

    let related_entries = db.mapped_entries_for_pdgid(&entry.pdgid)?;
    if !output.summary && !related_entries.is_empty() {
        println!("{}", "Related PDG IDs".cyan());
        print_pdgid_entries(&related_entries);
    }

    if !output.summary {
        let measurements = db.measurements_for(&entry.pdgid)?;
        print_measurements_for(&entry.pdgid, &measurements);
        print_unattached_footnotes(&db.footnotes_for(&entry.pdgid)?, &measurements);
        if output.related_details {
            print_related_details(db, &entry.pdgid, particle, &children, &related_entries)?;
        }
    }

    Ok(())
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
    table.add_row([title.bold().to_string()]);
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

fn print_particle_identity(particle: &PdgParticle<'_>) -> CliResult<()> {
    println!("{}", "Particle".cyan());
    println!("{}", particle_table(std::slice::from_ref(particle), false)?);
    Ok(())
}

fn print_related_details(
    db: &Pdg,
    root_pdgid: &str,
    particle: Option<&PdgParticle<'_>>,
    children: &[PdgIdEntry],
    related_entries: &[PdgIdEntry],
) -> CliResult<()> {
    let ids = collect_detail_pdgids(db, root_pdgid, particle, children, related_entries)?;
    if ids.is_empty() {
        return Ok(());
    }

    println!("{}", "Related details".cyan());
    for pdgid in ids {
        let Some(entry) = db.pdgid(&pdgid)? else {
            continue;
        };
        let data = db.data_for(&entry.pdgid)?;
        let texts = db.texts_for(&entry.pdgid)?;
        let measurements = db.measurements_for(&entry.pdgid)?;
        let footnotes = db.footnotes_for(&entry.pdgid)?;
        if data.is_empty() && texts.is_empty() && measurements.is_empty() && footnotes.is_empty() {
            continue;
        }

        print_title(&format!("{} {}", entry.pdgid, entry.description), &texts);
        print_entry_summary(&entry);
        if !data.is_empty() {
            println!("{}", "Data".cyan());
            print_data_entries(&data);
        }
        print_measurements_for(&entry.pdgid, &measurements);
        print_unattached_footnotes(&footnotes, &measurements);
    }
    Ok(())
}

fn collect_detail_pdgids(
    db: &Pdg,
    root_pdgid: &str,
    particle: Option<&PdgParticle<'_>>,
    children: &[PdgIdEntry],
    related_entries: &[PdgIdEntry],
) -> CliResult<Vec<String>> {
    let mut seen = HashSet::new();
    let mut ordered = Vec::new();
    let mut stack = Vec::new();

    for entry in children.iter().chain(related_entries.iter()) {
        stack.push(entry.pdgid.clone());
    }

    if let Some(particle) = particle {
        for decay in particle.branching_fractions()? {
            stack.push(decay.pdgid);
            for related in decay.related_data {
                stack.push(related.pdgid);
            }
        }
        for ratio in particle.branching_ratios()? {
            stack.push(ratio.pdgid);
        }
    }

    while let Some(pdgid) = stack.pop() {
        if pdgid == root_pdgid || !seen.insert(pdgid.clone()) {
            continue;
        }
        ordered.push(pdgid.clone());
        for child in db.children_for_pdgid(&pdgid)? {
            stack.push(child.pdgid);
        }
        for related in db.mapped_entries_for_pdgid(&pdgid)? {
            stack.push(related.pdgid);
        }
    }

    Ok(ordered)
}

fn print_headline_properties(db: &Pdg, particle: &PdgParticle<'_>) -> CliResult<()> {
    let rows = headline_property_rows(db, particle)?;

    if rows.is_empty() {
        return Ok(());
    }

    println!("{}", "Properties".cyan());
    let mut table = table();
    table.set_header(["Property", "Value", "Source PDG ID", "Source"]);
    for row in rows {
        table.add_row([
            row[0].clone(),
            row[1].to_value_string(),
            row[2].to_pdgid_string(),
            row[3].clone(),
        ]);
    }
    println!("{table}");
    Ok(())
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

fn show_dto(
    db: &Pdg,
    entry: &PdgIdEntry,
    particle: Option<&PdgParticle<'_>>,
    output: ShowOutput,
) -> CliResult<ShowDto> {
    Ok(ShowDto {
        entry: PdgIdEntryDto::from(entry),
        particle: particle.map(ParticleDto::try_from).transpose()?,
        data: db
            .data_for(&entry.pdgid)?
            .iter()
            .map(DataEntryDto::try_from)
            .collect::<CliResult<Vec<_>>>()?,
        children: if output.summary {
            Vec::new()
        } else {
            db.children_for_pdgid(&entry.pdgid)?
                .iter()
                .map(PdgIdEntryDto::from)
                .collect()
        },
        related_entries: if output.summary {
            Vec::new()
        } else {
            db.mapped_entries_for_pdgid(&entry.pdgid)?
                .iter()
                .map(PdgIdEntryDto::from)
                .collect()
        },
        texts: (!output.summary)
            .then(|| {
                db.texts_for(&entry.pdgid)
                    .map(|texts| texts.iter().map(TextDto::from).collect())
            })
            .transpose()?,
        footnotes: (!output.summary)
            .then(|| {
                db.footnotes_for(&entry.pdgid)
                    .map(|footnotes| footnotes.iter().map(FootnoteDto::from).collect())
            })
            .transpose()?,
        measurements: (!output.summary)
            .then(|| {
                db.measurements_for(&entry.pdgid)
                    .map(|measurements| measurements.iter().map(MeasurementDto::from).collect())
            })
            .transpose()?,
    })
}

fn build_query(query: Option<String>, filters: &ParticleFilters) -> CliResult<ParticleSearchQuery> {
    let mut search = ParticleSearchQuery::new();
    if let Some(query) = query {
        search = search.name_contains(query);
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

fn apply_limit<T>(items: Vec<T>, limit: usize, all: bool) -> Vec<T> {
    if all {
        items
    } else {
        items.into_iter().take(limit).collect()
    }
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

fn print_particles(particles: &[PdgParticle<'_>]) -> CliResult<()> {
    if particles.is_empty() {
        println!("{}", "No particles found.".yellow());
        return Ok(());
    }

    let table = particle_table(particles, true)?;
    println!("{table}");
    Ok(())
}

fn print_related_particles(particle: &PdgParticle<'_>) -> CliResult<()> {
    let particles = particle.related_particles()?;
    if particles.is_empty() {
        return Ok(());
    }
    println!("{}", "Related particles".cyan());
    let table = particle_table(&particles, true)?;
    println!("{table}");
    Ok(())
}

fn print_branching_fractions(particle: &PdgParticle<'_>) -> CliResult<()> {
    let decays = particle.branching_fractions()?;
    if decays.is_empty() {
        return Ok(());
    }
    println!("{}", "Branching fractions".cyan());
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
    for decay in &decays {
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
    Ok(())
}

fn print_branching_ratios(particle: &PdgParticle<'_>) -> CliResult<()> {
    let ratios = particle.branching_ratios()?;
    if ratios.is_empty() {
        return Ok(());
    }
    println!("{}", "Branching ratios".cyan());
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
    for ratio in &ratios {
        table.add_row([
            ratio.pdgid.to_pdgid_string(),
            ratio.value.to_string().to_value_string(),
            ratio.description.clone(),
        ]);
    }
    println!("{table}");
    Ok(())
}

fn print_text_results(
    db: &Pdg,
    results: &[TextSearchResult],
    show_full_text: bool,
) -> CliResult<()> {
    if results.is_empty() {
        println!("{}", "No text results found.".yellow());
        return Ok(());
    }
    let mut table = table();
    table.set_header(["PDG ID", "Title", "Match"]);
    table.set_constraints([
        ColumnConstraint::ContentWidth,
        ColumnConstraint::Boundaries {
            lower: Width::Fixed(24),
            upper: Width::Fixed(46),
        },
        ColumnConstraint::Boundaries {
            lower: Width::Fixed(35),
            upper: Width::Fixed(72),
        },
    ]);
    for result in results {
        let title = db
            .pdgid(&result.pdgid)?
            .map(|entry| entry.description)
            .unwrap_or_default();
        table.add_row([
            result.pdgid.to_pdgid_string(),
            title,
            if show_full_text {
                result.text.clone()
            } else {
                result.snippet.clone()
            },
        ]);
    }
    println!("{table}");
    Ok(())
}

fn print_footnotes(footnotes: &[PdgFootnote]) {
    if footnotes.is_empty() {
        return;
    }
    println!("{}", "Footnotes".cyan());
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
    println!("{}", format!("Measurements for {pdgid}").cyan());
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
        "0or1" | "photon" => Isospin::Photon,
        "unknown" | "?" => Isospin::Unknown,
        _ => return Err("expected 0, 1/2, 1, 3/2, 0-or-1, unknown, or missing".into()),
    }))
}

fn parse_parity_filter(value: &str) -> Result<QuantumArg<Parity>, String> {
    if is_missing(value) {
        return Ok(QuantumArg::Missing);
    }
    Ok(QuantumArg::Value(match normalize(value).as_str() {
        "+" | "plus" => Parity::Plus,
        "-" | "minus" => Parity::Minus,
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
