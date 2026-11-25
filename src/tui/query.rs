use std::ops::Range;

use pdg_rs::{DecayStateExpansion, Pdg, PdgResult};

use super::{
    SearchInput,
    filters::{Draft, Field, decay_token},
};
use crate::{CliError, CliResult};

const UNKNOWN_VALUES: &str = "--include-unknown-properties";

pub(super) struct InlineQuery {
    pub(super) name: String,
    pub(super) draft: Draft,
}

impl InlineQuery {
    pub(super) fn parse(input: &str) -> CliResult<Self> {
        let mut name = Vec::new();
        let mut options = Vec::new();
        for token in tokens(input) {
            if !token.closed {
                return invalid("Close the quoted search value.");
            }
            let Some((tag, raw)) = tag_value(token.raw) else {
                name.push(decode(token.raw));
                continue;
            };
            let Some(flag) = find_flag(tag) else {
                return invalid(format!("Unknown filter {tag}."));
            };
            if options.iter().any(|(existing, _)| *existing == flag) {
                return invalid(format!(
                    "Use {tag} once. Separate decay states with commas."
                ));
            }
            let value = decode(raw);
            if value.trim().is_empty() {
                return invalid(format!("Enter a value for {tag}."));
            }
            if flag == UNKNOWN_VALUES && !matches!(value.as_str(), "true" | "false") {
                return invalid(format!("Use true or false for {tag}."));
            }
            options.push((flag, value));
        }
        if options.is_empty() {
            return Ok(Self {
                name: name.join(" "),
                draft: Draft::default(),
            });
        }
        let options = options.iter().filter_map(|&(flag, ref value)| {
            if flag == UNKNOWN_VALUES {
                (value == "true").then_some((flag, None))
            } else {
                Some((flag, Some(value.as_str())))
            }
        });
        Ok(Self {
            name: name.join(" "),
            draft: Draft::from_options(options)?,
        })
    }
}

fn invalid<T>(message: impl Into<String>) -> CliResult<T> {
    Err(CliError::InvalidArgument(message.into()))
}

pub(super) fn format_query(name: &str, draft: &Draft) -> String {
    let mut tokens = Vec::new();
    if !name.is_empty() {
        tokens.push(quote_name(name));
    }
    tokens.extend(
        draft
            .command_options()
            .map(|(flag, value)| format!("{}:{}", tag(flag), quote(value.unwrap_or("true")))),
    );
    tokens.join(" ")
}

pub(super) fn quote_name(name: &str) -> String {
    if name.contains([':', '"']) {
        quote(name)
    } else {
        name.to_owned()
    }
}

fn quote(value: &str) -> String {
    if value.chars().any(char::is_whitespace) || value.contains(['"', ':']) {
        format!("\"{}\"", escape(value))
    } else {
        value.to_owned()
    }
}

fn escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn decode(raw: &str) -> String {
    let mut decoded = String::with_capacity(raw.len());
    let mut quoted = false;
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => quoted = !quoted,
            '\\' if quoted && matches!(chars.peek(), Some('\\' | '"')) => {
                decoded.extend(chars.next());
            }
            _ => decoded.push(ch),
        }
    }
    decoded
}

struct Token<'a> {
    raw: &'a str,
    range: Range<usize>,
    closed: bool,
}

fn tokens(input: &str) -> impl Iterator<Item = Token<'_>> {
    let mut start = 0;
    std::iter::from_fn(move || {
        let tail = input[start..].trim_start();
        if tail.is_empty() {
            return None;
        }
        start = input.len() - tail.len();
        let mut quoted = false;
        let mut escaped = false;
        let mut end = tail.len();
        for (index, ch) in tail.char_indices() {
            if escaped {
                escaped = false;
            } else if ch == '\\' && quoted {
                escaped = true;
            } else if ch == '"' {
                quoted = !quoted;
            } else if ch.is_whitespace() && !quoted {
                end = index;
                break;
            }
        }
        let range = start..start + end;
        start = range.end;
        Some(Token {
            raw: &tail[..end],
            range,
            closed: !quoted,
        })
    })
}

fn tag_value(token: &str) -> Option<(&str, &str)> {
    let (key, value) = token.split_once(':')?;
    (!key.is_empty() && key.bytes().all(|ch| ch.is_ascii_alphabetic() || ch == b'-'))
        .then_some((key, value))
}

fn tag(flag: &str) -> &str {
    flag.trim_start_matches('-')
}

fn find_flag(key: &str) -> Option<&'static str> {
    Field::flags().find(|flag| tag(flag).eq_ignore_ascii_case(key))
}

pub(super) fn editing_filter_value(input: &SearchInput) -> bool {
    tokens(&input.text).any(|token| {
        (token.range.start..=token.range.end).contains(&input.cursor)
            && tag_value(token.raw).is_some_and(|(key, _)| {
                input.cursor > token.range.start + key.len() && find_flag(key).is_some()
            })
    })
}

pub(super) struct Suggestion {
    pub(super) label: String,
    replacement: String,
    cursor: usize,
}

pub(super) struct Completions {
    range: Range<usize>,
    pub(super) suggestions: Vec<Suggestion>,
}

impl Completions {
    pub(super) fn accept(&self, input: &mut SearchInput, index: usize) {
        if let Some(suggestion) = self.suggestions.get(index) {
            input
                .text
                .replace_range(self.range.clone(), &suggestion.replacement);
            input.cursor = self.range.start + suggestion.cursor;
        }
    }
}

pub(super) fn complete(db: &Pdg, input: &SearchInput) -> PdgResult<Completions> {
    let token = tokens(&input.text)
        .find(|token| (token.range.start..=token.range.end).contains(&input.cursor))
        .unwrap_or(Token {
            raw: "",
            range: input.cursor..input.cursor,
            closed: true,
        });
    let position = input.cursor - token.range.start;
    let suggestions = if let Some((key, raw)) = tag_value(token.raw)
        && position > key.len()
        && let Some(flag) = find_flag(key)
    {
        complete_value(db, flag, raw, position - key.len() - 1, &input.text)?
    } else {
        let prefix = token.raw[..position]
            .split_once(':')
            .map_or(&token.raw[..position], |(key, _)| key)
            .to_ascii_lowercase();
        Field::flags()
            .map(tag)
            .filter(|key| key.starts_with(&prefix))
            .map(|key| {
                let tail = token.raw.split_once(':').map_or("", |(_, value)| value);
                Suggestion {
                    label: format!("{key}:"),
                    replacement: format!("{key}:{tail}"),
                    cursor: key.len() + 1,
                }
            })
            .collect()
    };
    Ok(Completions {
        range: token.range,
        suggestions,
    })
}

fn complete_value(
    db: &Pdg,
    flag: &str,
    raw: &str,
    cursor: usize,
    query: &str,
) -> PdgResult<Vec<Suggestion>> {
    let value = decode(raw);
    let position = decode(&raw[..cursor]).len();
    let field = Field::for_flag(flag).expect("all filter tags map to form fields");
    let (names, active) = if let Some(side) = field.decay_side() {
        let active = decay_token(&value, position);
        let literal = tokens(query).any(|token| {
            tag_value(token.raw).is_some_and(|(key, value)| {
                key.eq_ignore_ascii_case("decay-expansion") && decode(value) == "literal"
            })
        });
        let expansion = if literal {
            DecayStateExpansion::Literal
        } else {
            DecayStateExpansion::Inclusive
        };
        (
            db.suggest_decay_states(&value[active.clone()], side, expansion, 8)?,
            active,
        )
    } else {
        let choices = if flag == UNKNOWN_VALUES {
            &["true", "false"][..]
        } else {
            field.choices()
        };
        let prefix = value[..position].to_ascii_lowercase();
        (
            choices
                .iter()
                .filter(|choice| choice.starts_with(&prefix))
                .map(|choice| (*choice).into())
                .collect(),
            0..value.len(),
        )
    };
    Ok(names
        .into_iter()
        .map(|name| {
            let mut filled = value.clone();
            filled.replace_range(active.clone(), &name);
            let position = active.start + name.len();
            let encoded = quote(&filled);
            let cursor = if encoded.starts_with('"') {
                // Keep the cursor at the end of the inserted product, within a quoted list.
                1 + escape(&filled[..position]).len()
            } else {
                position
            };
            let key = tag(flag);
            Suggestion {
                label: name,
                replacement: format!("{key}:{encoded}"),
                cursor: key.len() + 1 + cursor,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_tags_round_trip_aliases_quotes_signed_values_and_flags() {
        let query = InlineQuery::parse(
            r#"f980 decays-to:"neutral pion,neutral pion" c-parity:plus type:self spin:1/2 mcid:-211 include-unknown-properties:true mass:900..1100"#,
        ).unwrap();
        assert_eq!(query.name, "f980");
        assert_eq!(
            query.draft.command_options().collect::<Vec<_>>(),
            [
                ("--type", Some("self-conjugate")),
                ("--mcid", Some("-211")),
                ("--spin", Some("1/2")),
                ("--c-parity", Some("+")),
                ("--mass", Some("900..1100")),
                ("--include-unknown-properties", None),
                ("--decays-to", Some("neutral pion,neutral pion")),
            ]
        );
        let formatted = format_query(&query.name, &query.draft);
        let restored = InlineQuery::parse(&formatted).unwrap();
        assert_eq!(restored.name, query.name);
        assert!(
            restored
                .draft
                .command_options()
                .eq(query.draft.command_options())
        );
        assert_eq!(
            InlineQuery::parse(r#""mass:label""#).unwrap().name,
            "mass:label"
        );
    }

    #[test]
    fn inline_filter_errors_are_reported_before_search() {
        for input in [
            "mass:",
            "mass:2000..1000",
            "unknown-tag:0",
            "charge:0 charge:+",
            "decays-to:KS,",
            "decays-to:KS decay-contains:KS",
            "decays-from:\"uneven",
        ] {
            assert!(InlineQuery::parse(input).is_err(), "{input}");
        }
    }
}
