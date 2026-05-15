use crate::error::fail;
use crate::repo::RepoPaths;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub fn copy_host_template(
    paths: &RepoPaths,
    hostname: &str,
    template: Option<&str>,
    force: bool,
) -> Result<PathBuf> {
    let dst_dir = paths.host_dir(hostname);
    if dst_dir.exists() {
        if !force {
            return fail(format!(
                "host directory already exists: {}",
                dst_dir.display()
            ));
        }
        fs::remove_dir_all(&dst_dir)?;
    }

    let src_dir = match template {
        Some(template) => paths.named_host_template_dir(template),
        None => paths.default_host_template_dir(),
    };
    if !src_dir.exists() {
        return fail(format!("host template does not exist: {}", src_dir.display()));
    }

    copy_dir_recursive(&src_dir, &dst_dir)?;
    update_template_contents(&dst_dir, hostname)?;
    Ok(dst_dir)
}

pub fn ensure_facter_file(dst_dir: &Path) -> Result<()> {
    let facter_path = dst_dir.join("facter.json");
    let needs_file = match fs::read_to_string(&facter_path) {
        Ok(contents) => contents.trim().is_empty(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => true,
        Err(err) => return Err(err.into()),
    };
    if needs_file {
        fs::write(facter_path, "{}\n")?;
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let source_path = entry.path();
        let destination_path = dst.join(entry.file_name());
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            copy_dir_recursive(&source_path, &destination_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &destination_path)?;
        }
    }
    Ok(())
}

fn update_template_contents(dst_dir: &Path, hostname: &str) -> Result<()> {
    set_recursive_permissions(dst_dir)?;
    for entry in fs::read_dir(dst_dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            update_template_contents(&path, hostname)?;
        } else if file_type.is_file() {
            let contents = fs::read_to_string(&path)?;
            fs::write(&path, contents.replace("TEMPLATE_HOSTNAME", hostname))?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn set_recursive_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = fs::metadata(path)?;
    let mode = metadata.permissions().mode();
    fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_recursive_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::copy_host_template;
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
host_template_dir = "hosts/templates"
default_host_template = "default"
ssh_host_keys_dir = "ssh_host_keys"
disk_keys_dir = "disk_keys"
sops_config_file = ".sops.yaml"
network_secrets_file = "secrets/network.yaml"
"#,
        )
        .unwrap();
    }

    // Verify the default template root is used when no template name is provided.
    #[test]
    fn copies_default_template_when_template_is_omitted() {
        let tempdir = tempdir().unwrap();
        write_repo_config(tempdir.path());

        let default_dir = tempdir.path().join("hosts/templates/default");
        fs::create_dir_all(&default_dir).unwrap();
        fs::write(default_dir.join("default.nix"), "host = TEMPLATE_HOSTNAME;\n").unwrap();

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let dst = copy_host_template(&paths, "atlas", None, false).unwrap();

        assert_eq!(dst, tempdir.path().join("hosts/atlas"));
        assert!(dst.join("default.nix").exists());
        assert_eq!(fs::read_to_string(dst.join("default.nix")).unwrap(), "host = atlas;\n");
    }

    // Verify an explicit template name resolves under the configured template root.
    #[test]
    fn copies_named_template_when_requested() {
        let tempdir = tempdir().unwrap();
        write_repo_config(tempdir.path());

        let named_dir = tempdir.path().join("hosts/templates/microvm");
        fs::create_dir_all(&named_dir).unwrap();
        fs::write(named_dir.join("default.nix"), "template = TEMPLATE_HOSTNAME;\n").unwrap();

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let dst = copy_host_template(&paths, "atlas", Some("microvm"), false).unwrap();

        assert_eq!(fs::read_to_string(dst.join("default.nix")).unwrap(), "template = atlas;\n");
    }

    // Verify missing templates are rejected before any destination directory is created.
    #[test]
    fn rejects_missing_template_before_copying() {
        let tempdir = tempdir().unwrap();
        write_repo_config(tempdir.path());

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let error = copy_host_template(&paths, "atlas", Some("missing"), false).unwrap_err();

        assert!(error.to_string().contains("host template does not exist"));
        assert!(!tempdir.path().join("hosts/atlas").exists());
    }
}
