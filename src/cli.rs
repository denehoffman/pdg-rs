#![allow(clippy::redundant_pub_crate)]

use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL};
use pdg_rs::{
    BranchingFraction, BranchingRatio, DataEntry, DataType, DataValue, DecayProductSummary,
    EntryContent, EntryDetails, OwnedDecayChannelMatch, ParticleFilterHit, ParticlePropertyDetail,
    ParticleSummary, Pdg, PdgDatabaseInfo, PdgFootnote, PdgIdEntry, PdgMeasurement,
    PdgMeasurementValue, PdgParticle, PdgReference, PdgResult, PdgText, RelatedDataEntry,
    TextSearchCorrection, TextSearchParent, TextSearchResult, TextSearchSource,
};
use serde::Serialize;

use crate::{CliError, CliResult, ShowView};

#[derive(Serialize)]
pub(crate) struct ParticleDto {
    #[serde(flatten)]
    particle: ParticleSummary,
    mass: Option<DataEntryDto>,
    lifetime: Option<DataEntryDto>,
    width: Option<DataEntryDto>,
    related_particles: Option<Vec<ParticleSummaryDto>>,
    branching_fractions: Option<Vec<BranchingFractionDto>>,
    branching_ratios: Option<Vec<BranchingRatioDto>>,
    texts: Option<Vec<PdgText>>,
    footnotes: Option<Vec<FootnoteDto>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    matching_decays: Vec<OwnedDecayChannelMatch>,
}

impl TryFrom<&PdgParticle<'_>> for ParticleDto {
    type Error = CliError;

    fn try_from(particle: &PdgParticle<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            particle: ParticleSummary::from(particle),
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
            matching_decays: Vec::new(),
        })
    }
}

impl TryFrom<&ParticleFilterHit<'_>> for ParticleDto {
    type Error = CliError;

    fn try_from(hit: &ParticleFilterHit<'_>) -> Result<Self, Self::Error> {
        let mut particle = Self::try_from(&hit.particle)?;
        particle.matching_decays = hit
            .matching_decays
            .iter()
            .map(OwnedDecayChannelMatch::from)
            .collect();
        Ok(particle)
    }
}

#[derive(Serialize)]
struct ParticleSummaryDto {
    pdgid: String,
    name: String,
    particle_type: String,
    particle_class: String,
    mcid: Option<isize>,
    charge: String,
}

#[derive(Serialize)]
pub(crate) struct DataEntryDto {
    pdgid: String,
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
    texts: Option<Vec<PdgText>>,
    footnotes: Option<Vec<FootnoteDto>>,
}

impl TryFrom<&DataEntry<'_>> for DataEntryDto {
    type Error = CliError;

    fn try_from(entry: &DataEntry<'_>) -> Result<Self, Self::Error> {
        Ok(Self {
            pdgid: entry.pdgid.clone(),
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
    pdgid: String,
    description: String,
    mode_number: Option<u32>,
    kind: String,
    value: DataValue,
    products: Vec<DecayProductSummary>,
    related_data: Vec<RelatedDataDto>,
}

impl From<&BranchingFraction<'_>> for BranchingFractionDto {
    fn from(decay: &BranchingFraction<'_>) -> Self {
        Self {
            pdgid: decay.pdgid.clone(),
            description: decay.description.clone(),
            mode_number: decay.mode_number,
            kind: decay.kind.to_string(),
            value: decay.value.clone().into(),
            products: decay
                .products
                .iter()
                .map(DecayProductSummary::from)
                .collect(),
            related_data: decay
                .related_data
                .iter()
                .map(RelatedDataDto::from)
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct BranchingRatioDto {
    pdgid: String,
    description: String,
    mode_number: Option<u32>,
    value: DataValue,
}

impl From<&BranchingRatio<'_>> for BranchingRatioDto {
    fn from(ratio: &BranchingRatio<'_>) -> Self {
        Self {
            pdgid: ratio.pdgid.clone(),
            description: ratio.description.clone(),
            mode_number: ratio.mode_number,
            value: ratio.value.clone().into(),
        }
    }
}

#[derive(Serialize)]
struct RelatedDataDto {
    pdgid: String,
    description: String,
    data_type: DataType,
    mode_number: Option<u32>,
    value: DataValue,
}

impl From<&RelatedDataEntry<'_>> for RelatedDataDto {
    fn from(entry: &RelatedDataEntry<'_>) -> Self {
        Self {
            pdgid: entry.pdgid.clone(),
            description: entry.description.clone(),
            data_type: entry.data_type,
            mode_number: entry.mode_number,
            value: entry.value.clone().into(),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct MeasurementDto {
    pdgid: String,
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
    fn from(measurement: &PdgMeasurement) -> Self {
        Self {
            pdgid: measurement.pdgid.clone(),
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
    fn from(reference: &PdgReference) -> Self {
        Self {
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
    fn from(value: &PdgMeasurementValue) -> Self {
        Self {
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
pub(crate) struct TextSearchDto {
    pdgid: String,
    title: Option<String>,
    parent: Option<TextSearchParent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    correction: Option<TextSearchCorrection>,
    source: String,
    text_type: Option<String>,
    sort: Option<isize>,
    text: String,
    snippet: String,
    score: f64,
}

impl From<&TextSearchResult> for TextSearchDto {
    fn from(result: &TextSearchResult) -> Self {
        let (source, text_type, sort) = match &result.source {
            TextSearchSource::Description => ("description".to_string(), None, None),
            TextSearchSource::Text { text_type, sort } => {
                ("text".to_string(), Some(text_type.clone()), Some(*sort))
            }
            TextSearchSource::Footnote { index } => ("footnote".to_string(), None, Some(*index)),
        };
        Self {
            pdgid: result.pdgid.clone(),
            title: result.title.clone(),
            parent: result.parent.clone(),
            correction: result.correction.clone(),
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
pub(crate) struct FootnoteDto {
    pdgid: Option<String>,
    index: Option<isize>,
    text: Option<String>,
    changebar: bool,
}

impl From<&PdgFootnote> for FootnoteDto {
    fn from(footnote: &PdgFootnote) -> Self {
        Self {
            pdgid: footnote.pdgid.clone(),
            index: footnote.index,
            text: footnote.text.clone(),
            changebar: footnote.changebar,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct ShowDto<'a> {
    view: ShowView,
    edition: &'a str,
    database: &'a PdgDatabaseInfo,
    entry: &'a PdgIdEntry,
    particle: Option<ParticleSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    variants: Vec<ParticleSummary>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    properties: &'a [ParticlePropertyDetail],
    #[serde(flatten)]
    content: EntryContentDto<'a>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    branching_fractions: Vec<BranchingFractionDto>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    branching_ratios: Vec<BranchingRatioDto>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    references: &'a [PdgReference],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    children: &'a [PdgIdEntry],
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    related_entries: &'a [PdgIdEntry],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    related_particles: Vec<ParticleSummary>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    related_details: Vec<RelatedEntryDto<'a>>,
}

impl<'a> ShowDto<'a> {
    pub(crate) fn new(
        db: &'a Pdg,
        entry: &'a PdgIdEntry,
        particles: &[PdgParticle<'_>],
        view: ShowView,
        details: &'a EntryDetails<'_>,
    ) -> Self {
        let (particle, variants) = match particles {
            [particle] => (Some(ParticleSummary::from(particle)), Vec::new()),
            _ => (None, particles.iter().map(ParticleSummary::from).collect()),
        };
        Self {
            view,
            edition: db.edition(),
            database: db.database_info(),
            entry,
            particle,
            variants,
            properties: &details.properties,
            content: EntryContentDto::from(&details.content),
            branching_fractions: details
                .branching_fractions
                .iter()
                .map(BranchingFractionDto::from)
                .collect(),
            branching_ratios: details
                .branching_ratios
                .iter()
                .map(BranchingRatioDto::from)
                .collect(),
            references: &details.references,
            children: &details.children,
            related_entries: &details.related_entries,
            related_particles: details
                .related_particles
                .iter()
                .map(ParticleSummary::from)
                .collect(),
            related_details: details
                .related_details
                .iter()
                .map(|related| RelatedEntryDto {
                    entry: &related.entry,
                    content: EntryContentDto::from(&related.content),
                })
                .collect(),
        }
    }
}

#[derive(Serialize)]
struct EntryContentDto<'a> {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    data: Vec<DataValue>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    texts: &'a [PdgText],
    #[serde(skip_serializing_if = "Vec::is_empty")]
    measurements: Vec<MeasurementDto>,
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    footnotes: &'a [PdgFootnote],
}

impl<'a> From<&'a EntryContent<'_>> for EntryContentDto<'a> {
    fn from(content: &'a EntryContent<'_>) -> Self {
        Self {
            data: content.data.iter().cloned().map(DataValue::from).collect(),
            texts: &content.texts,
            measurements: content
                .measurements
                .iter()
                .map(MeasurementDto::from)
                .collect(),
            footnotes: &content.footnotes,
        }
    }
}

#[derive(Serialize)]
struct RelatedEntryDto<'a> {
    entry: &'a PdgIdEntry,
    #[serde(flatten)]
    content: EntryContentDto<'a>,
}

pub(crate) fn table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth);
    table
}

pub(crate) fn particle_table<'a, 'pdg: 'a>(
    particles: impl IntoIterator<Item = &'a PdgParticle<'pdg>>,
    full: bool,
) -> CliResult<Table> {
    let mut table = table();
    if full {
        table.set_header([
            "PDG ID", "Name", "Class", "Type", "MCID", "Mass", "Lifetime", "Width", "Quantum",
        ]);
        for particle in particles {
            let [mass, lifetime, width] = property_summary(particle)?;
            table.add_row([
                particle.pdgid.clone(),
                particle.name.clone(),
                particle.particle_class.to_string(),
                particle.particle_type.to_string(),
                particle
                    .mcid
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                mass,
                lifetime,
                width,
                particle.quantum_summary(),
            ]);
        }
    } else {
        table.set_header(["Name", "PDG ID", "Class", "Type", "MCID", "Quantum"]);
        for particle in particles {
            table.add_row([
                particle.name.clone(),
                particle.pdgid.clone(),
                particle.particle_class.to_string(),
                particle.particle_type.to_string(),
                particle
                    .mcid
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                particle.quantum_summary(),
            ]);
        }
    }
    Ok(table)
}

fn property_summary(particle: &PdgParticle<'_>) -> PdgResult<[String; 3]> {
    let mut mass = String::new();
    let mut lifetime = String::new();
    let mut width = String::new();

    if let Some(property) = particle.property(DataType::Mass)? {
        mass = property.value.to_string();
    }
    if let Some(property) = particle.property(DataType::Lifetime)? {
        lifetime = property.value.to_string();
    }
    if let Some(property) = particle.property(DataType::FullWidth)? {
        width = property.value.to_string();
    }

    Ok([mass, lifetime, width])
}
