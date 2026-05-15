#![allow(clippy::redundant_pub_crate)]

use comfy_table::{ContentArrangement, Table, presets::UTF8_FULL};
use pdg_rs::{
    DataEntry, DataType, ParticleProperty, Pdg, PdgFootnote, PdgIdEntry, PdgMeasurement,
    PdgMeasurementValue, PdgParticle, PdgReference, PdgResult, PdgText, TextSearchResult,
    TextSearchSource,
};
use serde::Serialize;

use crate::{CliError, CliResult};

#[derive(Serialize)]
pub(crate) struct ParticleDto {
    pdgid: String,
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
        Ok(Self {
            pdgid: particle.pdgid.clone(),
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
    texts: Option<Vec<TextDto>>,
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
    value: DataEntryDto,
    related_data: Vec<RelatedDataDto>,
}

#[derive(Serialize)]
struct BranchingRatioDto {
    pdgid: String,
    description: String,
    mode_number: Option<u32>,
    value: DataEntryDto,
}

#[derive(Serialize)]
struct RelatedDataDto {
    pdgid: String,
    description: String,
    data_type: String,
    mode_number: Option<u32>,
    value: DataEntryDto,
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
pub(crate) struct TextDto {
    pdgid: String,
    text_type: String,
    text: Option<String>,
    sort: isize,
}

impl From<&PdgText> for TextDto {
    fn from(text: &PdgText) -> Self {
        Self {
            pdgid: text.pdgid.clone(),
            text_type: text.text_type.clone(),
            text: text.text.clone(),
            sort: text.sort,
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
pub(crate) struct ShowDto {
    pub(crate) entry: PdgIdEntryDto,
    pub(crate) particle: Option<ParticleDto>,
    pub(crate) data: Vec<DataEntryDto>,
    pub(crate) children: Vec<PdgIdEntryDto>,
    pub(crate) related_entries: Vec<PdgIdEntryDto>,
    pub(crate) texts: Option<Vec<TextDto>>,
    pub(crate) footnotes: Option<Vec<FootnoteDto>>,
    pub(crate) measurements: Option<Vec<MeasurementDto>>,
}

#[derive(Serialize)]
pub(crate) struct PdgIdEntryDto {
    id: isize,
    pdgid: String,
    parent_pdgid: Option<String>,
    description: String,
    mode_number: Option<isize>,
    data_type: String,
    flags: String,
    year_added: Option<isize>,
    sort: isize,
}

impl From<&PdgIdEntry> for PdgIdEntryDto {
    fn from(entry: &PdgIdEntry) -> Self {
        Self {
            id: entry.id,
            pdgid: entry.pdgid.clone(),
            parent_pdgid: entry.parent_pdgid.clone(),
            description: entry.description.clone(),
            mode_number: entry.mode_number,
            data_type: entry.data_type.to_code().to_string(),
            flags: entry.flags.clone(),
            year_added: entry.year_added,
            sort: entry.sort,
        }
    }
}

pub(crate) fn table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .set_content_arrangement(ContentArrangement::DynamicFullWidth);
    table
}

pub(crate) fn headline_property_rows<'pdg>(
    db: &'pdg Pdg,
    particle: &PdgParticle<'pdg>,
) -> PdgResult<Vec<[String; 4]>> {
    Ok(headline_properties(db, particle)?
        .into_iter()
        .map(|property| {
            [
                property.data_type.to_string(),
                property.value.to_string(),
                property.value.pdgid.clone(),
                property.source.to_string(),
            ]
        })
        .collect())
}

pub(crate) fn particle_table(particles: &[PdgParticle<'_>], full: bool) -> CliResult<Table> {
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

fn headline_properties<'pdg>(
    db: &'pdg Pdg,
    particle: &PdgParticle<'pdg>,
) -> PdgResult<Vec<ParticleProperty<'pdg>>> {
    let mut rows = Vec::new();

    for data_type in [DataType::Mass, DataType::Lifetime, DataType::FullWidth] {
        if let Some(property) = particle.property(data_type)? {
            rows.push(property);
        }
    }

    for section in db.children_for_pdgid(&particle.pdgid)? {
        if !matches!(section.data_type, DataType::Section) {
            continue;
        }
        for child in db.children_for_pdgid(&section.pdgid)? {
            if !child.data_type.is_particle_property() {
                continue;
            }
            if matches!(
                child.data_type,
                DataType::Mass | DataType::Lifetime | DataType::FullWidth
            ) {
                continue;
            }
            if rows
                .iter()
                .any(|property| property.data_type == child.data_type)
            {
                continue;
            }
            let data = db.data_for(&child.pdgid)?;
            if let Some(value) = data.into_iter().next() {
                rows.push(ParticleProperty {
                    data_type: child.data_type,
                    value,
                    source: pdg_rs::PropertySource::Section {
                        section_pdgid: section.pdgid.clone(),
                    },
                });
            }
        }
    }

    Ok(rows)
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
