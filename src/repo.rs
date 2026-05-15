use crate::config::{BuilderPolicyConfig, SembleConfig};
use anyhow::Context;
use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[derive(Debug, Clone)]
pub struct RepoPaths {
    root: PathBuf,
    config: SembleConfig,
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

    pub fn disk_keys_dir(&self) -> PathBuf {
        self.root.join(&self.config.paths.disk_keys_dir)
    }

    pub fn luks_host_keys_dir(&self, hostname: &str) -> PathBuf {
        self.disk_keys_dir().join(hostname)
    }

    pub fn sops_config_file(&self) -> PathBuf {
        self.root.join(&self.config.paths.sops_config_file)
    }

    pub fn network_secrets_file(&self) -> PathBuf {
        self.root.join(&self.config.paths.network_secrets_file)
    }

    pub fn host_template_dir(&self) -> PathBuf {
        self.root.join(&self.config.paths.host_template_dir)
    }

    pub fn default_host_template_dir(&self) -> PathBuf {
        self.host_template_dir()
            .join(&self.config.paths.default_host_template)
    }

    pub fn named_host_template_dir(&self, template: &str) -> PathBuf {
        self.host_template_dir().join(template)
    }

    pub fn builder_policy(&self, name: &str) -> Option<&BuilderPolicyConfig> {
        self.config
            .builder_policies
            .iter()
            .find(|policy| policy.name == name)
    }
}

#[derive(Debug, Clone, serde::Deserialize, PartialEq, Eq)]
pub struct ImagePrepareConfig {
    pub partition_label: String,
}

pub fn load_image_prepare_config(
    paths: &RepoPaths,
    image_name: &str,
) -> Result<ImagePrepareConfig> {
    load_image_prepare_config_from_nix(paths, image_name)?.ok_or_else(|| {
        anyhow::anyhow!(
            "missing image prepare metadata for `{image_name}`; expected `prepare.partitionLabel` in the image definition"
        )
    })
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum HostType {
    Physical,
    Microvm,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct HostProvisionConfig {
    pub system: String,
    #[serde(rename = "type")]
    pub host_type: HostType,
    #[serde(default, rename = "provisionTarget")]
    pub provision_target: Option<String>,
}

pub fn load_host_provision_config(
    paths: &RepoPaths,
    host_name: &str,
) -> Result<HostProvisionConfig> {
    load_host_provision_config_from_nix(paths, host_name)?.ok_or_else(|| {
        anyhow::anyhow!(
            "missing host metadata for `{host_name}`; expected `type` and optional `provisionTarget` in the host definition"
        )
    })
}

fn load_host_provision_config_from_nix(
    paths: &RepoPaths,
    host_name: &str,
) -> Result<Option<HostProvisionConfig>> {
    load_host_provision_config_with(paths, host_name, run_nix_eval_host_provision_config)
}

fn load_host_provision_config_with(
    paths: &RepoPaths,
    host_name: &str,
    eval: impl FnOnce(&RepoPaths, &str) -> Result<Option<Vec<u8>>>,
) -> Result<Option<HostProvisionConfig>> {
    let Some(stdout) = eval(paths, host_name)? else {
        return Ok(None);
    };

    let metadata: Option<HostProvisionConfig> =
        serde_json::from_slice(&stdout).context("failed to parse host metadata from `nix eval`")?;

    Ok(metadata)
}

fn run_nix_eval_host_provision_config(
    paths: &RepoPaths,
    host_name: &str,
) -> Result<Option<Vec<u8>>> {
    let canonical_root = paths
        .root()
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", paths.root().display()))?;
    let expr = format!(
        "let flake = builtins.getFlake (toString {}); in flake._semble.hosts.\"{}\" or null",
        canonical_root.display(),
        host_name
    );
    let output = Command::new("nix")
        .args([
            "eval",
            "--extra-experimental-features",
            "nix-command flakes",
            "--impure",
            "--json",
            "--expr",
            &expr,
        ])
        .current_dir(paths.root())
        .output();

    let output = match output {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).context("failed to run `nix eval` for host metadata"),
    };

    parse_nix_eval_output(output)
}

fn load_image_prepare_config_from_nix(
    paths: &RepoPaths,
    image_name: &str,
) -> Result<Option<ImagePrepareConfig>> {
    load_image_prepare_config_with(paths, image_name, run_nix_eval)
}

fn load_image_prepare_config_with(
    paths: &RepoPaths,
    image_name: &str,
    eval: impl FnOnce(&RepoPaths, &str) -> Result<Option<Vec<u8>>>,
) -> Result<Option<ImagePrepareConfig>> {
    #[derive(Debug, serde::Deserialize)]
    struct FlakePrepare {
        #[serde(rename = "partitionLabel")]
        partition_label: Option<String>,
    }

    #[derive(Debug, serde::Deserialize)]
    struct FlakeImageMetadata {
        prepare: Option<FlakePrepare>,
    }

    let Some(stdout) = eval(paths, image_name)? else {
        return Ok(None);
    };

    let metadata: Option<FlakeImageMetadata> = serde_json::from_slice(&stdout)
        .context("failed to parse image metadata from `nix eval`")?;

    Ok(metadata.and_then(|metadata| {
        metadata.prepare.and_then(|prepare| {
            prepare
                .partition_label
                .map(|partition_label| ImagePrepareConfig { partition_label })
        })
    }))
}

fn run_nix_eval(paths: &RepoPaths, image_name: &str) -> Result<Option<Vec<u8>>> {
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
        .args([
            "eval",
            "--extra-experimental-features",
            "nix-command flakes",
            "--impure",
            "--json",
            "--expr",
            &expr,
        ])
        .current_dir(paths.root())
        .output();

    let output = match output {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).context("failed to run `nix eval` for image metadata"),
    };

    parse_nix_eval_output(output)
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct HostInitrdConfig {
    pub ip: String,
    #[serde(rename = "sshPort")]
    pub ssh_port: u16,
    #[serde(rename = "bastionIP")]
    pub bastion_ip: String,
    #[serde(rename = "requiresJump")]
    pub requires_jump: bool,
}

impl HostInitrdConfig {
    pub fn host_ip(&self) -> &str {
        self.ip.split('/').next().unwrap_or(&self.ip)
    }
}

pub fn load_host_initrd_config(
    paths: &RepoPaths,
    host_name: &str,
) -> Result<HostInitrdConfig> {
    load_host_initrd_config_with(paths, host_name, run_nix_eval_host_initrd_config)
}

pub(crate) fn load_host_initrd_config_with(
    paths: &RepoPaths,
    host_name: &str,
    eval: impl FnOnce(&RepoPaths, &str) -> Result<Vec<u8>>,
) -> Result<HostInitrdConfig> {
    let stdout = eval(paths, host_name)?;
    serde_json::from_slice(&stdout)
        .context("failed to parse initrd config from `nix eval` output")
}

fn run_nix_eval_host_initrd_config(paths: &RepoPaths, host_name: &str) -> Result<Vec<u8>> {
    let host_nix = paths.host_dir(host_name).join("host.nix");
    if !host_nix.exists() {
        anyhow::bail!(
            "missing host.nix for `{host_name}`; expected {}",
            host_nix.display()
        );
    }
    let expr = "h: { ip = h.ip; sshPort = h.initrd.sshPort; bastionIP = h.initrd.bastionIP; requiresJump = h.initrd.requiresJump; }";
    let output = Command::new("nix")
        .args([
            "eval",
            "--extra-experimental-features",
            "nix-command",
            "--json",
            "--file",
            &host_nix.to_string_lossy(),
            "--apply",
            expr,
        ])
        .current_dir(paths.root())
        .output();

    let output = match output {
        Ok(output) => output,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            anyhow::bail!("`nix` not found; cannot load initrd config for `{host_name}`")
        }
        Err(err) => return Err(err).context("failed to run `nix eval` for initrd config"),
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        anyhow::bail!("`nix eval` failed for host.nix: {stderr}");
    }

    Ok(output.stdout)
}

fn parse_nix_eval_output(output: Output) -> Result<Option<Vec<u8>>> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr = stderr.trim();
        let detail = if stderr.is_empty() {
            format!("`nix eval` exited with status {}", output.status)
        } else {
            format!("`nix eval` exited with status {}: {stderr}", output.status)
        };
        return Err(anyhow::anyhow!(detail))
            .context("failed to evaluate image metadata with `nix eval`");
    }

    Ok(Some(output.stdout))
}

#[cfg(test)]
mod tests {
    use super::{
        load_host_initrd_config_with, load_host_provision_config_with,
        load_image_prepare_config_with, parse_nix_eval_output, HostInitrdConfig,
        HostProvisionConfig, HostType, ImagePrepareConfig, RepoPaths,
    };
    use anyhow::anyhow;
    use std::fs;
    use std::os::unix::process::ExitStatusExt;
    use std::path::Path;
    use std::process::Output;
    use tempfile::tempdir;

    fn write_minimal_semble_toml(root: &Path) {
        fs::write(
            root.join("semble.toml"),
            r#"
[paths]
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
    }

    #[test]
    fn prefers_nix_image_metadata_when_available() {
        // Verify JSON metadata from the eval layer maps into ImagePrepareConfig.
        let tempdir = tempdir().unwrap();
        write_minimal_semble_toml(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let config = load_image_prepare_config_with(&paths, "vishnu", |_paths, _image_name| {
            Ok(Some(
                br#"{"prepare":{"partitionLabel":"NIXOS_SD"}}"#.to_vec(),
            ))
        })
        .unwrap();

        assert_eq!(
            config.unwrap(),
            ImagePrepareConfig {
                partition_label: "NIXOS_SD".into()
            }
        );
    }

    #[test]
    fn parses_host_metadata_from_eval_output() {
        let tempdir = tempdir().unwrap();
        write_minimal_semble_toml(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let config = load_host_provision_config_with(&paths, "atlas", |_paths, _host_name| {
            Ok(Some(
                br#"{"system":"x86_64-linux","type":"microvm","provisionTarget":"thor-admin"}"#
                    .to_vec(),
            ))
        })
        .unwrap();

        assert_eq!(
            config.unwrap(),
            HostProvisionConfig {
                system: "x86_64-linux".into(),
                host_type: HostType::Microvm,
                provision_target: Some("thor-admin".into())
            }
        );
    }

    #[test]
    fn returns_none_when_metadata_is_missing() {
        // Verify missing metadata stays distinguishable from a real eval failure.
        let tempdir = tempdir().unwrap();
        write_minimal_semble_toml(tempdir.path());

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let config =
            load_image_prepare_config_with(&paths, "vishnu", |_paths, _image_name| Ok(None))
                .unwrap();

        assert_eq!(config, None);
    }

    #[test]
    fn preserves_nix_eval_failures() {
        // Verify eval errors propagate instead of being rewritten as missing metadata.
        let tempdir = tempdir().unwrap();
        write_minimal_semble_toml(tempdir.path());

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let error = load_image_prepare_config_with(&paths, "vishnu", |_paths, _image_name| {
            Err(anyhow!("boom"))
        })
        .unwrap_err();

        assert!(error.to_string().contains("boom"));
    }

    #[test]
    fn parse_nix_eval_output_reports_stderr() {
        // Verify non-zero nix eval exits preserve stderr for debugging.
        let output = Output {
            status: std::process::ExitStatus::from_raw(256),
            stdout: Vec::new(),
            stderr: b"permission denied".to_vec(),
        };

        let error = parse_nix_eval_output(output).unwrap_err();
        assert!(format!("{error:#}").contains("permission denied"));
    }

    #[test]
    fn resolves_builder_policy_by_name() {
        let tempdir = tempdir().unwrap();
        fs::write(
            tempdir.path().join("semble.toml"),
            r#"
[paths]
hosts_dir = "hosts"
host_template_dir = "hosts/_template"
default_host_template = "default"
ssh_host_keys_dir = "ssh_host_keys"
disk_keys_dir = "disk_keys"
sops_config_file = ".sops.yaml"
network_secrets_file = "secrets/network.yaml"

[[builder_policies]]
name = "l380y"
host = "l380y-deploy"
system = "x86_64-linux"
max_jobs = 6
speed_factor = 1
supported_features = ["benchmark", "big-parallel"]
"#,
        )
        .unwrap();

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let policy = paths.builder_policy("l380y").unwrap();
        assert_eq!(policy.host, "l380y-deploy");
        assert_eq!(policy.system, "x86_64-linux");
    }

    #[test]
    fn parses_host_initrd_config_from_eval_output() {
        let tempdir = tempdir().unwrap();
        write_minimal_semble_toml(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();

        let config = load_host_initrd_config_with(&paths, "thor", |_paths, _host| {
            Ok(br#"{"ip":"192.168.0.40/24","sshPort":2222,"bastionIP":"192.168.0.20","requiresJump":true}"#.to_vec())
        })
        .unwrap();

        assert_eq!(
            config,
            HostInitrdConfig {
                ip: "192.168.0.40/24".into(),
                ssh_port: 2222,
                bastion_ip: "192.168.0.20".into(),
                requires_jump: true,
            }
        );
    }

    #[test]
    fn host_ip_strips_cidr_prefix() {
        let config = HostInitrdConfig {
            ip: "192.168.0.40/24".into(),
            ssh_port: 2222,
            bastion_ip: "192.168.0.20".into(),
            requires_jump: true,
        };
        assert_eq!(config.host_ip(), "192.168.0.40");
    }

    #[test]
    fn host_ip_passthrough_when_no_cidr() {
        let config = HostInitrdConfig {
            ip: "192.168.0.40".into(),
            ssh_port: 2222,
            bastion_ip: "192.168.0.20".into(),
            requires_jump: false,
        };
        assert_eq!(config.host_ip(), "192.168.0.40");
    }

    #[test]
    fn initrd_config_eval_failure_propagates() {
        let tempdir = tempdir().unwrap();
        write_minimal_semble_toml(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();

        let error = load_host_initrd_config_with(&paths, "thor", |_paths, _host| {
            Err(anyhow!("nix eval failed"))
        })
        .unwrap_err();

        assert!(error.to_string().contains("nix eval failed"));
    }
}
