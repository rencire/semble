use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct SembleConfig {
    pub paths: PathsConfig,
    pub ssh: SshConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PathsConfig {
    pub hosts_dir: PathBuf,
    pub host_template_dir: PathBuf,
    pub ssh_host_keys_dir: PathBuf,
    pub sops_config_file: PathBuf,
    pub network_secrets_file: PathBuf,
    #[serde(default)]
    pub ssh_config_module_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SshConfig {
    #[serde(default)]
    pub managed_config_file: Option<PathBuf>,
    pub dns_suffix: String,
    pub aliases: Vec<SshAliasConfig>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SshAliasConfig {
    pub name_suffix: String,
    pub user: String,
    pub identity_file: String,
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
        let config: Self = toml::from_str(&raw)
            .with_context(|| format!("failed to parse {}", config_path.display()))?;
        if config.ssh.managed_config_file.is_none() && config.paths.ssh_config_module_file.is_none()
        {
            anyhow::bail!(
                "{} must define either [ssh].managed_config_file or the legacy [paths].ssh_config_module_file",
                config_path.display()
            );
        }
        Ok(config)
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
    fn accepts_legacy_ssh_config_path() {
        let tempdir = tempdir().unwrap();
        fs::write(
            tempdir.path().join("semble.toml"),
            r#"[paths]
hosts_dir = "hosts"
host_template_dir = "hosts/_template"
ssh_host_keys_dir = "ssh_host_keys"
sops_config_file = ".sops.yaml"
network_secrets_file = "secrets/network.yaml"
ssh_config_module_file = "nix/homeModules/network.nix"

[ssh]
dns_suffix = "example.ts.net"

[[ssh.aliases]]
name_suffix = "deploy"
user = "deploy"
identity_file = "~/.ssh/id_test"
"#,
        )
        .unwrap();

        assert!(SembleConfig::load(tempdir.path()).is_ok());
    }
}
