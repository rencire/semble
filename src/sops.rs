use crate::error::fail;
use crate::host::sanitized_anchor;
use crate::repo::RepoPaths;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

pub fn sops_key_aliases(paths: &RepoPaths) -> Result<HashMap<String, String>> {
    let mut aliases = HashMap::new();
    for line in fs::read_to_string(paths.sops_config_file())?.lines() {
        let line = line.trim();
        if !line.starts_with("- &") || !line.contains('\"') {
            continue;
        }
        let mut parts = line.split('"');
        let prefix = parts.next().unwrap_or_default();
        let value = parts.next().unwrap_or_default();
        if let Some(alias) = prefix.trim().strip_prefix("- &") {
            aliases.insert(alias.to_string(), value.to_string());
        }
    }
    Ok(aliases)
}

pub fn network_rule_aliases(paths: &RepoPaths) -> Result<Vec<String>> {
    let lines = fs::read_to_string(paths.sops_config_file())?;
    let mut aliases = Vec::new();
    let mut in_network_rule = false;
    let mut in_age_block = false;

    for line in lines.lines() {
        if line.starts_with("  - path_regex: ") {
            in_network_rule = line.contains("secrets/network\\.(yaml|json|env|ini)$");
            in_age_block = false;
            continue;
        }
        if !in_network_rule {
            continue;
        }
        if line.starts_with("      - age:") {
            in_age_block = true;
            continue;
        }
        if in_age_block {
            if let Some(alias) = line.trim().strip_prefix("- *") {
                aliases.push(alias.to_string());
                continue;
            }
            if line.starts_with("  - path_regex: ")
                || (line.starts_with("    ")
                    && !line.starts_with("          - ")
                    && !line.trim().is_empty())
            {
                break;
            }
        }
    }

    Ok(aliases)
}

pub fn update_sops_yaml_add(paths: &RepoPaths, hostname: &str, public_key: &str) -> Result<bool> {
    let mut text = fs::read_to_string(paths.sops_config_file())?;
    let anchor = sanitized_anchor(hostname);
    let key_anchor = format!("&{anchor}");
    let key_alias = format!("*{anchor}");
    let key_line = format!("  - {key_anchor} \"{public_key}\"");
    let mut changed = false;

    if !text.contains(&key_anchor) {
        let marker = "creation_rules:\n";
        if !text.contains(marker) {
            return fail(format!(
                "could not find `creation_rules:` in {}",
                paths.sops_config_file().display()
            ));
        }
        text = text.replacen(marker, &format!("{key_line}\n{marker}"), 1);
        changed = true;
    }

    if !text.contains(&key_alias) {
        let rule_marker = "  - path_regex: secrets/network\\.(yaml|json|env|ini)$\n";
        let age_marker = "      - age:\n";
        if !text.contains(rule_marker) || !text.contains(age_marker) {
            return fail(format!(
                "could not find the network.yaml creation rule in {}",
                paths.sops_config_file().display()
            ));
        }
        let rule_start = text.find(rule_marker).unwrap();
        let age_start = text[rule_start..].find(age_marker).unwrap() + rule_start;
        let age_list_start = age_start + age_marker.len();
        let next_rule = text[age_list_start..]
            .find("  - path_regex:")
            .map(|offset| age_list_start + offset)
            .unwrap_or(text.len());
        let age_block = &text[age_list_start..next_rule];
        let insertion = format!("          - {key_alias}\n");
        text = format!(
            "{}{}{}{}",
            &text[..age_list_start],
            age_block,
            insertion,
            &text[next_rule..]
        );
        changed = true;
    }

    if changed {
        fs::write(paths.sops_config_file(), text)?;
    }
    Ok(changed)
}

pub fn update_sops_yaml_delete(paths: &RepoPaths, hostname: &str) -> Result<bool> {
    let anchor = sanitized_anchor(hostname);
    let original = fs::read_to_string(paths.sops_config_file())?;
    let mut removed = false;
    let mut in_network_rule = false;
    let mut in_age_block = false;
    let mut new_lines = Vec::new();

    for line in original.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&format!("- &{anchor} ")) {
            removed = true;
            continue;
        }
        if line.starts_with("  - path_regex: ") {
            in_network_rule = line.contains("secrets/network\\.(yaml|json|env|ini)$");
            in_age_block = false;
            new_lines.push(line.to_string());
            continue;
        }
        if in_network_rule && line.starts_with("      - age:") {
            in_age_block = true;
            new_lines.push(line.to_string());
            continue;
        }
        if in_network_rule && in_age_block && trimmed == format!("- *{anchor}") {
            removed = true;
            continue;
        }
        if in_network_rule
            && (line.starts_with("  - path_regex: ")
                || (line.starts_with("    ")
                    && !line.starts_with("          - ")
                    && !line.trim().is_empty()))
        {
            in_age_block = false;
        }
        new_lines.push(line.to_string());
    }

    if removed {
        fs::write(
            paths.sops_config_file(),
            format!("{}\n", new_lines.join("\n")),
        )?;
    }
    Ok(removed)
}

pub fn network_rule_recipients(
    paths: &RepoPaths,
    exclude_hostname: Option<&str>,
) -> Result<HashSet<String>> {
    let aliases = sops_key_aliases(paths)?;
    let excluded_alias = exclude_hostname.map(sanitized_anchor);
    let mut recipients = HashSet::new();
    for alias in network_rule_aliases(paths)? {
        if excluded_alias.as_deref() == Some(alias.as_str()) {
            continue;
        }
        if let Some(recipient) = aliases.get(&alias) {
            recipients.insert(recipient.clone());
        }
    }
    Ok(recipients)
}

pub fn autodetect_sops_key_file(
    paths: &RepoPaths,
    exclude_hostname: Option<&str>,
) -> Result<PathBuf> {
    let recipients = network_rule_recipients(paths, exclude_hostname)?;
    let mut candidate_keys = Vec::new();
    for entry in fs::read_dir(paths.ssh_keys_dir())? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let directory = entry.path();
        if exclude_hostname.is_some()
            && directory.file_name().and_then(|v| v.to_str()) == exclude_hostname
        {
            continue;
        }
        let private_key = directory.join("ssh_host_ed25519_key");
        let public_key = directory.join("ssh_host_ed25519_key.pub");
        if private_key.exists() && public_key.exists() {
            candidate_keys.push(private_key);
        }
    }
    candidate_keys.sort();

    for private_key in candidate_keys {
        let public_key = private_key.with_extension("key.pub");
        let public_key = if public_key.exists() {
            public_key
        } else {
            private_key.with_file_name("ssh_host_ed25519_key.pub")
        };
        let public_value = fs::read_to_string(public_key)?;
        if recipients.contains(public_value.trim()) {
            return Ok(private_key.canonicalize()?);
        }
    }

    fail(format!(
        "could not auto-detect a decrypt-capable SSH private key for {}. Pass `--sops-key-file` explicitly.",
        paths.network_secrets_file().display()
    ))
}

pub fn reencrypt_network_yaml(
    paths: &RepoPaths,
    skip: bool,
    sops_key_file: Option<&str>,
    exclude_hostname: Option<&str>,
) -> Result<Option<PathBuf>> {
    if skip {
        return Ok(None);
    }

    let key_path = if let Some(path) = sops_key_file {
        let path = PathBuf::from(path);
        if path.is_absolute() {
            path
        } else {
            paths.root().join(path)
        }
    } else {
        autodetect_sops_key_file(paths, exclude_hostname)?
    };

    if !key_path.exists() {
        return fail(format!(
            "existing SSH private key for sops updatekeys not found: {}",
            key_path.display()
        ));
    }

    let status = Command::new("sops")
        .args(["updatekeys", "-y"])
        .arg(paths.network_secrets_file())
        .current_dir(paths.root())
        .env("SOPS_AGE_SSH_PRIVATE_KEY_FILE", &key_path)
        .status()?;
    if !status.success() {
        return fail(format!(
            "command failed with exit code {}: sops",
            status.code().unwrap_or(1)
        ));
    }

    Ok(Some(key_path))
}

#[cfg(test)]
mod tests {
    use super::{
        autodetect_sops_key_file, network_rule_aliases, network_rule_recipients, sops_key_aliases,
        update_sops_yaml_add, update_sops_yaml_delete,
    };
    use crate::repo::RepoPaths;
    use std::fs;
    use tempfile::tempdir;

    const SOPS_BASE: &str = r#"keys:
  - &genesis "ssh-ed25519 AAAABASE root@genesis"
creation_rules:
  - path_regex: secrets/network\.(yaml|json|env|ini)$
    key_groups:
      - age:
          - *genesis
"#;

    const SEMBLE_TOML: &str = r#"[paths]
hosts_dir = "hosts"
host_template_dir = "hosts/host.template"
ssh_host_keys_dir = "ssh_host_keys"
sops_config_file = ".sops.yaml"
network_secrets_file = "secrets/network.yaml"

[ssh]
managed_config_file = ".ssh/semble_hosts"
dns_suffix = "baiji-carat.ts.net"

[[ssh.aliases]]
name_suffix = "admin"
user = "admin"
identity_file = "~/.ssh/homelab_admin"

[[ssh.aliases]]
name_suffix = "deploy"
user = "deploy"
identity_file = "~/.ssh/homelab_deploy"
"#;

    fn setup_repo() -> (tempfile::TempDir, RepoPaths) {
        let tempdir = tempdir().unwrap();
        let root = tempdir.path().to_path_buf();
        fs::create_dir_all(root.join("ssh_host_keys").join("genesis")).unwrap();
        fs::create_dir_all(root.join("secrets")).unwrap();
        fs::write(root.join("semble.toml"), SEMBLE_TOML).unwrap();
        fs::write(root.join(".sops.yaml"), SOPS_BASE).unwrap();
        fs::write(root.join("secrets").join("network.yaml"), "encrypted\n").unwrap();
        fs::write(
            root.join("ssh_host_keys")
                .join("genesis")
                .join("ssh_host_ed25519_key"),
            "PRIVATE\n",
        )
        .unwrap();
        fs::write(
            root.join("ssh_host_keys")
                .join("genesis")
                .join("ssh_host_ed25519_key.pub"),
            "ssh-ed25519 AAAABASE root@genesis\n",
        )
        .unwrap();
        (tempdir, RepoPaths::new(root).unwrap())
    }

    #[test]
    fn reads_key_aliases_and_network_rule_aliases() {
        let (_tempdir, paths) = setup_repo();
        let aliases = sops_key_aliases(&paths).unwrap();
        assert_eq!(
            aliases.get("genesis"),
            Some(&"ssh-ed25519 AAAABASE root@genesis".to_string())
        );
        assert_eq!(network_rule_aliases(&paths).unwrap(), vec!["genesis"]);
    }

    #[test]
    fn updates_sops_yaml_add_and_delete() {
        let (_tempdir, paths) = setup_repo();
        let changed =
            update_sops_yaml_add(&paths, "atlas", "ssh-ed25519 AAAAATLAS root@atlas").unwrap();
        assert!(changed);
        let contents = fs::read_to_string(paths.sops_config_file()).unwrap();
        assert!(contents.contains("&atlas \"ssh-ed25519 AAAAATLAS root@atlas\""));
        assert!(contents.contains("*atlas"));

        let changed = update_sops_yaml_delete(&paths, "atlas").unwrap();
        assert!(changed);
        let contents = fs::read_to_string(paths.sops_config_file()).unwrap();
        assert!(!contents.contains("&atlas"));
        assert!(!contents.contains("*atlas"));
    }

    #[test]
    fn finds_network_rule_recipients_and_autodetects_key() {
        let (_tempdir, paths) = setup_repo();
        let recipients = network_rule_recipients(&paths, None).unwrap();
        assert!(recipients.contains("ssh-ed25519 AAAABASE root@genesis"));
        let key = autodetect_sops_key_file(&paths, None).unwrap();
        assert!(key.ends_with("ssh_host_ed25519_key"));
    }

    #[test]
    fn autodetect_skips_newly_added_hostname_when_requested() {
        let (_tempdir, paths) = setup_repo();
        fs::create_dir_all(paths.host_keys_dir("alpha")).unwrap();
        fs::write(
            paths.host_keys_dir("alpha").join("ssh_host_ed25519_key"),
            "PRIVATE-ALPHA\n",
        )
        .unwrap();
        fs::write(
            paths.host_keys_dir("alpha").join("ssh_host_ed25519_key.pub"),
            "ssh-ed25519 AAAAALPHA root@alpha\n",
        )
        .unwrap();

        update_sops_yaml_add(&paths, "alpha", "ssh-ed25519 AAAAALPHA root@alpha").unwrap();

        let key = autodetect_sops_key_file(&paths, Some("alpha")).unwrap();
        assert!(key.ends_with("genesis/ssh_host_ed25519_key"));
    }
}
