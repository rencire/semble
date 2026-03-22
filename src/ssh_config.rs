use crate::error::fail;
use crate::repo::RepoPaths;
use anyhow::Result;
use std::fs;

fn ssh_alias_block(host_alias: &str, dns_name: &str, user: &str, identity_file: &str) -> String {
    format!(
        "        \"{host_alias}\" = {{\n          hostname = \"{dns_name}\";\n          user = \"{user}\";\n          identityFile = \"{identity_file}\";\n          identitiesOnly = true;\n        }};\n"
    )
}

pub fn update_ssh_aliases_add(paths: &RepoPaths, hostname: &str) -> Result<bool> {
    let ssh_config_path = paths.ssh_config_module_file();
    let mut text = fs::read_to_string(&ssh_config_path)?;
    let mut additions = Vec::new();

    for alias in paths.ssh_aliases_for_host(hostname) {
        let marker = format!("\"{}\" = {{", alias.host_alias);
        if text.contains(&marker) {
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

    let insertion = additions.join("");
    for anchor in [
        "        \"genesis-nixos\" = {",
        "        \"*\" = {",
        "      };",
    ] {
        if text.contains(anchor) {
            text = text.replacen(anchor, &format!("{insertion}{anchor}"), 1);
            fs::write(ssh_config_path, text)?;
            return Ok(true);
        }
    }

    fail(format!(
        "could not find an insertion point for SSH matchBlocks in {}",
        paths.ssh_config_module_file().display()
    ))
}

pub fn update_ssh_aliases_delete(paths: &RepoPaths, hostname: &str) -> Result<bool> {
    let ssh_config_path = paths.ssh_config_module_file();
    let original = fs::read_to_string(&ssh_config_path)?;
    let mut lines = original.lines().peekable();
    let mut kept = Vec::new();
    let mut removed = false;
    let aliases = paths
        .ssh_aliases_for_host(hostname)
        .into_iter()
        .map(|alias| alias.host_alias)
        .collect::<Vec<_>>();

    while let Some(line) = lines.next() {
        let trimmed = line.trim_start();
        let mut skipped_alias = false;

        for alias in &aliases {
            if trimmed == format!("\"{alias}\" = {{") {
                removed = true;
                skipped_alias = true;
                let mut depth = 1usize;
                for block_line in lines.by_ref() {
                    if block_line.trim_end().ends_with("{") {
                        depth += 1;
                    }
                    if block_line.trim() == "};" {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                }
                break;
            }
        }

        if skipped_alias {
            continue;
        }

        kept.push(line);
    }

    if removed {
        fs::write(ssh_config_path, format!("{}\n", kept.join("\n")))?;
    }

    Ok(removed)
}
