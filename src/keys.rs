use crate::error::fail;
use crate::repo::RepoPaths;
use anyhow::Result;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

fn generate_ssh_keypair(keys_dir: PathBuf, hostname: &str, force: bool) -> Result<PathBuf> {
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

pub fn generate_ssh_host_keys(paths: &RepoPaths, hostname: &str, force: bool) -> Result<PathBuf> {
    generate_ssh_keypair(paths.host_keys_dir(hostname), hostname, force)
}

pub fn generate_initrd_ssh_host_keys(
    paths: &RepoPaths,
    hostname: &str,
    force: bool,
) -> Result<PathBuf> {
    generate_ssh_keypair(paths.initrd_host_keys_dir(hostname), hostname, force)
}

pub fn generate_luks_root_key(paths: &RepoPaths, hostname: &str, force: bool) -> Result<PathBuf> {
    let keys_dir = paths.luks_host_keys_dir(hostname);
    let key_path = keys_dir.join("root.key");

    if keys_dir.exists() {
        if !force {
            return fail(format!(
                "LUKS root key directory already exists: {}",
                keys_dir.display()
            ));
        }
        fs::remove_dir_all(&keys_dir)?;
    }

    fs::create_dir_all(&keys_dir)?;
    let mut urandom = File::open("/dev/urandom")?;
    let mut key = [0u8; 64];
    urandom.read_exact(&mut key)?;
    File::create(&key_path)?.write_all(&key)?;

    if !key_path.exists() {
        return fail(format!(
            "failed to generate LUKS root key under {}",
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

#[cfg(test)]
mod tests {
    use super::{generate_initrd_ssh_host_keys, generate_luks_root_key, generate_ssh_host_keys};
    use crate::repo::RepoPaths;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_repo_config(root: &Path) {
        fs::write(
            root.join("semble.toml"),
            r#"
[paths]
hosts_dir = "hosts"
host_template_dir = "hosts/_template"
ssh_host_keys_dir = "ssh_host_keys"
initrd_ssh_host_keys_dir = "initrd_ssh_host_keys"
luks_root_keys_dir = "luks_root_keys"
sops_config_file = ".sops.yaml"
network_secrets_file = "secrets/network.yaml"
"#,
        )
        .unwrap();
    }

    #[test]
    fn generates_ssh_host_keys() {
        let tempdir = tempdir().unwrap();
        write_repo_config(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();

        let keys_dir = generate_ssh_host_keys(&paths, "atlas", true).unwrap();

        assert!(keys_dir.join("ssh_host_ed25519_key").exists());
        assert!(keys_dir.join("ssh_host_ed25519_key.pub").exists());
    }

    #[test]
    fn generates_initrd_ssh_host_keys() {
        let tempdir = tempdir().unwrap();
        write_repo_config(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();

        let keys_dir = generate_initrd_ssh_host_keys(&paths, "atlas", true).unwrap();

        assert!(keys_dir.join("ssh_host_ed25519_key").exists());
        assert!(keys_dir.join("ssh_host_ed25519_key.pub").exists());
    }

    #[test]
    fn generates_luks_root_key() {
        let tempdir = tempdir().unwrap();
        write_repo_config(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();

        let keys_dir = generate_luks_root_key(&paths, "atlas", true).unwrap();
        let key_path = keys_dir.join("root.key");

        assert!(key_path.exists());
        assert_eq!(fs::metadata(&key_path).unwrap().len(), 64);
    }
}
