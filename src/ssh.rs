use crate::error::fail;
use crate::repo::RepoPaths;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(unix)]
fn symlink_file(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

pub fn build_ssh_setup_args() -> Vec<&'static str> {
    vec!["build", ".#semble-hosts", "--print-out-paths", "--no-link"]
}

pub fn build_ssh_alias_store_path(paths: &RepoPaths) -> Result<PathBuf> {
    let output = Command::new("nix")
        .args(build_ssh_setup_args())
        .current_dir(paths.root())
        .output()
        .context("failed to invoke `nix build` for .#semble-hosts")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let detail = if stderr.is_empty() {
            String::from("nix build .#semble-hosts failed")
        } else {
            stderr
        };
        return fail(detail);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let out_path = stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .ok_or_else(|| anyhow::anyhow!("nix build did not return a store path for .#semble-hosts"))?;

    Ok(PathBuf::from(out_path))
}

pub fn link_ssh_aliases(paths: &RepoPaths, target: &Path) -> Result<()> {
    let link = paths.ssh_managed_config_file();

    if let Some(parent) = link.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!("failed to create parent directory for {}", link.display())
        })?;
    }

    if let Ok(existing) = fs::symlink_metadata(&link) {
        if existing.file_type().is_dir() {
            return fail(format!(
                "refusing to replace directory with SSH alias symlink: {}",
                link.display()
            ));
        }
        fs::remove_file(&link)
            .with_context(|| format!("failed to remove existing {}", link.display()))?;
    }

    symlink_file(target, &link)
        .with_context(|| format!("failed to symlink {} -> {}", link.display(), target.display()))?;
    Ok(())
}

pub fn run_ssh_setup(paths: &RepoPaths) -> Result<()> {
    let target = build_ssh_alias_store_path(paths)?;
    link_ssh_aliases(paths, &target)?;
    println!("Updated SSH alias symlink: {}", paths.ssh_managed_config_file().display());
    println!("Using generated SSH aliases from: {}", target.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_ssh_setup_args, link_ssh_aliases};
    use crate::repo::RepoPaths;
    use std::fs;
    use tempfile::tempdir;

    const SEMBLE_TOML: &str = r#"[paths]
hosts_dir = "hosts"
host_template_dir = "hosts/_template"
ssh_host_keys_dir = "ssh_host_keys"
sops_config_file = ".sops.yaml"
network_secrets_file = "secrets/network.yaml"

[ssh]
managed_config_file = ".ssh/semble_hosts"
dns_suffix = "baiji-carat.ts.net"

[[ssh.aliases]]
name_suffix = "admin"
user = "admin"
identity_file = "~/.ssh/admin_key"
"#;

    fn setup_repo() -> (tempfile::TempDir, RepoPaths) {
        let tempdir = tempdir().unwrap();
        let root = tempdir.path().to_path_buf();
        fs::create_dir_all(root.join("hosts")).unwrap();
        fs::create_dir_all(root.join("ssh_host_keys")).unwrap();
        fs::create_dir_all(root.join("secrets")).unwrap();
        fs::write(root.join("semble.toml"), SEMBLE_TOML).unwrap();
        (tempdir, RepoPaths::new(root).unwrap())
    }

    #[test]
    fn builds_expected_nix_args() {
        assert_eq!(
            build_ssh_setup_args(),
            vec!["build", ".#semble-hosts", "--print-out-paths", "--no-link"]
        );
    }

    #[test]
    fn links_generated_alias_file() {
        let (_tempdir, paths) = setup_repo();
        let generated = paths.root().join("generated-hosts");
        fs::write(&generated, "Host atlas-admin\n").unwrap();

        link_ssh_aliases(&paths, &generated).unwrap();

        let link = paths.ssh_managed_config_file();
        let linked_target = fs::read_link(&link).unwrap();
        assert_eq!(linked_target, generated);
    }
}
