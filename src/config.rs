use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct SembleConfig {
    pub paths: PathsConfig,
    #[serde(default)]
    pub builder_policies: Vec<BuilderPolicyConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathsConfig {
    pub hosts_dir: PathBuf,
    pub host_template_dir: PathBuf,
    pub default_host_template: String,
    pub ssh_host_keys_dir: PathBuf,
    pub disk_keys_dir: PathBuf,
    pub sops_config_file: PathBuf,
    pub network_secrets_file: PathBuf,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BuilderPolicyConfig {
    pub name: String,
    #[serde(default)]
    pub local: BuilderPolicyLocalConfig,
    #[serde(default)]
    pub builders: Vec<BuilderMachineConfig>,
}

#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct BuilderPolicyLocalConfig {
    #[serde(default)]
    pub max_jobs: u32,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BuilderMachineConfig {
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
        fs::write(tempdir.path().join("semble.toml"), "").unwrap();

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
default_host_template = "default"
ssh_host_keys_dir = "ssh_host_keys"
disk_keys_dir = "disk_keys"
sops_config_file = ".sops.yaml"
network_secrets_file = "secrets/network.yaml"
"#,
        )
        .unwrap();

        let config = SembleConfig::load(tempdir.path()).unwrap();
        assert!(config.builder_policies.is_empty());
    }

    #[test]
    fn parses_nested_builder_policy_scheduling() {
        let tempdir = tempdir().unwrap();
        fs::write(
            tempdir.path().join("semble.toml"),
            r#"[paths]
hosts_dir = "hosts"
host_template_dir = "hosts/_template"
default_host_template = "default"
ssh_host_keys_dir = "ssh_host_keys"
disk_keys_dir = "disk_keys"
sops_config_file = ".sops.yaml"
network_secrets_file = "secrets/network.yaml"

[[builder_policies]]
name = "thor-aarch64-builder-preferred"

[builder_policies.local]
max_jobs = 1

[[builder_policies.builders]]
host = "thor-builder"
system = "aarch64-linux"
ssh_key = "/etc/nix/ssh/homelab_builder"
max_jobs = 6
speed_factor = 20
supported_features = ["benchmark", "big-parallel"]
"#,
        )
        .unwrap();

        let config = SembleConfig::load(tempdir.path()).unwrap();
        let policy = &config.builder_policies[0];
        assert_eq!(policy.local.max_jobs, 1);
        assert_eq!(policy.builders[0].host, "thor-builder");
        assert_eq!(policy.builders[0].max_jobs, 6);
    }
}
