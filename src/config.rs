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
    pub ssh_config_module_file: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SshConfig {
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
}
