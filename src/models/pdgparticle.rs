use std::{collections::HashMap, fmt::Display, ops::Deref};

use rusqlite::{OptionalExtension, Row, params_from_iter};

use crate::{DataEntry, DataType, LATEST_EDITION, LimitType, Pdg, PdgId, PdgResult};

#[derive(Debug, Clone)]
pub struct PdgParticle<'pdg> {
    pub(crate) db: &'pdg Pdg,
    pub pdg_id: PdgId,
    pub name: String,
    pub cc_type: String,
    pub mcid: Option<isize>,
    pub charge: f64,
    pub quantum_i: Option<String>,
    pub quantum_g: Option<String>,
    pub quantum_j: Option<String>,
    pub quantum_p: Option<String>,
    pub quantum_c: Option<String>,
}

impl<'pdg> PdgParticle<'pdg> {
    pub fn mass(&self) -> PdgResult<Option<Mass>> {
        Ok(self.query_map(DataType::Mass, LATEST_EDITION, |data| {
            ParticleData::try_from(data).ok().map(|data| Mass { data })
        })?)
    }
    pub fn lifetime(&self) -> PdgResult<Option<Lifetime>> {
        Ok(self.query_map(DataType::Lifetime, LATEST_EDITION, |data| {
            ParticleData::try_from(data)
                .ok()
                .map(|data| Lifetime { data })
        })?)
    }
    pub fn branching_fractions(&self) -> PdgResult<Vec<BranchingFraction>> {
        let mut branching_fractions = self.branching_fractions_for(&[
            (
                DataType::ExclusiveBranchingFraction,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction1,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction2,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction3,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction4,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction5,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::InclusiveBranchingFraction,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction1,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction2,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction3,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction4,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction5,
                BranchingFractionKind::Inclusive,
            ),
        ])?;
        self.attach_decay_products(&mut branching_fractions)?;
        Ok(branching_fractions
            .into_iter()
            .map(|branching_fraction| branching_fraction.data)
            .collect())
    }
    pub fn exclusive_branching_fractions(&self) -> PdgResult<Vec<BranchingFraction>> {
        let mut branching_fractions = self.branching_fractions_for(&[
            (
                DataType::ExclusiveBranchingFraction,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction1,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction2,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction3,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction4,
                BranchingFractionKind::Exclusive,
            ),
            (
                DataType::ExclusiveBranchingFraction5,
                BranchingFractionKind::Exclusive,
            ),
        ])?;
        self.attach_decay_products(&mut branching_fractions)?;
        Ok(branching_fractions
            .into_iter()
            .map(|branching_fraction| branching_fraction.data)
            .collect())
    }
    pub fn inclusive_branching_fractions(&self) -> PdgResult<Vec<BranchingFraction>> {
        let mut branching_fractions = self.branching_fractions_for(&[
            (
                DataType::InclusiveBranchingFraction,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction1,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction2,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction3,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction4,
                BranchingFractionKind::Inclusive,
            ),
            (
                DataType::InclusiveBranchingFraction5,
                BranchingFractionKind::Inclusive,
            ),
        ])?;
        self.attach_decay_products(&mut branching_fractions)?;
        Ok(branching_fractions
            .into_iter()
            .map(|branching_fraction| branching_fraction.data)
            .collect())
    }
    pub fn branching_ratios(&self) -> PdgResult<Vec<BranchingRatio>> {
        Ok(self
            .decay_data(DataType::BranchingRatio, LATEST_EDITION)?
            .into_iter()
            .filter_map(|data| {
                let value = ParticleData::try_from(data.data).ok()?;
                Some(BranchingRatio {
                    pdg_id: data.pdg_id,
                    description: data.description,
                    mode_number: data.mode_number,
                    value,
                })
            })
            .collect())
    }
    pub fn query_all_map<P, T>(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
        predicate: P,
    ) -> PdgResult<Vec<T>>
    where
        P: Fn(DataEntry) -> Option<T>,
    {
        Ok(self
            .query_all(data_type, edition)?
            .into_iter()
            .filter_map(predicate)
            .collect())
    }
    pub fn query_map<P, T>(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
        predicate: P,
    ) -> PdgResult<Option<T>>
    where
        P: Fn(DataEntry) -> Option<T>,
    {
        Ok(self.query(data_type, edition)?.map(predicate).flatten())
    }
    pub fn query_all(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
    ) -> PdgResult<Vec<DataEntry>> {
        let sql = format!(
            "SELECT {} FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgid.parent_pdgid = ?2 AND pdgdata.edition = ?3 ORDER BY edition DESC, pdgdata.sort ASC",
            DataEntry::COLUMNS
        );
        let mut stmt = self.db.db().prepare(&sql)?;
        Ok(stmt
            .query_map(
                [&data_type.to_string(), &self.pdg_id, &edition.into()],
                |row| DataEntry::try_from(row),
            )?
            .collect::<Result<Vec<_>, _>>()?)
    }
    pub fn query(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
    ) -> PdgResult<Option<DataEntry>> {
        let sql = format!(
            "SELECT {} FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgid.parent_pdgid = ?2 AND pdgdata.edition = ?3 ORDER BY edition DESC, pdgdata.sort ASC",
            DataEntry::COLUMNS
        );
        let mut stmt = self.db.db().prepare(&sql)?;
        Ok(stmt
            .query_row(
                [&data_type.to_string(), &self.pdg_id, &edition.into()],
                |row| DataEntry::try_from(row),
            )
            .optional()?)
    }

    fn branching_fractions_for(
        &self,
        data_types: &[(DataType, BranchingFractionKind)],
    ) -> PdgResult<Vec<BranchingFractionWithSort>> {
        let mut branching_fractions = Vec::new();
        for (data_type, kind) in data_types {
            branching_fractions.extend(
                self.decay_data(*data_type, LATEST_EDITION)?
                    .into_iter()
                    .filter_map(|data| {
                        let value = ParticleData::try_from(data.data).ok()?;
                        Some(BranchingFractionWithSort {
                            data: BranchingFraction {
                                pdg_id: data.pdg_id,
                                description: data.description,
                                mode_number: data.mode_number,
                                value,
                                kind: *kind,
                                products: Vec::new(),
                            },
                            sort: data.sort,
                        })
                    }),
            );
        }
        branching_fractions.sort_by_key(|branching_fraction| branching_fraction.sort);
        Ok(branching_fractions)
    }

    fn decay_data(
        &self,
        data_type: DataType,
        edition: impl Into<String>,
    ) -> PdgResult<Vec<DecayData>> {
        let data_type = data_type.to_string();
        let edition = edition.into();
        let sql = format!(
            "SELECT {}, pdgid.description, pdgid.mode_number, pdgid.sort FROM pdgdata JOIN pdgid ON pdgid.id = pdgdata.pdgid_id WHERE pdgid.data_type = ?1 AND pdgid.parent_pdgid = ?2 AND pdgdata.edition = ?3 ORDER BY pdgid.sort ASC, pdgdata.sort ASC",
            DataEntry::COLUMNS
        );
        let mut stmt = self.db.db().prepare(&sql)?;
        Ok(stmt
            .query_map([&data_type, &self.pdg_id, &edition], |row| {
                DecayData::try_from(row)
            })?
            .collect::<Result<Vec<_>, _>>()?)
    }

    fn attach_decay_products(
        &self,
        branching_fractions: &mut [BranchingFractionWithSort],
    ) -> PdgResult<()> {
        if branching_fractions.is_empty() {
            return Ok(());
        }

        let pdg_ids = branching_fractions
            .iter()
            .map(|branching_fraction| branching_fraction.data.pdg_id.as_str())
            .collect::<Vec<_>>();
        let placeholders = std::iter::repeat_n("?", pdg_ids.len())
            .collect::<Vec<_>>()
            .join(", ");
        let sql = format!(
            "SELECT pdgid, name, is_outgoing, multiplier FROM pdgdecay WHERE pdgid IN ({placeholders}) ORDER BY pdgid ASC, sort ASC"
        );
        let mut stmt = self.db.db().prepare(&sql)?;
        let mut products_by_pdg_id: HashMap<PdgId, Vec<DecayProduct>> = HashMap::new();
        let rows = stmt.query_map(params_from_iter(pdg_ids), |row| {
            Ok((
                row.get::<_, PdgId>(0)?,
                DecayProduct {
                    name: row.get(1)?,
                    is_outgoing: row.get(2)?,
                    multiplier: row.get::<_, i64>(3)? as usize,
                },
            ))
        })?;

        for row in rows {
            let (pdg_id, product) = row?;
            products_by_pdg_id.entry(pdg_id).or_default().push(product);
        }

        for branching_fraction in branching_fractions {
            branching_fraction.data.products = products_by_pdg_id
                .remove(&branching_fraction.data.pdg_id)
                .unwrap_or_default();
        }
        Ok(())
    }
}

#[derive(Debug)]
struct DecayData {
    pdg_id: PdgId,
    description: String,
    mode_number: Option<usize>,
    data: DataEntry,
    sort: usize,
}

impl TryFrom<&Row<'_>> for DecayData {
    type Error = rusqlite::Error;

    fn try_from(row: &Row<'_>) -> Result<Self, Self::Error> {
        let data = DataEntry::try_from(row)?;
        Ok(Self {
            pdg_id: data.pdgid.clone(),
            description: row.get(DataEntry::COLUMN_COUNT)?,
            mode_number: row
                .get::<_, Option<isize>>(DataEntry::COLUMN_COUNT + 1)?
                .map(|mode_number| mode_number as usize),
            data,
            sort: row.get::<_, isize>(DataEntry::COLUMN_COUNT + 2)? as usize,
        })
    }
}

#[derive(Debug)]
struct BranchingFractionWithSort {
    data: BranchingFraction,
    sort: usize,
}

#[derive(Debug, Clone)]
pub struct ParticleData {
    pub pdg_id: PdgId,
    pub edition: String,
    pub value_type: crate::ValueType,
    pub in_summary_table: bool,
    pub confidence_level: Option<f64>,
    pub limit_type: Option<LimitType>,
    pub comment: Option<String>,
    pub value: f64,
    pub error: Option<(f64, f64)>,
    pub scale_factor: Option<f64>,
    pub unit_text: String,
    pub display_value_text: String,
    pub display_power_of_ten: isize,
    pub display_in_percent: bool,
    pub sort: Option<isize>,
}

impl TryFrom<DataEntry> for ParticleData {
    type Error = DataEntry;

    fn try_from(data: DataEntry) -> Result<Self, Self::Error> {
        let value = match data.value {
            Some(value) => value,
            None => return Err(data),
        };
        Ok(Self {
            pdg_id: data.pdgid,
            edition: data.edition,
            value_type: data.value_type,
            in_summary_table: data.in_summary_table,
            confidence_level: data.confidence_level,
            limit_type: data.limit_type,
            comment: data.comment,
            value,
            error: match (data.error_positive, data.error_negative) {
                (Some(error_positive), Some(error_negative)) => {
                    Some((error_positive, error_negative))
                }
                _ => None,
            },
            scale_factor: data.scale_factor,
            unit_text: data.unit_text,
            display_value_text: data.display_value_text,
            display_power_of_ten: data.display_power_of_ten,
            display_in_percent: data.display_in_percent,
            sort: data.sort,
        })
    }
}
impl Display for ParticleData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let error = self.error.unwrap_or_default();
        write!(
            f,
            "{}+{}-{} {}",
            self.value, error.0, error.1, self.unit_text
        )
    }
}

#[derive(Debug, Clone)]
pub struct Mass {
    pub data: ParticleData,
}
impl Deref for Mass {
    type Target = ParticleData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
impl Display for Mass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.data)
    }
}

#[derive(Debug, Clone)]
pub struct Lifetime {
    pub data: ParticleData,
}
impl Deref for Lifetime {
    type Target = ParticleData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}
impl Display for Lifetime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.data)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchingFractionKind {
    Exclusive,
    Inclusive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecayProduct {
    pub name: String,
    pub is_outgoing: bool,
    pub multiplier: usize,
}

#[derive(Debug, Clone)]
pub struct BranchingFraction {
    pub pdg_id: PdgId,
    pub description: String,
    pub mode_number: Option<usize>,
    pub value: ParticleData,
    pub kind: BranchingFractionKind,
    pub products: Vec<DecayProduct>,
}

#[derive(Debug, Clone)]
pub struct BranchingRatio {
    pub pdg_id: PdgId,
    pub description: String,
    pub mode_number: Option<usize>,
    pub value: ParticleData,
}

#[cfg(test)]
mod tests {
    use crate::{BranchingFractionKind, LimitType, Pdg};

    #[test]
    fn lifetime_uses_lifetime_data_type() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();

        let mass = pion.mass().unwrap().unwrap();
        let lifetime = pion.lifetime().unwrap().unwrap();

        assert!(mass.value > 100.0);
        assert!(lifetime.value < 0.000001);
    }

    #[test]
    fn exclusive_branching_fractions_include_decay_products() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();

        let branching_fractions = pion.exclusive_branching_fractions().unwrap();
        let muon_mode = branching_fractions
            .iter()
            .find(|branching_fraction| branching_fraction.pdg_id == "S008.1")
            .unwrap();

        assert_eq!(muon_mode.description, "pi+ --> mu+ nu_mu");
        assert_eq!(muon_mode.mode_number, Some(1));
        assert_eq!(muon_mode.kind, BranchingFractionKind::Exclusive);
        assert_eq!(muon_mode.products.len(), 3);
        assert_eq!(muon_mode.products[0].name, "pi+");
        assert!(!muon_mode.products[0].is_outgoing);
        assert_eq!(muon_mode.products[1].name, "mu+");
        assert!(muon_mode.products[1].is_outgoing);
    }

    #[test]
    fn inclusive_and_exclusive_branching_fractions_are_grouped() {
        let db = Pdg::open().unwrap();
        let b0 = db.particle("B0").unwrap().unwrap();

        let inclusive = b0.inclusive_branching_fractions().unwrap();
        let exclusive = b0.exclusive_branching_fractions().unwrap();
        let all = b0.branching_fractions().unwrap();

        assert!(inclusive.iter().any(|mode| mode.pdg_id == "S042.94"));
        assert!(exclusive.iter().any(|mode| mode.pdg_id == "S042.30"));
        assert_eq!(all.len(), inclusive.len() + exclusive.len());
        let inclusive_position = all
            .iter()
            .position(|mode| mode.pdg_id == "S042.94")
            .unwrap();
        let exclusive_position = all
            .iter()
            .position(|mode| mode.pdg_id == "S042.30")
            .unwrap();

        assert!(inclusive_position < exclusive_position);
    }

    #[test]
    fn branching_ratios_include_descriptions() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();

        let branching_ratios = pion.branching_ratios().unwrap();
        let ratio = branching_ratios
            .iter()
            .find(|ratio| ratio.pdg_id == "S008R2")
            .unwrap();

        assert_eq!(ratio.description, "G(pi+ --> e+ nu_e)/G(total)");
        assert!(ratio.value.value > 0.0);
    }

    #[test]
    fn branching_ratios_include_errors() {
        let db = Pdg::open().unwrap();
        let sigma = db.search("Sigma(2010)").unwrap().remove(0);

        let branching_ratios = sigma.branching_ratios().unwrap();
        let ratio = branching_ratios
            .iter()
            .find(|ratio| ratio.pdg_id == "B002R1")
            .unwrap();

        assert_eq!(ratio.value.error, Some((0.03, 0.03)));
    }

    #[test]
    fn mass_includes_confidence_level_and_limit_type() {
        let db = Pdg::open().unwrap();
        let down_quark = db.particle("d").unwrap().unwrap();
        let mass = down_quark.mass().unwrap().unwrap();

        assert_eq!(mass.confidence_level, Some(90.0));

        let n1895 = db.particle("N(1895)0").unwrap().unwrap();
        let mass_limit = n1895.mass().unwrap().unwrap();

        assert_eq!(mass_limit.limit_type, Some(LimitType::Range));
    }

    #[test]
    fn lifetime_includes_confidence_level_and_limit_type() {
        let db = Pdg::open().unwrap();
        let proton = db.particle("p").unwrap().unwrap();
        let lifetime = proton.lifetime().unwrap().unwrap();

        assert_eq!(lifetime.confidence_level, Some(90.0));
        assert_eq!(lifetime.limit_type, Some(LimitType::LowerLimit));
    }

    #[test]
    fn upper_limit_branching_fractions_preserve_limit_type() {
        let db = Pdg::open().unwrap();
        let pion = db.particle("pi+").unwrap().unwrap();

        let branching_fractions = pion.exclusive_branching_fractions().unwrap();
        let limit_mode = branching_fractions
            .iter()
            .find(|branching_fraction| branching_fraction.pdg_id == "S008.10")
            .unwrap();

        assert_eq!(limit_mode.value.limit_type, Some(LimitType::UpperLimit));
        assert_eq!(limit_mode.value.error, Some((0.0, 0.0)));
    }
}
