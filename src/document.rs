use serde::de::DeserializeOwned;

pub fn parse<T: DeserializeOwned>(text: &str, name: &'static str) -> anyhow::Result<T> {
    toml::from_str(text).map_err(|error| DecodeError { name, error }.into())
}

// TOML errors may include secret defaults or credentials pasted into invalid settings.
struct DecodeError {
    name: &'static str,
    error: toml::de::Error,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid {} at byte range {:?}; check syntax and required field types",
            self.name,
            self.error.span()
        )
    }
}

impl std::fmt::Debug for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl std::error::Error for DecodeError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    #[serde(deny_unknown_fields)]
    struct Settings {
        enabled: bool,
    }

    #[test]
    fn redacts_both_syntax_and_type_errors() {
        for text in [
            "enabled = 'never-print-this'",
            "never-print-this = true",
            "enabled = 'never-print-this",
        ] {
            let error = parse::<Settings>(text, "test settings").unwrap_err();
            assert!(!format!("{error:#}").contains("never-print-this"));
            assert!(!format!("{error:?}").contains("never-print-this"));
        }
        assert!(
            parse::<Settings>("enabled = true", "test settings")
                .unwrap()
                .enabled
        );
    }
}
