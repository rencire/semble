use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct SembleConfig {
    pub paths: PathsConfig,
    pub ssh: SshConfig,
    #[serde(default)]
    pub builder_policies: Vec<BuilderPolicyConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathsConfig {
    pub hosts_dir: PathBuf,
    pub host_template_dir: PathBuf,
    pub ssh_host_keys_dir: PathBuf,
    pub sops_config_file: PathBuf,
    pub network_secrets_file: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SshConfig {
    pub managed_config_file: PathBuf,
    pub dns_suffix: String,
    pub aliases: Vec<SshAliasConfig>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SshAliasConfig {
    pub name_suffix: String,
    pub user: String,
    pub identity_file: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BuilderPolicyConfig {
    pub name: String,
    pub host: String,
    pub system: String,
    #[serde(default)]
    pub ssh_key: Option<String>,
    pub max_jobs: u32,
    pub speed_factor: u32,
    #[serde(default)]
    pub supported_features: Vec<String>,
}

impl SembleConfig {
    pub fn load(root: &Path) -> Result<Self> {
        let config_path = root.join("semble.toml");
        let raw = fs::read_to_string(&config_path).with_context(|| {
            format!(
                "failed to read {}. A semble-managed repo must define a root-level semble.toml",
                config_path.display()
            )
        })?;
        toml::from_str(&raw).with_context(|| format!("failed to parse {}", config_path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::SembleConfig;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn rejects_missing_required_fields() {
        let tempdir = tempdir().unwrap();
        fs::write(
            tempdir.path().join("semble.toml"),
            "[ssh]\ndns_suffix = \"example.ts.net\"\n",
        )
        .unwrap();

        assert!(SembleConfig::load(tempdir.path()).is_err());
    }

    #[test]
    fn defaults_builder_policies_to_empty() {
        let tempdir = tempdir().unwrap();
        fs::write(
            tempdir.path().join("semble.toml"),
            r#"[paths]
hosts_dir = "hosts"
host_template_dir = "hosts/_template"
ssh_host_keys_dir = "ssh_host_keys"
sops_config_file = ".sops.yaml"
network_secrets_file = "secrets/network.yaml"

[ssh]
managed_config_file = "~/.ssh/semble_hosts"
dns_suffix = "example.ts.net"

[[ssh.aliases]]
name_suffix = "admin"
user = "admin"
identity_file = "~/.ssh/id_ed25519"
"#,
        )
        .unwrap();

        let config = SembleConfig::load(tempdir.path()).unwrap();
        assert!(config.builder_policies.is_empty());
    }
}
