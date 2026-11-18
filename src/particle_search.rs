use std::collections::HashMap;

use crate::{
    DecayChannelMatch, OwnedDecayChannelMatch, ParticleFilterHit, ParticleSearchQuery,
    ParticleSummary, Pdg, PdgParticle, PdgResult,
};

/// How a particle's name or alias matched a ranked search query.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ParticleMatchKind {
    /// The normalized query equals the particle's normalized canonical name.
    ExactName,
    /// The normalized query equals a normalized PDG or common alias.
    ExactAlias,
    /// The normalized query matches the beginning of a name or alias.
    Prefix,
    /// The normalized query appears within the canonical name.
    Substring,
    /// A name or alias matches after correcting a single letter edit or transposition.
    Typo,
}

/// A particle and the spelling that produced its best search match.
#[derive(Debug)]
pub struct ParticleSearchHit<'pdg> {
    /// The matching particle, with its canonical name and identifiers.
    pub particle: PdgParticle<'pdg>,
    /// The reason this particle matched.
    pub kind: ParticleMatchKind,
    /// The original spelling of the canonical name or alias that matched.
    pub matched_name: String,
    /// Matching PDG channels, empty for a search without decay filters.
    pub matching_decays: Vec<DecayChannelMatch<'pdg>>,
}

impl ParticleSearchHit<'_> {
    /// Detaches this hit from the database so it can be stored or serialized.
    #[must_use]
    pub fn into_owned(self) -> OwnedParticleSearchHit {
        OwnedParticleSearchHit {
            particle: ParticleSummary::from(&self.particle),
            kind: self.kind,
            matched_name: self.matched_name,
            matching_decays: self
                .matching_decays
                .iter()
                .map(OwnedDecayChannelMatch::from)
                .collect(),
        }
    }
}

/// A ranked particle search hit that can outlive the database connection.
#[derive(Clone, Debug, serde::Serialize)]
pub struct OwnedParticleSearchHit {
    /// Particle identity and quantum numbers.
    pub particle: ParticleSummary,
    /// The reason the name matched.
    pub kind: ParticleMatchKind,
    /// The original spelling of the name or alias that matched.
    pub matched_name: String,
    /// Matching PDG channels, empty for a search without decay filters.
    pub matching_decays: Vec<OwnedDecayChannelMatch>,
}

impl Pdg {
    /// Finds particles by name or alias, subject to the supplied filters.
    ///
    /// Matching accepts common Greek letters and subscript and superscript spellings.
    /// Exact names rank first, followed by exact aliases, prefixes, substrings, and
    /// typo matches. Typo matching requires at least four letters in the query and
    /// preserves digits, signs, and antiparticle markers. Ties prefer shorter
    /// canonical names, then the database's PDG ID and name ordering. A query
    /// that is empty after normalization returns no hits.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use pdg_rs::{Charge, ParticleSearchQuery, Pdg};
    ///
    /// # fn main() -> pdg_rs::PdgResult<()> {
    /// let pdg = Pdg::open()?;
    /// let hits = pdg.find_particles("K_S^0", ParticleSearchQuery::new().charge(Charge::Neutral))?;
    /// assert_eq!(hits.first().map(|hit| hit.particle.name.as_str()), Some("K(S)0"));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    /// Returns an error if a property range is invalid or a database query fails.
    pub fn find_particles(
        &self,
        text: &str,
        filters: ParticleSearchQuery,
    ) -> PdgResult<Vec<ParticleSearchHit<'_>>> {
        let query = normalize_name(text);
        if query.is_empty() {
            filters.validate_ranges()?;
            return Ok(Vec::new());
        }
        let filtered = self.search_particle_hits(filters)?;
        if filtered.is_empty() {
            return Ok(Vec::new());
        }

        let aliases = self.particle_aliases()?;
        let mut hits = filtered
            .into_iter()
            .filter_map(|hit| {
                let ParticleFilterHit {
                    particle,
                    matching_decays,
                } = hit;
                let aliases = aliases_for_name(&aliases, &particle.name);
                let (kind, matched_name) = std::iter::once((particle.name.as_str(), false))
                    .chain(aliases.map(|alias| (alias, true)))
                    .filter_map(|(spelling, is_alias)| {
                        match_kind(&query, &normalize_name(spelling), is_alias)
                            .map(|kind| (kind, spelling))
                    })
                    .min_by_key(|(kind, spelling)| (*kind, spelling.len(), *spelling))?;
                let matched_name = matched_name.to_owned();
                Some(ParticleSearchHit {
                    particle,
                    kind,
                    matched_name,
                    matching_decays,
                })
            })
            .collect::<Vec<_>>();
        hits.sort_by_key(|hit| (hit.kind, hit.particle.name.chars().count()));
        Ok(hits)
    }

    pub(super) fn resolve_decay_names(&self, states: Vec<String>) -> PdgResult<Vec<Vec<String>>> {
        let mut stmt = self.conn.prepare(
            "SELECT DISTINCT name FROM pdgitem WHERE item_type NOT IN ('A', 'S', 'W') ORDER BY name",
        )?;
        let names = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        let aliases = self.particle_aliases()?;
        Ok(states
            .into_iter()
            .map(|state| {
                let spelling = state.trim();
                // Preserve exact PDG item names before folding case or accepting aliases.
                if names.iter().any(|name| name == spelling) {
                    return vec![spelling.to_owned()];
                }
                let normalized = normalize_name(spelling);
                let exact = names
                    .iter()
                    .filter(|name| normalize_name(name) == normalized)
                    .cloned()
                    .collect::<Vec<_>>();
                if !exact.is_empty() {
                    return exact;
                }
                let resolved = names
                    .iter()
                    .filter(|name| {
                        aliases_for_name(&aliases, name)
                            .any(|alias| normalize_name(alias) == normalized)
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if resolved.is_empty() {
                    vec![spelling.to_owned()]
                } else {
                    resolved
                }
            })
            .collect())
    }

    fn particle_aliases(&self) -> PdgResult<HashMap<String, Vec<String>>> {
        // Follow alias/synonym/redirect targets through groups and multiplets.
        // A particle target is terminal. UNION also stops cycles in the map.
        let mut stmt = self.conn.prepare(
            "WITH RECURSIVE alias_targets(alias, target_id) AS (
                SELECT item.name, mapping.target_id
                FROM pdgitem item JOIN pdgitem_map mapping ON mapping.pdgitem_id = item.id
                WHERE item.item_type IN ('A', 'S', 'W')
                UNION
                SELECT alias_targets.alias, mapping.target_id
                FROM alias_targets
                JOIN pdgitem item ON item.id = alias_targets.target_id
                JOIN pdgitem_map mapping ON mapping.pdgitem_id = item.id
                WHERE item.item_type IN ('A', 'S', 'W', 'G', 'B')
            )
            SELECT item.name, alias_targets.alias
            FROM alias_targets JOIN pdgitem item ON item.id = alias_targets.target_id
            WHERE item.item_type = 'P'",
        )?;
        let mut aliases = HashMap::<String, Vec<String>>::new();
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            aliases.entry(row.get(0)?).or_default().push(row.get(1)?);
        }
        Ok(aliases)
    }
}

fn aliases_for_name<'a>(
    aliases: &'a HashMap<String, Vec<String>>,
    name: &str,
) -> impl Iterator<Item = &'a str> {
    aliases
        .get(name)
        .into_iter()
        .flatten()
        .map(String::as_str)
        .chain(common_aliases(name).iter().copied())
}

fn common_aliases(name: &str) -> &'static [&'static str] {
    match name {
        "p" => &["proton"],
        "pbar" => &["antiproton"],
        "gamma" => &["photon"],
        "e-" => &["electron"],
        "e+" => &["positron"],
        "mu-" => &["muon"],
        "mu+" => &["antimuon"],
        "n" => &["neutron"],
        "nbar" => &["antineutron"],
        "pi+" => &["pion", "positive pion"],
        "pi-" => &["pion", "negative pion"],
        "pi0" => &["pion", "neutral pion"],
        _ => &[],
    }
}

fn match_kind(query: &str, spelling: &str, is_alias: bool) -> Option<ParticleMatchKind> {
    use ParticleMatchKind::{ExactAlias, ExactName, Prefix, Substring, Typo};

    if query == spelling {
        return Some(if is_alias { ExactAlias } else { ExactName });
    }
    // A trailing sign is an explicit charge. A prefix match must not turn + into ++.
    let charge = |name: &str| name.trim_end_matches(['+', '-']).len();
    let query_charge = &query[charge(query)..];
    if !query_charge.is_empty() && query_charge != &spelling[charge(spelling)..] {
        return None;
    }
    if spelling.starts_with(query) && (!is_alias || query.chars().count() >= 2) {
        return Some(Prefix);
    }
    if !is_alias && spelling.contains(query) {
        return Some(Substring);
    }
    (query
        .bytes()
        .filter(u8::is_ascii_alphabetic)
        .take(4)
        .count()
        == 4
        && has_antiparticle_marker(query) == has_antiparticle_marker(spelling)
        && single_letter_typo(query, spelling))
    .then_some(Typo)
}

fn has_antiparticle_marker(name: &str) -> bool {
    name.starts_with("anti") || name.contains("bar")
}

fn single_letter_typo(query: &str, spelling: &str) -> bool {
    let is_separator = |ch: char| !ch.is_ascii_alphabetic();
    // Compare letter runs separately so an edit cannot change a non-letter or
    // move a letter across one, such as swapping the s and 2 in "b_s2".
    query.len().abs_diff(spelling.len()) <= 1
        && query
            .chars()
            .filter(|&ch| is_separator(ch))
            .eq(spelling.chars().filter(|&ch| is_separator(ch)))
        && query
            .split(is_separator)
            .zip(spelling.split(is_separator))
            .filter(|(query_run, spelling_run)| query_run != spelling_run)
            .map(|(query_run, spelling_run)| strsim::osa_distance(query_run, spelling_run))
            .sum::<usize>()
            == 1
}

fn normalize_name(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    for ch in name.chars().flat_map(char::to_lowercase) {
        normalized.push_str(match ch {
            ch if ch.is_whitespace() => continue,
            '_' | '^' => continue,
            'α' => "alpha",
            'β' => "beta",
            'γ' => "gamma",
            'δ' => "delta",
            'ε' => "epsilon",
            'ζ' => "zeta",
            'η' => "eta",
            'θ' => "theta",
            'ι' => "iota",
            'κ' => "kappa",
            'λ' => "lambda",
            'μ' | 'µ' => "mu",
            'ν' => "nu",
            'ξ' => "xi",
            'ο' => "omicron",
            'π' => "pi",
            'ρ' => "rho",
            'σ' | 'ς' => "sigma",
            'τ' => "tau",
            'υ' => "upsilon",
            'φ' | 'ϕ' => "phi",
            'χ' => "chi",
            'ψ' => "psi",
            'ω' => "omega",
            '₀' | '⁰' => "0",
            '₁' | '¹' => "1",
            '₂' | '²' => "2",
            '₃' | '³' => "3",
            '₄' | '⁴' => "4",
            '₅' | '⁵' => "5",
            '₆' | '⁶' => "6",
            '₇' | '⁷' => "7",
            '₈' | '⁸' => "8",
            '₉' | '⁹' => "9",
            '₊' | '⁺' => "+",
            '₋' | '⁻' | '−' => "-",
            'ₑ' => "e",
            'ₗ' | 'ᴸ' => "l",
            'ₛ' | 'ˢ' => "s",
            '\u{0304}' | '\u{0305}' => "bar",
            _ => {
                normalized.push(ch);
                continue;
            }
        });
    }
    if let Some(rest) = normalized.strip_prefix("anti-") {
        return format!("anti{rest}");
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::single_letter_typo;

    #[test]
    fn typo_matching_keeps_non_letters_in_place() {
        assert!(single_letter_typo("alpha2btea", "alpha2beta"));
        assert!(!single_letter_typo("alph2abeta", "alpha2beta"));
        assert!(!single_letter_typo("alpha3beta", "alpha2beta"));
        assert!(!single_letter_typo("alhpa2btea", "alpha2beta"));
    }
}
