use crate::config::{SembleConfig, SshAliasConfig};
use anyhow::Context;
use anyhow::Result;
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    load_image_prepare_config_from_nix(paths, image_name)?.ok_or_else(|| {
        anyhow::anyhow!(
            "missing image prepare metadata for `{image_name}`; expected `prepare.partitionLabel` in the image definition"
        )
    })
}

fn load_image_prepare_config_from_nix(paths: &RepoPaths, image_name: &str) -> Result<Option<ImagePrepareConfig>> {
    #[derive(Debug, serde::Deserialize)]
    struct FlakePrepare {
        #[serde(rename = "partitionLabel")]
        partition_label: Option<String>,
    }

    #[derive(Debug, serde::Deserialize)]
    struct FlakeImageMetadata {
        prepare: Option<FlakePrepare>,
    }

    let canonical_root = paths
        .root()
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", paths.root().display()))?;
    let expr = format!(
        "let flake = builtins.getFlake (toString {}); in flake._semble.images.\"{}\" or null",
        canonical_root.display(),
        image_name
    );
    let output = Command::new("nix")
        .args(["eval", "--impure", "--json", "--expr", &expr])
        .current_dir(paths.root())
        .output();

    let output = match output {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).context("failed to run `nix eval` for image metadata"),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        let detail = if stderr.is_empty() {
            format!("`nix eval` exited with status {}", output.status)
        } else {
            format!("`nix eval` exited with status {}: {stderr}", output.status)
        };
        return Err(anyhow::anyhow!(detail)).context("failed to evaluate image metadata with `nix eval`");
    }

    let metadata: Option<FlakeImageMetadata> =
        serde_json::from_slice(&output.stdout).context("failed to parse image metadata from `nix eval`")?;

    Ok(metadata.and_then(|metadata| {
        metadata.prepare.and_then(|prepare| {
            prepare.partition_label.map(|partition_label| ImagePrepareConfig { partition_label })
        })
    }))
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

#[cfg(test)]
mod tests {
    use super::{load_image_prepare_config, ImagePrepareConfig, RepoPaths};
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_minimal_semble_toml(root: &Path) {
        fs::write(
            root.join("semble.toml"),
            r#"
[paths]
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
    }

    #[test]
    fn prefers_nix_image_metadata_when_available() {
        let tempdir = tempdir().unwrap();
        write_minimal_semble_toml(tempdir.path());
        fs::write(
            tempdir.path().join("flake.nix"),
            r#"
{
  outputs = { self }: {
    _semble.images.vishnu.prepare.partitionLabel = "NIXOS_SD";
  };
}
"#,
        )
        .unwrap();
        fs::write(
            tempdir.path().join("flake.nix"),
            r#"
{
  outputs = { self }: {
    _semble.images.vishnu.prepare.partitionLabel = "NIXOS_SD";
  };
}
"#,
        )
        .unwrap();

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let config = load_image_prepare_config(&paths, "vishnu").unwrap();

        assert_eq!(
            config,
            ImagePrepareConfig {
                partition_label: "NIXOS_SD".into()
            }
        );
    }
}
