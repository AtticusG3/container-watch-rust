//! Domain models: containers, connection draft, SSH options.

use serde::Deserialize;

/// Parsed row from `docker ps -a --format '{{json .}}'`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ContainerInfo {
    #[serde(rename = "ID")]
    pub id: Option<String>,
    #[serde(rename = "Names", default, deserialize_with = "deserialize_names")]
    pub names: Vec<String>,
    #[serde(rename = "Image")]
    pub image: Option<String>,
    #[serde(rename = "State")]
    pub state: Option<String>,
    #[serde(rename = "Status")]
    pub status: Option<String>,
}

impl ContainerInfo {
    pub fn display_name(&self) -> String {
        if let Some(first) = self.names.first() {
            first.trim_start_matches('/').to_string()
        } else {
            self.id.clone().unwrap_or_default()
        }
    }

    /// Prefer container ID for docker lifecycle commands.
    pub fn restart_target(&self) -> String {
        if let Some(id) = self.id.as_ref().filter(|s| !s.trim().is_empty()) {
            id.clone()
        } else {
            self.display_name()
        }
    }
}

fn deserialize_names<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct NamesVisitor;

    impl<'de> Visitor<'de> for NamesVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or array of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value.is_empty() {
                Ok(Vec::new())
            } else {
                Ok(vec![value.to_string()])
            }
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Vec::new())
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Vec::new())
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut names = Vec::new();
            while let Some(value) = seq.next_element::<String>()? {
                if !value.is_empty() {
                    names.push(value);
                }
            }
            Ok(names)
        }
    }

    deserializer.deserialize_any(NamesVisitor)
}

/// Effective SSH connection parameters for one session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConnectionDraft {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub command_timeout_seconds: u64,
}

impl ConnectionDraft {
    pub fn command_timeout(&self) -> std::time::Duration {
        let secs = self.command_timeout_seconds.clamp(5, 600);
        std::time::Duration::from_secs(secs)
    }
}

/// Layered configuration defaults (`appsettings.json` + env).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Default)]
pub struct AppSettings {
    #[serde(default)]
    pub ssh: SshOptions,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SshOptions {
    #[serde(default)]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default = "default_timeout")]
    pub command_timeout_seconds: u64,
}

impl Default for SshOptions {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: default_port(),
            username: String::new(),
            password: None,
            command_timeout_seconds: default_timeout(),
        }
    }
}

fn default_port() -> u16 {
    22
}

fn default_timeout() -> u64 {
    120
}

/// `%LocalAppData%\\ContainerWatch\\session.json` (separate from .NET Script Reloader).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SavedConnectionDto {
    #[serde(rename = "Host")]
    pub host: String,
    #[serde(rename = "Port")]
    pub port: u16,
    #[serde(rename = "Username")]
    pub username: String,
    #[serde(rename = "PasswordProtected")]
    pub password_protected: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_names_as_string() {
        let json = r#"{"ID":"abc","Names":"/myapp","Image":"nginx","State":"running","Status":"Up"}"#;
        let row: ContainerInfo = serde_json::from_str(json).unwrap();
        assert_eq!(row.display_name(), "myapp");
        assert_eq!(row.restart_target(), "abc");
    }

    #[test]
    fn parse_names_as_array() {
        let json = r#"{"ID":"abc","Names":["/a","/b"],"Image":"nginx","State":"exited","Status":"Exited"}"#;
        let row: ContainerInfo = serde_json::from_str(json).unwrap();
        assert_eq!(row.display_name(), "a");
    }

    #[test]
    fn restart_target_falls_back_to_display_name() {
        let row = ContainerInfo {
            id: None,
            names: vec!["/onlyname".to_string()],
            image: None,
            state: None,
            status: None,
        };
        assert_eq!(row.restart_target(), "onlyname");
    }
}
