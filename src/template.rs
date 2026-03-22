use crate::error::fail;
use crate::repo::RepoPaths;
use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};

pub fn copy_host_template(paths: &RepoPaths, hostname: &str, force: bool) -> Result<PathBuf> {
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

    copy_dir_recursive(&paths.host_template_dir(), &dst_dir)?;
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
