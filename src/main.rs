use std::str::FromStr;

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use comfy_table::{Table, presets::UTF8_FULL};
use owo_colors::OwoColorize;
use pdg_rs::{
    AngularMomentum, BranchingFraction, BranchingFractionKind, BranchingRatio, Charge, DataEntry,
    DecayStateExpansion, Isospin, Parity, ParticleClass, ParticleSearchQuery, ParticleType, Pdg,
    PdgError, PdgFootnote, PdgMeasurement, PdgMeasurementValue, PdgParticle, PdgReference, PdgText,
    TextSearchResult, TextSearchSource,
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
    /// Search particles by name and particle properties.
    Particle(ParticleCommand),
    /// Look up a particle by string PDG ID, such as S008.
    Pdgid(PdgIdCommand),
    /// Search PDG descriptions and text entries.
    Text(TextCommand),
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
struct PdgIdCommand {
    /// String PDG ID, such as S008.
    pdg_id: String,
    #[command(flatten)]
    output: ParticleOutput,
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
    #[arg(long, value_parser = parse_class)]
    class: Option<ParticleClass>,
    #[arg(long = "type", value_parser = parse_particle_type)]
    particle_type: Option<ParticleType>,
    #[arg(long, allow_hyphen_values = true, value_parser = parse_charge)]
    charge: Option<Charge>,
    #[arg(long, value_parser = parse_isospin_filter)]
    isospin: Option<QuantumArg<Isospin>>,
    #[arg(long, value_parser = parse_parity_filter)]
    g_parity: Option<QuantumArg<Parity>>,
    #[arg(long, value_parser = parse_spin_filter)]
    spin: Option<QuantumArg<AngularMomentum>>,
    #[arg(long, value_parser = parse_parity_filter)]
    parity: Option<QuantumArg<Parity>>,
    #[arg(long, value_parser = parse_parity_filter)]
    c_parity: Option<QuantumArg<Parity>>,
    #[arg(long, value_parser = parse_range)]
    mass: Option<(f64, f64)>,
    #[arg(long, value_parser = parse_range)]
    width: Option<(f64, f64)>,
    #[arg(long, value_parser = parse_range)]
    lifetime: Option<(f64, f64)>,
    #[arg(long, value_delimiter = ',')]
    decays_to: Vec<String>,
    #[arg(long, value_delimiter = ',')]
    decay_contains: Vec<String>,
    #[arg(long, value_delimiter = ',')]
    decays_from: Vec<String>,
    #[arg(long, value_enum, default_value_t = DecayExpansionArg::Inclusive)]
    decay_expansion: DecayExpansionArg,
    #[arg(long)]
    mcid: Option<isize>,
}

#[derive(Parser, Clone)]
struct ParticleOutput {
    #[arg(long, value_enum, default_value_t = OutputFormat::Pretty)]
    format: OutputFormat,
    #[arg(long, default_value_t = 20)]
    limit: usize,
    #[arg(long)]
    all: bool,
    #[arg(long)]
    details: bool,
    #[arg(long)]
    with_related: bool,
    #[arg(long)]
    with_decays: bool,
    #[arg(long)]
    with_ratios: bool,
    #[arg(long)]
    with_measurements: bool,
    #[arg(long)]
    with_texts: bool,
    #[arg(long)]
    with_footnotes: bool,
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
    comment: Option<String>,
    values: Vec<MeasurementValueDto>,
    footnotes: Vec<FootnoteDto>,
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

#[derive(Serialize)]
struct MeasurementValueDto {
    column_name: Option<String>,
    display: String,
    value: Option<f64>,
    error_positive: Option<f64>,
    error_negative: Option<f64>,
    stat_error_positive: Option<f64>,
    stat_error_negative: Option<f64>,
    syst_error_positive: Option<f64>,
    syst_error_negative: Option<f64>,
    used_in_average: bool,
    used_in_fit: bool,
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

#[derive(Serialize)]
struct TextDto {
    pdg_id: String,
    text_type: String,
    text: Option<String>,
    sort: isize,
}

#[derive(Serialize)]
struct FootnoteDto {
    pdg_id: Option<String>,
    index: Option<isize>,
    text: Option<String>,
    changebar: bool,
}

fn main() -> CliResult<()> {
    let cli = Cli::parse();
    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        println!();
        return Ok(());
    };

    match command {
        Commands::Particle(command) => run_particle(command),
        Commands::Pdgid(command) => run_pdgid(command),
        Commands::Text(command) => run_text(command),
        Commands::Tui => Err(CliError::InvalidArgument(
            "TUI is not implemented yet; use `pdg particle`, `pdg pdgid`, or `pdg text`".into(),
        )),
    }
}

fn run_particle(command: ParticleCommand) -> CliResult<()> {
    let db = Pdg::open()?;
    let mut particles = if let Some(mcid) = command.filters.mcid {
        db.mcid(mcid)?.into_iter().collect()
    } else {
        db.search_particles(build_query(command.query, &command.filters)?)?
    };

    particles = apply_limit(particles, command.output.limit, command.output.all);
    output_particles(&db, particles, &command.output)
}

fn run_pdgid(command: PdgIdCommand) -> CliResult<()> {
    let db = Pdg::open()?;
    let particle = db
        .search_particles(ParticleSearchQuery::new())?
        .into_iter()
        .find(|particle| particle.pdg_id.eq_ignore_ascii_case(&command.pdg_id))
        .ok_or_else(|| CliError::NotFound(command.pdg_id.clone()))?;

    output_particles(&db, vec![particle], &command.output)
}

fn run_text(command: TextCommand) -> CliResult<()> {
    let db = Pdg::open()?;
    let results = apply_limit(db.search_text(command.query)?, command.limit, command.all);
    match command.format {
        OutputFormat::Pretty => print_text_results(&results, command.show_full_text),
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(
                    &results.iter().map(text_search_dto).collect::<Vec<_>>()
                )?
            );
            Ok(())
        }
    }
}

fn output_particles(
    db: &Pdg,
    particles: Vec<PdgParticle<'_>>,
    output: &ParticleOutput,
) -> CliResult<()> {
    match output.format {
        OutputFormat::Pretty => print_particles(&particles, output),
        OutputFormat::Json => {
            let dtos = particles
                .iter()
                .map(|particle| particle_dto(db, particle, output))
                .collect::<CliResult<Vec<_>>>()?;
            println!("{}", serde_json::to_string_pretty(&dtos)?);
            Ok(())
        }
    }
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

fn print_particles(particles: &[PdgParticle<'_>], output: &ParticleOutput) -> CliResult<()> {
    if particles.is_empty() {
        println!("{}", "No particles found.".yellow());
        return Ok(());
    }

    if !output.details
        && !output.with_related
        && !output.with_decays
        && !output.with_ratios
        && !output.with_measurements
        && !output.with_texts
        && !output.with_footnotes
    {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL);
        table.set_header([
            "PDG ID", "Name", "Class", "Type", "Charge", "MCID", "Mass", "Lifetime", "Width",
            "Decays", "Ratios", "Quantum",
        ]);
        for particle in particles {
            let decays = particle.branching_fractions()?.len();
            let ratios = particle.branching_ratios()?.len();
            table.add_row([
                particle.pdg_id.clone(),
                particle.name.clone(),
                particle.particle_class.to_string(),
                particle.particle_type.to_string(),
                particle.charge.to_string(),
                particle
                    .mcid
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                particle
                    .mass()?
                    .map(|entry| entry.to_string())
                    .unwrap_or_default(),
                particle
                    .lifetime()?
                    .map(|entry| entry.to_string())
                    .unwrap_or_default(),
                particle
                    .width()?
                    .map(|entry| entry.to_string())
                    .unwrap_or_default(),
                decays.to_string(),
                ratios.to_string(),
                quantum_summary(particle),
            ]);
        }
        println!("{table}");
        return Ok(());
    }

    for (index, particle) in particles.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("{}", particle.to_string().bold());
        print_core_data(particle)?;

        if output.with_related {
            print_related_particles(particle)?;
        }
        if output.with_decays {
            print_branching_fractions(particle, output)?;
        }
        if output.with_ratios {
            print_branching_ratios(particle, output)?;
        }
        if output.with_texts {
            print_texts(&particle.texts()?);
        }
        if output.with_footnotes {
            print_footnotes(&particle.footnotes()?);
        }
    }
    Ok(())
}

fn print_core_data(particle: &PdgParticle<'_>) -> CliResult<()> {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(["Property", "Value", "PDG ID"]);
    if let Some(mass) = particle.mass()? {
        table.add_row(["mass".to_string(), mass.to_string(), mass.pdgid]);
    }
    if let Some(lifetime) = particle.lifetime()? {
        table.add_row(["lifetime".to_string(), lifetime.to_string(), lifetime.pdgid]);
    }
    if let Some(width) = particle.width()? {
        table.add_row(["width".to_string(), width.to_string(), width.pdgid]);
    }
    println!("{table}");
    Ok(())
}

fn print_related_particles(particle: &PdgParticle<'_>) -> CliResult<()> {
    let related = particle.related_particles()?;
    if related.is_empty() {
        return Ok(());
    }
    println!("{}", "Related particles".cyan());
    for related_particle in related {
        println!("  {related_particle}");
    }
    Ok(())
}

fn print_branching_fractions(particle: &PdgParticle<'_>, output: &ParticleOutput) -> CliResult<()> {
    let decays = particle.branching_fractions()?;
    if decays.is_empty() {
        return Ok(());
    }
    println!("{}", "Branching fractions".cyan());
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(["PDG ID", "Kind", "Value", "Description"]);
    for decay in &decays {
        table.add_row([
            decay.pdg_id.clone(),
            branching_kind(decay.kind).to_string(),
            decay.value.to_string(),
            decay.description.clone(),
        ]);
    }
    println!("{table}");

    if output.with_measurements {
        for decay in decays {
            print_measurements(&decay.measurements()?);
            for related in &decay.related_data {
                print_measurements(&related.measurements()?);
            }
        }
    }
    Ok(())
}

fn print_branching_ratios(particle: &PdgParticle<'_>, output: &ParticleOutput) -> CliResult<()> {
    let ratios = particle.branching_ratios()?;
    if ratios.is_empty() {
        return Ok(());
    }
    println!("{}", "Branching ratios".cyan());
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(["PDG ID", "Value", "Description"]);
    for ratio in &ratios {
        table.add_row([
            ratio.pdg_id.clone(),
            ratio.value.to_string(),
            ratio.description.clone(),
        ]);
    }
    println!("{table}");

    if output.with_measurements {
        for ratio in ratios {
            print_measurements(&ratio.measurements()?);
        }
    }
    Ok(())
}

fn print_text_results(results: &[TextSearchResult], show_full_text: bool) -> CliResult<()> {
    if results.is_empty() {
        println!("{}", "No text results found.".yellow());
        return Ok(());
    }
    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(["PDG ID", "Source", "Score", "Text"]);
    for result in results {
        table.add_row([
            result.pdg_id.clone(),
            source_label(&result.source),
            format!("{:.3}", result.score),
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

fn print_texts(texts: &[PdgText]) {
    for text in texts {
        println!(
            "{} {}: {}",
            text.pdg_id.cyan(),
            text.text_type,
            text.text.clone().unwrap_or_default()
        );
    }
}

fn print_footnotes(footnotes: &[PdgFootnote]) {
    for footnote in footnotes {
        println!(
            "[{}] {}",
            footnote
                .index
                .map(|index| index.to_string())
                .unwrap_or_default(),
            footnote.text.clone().unwrap_or_default()
        );
    }
}

fn print_measurements(measurements: &[PdgMeasurement]) {
    if measurements.is_empty() {
        return;
    }
    println!("{}", "Measurements".cyan());
    for measurement in measurements {
        let reference = reference_label(&measurement.reference);
        let values = measurement
            .values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        println!("  {:<24} {}", measurement.reference.document_id, reference);
        if !values.is_empty() {
            println!("  {:<24} {}", "", values);
        }
    }
}

fn particle_dto(
    _db: &Pdg,
    particle: &PdgParticle<'_>,
    output: &ParticleOutput,
) -> CliResult<ParticleDto> {
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
            .map(|entry| data_entry_dto(&entry, output))
            .transpose()?,
        lifetime: particle
            .lifetime()?
            .map(|entry| data_entry_dto(&entry, output))
            .transpose()?,
        width: particle
            .width()?
            .map(|entry| data_entry_dto(&entry, output))
            .transpose()?,
        related_particles: output
            .with_related
            .then(|| {
                particle
                    .related_particles()
                    .map(|particles| particles.iter().map(summary_dto).collect::<Vec<_>>())
            })
            .transpose()?,
        branching_fractions: output
            .with_decays
            .then(|| {
                particle.branching_fractions().and_then(|decays| {
                    decays
                        .iter()
                        .map(|decay| branching_fraction_dto(decay, output))
                        .collect::<CliResult<Vec<_>>>()
                        .map_err(|err| PdgError::Custom(err.to_string()))
                })
            })
            .transpose()?,
        branching_ratios: output
            .with_ratios
            .then(|| {
                particle.branching_ratios().and_then(|ratios| {
                    ratios
                        .iter()
                        .map(|ratio| branching_ratio_dto(ratio, output))
                        .collect::<CliResult<Vec<_>>>()
                        .map_err(|err| PdgError::Custom(err.to_string()))
                })
            })
            .transpose()?,
        texts: output
            .with_texts
            .then(|| {
                particle
                    .texts()
                    .map(|texts| texts.iter().map(text_dto).collect())
            })
            .transpose()?,
        footnotes: output
            .with_footnotes
            .then(|| {
                particle
                    .footnotes()
                    .map(|footnotes| footnotes.iter().map(footnote_dto).collect())
            })
            .transpose()?,
    })
}

fn summary_dto(particle: &PdgParticle<'_>) -> ParticleSummaryDto {
    ParticleSummaryDto {
        pdg_id: particle.pdg_id.clone(),
        name: particle.name.clone(),
        particle_type: particle.particle_type.to_string(),
        particle_class: particle.particle_class.to_string(),
        mcid: particle.mcid,
        charge: particle.charge.to_string(),
    }
}

fn data_entry_dto(entry: &DataEntry<'_>, output: &ParticleOutput) -> CliResult<DataEntryDto> {
    Ok(DataEntryDto {
        pdg_id: entry.pdgid.clone(),
        edition: entry.edition.clone(),
        value_type: entry.value_type.to_string(),
        display: entry.to_string(),
        unit: entry.unit_text.clone(),
        comment: entry.comment.clone(),
        value: entry.value,
        error_positive: entry.error_positive,
        error_negative: entry.error_negative,
        confidence_level: entry.confidence_level,
        limit_type: entry.limit_type.map(|limit| limit.to_string()),
        in_summary_table: entry.in_summary_table,
        measurements: output
            .with_measurements
            .then(|| {
                entry
                    .measurements()
                    .map(|measurements| measurements.iter().map(measurement_dto).collect())
            })
            .transpose()?,
        texts: output
            .with_texts
            .then(|| {
                entry
                    .texts()
                    .map(|texts| texts.iter().map(text_dto).collect())
            })
            .transpose()?,
        footnotes: output
            .with_footnotes
            .then(|| {
                entry
                    .footnotes()
                    .map(|footnotes| footnotes.iter().map(footnote_dto).collect())
            })
            .transpose()?,
    })
}

fn branching_fraction_dto(
    decay: &BranchingFraction<'_>,
    output: &ParticleOutput,
) -> CliResult<BranchingFractionDto> {
    Ok(BranchingFractionDto {
        pdg_id: decay.pdg_id.clone(),
        description: decay.description.clone(),
        mode_number: decay.mode_number,
        kind: branching_kind(decay.kind).to_string(),
        value: data_entry_dto(&decay.value, output)?,
        related_data: decay
            .related_data
            .iter()
            .map(|related| {
                Ok(RelatedDataDto {
                    pdg_id: related.pdg_id.clone(),
                    description: related.description.clone(),
                    data_type: related.data_type.to_string(),
                    mode_number: related.mode_number,
                    value: data_entry_dto(&related.value, output)?,
                })
            })
            .collect::<CliResult<Vec<_>>>()?,
    })
}

fn branching_ratio_dto(
    ratio: &BranchingRatio<'_>,
    output: &ParticleOutput,
) -> CliResult<BranchingRatioDto> {
    Ok(BranchingRatioDto {
        pdg_id: ratio.pdg_id.clone(),
        description: ratio.description.clone(),
        mode_number: ratio.mode_number,
        value: data_entry_dto(&ratio.value, output)?,
    })
}

fn measurement_dto(measurement: &PdgMeasurement) -> MeasurementDto {
    MeasurementDto {
        pdg_id: measurement.pdg_id.clone(),
        reference: reference_dto(&measurement.reference),
        event_count: measurement.event_count.clone(),
        confidence_level: measurement.confidence_level,
        place: measurement.place.clone(),
        technique: measurement.technique.clone(),
        charge: measurement.charge.clone(),
        comment: measurement.comment.clone(),
        values: measurement
            .values
            .iter()
            .map(measurement_value_dto)
            .collect(),
        footnotes: measurement.footnotes.iter().map(footnote_dto).collect(),
    }
}

fn reference_dto(reference: &PdgReference) -> ReferenceDto {
    ReferenceDto {
        document_id: reference.document_id.clone(),
        publication_name: reference.publication_name.clone(),
        publication_year: reference.publication_year,
        doi: reference.doi.clone(),
        inspire_id: reference.inspire_id.clone(),
        title: reference.title.clone(),
    }
}

fn measurement_value_dto(value: &PdgMeasurementValue) -> MeasurementValueDto {
    MeasurementValueDto {
        column_name: value.column_name.clone(),
        display: value.to_string(),
        value: value.value,
        error_positive: value.error_positive,
        error_negative: value.error_negative,
        stat_error_positive: value.stat_error_positive,
        stat_error_negative: value.stat_error_negative,
        syst_error_positive: value.syst_error_positive,
        syst_error_negative: value.syst_error_negative,
        used_in_average: value.used_in_average,
        used_in_fit: value.used_in_fit,
    }
}

fn text_search_dto(result: &TextSearchResult) -> TextSearchDto {
    let (source, text_type, sort) = match &result.source {
        TextSearchSource::Description => ("description".to_string(), None, None),
        TextSearchSource::Text { text_type, sort } => {
            ("text".to_string(), Some(text_type.clone()), Some(*sort))
        }
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

fn text_dto(text: &PdgText) -> TextDto {
    TextDto {
        pdg_id: text.pdg_id.clone(),
        text_type: text.text_type.clone(),
        text: text.text.clone(),
        sort: text.sort,
    }
}

fn footnote_dto(footnote: &PdgFootnote) -> FootnoteDto {
    FootnoteDto {
        pdg_id: footnote.pdg_id.clone(),
        index: footnote.index,
        text: footnote.text.clone(),
        changebar: footnote.changebar,
    }
}

fn quantum_summary(particle: &PdgParticle<'_>) -> String {
    [
        particle
            .quantum_i
            .as_ref()
            .map(|value| format!("I={value}")),
        particle
            .quantum_g
            .as_ref()
            .map(|value| format!("G={value}")),
        particle
            .quantum_j
            .as_ref()
            .map(|value| format!("J={value}")),
        particle
            .quantum_p
            .as_ref()
            .map(|value| format!("P={value}")),
        particle
            .quantum_c
            .as_ref()
            .map(|value| format!("C={value}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(", ")
}

fn source_label(source: &TextSearchSource) -> String {
    match source {
        TextSearchSource::Description => "description".to_string(),
        TextSearchSource::Text { text_type, sort } => format!("text:{text_type}:{sort}"),
    }
}

fn reference_label(reference: &PdgReference) -> String {
    reference
        .doi
        .clone()
        .or(reference.inspire_id.clone())
        .or(reference.title.clone())
        .unwrap_or_default()
}

fn branching_kind(kind: BranchingFractionKind) -> &'static str {
    match kind {
        BranchingFractionKind::Exclusive => "exclusive",
        BranchingFractionKind::Inclusive => "inclusive",
    }
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
