use std::{collections::HashSet, str::FromStr};

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use comfy_table::{ColumnConstraint, ContentArrangement, Width};
use owo_colors::OwoColorize;
use pdg_rs::{
    table, AngularMomentum, Charge, DataEntry, DecayStateExpansion, Isospin, Parity, ParticleClass,
    ParticleSearchQuery, ParticleType, Pdg, PdgError, PdgFootnote, PdgIdEntry, PdgMeasurement,
    PdgMeasurementValue, PdgParticle, PdgReference, PdgText, TextSearchResult, TextSearchSource,
};
use serde::Serialize;
use thiserror::Error;

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
    fn to_pdg_id_string(&self) -> String;
    fn to_value_string(&self) -> String;
    fn to_link_string(&self) -> String;
    fn to_field_string(&self) -> String;
}

impl CliStringExt for str {
    fn to_pdg_id_string(&self) -> String {
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
            parts.push(format!("{}", column_name.to_field_string()));
        }
        if let Some(limit_type) = self.limit_type {
            parts.push(format!("{}", limit_type.to_string().to_field_string()));
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
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Look up any PDG database ID, such as S008 or S008245.
    Show(ShowCommand),
    /// Search particles or text.
    Search(SearchCommand),
    /// Reserved for the future terminal UI.
    Tui,
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
    pdg_id: String,
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

#[derive(Parser, Clone)]
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

#[derive(Serialize)]
struct ParticleDto {
    pdg_id: String,
    name: String,
    description: String,
    particle_type: String,
    particle_class: String,
    mcid: Option<isize>,
    charge: String,
    isospin: Option<String>,
    g_parity: Option<String>,
    spin: Option<String>,
    parity: Option<String>,
    c_parity: Option<String>,
    mass: Option<DataEntryDto>,
    lifetime: Option<DataEntryDto>,
    width: Option<DataEntryDto>,
    related_particles: Option<Vec<ParticleSummaryDto>>,
    branching_fractions: Option<Vec<BranchingFractionDto>>,
    branching_ratios: Option<Vec<BranchingRatioDto>>,
    texts: Option<Vec<TextDto>>,
    footnotes: Option<Vec<FootnoteDto>>,
}

impl TryFrom<&PdgParticle<'_>> for ParticleDto {
    type Error = CliError;

    fn try_from(particle: &PdgParticle<'_>) -> Result<Self, Self::Error> {
        Ok(ParticleDto {
            pdg_id: particle.pdg_id.clone(),
            name: particle.name.clone(),
            description: particle.description.clone(),
            particle_type: particle.particle_type.to_string(),
            particle_class: particle.particle_class.to_string(),
            mcid: particle.mcid,
            charge: particle.charge.to_string(),
            isospin: particle.quantum_i.as_ref().map(ToString::to_string),
            g_parity: particle.quantum_g.as_ref().map(ToString::to_string),
            spin: particle.quantum_j.as_ref().map(ToString::to_string),
            parity: particle.quantum_p.as_ref().map(ToString::to_string),
            c_parity: particle.quantum_c.as_ref().map(ToString::to_string),
            mass: particle
                .mass()?
                .as_ref()
                .map(DataEntryDto::try_from)
                .transpose()?,
            lifetime: particle
                .lifetime()?
                .as_ref()
                .map(DataEntryDto::try_from)
                .transpose()?,
            width: particle
                .width()?
                .as_ref()
                .map(DataEntryDto::try_from)
                .transpose()?,
            related_particles: None,
            branching_fractions: None,
            branching_ratios: None,
            texts: None,
            footnotes: None,
        })
    }
}

#[derive(Serialize)]
struct ParticleSummaryDto {
    pdg_id: String,
    name: String,
    particle_type: String,
    particle_class: String,
    mcid: Option<isize>,
    charge: String,
}

#[derive(Serialize)]
struct DataEntryDto {
    pdg_id: String,
    edition: String,
    value_type: String,
    display: String,
    unit: String,
    comment: Option<String>,
    value: Option<f64>,
    error_positive: Option<f64>,
    error_negative: Option<f64>,
    confidence_level: Option<f64>,
    limit_type: Option<String>,
    in_summary_table: bool,
    measurements: Option<Vec<MeasurementDto>>,
    texts: Option<Vec<TextDto>>,
    footnotes: Option<Vec<FootnoteDto>>,
}

impl TryFrom<&DataEntry<'_>> for DataEntryDto {
    type Error = CliError;

    fn try_from(entry: &DataEntry<'_>) -> Result<Self, Self::Error> {
        Ok(DataEntryDto {
            pdg_id: entry.pdgid.clone(),
            edition: entry.edition.clone(),
            value_type: entry.value_type.to_code().to_string(),
            display: entry.to_string(),
            unit: entry.unit_text.clone(),
            comment: entry.comment.clone(),
            value: entry.value,
            error_positive: entry.error_positive,
            error_negative: entry.error_negative,
            confidence_level: entry.confidence_level,
            limit_type: entry.limit_type.map(|limit| limit.to_code().to_string()),
            in_summary_table: entry.in_summary_table,
            measurements: None,
            texts: None,
            footnotes: None,
        })
    }
}

#[derive(Serialize)]
struct BranchingFractionDto {
    pdg_id: String,
    description: String,
    mode_number: Option<usize>,
    kind: String,
    value: DataEntryDto,
    related_data: Vec<RelatedDataDto>,
}

#[derive(Serialize)]
struct BranchingRatioDto {
    pdg_id: String,
    description: String,
    mode_number: Option<usize>,
    value: DataEntryDto,
}

#[derive(Serialize)]
struct RelatedDataDto {
    pdg_id: String,
    description: String,
    data_type: String,
    mode_number: Option<usize>,
    value: DataEntryDto,
}

#[derive(Serialize)]
struct MeasurementDto {
    pdg_id: String,
    reference: ReferenceDto,
    event_count: Option<String>,
    confidence_level: Option<f64>,
    place: Option<String>,
    technique: Option<String>,
    charge: Option<String>,
    changebar: bool,
    comment: Option<String>,
    sort: isize,
    values: Vec<MeasurementValueDto>,
    footnotes: Vec<FootnoteDto>,
}

impl From<&PdgMeasurement> for MeasurementDto {
    fn from(measurement: &PdgMeasurement) -> MeasurementDto {
        MeasurementDto {
            pdg_id: measurement.pdg_id.clone(),
            reference: ReferenceDto::from(&measurement.reference),
            event_count: measurement.event_count.clone(),
            confidence_level: measurement.confidence_level,
            place: measurement.place.clone(),
            technique: measurement.technique.clone(),
            charge: measurement.charge.clone(),
            changebar: measurement.changebar,
            comment: measurement.comment.clone(),
            sort: measurement.sort,
            values: measurement
                .values
                .iter()
                .map(MeasurementValueDto::from)
                .collect(),
            footnotes: measurement
                .footnotes
                .iter()
                .map(FootnoteDto::from)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct ReferenceDto {
    document_id: String,
    publication_name: Option<String>,
    publication_year: Option<isize>,
    doi: Option<String>,
    inspire_id: Option<String>,
    title: Option<String>,
}

impl From<&PdgReference> for ReferenceDto {
    fn from(reference: &PdgReference) -> ReferenceDto {
        ReferenceDto {
            document_id: reference.document_id.clone(),
            publication_name: reference.publication_name.clone(),
            publication_year: reference.publication_year,
            doi: reference.doi.clone(),
            inspire_id: reference.inspire_id.clone(),
            title: reference.title.clone(),
        }
    }
}

#[derive(Serialize)]
struct MeasurementValueDto {
    column_name: Option<String>,
    value_text: Option<String>,
    unit_text: Option<String>,
    display: String,
    display_value_text: Option<String>,
    display_power_of_ten: Option<isize>,
    display_in_percent: Option<bool>,
    limit_type: Option<String>,
    value: Option<f64>,
    error_positive: Option<f64>,
    error_negative: Option<f64>,
    stat_error_positive: Option<f64>,
    stat_error_negative: Option<f64>,
    syst_error_positive: Option<f64>,
    syst_error_negative: Option<f64>,
    used_in_average: bool,
    used_in_fit: bool,
    sort: isize,
}

impl From<&PdgMeasurementValue> for MeasurementValueDto {
    fn from(value: &PdgMeasurementValue) -> MeasurementValueDto {
        MeasurementValueDto {
            column_name: value.column_name.clone(),
            value_text: value.value_text.clone(),
            unit_text: value.unit_text.clone(),
            display: value.to_string(),
            display_value_text: value.display_value_text.clone(),
            display_power_of_ten: value.display_power_of_ten,
            display_in_percent: value.display_in_percent,
            limit_type: value
                .limit_type
                .map(|limit_type| limit_type.to_code().to_string()),
            value: value.value,
            error_positive: value.error_positive,
            error_negative: value.error_negative,
            stat_error_positive: value.stat_error_positive,
            stat_error_negative: value.stat_error_negative,
            syst_error_positive: value.syst_error_positive,
            syst_error_negative: value.syst_error_negative,
            used_in_average: value.used_in_average,
            used_in_fit: value.used_in_fit,
            sort: value.sort,
        }
    }
}

#[derive(Serialize)]
struct TextSearchDto {
    pdg_id: String,
    source: String,
    text_type: Option<String>,
    sort: Option<isize>,
    text: String,
    snippet: String,
    score: f64,
}

impl From<&TextSearchResult> for TextSearchDto {
    fn from(result: &TextSearchResult) -> TextSearchDto {
        let (source, text_type, sort) = match &result.source {
            TextSearchSource::Description => ("description".to_string(), None, None),
            TextSearchSource::Text { text_type, sort } => {
                ("text".to_string(), Some(text_type.clone()), Some(*sort))
            }
            TextSearchSource::Footnote { index } => ("footnote".to_string(), None, Some(*index)),
        };
        TextSearchDto {
            pdg_id: result.pdg_id.clone(),
            source,
            text_type,
            sort,
            text: result.text.clone(),
            snippet: result.snippet.clone(),
            score: result.score,
        }
    }
}

#[derive(Serialize)]
struct TextDto {
    pdg_id: String,
    text_type: String,
    text: Option<String>,
    sort: isize,
}

impl From<&PdgText> for TextDto {
    fn from(text: &PdgText) -> TextDto {
        TextDto {
            pdg_id: text.pdg_id.clone(),
            text_type: text.text_type.clone(),
            text: text.text.clone(),
            sort: text.sort,
        }
    }
}

#[derive(Serialize)]
struct FootnoteDto {
    pdg_id: Option<String>,
    index: Option<isize>,
    text: Option<String>,
    changebar: bool,
}

impl From<&PdgFootnote> for FootnoteDto {
    fn from(footnote: &PdgFootnote) -> FootnoteDto {
        FootnoteDto {
            pdg_id: footnote.pdg_id.clone(),
            index: footnote.index,
            text: footnote.text.clone(),
            changebar: footnote.changebar,
        }
    }
}

#[derive(Serialize)]
struct ShowDto {
    entry: PdgIdEntryDto,
    particle: Option<ParticleDto>,
    data: Vec<DataEntryDto>,
    children: Vec<PdgIdEntryDto>,
    related_entries: Vec<PdgIdEntryDto>,
    texts: Option<Vec<TextDto>>,
    footnotes: Option<Vec<FootnoteDto>>,
    measurements: Option<Vec<MeasurementDto>>,
}

#[derive(Serialize)]
struct PdgIdEntryDto {
    id: isize,
    pdg_id: String,
    parent_pdg_id: Option<String>,
    description: String,
    mode_number: Option<isize>,
    data_type: String,
    flags: String,
    year_added: Option<isize>,
    sort: isize,
}

impl From<&PdgIdEntry> for PdgIdEntryDto {
    fn from(entry: &PdgIdEntry) -> PdgIdEntryDto {
        PdgIdEntryDto {
            id: entry.id,
            pdg_id: entry.pdg_id.clone(),
            parent_pdg_id: entry.parent_pdg_id.clone(),
            description: entry.description.clone(),
            mode_number: entry.mode_number,
            data_type: entry.data_type.to_code().to_string(),
            flags: entry.flags.clone(),
            year_added: entry.year_added,
            sort: entry.sort,
        }
    }
}

fn main() -> CliResult<()> {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        println!();
        return Ok(());
    };

    match command {
        Commands::Show(command) => run_show(command.pdg_id, command.output),
        Commands::Search(command) => match command.command {
            SearchCommands::Particles(command) => run_particle(command),
            SearchCommands::Text(command) => run_text(command),
        },
        Commands::Tui => Err(CliError::InvalidArgument(
            "TUI is not implemented yet; use `pdg show`, `pdg search particles`, or `pdg search text`"
                .into(),
        )),
    }
}

fn run_particle(command: ParticleCommand) -> CliResult<()> {
    if command.query.is_none() && !has_particle_filters(&command.filters) {
        ParticleCommand::command().print_help()?;
        println!();
        return Ok(());
    }

    let db = Pdg::open()?;
    let mut particles: Vec<_> = if let Some(mcid) = command.filters.mcid {
        db.mcid(mcid)?.into_iter().collect()
    } else {
        db.search_particles(build_query(command.query, &command.filters)?)?
    };

    particles = apply_limit(particles, command.output.limit, command.output.all);
    output_particles(particles, &command.output)
}

fn run_text(command: TextCommand) -> CliResult<()> {
    let db = Pdg::open()?;
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

fn run_show(pdg_id: String, output: ShowOutput) -> CliResult<()> {
    let db = Pdg::open()?;
    let entry = db
        .pdg_id(&pdg_id)?
        .ok_or_else(|| CliError::NotFound(pdg_id.clone()))?;
    let particle = db.particle_by_pdg_id(&entry.pdg_id)?;

    match output.format {
        OutputFormat::Pretty => print_show(&db, &entry, particle.as_ref(), &output),
        OutputFormat::Json => {
            let dto = show_dto(&db, &entry, particle.as_ref(), &output)?;
            println!("{}", serde_json::to_string_pretty(&dto)?);
            Ok(())
        }
    }
}

fn output_particles(particles: Vec<PdgParticle<'_>>, output: &ParticleOutput) -> CliResult<()> {
    match output.format {
        OutputFormat::Pretty => print_particles(&particles),
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
    output: &ShowOutput,
) -> CliResult<()> {
    let texts = if output.summary {
        Vec::new()
    } else {
        db.texts_for(&entry.pdg_id)?
    };
    print_title(&format!("{} {}", entry.pdg_id, entry.description), &texts);
    print_entry_summary(entry);

    if let Some(particle) = particle {
        print_particle_identity(particle)?;
        print_headline_properties(particle)?;
        if !output.summary {
            print_related_particles(particle)?;
            print_branching_fractions(particle)?;
            print_branching_ratios(particle)?;
        }
    }

    let data = db.data_for(&entry.pdg_id)?;
    if !data.is_empty() {
        println!("{}", "Data".cyan());
        print_data_entries(&data);
    }

    let children = db.children_for_pdg_id(&entry.pdg_id)?;
    if !output.summary && !children.is_empty() {
        println!("{}", "Child PDG IDs".cyan());
        print_pdg_id_entries(&children);
    }

    let related_entries = db.mapped_entries_for_pdg_id(&entry.pdg_id)?;
    if !output.summary && !related_entries.is_empty() {
        println!("{}", "Related PDG IDs".cyan());
        print_pdg_id_entries(&related_entries);
    }

    if !output.summary {
        let measurements = db.measurements_for(&entry.pdg_id)?;
        print_measurements_for(&entry.pdg_id, &measurements);
        print_unattached_footnotes(&db.footnotes_for(&entry.pdg_id)?, &measurements);
        if output.related_details {
            print_related_details(db, &entry.pdg_id, particle, &children, &related_entries)?;
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
        if let Some(text) = &text.text {
            if !text.is_empty() {
                table.add_row([text.clone()]);
            }
        }
    }
    println!("{table}");
}

fn print_entry_summary(entry: &PdgIdEntry) {
    let mut headers = vec!["PDG ID".to_string(), "Type".to_string()];
    let mut values = vec![entry.pdg_id.to_pdg_id_string(), entry.data_type.to_string()];
    if let Some(parent) = &entry.parent_pdg_id {
        if !parent.is_empty() {
            headers.push("Parent".to_string());
            values.push(parent.to_pdg_id_string());
        }
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
    println!("{}", PdgParticle::make_table(&[particle.clone()], false)?);
    Ok(())
}

fn print_related_details(
    db: &Pdg,
    root_pdg_id: &str,
    particle: Option<&PdgParticle<'_>>,
    children: &[PdgIdEntry],
    related_entries: &[PdgIdEntry],
) -> CliResult<()> {
    let ids = collect_detail_pdg_ids(db, root_pdg_id, particle, children, related_entries)?;
    if ids.is_empty() {
        return Ok(());
    }

    println!("{}", "Related details".cyan());
    for pdg_id in ids {
        let Some(entry) = db.pdg_id(&pdg_id)? else {
            continue;
        };
        let data = db.data_for(&entry.pdg_id)?;
        let texts = db.texts_for(&entry.pdg_id)?;
        let measurements = db.measurements_for(&entry.pdg_id)?;
        let footnotes = db.footnotes_for(&entry.pdg_id)?;
        if data.is_empty() && texts.is_empty() && measurements.is_empty() && footnotes.is_empty() {
            continue;
        }

        print_title(&format!("{} {}", entry.pdg_id, entry.description), &texts);
        print_entry_summary(&entry);
        if !data.is_empty() {
            println!("{}", "Data".cyan());
            print_data_entries(&data);
        }
        print_measurements_for(&entry.pdg_id, &measurements);
        print_unattached_footnotes(&footnotes, &measurements);
    }
    Ok(())
}

fn collect_detail_pdg_ids(
    db: &Pdg,
    root_pdg_id: &str,
    particle: Option<&PdgParticle<'_>>,
    children: &[PdgIdEntry],
    related_entries: &[PdgIdEntry],
) -> CliResult<Vec<String>> {
    let mut seen = HashSet::new();
    let mut ordered = Vec::new();
    let mut stack = Vec::new();

    for entry in children.iter().chain(related_entries.iter()) {
        stack.push(entry.pdg_id.clone());
    }

    if let Some(particle) = particle {
        for decay in particle.branching_fractions()? {
            stack.push(decay.pdg_id);
            for related in decay.related_data {
                stack.push(related.pdg_id);
            }
        }
        for ratio in particle.branching_ratios()? {
            stack.push(ratio.pdg_id);
        }
    }

    while let Some(pdg_id) = stack.pop() {
        if pdg_id == root_pdg_id || !seen.insert(pdg_id.clone()) {
            continue;
        }
        ordered.push(pdg_id.clone());
        for child in db.children_for_pdg_id(&pdg_id)? {
            stack.push(child.pdg_id);
        }
        for related in db.mapped_entries_for_pdg_id(&pdg_id)? {
            stack.push(related.pdg_id);
        }
    }

    Ok(ordered)
}

fn print_headline_properties(particle: &PdgParticle<'_>) -> CliResult<()> {
    let rows = particle.headline_property_rows()?;

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
            row[2].to_pdg_id_string(),
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
            entry.pdgid.to_pdg_id_string(),
            entry.pretty_string(),
            entry.value_type.to_string(),
            entry.comment.clone().unwrap_or_default(),
        ]);
    }
    println!("{table}");
}

fn print_pdg_id_entries(entries: &[PdgIdEntry]) {
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
            entry.pdg_id.to_pdg_id_string(),
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
    output: &ShowOutput,
) -> CliResult<ShowDto> {
    Ok(ShowDto {
        entry: PdgIdEntryDto::from(entry),
        particle: particle.map(ParticleDto::try_from).transpose()?,
        data: db
            .data_for(&entry.pdg_id)?
            .iter()
            .map(DataEntryDto::try_from)
            .collect::<CliResult<Vec<_>>>()?,
        children: if output.summary {
            Vec::new()
        } else {
            db.children_for_pdg_id(&entry.pdg_id)?
                .iter()
                .map(PdgIdEntryDto::from)
                .collect()
        },
        related_entries: if output.summary {
            Vec::new()
        } else {
            db.mapped_entries_for_pdg_id(&entry.pdg_id)?
                .iter()
                .map(PdgIdEntryDto::from)
                .collect()
        },
        texts: (!output.summary)
            .then(|| {
                db.texts_for(&entry.pdg_id)
                    .map(|texts| texts.iter().map(TextDto::from).collect())
            })
            .transpose()?,
        footnotes: (!output.summary)
            .then(|| {
                db.footnotes_for(&entry.pdg_id)
                    .map(|footnotes| footnotes.iter().map(FootnoteDto::from).collect())
            })
            .transpose()?,
        measurements: (!output.summary)
            .then(|| {
                db.measurements_for(&entry.pdg_id)
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

fn has_particle_filters(filters: &ParticleFilters) -> bool {
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

    let table = PdgParticle::make_table(particles, true)?;
    println!("{table}");
    Ok(())
}

fn print_related_particles(particle: &PdgParticle<'_>) -> CliResult<()> {
    let particles = particle.related_particles()?;
    if particles.is_empty() {
        return Ok(());
    }
    println!("{}", "Related particles".cyan());
    let table = PdgParticle::make_table(&particles, true)?;
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
            .map(|entry| entry.pdg_id.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        table.add_row([
            decay.pdg_id.to_pdg_id_string(),
            decay.kind.to_string(),
            decay.value.to_string().to_value_string(),
            decay.description.clone(),
            related_ids
                .split(", ")
                .filter(|pdg_id| !pdg_id.is_empty())
                .map(CliStringExt::to_pdg_id_string)
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
            ratio.pdg_id.to_pdg_id_string(),
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
            .pdg_id(&result.pdg_id)?
            .map(|entry| entry.description)
            .unwrap_or_default();
        table.add_row([
            result.pdg_id.to_pdg_id_string(),
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

fn print_measurements_for(pdg_id: &str, measurements: &[PdgMeasurement]) {
    if measurements.is_empty() {
        return;
    }
    println!("{}", format!("Measurements for {pdg_id}").cyan());
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
