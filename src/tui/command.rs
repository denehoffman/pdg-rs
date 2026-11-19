use std::path::{self, PathBuf};

use super::{Mode, RESULT_LIMIT, filters::AppliedFilters};
use crate::{CliError, CliResult, DatabaseOptions, ParticleSort};

#[derive(Default)]
pub(super) struct SearchSource {
    db_path: Option<PathBuf>,
    offline: bool,
    edition: Option<String>,
}

impl SearchSource {
    pub(super) fn new(database: &DatabaseOptions, edition: Option<&str>) -> Self {
        Self {
            db_path: database.override_path(),
            offline: database.offline,
            edition: edition.map(str::to_owned),
        }
    }

    pub(super) fn find_command(
        &self,
        mode: Mode,
        query: &str,
        filters: Option<&AppliedFilters>,
        sort: ParticleSort,
    ) -> CliResult<Option<String>> {
        let filters = if mode == Mode::Particles {
            filters
        } else {
            None
        };
        if query.is_empty() && filters.is_none() {
            return Ok(None);
        }

        let mut command = String::from("pdg");
        if self.offline {
            command.push_str(" --offline");
        }
        if let Some(db_path) = &self.db_path {
            let absolute = path::absolute(db_path)?;
            let path = absolute.to_str().ok_or_else(|| {
                CliError::InvalidArgument("The database path cannot be copied as UTF-8.".into())
            })?;
            push_option(&mut command, "--db-path", path);
        }

        command.push_str(" find");
        push_option(&mut command, "--limit", &RESULT_LIMIT.to_string());
        match mode {
            Mode::Particles => {
                if sort == ParticleSort::Mass {
                    push_option(&mut command, "--sort", "mass");
                }
                if let Some(edition) = &self.edition {
                    push_option(&mut command, "--edition", edition);
                }
                if let Some(filters) = filters {
                    for (flag, value) in filters.command_options() {
                        if let Some(value) = value {
                            push_option(&mut command, flag, value);
                        } else {
                            command.push(' ');
                            command.push_str(flag);
                        }
                    }
                }
            }
            Mode::Text => command.push_str(" --text"),
        }

        if !query.is_empty() {
            if query.starts_with('-') {
                command.push_str(" --");
            }
            command.push(' ');
            push_shell_value(&mut command, query);
        }
        Ok(Some(command))
    }
}

fn push_option(command: &mut String, name: &str, value: &str) {
    command.push(' ');
    command.push_str(name);
    command.push('=');
    push_shell_value(command, value);
}

fn push_shell_value(command: &mut String, value: &str) {
    if !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_./:+,".contains(&byte))
    {
        command.push_str(value);
    } else {
        command.push('\'');
        command.push_str(&value.replace('\'', "'\\''"));
        command.push('\'');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_export_uses_the_query_without_particle_options() {
        let source = SearchSource {
            edition: Some("2024".into()),
            ..SearchSource::default()
        };
        assert_eq!(
            source
                .find_command(Mode::Text, "form factors", None, ParticleSort::Mass)
                .unwrap()
                .unwrap(),
            "pdg find --limit=50 --text 'form factors'"
        );
        assert!(
            source
                .find_command(Mode::Text, "", None, ParticleSort::Default)
                .unwrap()
                .is_none()
        );
    }

    #[cfg(unix)]
    #[test]
    fn exported_arguments_survive_shell_parsing() {
        use std::process::Command;

        use clap::Parser;

        use crate::{Cli, Commands};

        let source = SearchSource {
            db_path: Some(PathBuf::from("/tmp/PDG's data.sqlite")),
            offline: true,
            edition: Some("2024".into()),
        };
        let query = "--π (980) 'quoted' \"$HOME\"";
        let command = source
            .find_command(Mode::Particles, query, None, ParticleSort::Mass)
            .unwrap()
            .unwrap();
        let output = Command::new("sh")
            .args([
                "-c",
                &format!("pdg() {{ printf '%s\\n' \"$@\"; }}\n{command}"),
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        let args = std::str::from_utf8(&output.stdout).unwrap();
        let cli = Cli::try_parse_from(std::iter::once("pdg").chain(args.lines())).unwrap();
        let Some(Commands::Find(find)) = cli.command else {
            panic!("the export should parse as a find command");
        };
        assert!(cli.database.offline);
        assert_eq!(cli.database.db_path, source.db_path);
        assert_eq!(find.query.as_deref(), Some(query));
        assert_eq!(find.edition.as_deref(), source.edition.as_deref());
        assert_eq!(find.output.limit, Some(RESULT_LIMIT));
        assert_eq!(find.sort, Some(ParticleSort::Mass));
    }
}
