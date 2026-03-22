use crate::error::fail;
use crate::repo::RepoPaths;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn generate_ssh_host_keys(paths: &RepoPaths, hostname: &str, force: bool) -> Result<PathBuf> {
    let keys_dir = paths.host_keys_dir(hostname);
    let private_key = keys_dir.join("ssh_host_ed25519_key");
    let public_key = keys_dir.join("ssh_host_ed25519_key.pub");

    if keys_dir.exists() {
        if !force {
            return fail(format!(
                "SSH host key directory already exists: {}",
                keys_dir.display()
            ));
        }
        fs::remove_dir_all(&keys_dir)?;
    }

    fs::create_dir_all(&keys_dir)?;
    let status = Command::new("ssh-keygen")
        .args([
            "-q",
            "-t",
            "ed25519",
            "-N",
            "",
            "-C",
            &format!("root@{hostname}"),
            "-f",
            private_key.to_string_lossy().as_ref(),
        ])
        .status()?;
    if !status.success() {
        return fail(format!(
            "command failed with exit code {}: ssh-keygen",
            status.code().unwrap_or(1)
        ));
    }

    if !private_key.exists() || !public_key.exists() {
        return fail(format!(
            "failed to generate SSH host keys under {}",
            keys_dir.display()
        ));
    }

    Ok(keys_dir)
}

pub fn read_public_key_from_dir(keys_dir: &Path) -> Result<String> {
    Ok(
        fs::read_to_string(keys_dir.join("ssh_host_ed25519_key.pub"))?
            .trim()
            .to_string(),
    )
}
