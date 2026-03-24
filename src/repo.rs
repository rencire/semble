use crate::config::{SembleConfig, SshAliasConfig};
use anyhow::Context;
use anyhow::Result;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RepoPaths {
    root: PathBuf,
    config: SembleConfig,
}

#[derive(Debug, Clone)]
pub struct ResolvedSshAlias {
    pub host_alias: String,
    pub dns_name: String,
    pub user: String,
    pub identity_file: String,
}

impl RepoPaths {
    pub fn new(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let config = SembleConfig::load(&root)?;
        Ok(Self { root, config })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config(&self) -> &SembleConfig {
        &self.config
    }

    pub fn hosts_dir(&self) -> PathBuf {
        self.root.join(&self.config.paths.hosts_dir)
    }

    pub fn host_dir(&self, hostname: &str) -> PathBuf {
        self.hosts_dir().join(hostname)
    }

    pub fn ssh_keys_dir(&self) -> PathBuf {
        self.root.join(&self.config.paths.ssh_host_keys_dir)
    }

    pub fn host_keys_dir(&self, hostname: &str) -> PathBuf {
        self.ssh_keys_dir().join(hostname)
    }

    pub fn sops_config_file(&self) -> PathBuf {
        self.root.join(&self.config.paths.sops_config_file)
    }

    pub fn network_secrets_file(&self) -> PathBuf {
        self.root.join(&self.config.paths.network_secrets_file)
    }

    pub fn ssh_managed_config_file(&self) -> PathBuf {
        resolve_user_path(&self.config.ssh.managed_config_file, &self.root)
    }

    pub fn host_template_dir(&self) -> PathBuf {
        self.root.join(&self.config.paths.host_template_dir)
    }

    pub fn image_dir(&self, image_name: &str) -> PathBuf {
        self.root.join("images").join(image_name)
    }

    pub fn image_prepare_file(&self, image_name: &str) -> PathBuf {
        self.image_dir(image_name).join("prepare.toml")
    }

    pub fn image_prepare_legacy_file(&self, image_name: &str) -> PathBuf {
        self.root.join("images").join(format!("{image_name}.prepare.toml"))
    }

    pub fn ssh_aliases_for_host(&self, hostname: &str) -> Vec<ResolvedSshAlias> {
        self.config
            .ssh
            .aliases
            .iter()
            .map(|alias| self.resolve_ssh_alias(hostname, alias))
            .collect()
    }

    fn resolve_ssh_alias(&self, hostname: &str, alias: &SshAliasConfig) -> ResolvedSshAlias {
        ResolvedSshAlias {
            host_alias: format!("{hostname}-{}", alias.name_suffix),
            dns_name: format!("{hostname}.{}", self.config.ssh.dns_suffix),
            user: alias.user.clone(),
            identity_file: alias.identity_file.clone(),
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
pub struct ImagePrepareConfig {
    pub partition_label: String,
}

pub fn load_image_prepare_config(paths: &RepoPaths, image_name: &str) -> Result<ImagePrepareConfig> {
    let candidates = [
        paths.image_prepare_file(image_name),
        paths.image_prepare_legacy_file(image_name),
    ];

    for prepare_path in candidates {
        match fs::read_to_string(&prepare_path) {
            Ok(raw) => {
                return toml::from_str(&raw)
                    .with_context(|| format!("failed to parse {}", prepare_path.display()));
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("failed to read {}", prepare_path.display()));
            }
        }
    }

    anyhow::bail!(
        "missing image prepare config for `{image_name}`; expected {} or {}",
        paths.image_prepare_file(image_name).display(),
        paths.image_prepare_legacy_file(image_name).display()
    )
}

fn resolve_user_path(path: &Path, root: &Path) -> PathBuf {
    let rendered = path.to_string_lossy();
    if let Some(stripped) = rendered.strip_prefix("~/") {
        if let Some(home) = env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }

    if path.is_absolute() {
        path.to_path_buf()
    } else {
        root.join(path)
    }
}
