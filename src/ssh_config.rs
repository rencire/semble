use crate::repo::RepoPaths;
use anyhow::Result;
use std::fs;

fn ssh_alias_block(host_alias: &str, dns_name: &str, user: &str, identity_file: &str) -> String {
    format!(
        "# semble:begin {host_alias}\nHost {host_alias}\n  HostName {dns_name}\n  User {user}\n  IdentityFile {identity_file}\n  IdentitiesOnly yes\n# semble:end {host_alias}\n"
    )
}

pub fn update_ssh_aliases_add(paths: &RepoPaths, hostname: &str) -> Result<bool> {
    let ssh_config_path = paths.ssh_managed_config_file();
    let original = match fs::read_to_string(&ssh_config_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };

    let mut additions = Vec::new();
    for alias in paths.ssh_aliases_for_host(hostname) {
        let marker = format!("# semble:begin {}", alias.host_alias);
        if original.contains(&marker) {
            continue;
        }
        additions.push(ssh_alias_block(
            &alias.host_alias,
            &alias.dns_name,
            &alias.user,
            &alias.identity_file,
        ));
    }

    if additions.is_empty() {
        return Ok(false);
    }

    if let Some(parent) = ssh_config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut updated = original;
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    if !updated.is_empty() && !updated.ends_with("\n\n") {
        updated.push('\n');
    }
    updated.push_str(&additions.join("\n"));
    fs::write(ssh_config_path, updated)?;
    Ok(true)
}

pub fn update_ssh_aliases_delete(paths: &RepoPaths, hostname: &str) -> Result<bool> {
    let ssh_config_path = paths.ssh_managed_config_file();
    let original = match fs::read_to_string(&ssh_config_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    };

    let aliases = paths
        .ssh_aliases_for_host(hostname)
        .into_iter()
        .map(|alias| alias.host_alias)
        .collect::<Vec<_>>();

    let mut kept = Vec::new();
    let mut removed = false;
    let mut skipping_alias: Option<String> = None;

    for line in original.lines() {
        if let Some(alias) = &skipping_alias {
            if line == format!("# semble:end {alias}") {
                skipping_alias = None;
            }
            removed = true;
            continue;
        }

        if let Some(alias) = aliases
            .iter()
            .find(|alias| line == format!("# semble:begin {alias}"))
        {
            skipping_alias = Some(alias.clone());
            removed = true;
            continue;
        }

        kept.push(line);
    }

    if removed {
        let mut updated = kept.join("\n");
        while updated.contains("\n\n\n") {
            updated = updated.replace("\n\n\n", "\n\n");
        }
        if !updated.is_empty() {
            updated.push('\n');
        }
        fs::write(ssh_config_path, updated)?;
    }

    Ok(removed)
}
