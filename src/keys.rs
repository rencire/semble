use crate::error::fail;
use crate::repo::RepoPaths;
use anyhow::Result;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

pub(crate) const SSH_PRIVATE_KEY_FILENAME: &str = "ssh_host_ed25519_key";
pub(crate) const SSH_PUBLIC_KEY_FILENAME: &str = "ssh_host_ed25519_key.pub";
pub(crate) const INITRD_SSH_PRIVATE_KEY_FILENAME: &str = "initrd_ssh_host_ed25519_key";
pub(crate) const INITRD_SSH_PUBLIC_KEY_FILENAME: &str = "initrd_ssh_host_ed25519_key.pub";
pub(crate) const LUKS_ROOT_KEY_FILENAME: &str = "luks-root.key";

fn generate_ssh_keypair(
    keys_dir: PathBuf,
    hostname: &str,
    force: bool,
    private_key_name: &str,
    public_key_name: &str,
    label: &str,
) -> Result<PathBuf> {
    let private_key = keys_dir.join(private_key_name);
    let public_key = keys_dir.join(public_key_name);

    fs::create_dir_all(&keys_dir)?;

    for key_path in [&private_key, &public_key] {
        if key_path.exists() {
            if !force {
                return fail(format!("{label} already exists: {}", key_path.display()));
            }
            fs::remove_file(key_path)?;
        }
    }

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
        return fail(format!("failed to generate {label} under {}", keys_dir.display()));
    }

    Ok(keys_dir)
}

pub fn generate_ssh_host_keys(paths: &RepoPaths, hostname: &str, force: bool) -> Result<PathBuf> {
    generate_ssh_keypair(
        paths.host_keys_dir(hostname),
        hostname,
        force,
        SSH_PRIVATE_KEY_FILENAME,
        SSH_PUBLIC_KEY_FILENAME,
        "SSH host key",
    )
}

pub fn generate_initrd_ssh_host_keys(
    paths: &RepoPaths,
    hostname: &str,
    force: bool,
) -> Result<PathBuf> {
    generate_ssh_keypair(
        paths.host_keys_dir(hostname),
        hostname,
        force,
        INITRD_SSH_PRIVATE_KEY_FILENAME,
        INITRD_SSH_PUBLIC_KEY_FILENAME,
        "initrd SSH host key",
    )
}

pub fn generate_luks_root_key(paths: &RepoPaths, hostname: &str, force: bool) -> Result<PathBuf> {
    let keys_dir = paths.luks_host_keys_dir(hostname);
    let key_path = keys_dir.join(LUKS_ROOT_KEY_FILENAME);

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
    use super::{
        generate_initrd_ssh_host_keys, generate_luks_root_key, generate_ssh_host_keys,
        LUKS_ROOT_KEY_FILENAME,
    };
    use crate::repo::RepoPaths;
    use std::fs;
    use std::path::Path;
    use std::thread;
    use std::time::Duration;
    use tempfile::tempdir;

    fn write_repo_config(root: &Path) {
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

        assert!(keys_dir.join("initrd_ssh_host_ed25519_key").exists());
        assert!(keys_dir.join("initrd_ssh_host_ed25519_key.pub").exists());
    }

    #[test]
    fn force_generation_preserves_existing_directory_contents() {
        let tempdir = tempdir().unwrap();
        write_repo_config(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let keys_dir = paths.host_keys_dir("atlas");
        fs::create_dir_all(&keys_dir).unwrap();
        fs::write(keys_dir.join("keep.txt"), "preserve me\n").unwrap();

        generate_ssh_host_keys(&paths, "atlas", true).unwrap();

        assert_eq!(fs::read_to_string(keys_dir.join("keep.txt")).unwrap(), "preserve me\n");
        assert!(keys_dir.join("ssh_host_ed25519_key").exists());
        assert!(keys_dir.join("ssh_host_ed25519_key.pub").exists());
    }

    #[test]
    fn normal_and_initrd_ssh_keys_can_coexist_in_shared_directory() {
        let tempdir = tempdir().unwrap();
        write_repo_config(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();

        let ssh_dir = generate_ssh_host_keys(&paths, "atlas", true).unwrap();
        let initrd_dir = generate_initrd_ssh_host_keys(&paths, "atlas", false).unwrap();

        assert_eq!(ssh_dir, paths.host_keys_dir("atlas"));
        assert_eq!(initrd_dir, ssh_dir);
        assert!(ssh_dir.join("ssh_host_ed25519_key").exists());
        assert!(ssh_dir.join("ssh_host_ed25519_key.pub").exists());
        assert!(ssh_dir.join("initrd_ssh_host_ed25519_key").exists());
        assert!(ssh_dir.join("initrd_ssh_host_ed25519_key.pub").exists());
    }

    #[test]
    fn force_overwrites_only_matching_ssh_files() {
        let tempdir = tempdir().unwrap();
        write_repo_config(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let keys_dir = generate_ssh_host_keys(&paths, "atlas", true).unwrap();
        generate_initrd_ssh_host_keys(&paths, "atlas", false).unwrap();

        let original_private = fs::read(&keys_dir.join("ssh_host_ed25519_key")).unwrap();
        let original_public = fs::read_to_string(keys_dir.join("ssh_host_ed25519_key.pub")).unwrap();
        let initrd_private = fs::read(&keys_dir.join("initrd_ssh_host_ed25519_key")).unwrap();
        let initrd_public = fs::read_to_string(keys_dir.join("initrd_ssh_host_ed25519_key.pub")).unwrap();

        thread::sleep(Duration::from_millis(10));
        generate_ssh_host_keys(&paths, "atlas", true).unwrap();

        assert_ne!(fs::read(&keys_dir.join("ssh_host_ed25519_key")).unwrap(), original_private);
        assert_ne!(
            fs::read_to_string(keys_dir.join("ssh_host_ed25519_key.pub")).unwrap(),
            original_public
        );
        assert_eq!(fs::read(&keys_dir.join("initrd_ssh_host_ed25519_key")).unwrap(), initrd_private);
        assert_eq!(
            fs::read_to_string(keys_dir.join("initrd_ssh_host_ed25519_key.pub")).unwrap(),
            initrd_public
        );
    }

    #[test]
    fn force_overwrites_only_matching_initrd_files() {
        let tempdir = tempdir().unwrap();
        write_repo_config(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let keys_dir = generate_ssh_host_keys(&paths, "atlas", true).unwrap();
        generate_initrd_ssh_host_keys(&paths, "atlas", false).unwrap();

        let ssh_private = fs::read(&keys_dir.join("ssh_host_ed25519_key")).unwrap();
        let ssh_public = fs::read_to_string(keys_dir.join("ssh_host_ed25519_key.pub")).unwrap();
        let initrd_private = fs::read(&keys_dir.join("initrd_ssh_host_ed25519_key")).unwrap();
        let initrd_public =
            fs::read_to_string(keys_dir.join("initrd_ssh_host_ed25519_key.pub")).unwrap();

        thread::sleep(Duration::from_millis(10));
        generate_initrd_ssh_host_keys(&paths, "atlas", true).unwrap();

        assert_eq!(fs::read(&keys_dir.join("ssh_host_ed25519_key")).unwrap(), ssh_private);
        assert_eq!(
            fs::read_to_string(keys_dir.join("ssh_host_ed25519_key.pub")).unwrap(),
            ssh_public
        );
        assert_ne!(fs::read(&keys_dir.join("initrd_ssh_host_ed25519_key")).unwrap(), initrd_private);
        assert_ne!(
            fs::read_to_string(keys_dir.join("initrd_ssh_host_ed25519_key.pub")).unwrap(),
            initrd_public
        );
    }

    #[test]
    fn generates_luks_root_key() {
        let tempdir = tempdir().unwrap();
        write_repo_config(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();

        let keys_dir = generate_luks_root_key(&paths, "atlas", true).unwrap();
        let key_path = keys_dir.join(LUKS_ROOT_KEY_FILENAME);

        assert!(key_path.exists());
        assert_eq!(fs::metadata(&key_path).unwrap().len(), 64);
    }
}
