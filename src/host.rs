use crate::confirm::require_delete_confirmation;
use crate::error::fail;
use crate::keys::{generate_ssh_host_keys, read_public_key_from_dir};
use crate::repo::RepoPaths;
use crate::sops::{
    network_rule_aliases, reencrypt_network_yaml, update_sops_yaml_add, update_sops_yaml_delete,
};
use crate::ssh;
use crate::template::{copy_host_template, ensure_facter_file};
use anyhow::Result;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostPresence {
    pub host_dir: bool,
    pub keys_dir: bool,
    pub sops: bool,
}

pub fn validate_hostname(hostname: &str) -> Result<()> {
    let mut chars = hostname.chars();
    let Some(first) = chars.next() else {
        return fail("hostname must match `[a-z0-9][a-z0-9-]*`; got \"\"");
    };

    if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return fail(format!(
            "hostname must match `[a-z0-9][a-z0-9-]*`; got {hostname:?}"
        ));
    }

    if !chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-') {
        return fail(format!(
            "hostname must match `[a-z0-9][a-z0-9-]*`; got {hostname:?}"
        ));
    }

    Ok(())
}

pub fn sanitized_anchor(hostname: &str) -> String {
    hostname.replace('-', "_")
}

pub fn host_presence(paths: &RepoPaths, hostname: &str) -> Result<HostPresence> {
    let anchor = sanitized_anchor(hostname);
    let sops_text = fs::read_to_string(paths.sops_config_file())?;

    Ok(HostPresence {
        host_dir: paths.host_dir(hostname).exists(),
        keys_dir: paths.host_keys_dir(hostname).exists(),
        sops: sops_text.contains(&format!("&{anchor}"))
            || sops_text.contains(&format!("*{anchor}")),
    })
}

pub fn assert_hostname_is_new(paths: &RepoPaths, hostname: &str) -> Result<()> {
    let presence = host_presence(paths, hostname)?;
    let mut conflicts = Vec::new();

    if presence.host_dir {
        conflicts.push(format!(
            "host directory exists: {}",
            paths.host_dir(hostname).display()
        ));
    }
    if presence.keys_dir {
        conflicts.push(format!(
            "SSH host keys directory exists: {}",
            paths.host_keys_dir(hostname).display()
        ));
    }
    if presence.sops {
        conflicts.push(format!(
            "hostname already present in {}",
            paths.sops_config_file().display()
        ));
    }

    if conflicts.is_empty() {
        return Ok(());
    }

    fail(format!(
        "refusing to create host because hostname already exists:\n{}",
        conflicts
            .iter()
            .map(|item| format!("  - {item}"))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

pub fn assert_hostname_exists_for_delete(
    paths: &RepoPaths,
    hostname: &str,
    keys_only: bool,
) -> Result<()> {
    let presence = host_presence(paths, hostname)?;
    if keys_only {
        if presence.keys_dir || presence.sops {
            return Ok(());
        }

        return fail(format!(
            "refusing to delete host keys because hostname has no key-related entries:\n  - key directory: {} (missing)\n  - SOPS entry: {} (missing for {hostname})",
            paths.host_keys_dir(hostname).display(),
            paths.sops_config_file().display(),
        ));
    }

    if [presence.host_dir, presence.keys_dir, presence.sops]
        .into_iter()
        .any(|value| value)
    {
        return Ok(());
    }

    fail(format!(
        "refusing to delete host because hostname was not found in any managed location:\n  - host directory: {} (missing)\n  - key directory: {} (missing)\n  - SOPS entry: {} (missing for {hostname})",
        paths.host_dir(hostname).display(),
        paths.host_keys_dir(hostname).display(),
        paths.sops_config_file().display(),
    ))
}

pub fn remove_host_files(paths: &RepoPaths, hostname: &str) -> Result<(bool, bool)> {
    let mut removed_host = false;
    let mut removed_keys = false;
    let host_dir = paths.host_dir(hostname);
    let keys_dir = paths.host_keys_dir(hostname);

    if host_dir.exists() {
        fs::remove_dir_all(&host_dir)?;
        removed_host = true;
    }
    if keys_dir.exists() {
        fs::remove_dir_all(&keys_dir)?;
        removed_keys = true;
    }
    Ok((removed_host, removed_keys))
}

pub fn run_host_create(
    paths: &RepoPaths,
    hostname: &str,
    force: bool,
    skip_reencrypt: bool,
    sops_key_file: Option<&str>,
) -> Result<()> {
    assert_hostname_is_new(paths, hostname)?;
    let dst_dir = copy_host_template(paths, hostname, force)?;
    ensure_facter_file(&dst_dir)?;
    let keys_dir = generate_ssh_host_keys(paths, hostname, force)?;

    if !skip_reencrypt {
        let public_key = read_public_key_from_dir(&keys_dir)?;
        update_sops_yaml_add(paths, hostname, &public_key)?;
    }
    let sops_key_path =
        reencrypt_network_yaml(paths, skip_reencrypt, sops_key_file, Some(hostname))?;
    ssh::run_ssh_setup(paths).map_err(|error| {
        anyhow::anyhow!(
            "host was created but failed to refresh generated SSH aliases: {error}"
        )
    })?;
    print_create_summary(
        paths,
        &dst_dir,
        &keys_dir,
        !skip_reencrypt,
        sops_key_path.as_deref(),
    );
    Ok(())
}

pub fn run_host_keys_add(
    paths: &RepoPaths,
    hostname: &str,
    force: bool,
    skip_reencrypt: bool,
    sops_key_file: Option<&str>,
) -> Result<()> {
    let keys_dir = generate_ssh_host_keys(paths, hostname, force)?;
    if !skip_reencrypt {
        let public_key = read_public_key_from_dir(&keys_dir)?;
        update_sops_yaml_add(paths, hostname, &public_key)?;
    }
    let sops_key_path =
        reencrypt_network_yaml(paths, skip_reencrypt, sops_key_file, Some(hostname))?;
    print_keys_summary(paths, &keys_dir, !skip_reencrypt, sops_key_path.as_deref());
    Ok(())
}

pub fn run_host_keys_delete(
    paths: &RepoPaths,
    hostname: &str,
    assume_yes: bool,
    skip_reencrypt: bool,
    sops_key_file: Option<&str>,
) -> Result<()> {
    assert_hostname_exists_for_delete(paths, hostname, true)?;
    require_delete_confirmation(
        hostname,
        "keys delete",
        &[
            format!(
                "delete SSH host keys under {}",
                paths.host_keys_dir(hostname).display()
            ),
            format!(
                "remove `{hostname}` recipient from {}",
                paths.sops_config_file().display()
            ),
            format!(
                "re-encrypt {} (unless --skip-reencrypt)",
                paths.network_secrets_file().display()
            ),
        ],
        assume_yes,
    )?;

    let mut sops_changed = false;
    if !skip_reencrypt {
        sops_changed = update_sops_yaml_delete(paths, hostname)?;
        if sops_changed && network_rule_aliases(paths)?.is_empty() {
            return fail("refusing to leave secrets/network.yaml with no recipients");
        }
    }

    let sops_key_path = if sops_changed {
        reencrypt_network_yaml(paths, skip_reencrypt, sops_key_file, Some(hostname))?
    } else {
        None
    };

    let keys_dir = paths.host_keys_dir(hostname);
    let removed_keys = if keys_dir.exists() {
        fs::remove_dir_all(&keys_dir)?;
        true
    } else {
        false
    };

    println!(
        "Deleted SSH host keys: {} ({})",
        relative_to_root(paths, &keys_dir),
        yes_no(removed_keys)
    );
    if sops_changed {
        if skip_reencrypt {
            println!(
                "Removed host from .sops.yaml but skipped re-encryption for: {}",
                relative_to_root(paths, &paths.network_secrets_file())
            );
        } else {
            println!(
                "Updated SOPS recipients for: {}",
                relative_to_root(paths, &paths.network_secrets_file())
            );
            println!("Used SOPS update key: {}", sops_key_path.unwrap().display());
        }
    } else {
        println!("No .sops.yaml recipient changes were needed for: {hostname}");
    }
    Ok(())
}

pub fn run_host_delete(
    paths: &RepoPaths,
    hostname: &str,
    assume_yes: bool,
    skip_reencrypt: bool,
    sops_key_file: Option<&str>,
) -> Result<()> {
    assert_hostname_exists_for_delete(paths, hostname, false)?;
    require_delete_confirmation(
        hostname,
        "delete",
        &[
            format!(
                "delete host scaffold {} (including facter.json)",
                relative_to_root(paths, &paths.host_dir(hostname))
            ),
            format!(
                "delete SSH host keys under {}",
                relative_to_root(paths, &paths.host_keys_dir(hostname))
            ),
            format!(
                "remove `{hostname}` recipient from {}",
                relative_to_root(paths, &paths.sops_config_file())
            ),
            format!(
                "re-encrypt {} (unless --skip-reencrypt)",
                relative_to_root(paths, &paths.network_secrets_file())
            ),
            format!(
                "refresh generated SSH aliases at {}",
                relative_to_root(paths, &paths.ssh_managed_config_file())
            ),
        ],
        assume_yes,
    )?;

    let mut sops_changed = false;
    if !skip_reencrypt {
        sops_changed = update_sops_yaml_delete(paths, hostname)?;
        if sops_changed && network_rule_aliases(paths)?.is_empty() {
            return fail("refusing to leave secrets/network.yaml with no recipients");
        }
    }

    let sops_key_path = if sops_changed {
        reencrypt_network_yaml(paths, skip_reencrypt, sops_key_file, Some(hostname))?
    } else {
        None
    };
    let (removed_host, removed_keys) = remove_host_files(paths, hostname)?;
    ssh::run_ssh_setup(paths).map_err(|error| {
        anyhow::anyhow!(
            "host was deleted but failed to refresh generated SSH aliases: {error}"
        )
    })?;
    print_delete_summary(
        paths,
        hostname,
        DeleteSummary {
            removed_host,
            removed_keys,
            reencrypted: sops_changed && !skip_reencrypt,
            sops_changed,
            sops_key_path: sops_key_path.as_deref(),
        },
    );
    Ok(())
}

fn print_create_summary(
    paths: &RepoPaths,
    dst_dir: &Path,
    keys_dir: &Path,
    reencrypted: bool,
    sops_key_path: Option<&Path>,
) {
    print!(
        "{}",
        create_summary_text(
            paths,
            dst_dir,
            keys_dir,
            reencrypted,
            sops_key_path,
        )
    );
}

fn create_summary_text(
    paths: &RepoPaths,
    dst_dir: &Path,
    keys_dir: &Path,
    reencrypted: bool,
    sops_key_path: Option<&Path>,
) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "Created host scaffold: {}",
        relative_to_root(paths, dst_dir)
    );
    let _ = writeln!(
        output,
        "Created SSH host keys: {}",
        relative_to_root(paths, keys_dir)
    );
    let _ = writeln!(
        output,
        "Refreshed generated SSH aliases at {}",
        relative_to_root(paths, &paths.ssh_managed_config_file()),
    );
    if reencrypted {
        let _ = writeln!(
            output,
            "Updated SOPS recipients for: {}",
            relative_to_root(paths, &paths.network_secrets_file())
        );
        if let Some(path) = sops_key_path {
            let _ = writeln!(output, "Used SOPS update key: {}", path.display());
        }
    } else {
        let _ = writeln!(
            output,
            "Skipped SOPS re-encryption for: {}",
            relative_to_root(paths, &paths.network_secrets_file())
        );
    }
    let _ = writeln!(output);
    let _ = writeln!(
        output,
        "Next, review and adjust the host-specific settings in {}/.",
        relative_to_root(paths, dst_dir),
    );
    output
}

fn print_keys_summary(
    paths: &RepoPaths,
    keys_dir: &Path,
    reencrypted: bool,
    sops_key_path: Option<&Path>,
) {
    print!("{}", keys_summary_text(paths, keys_dir, reencrypted, sops_key_path));
}

fn keys_summary_text(
    paths: &RepoPaths,
    keys_dir: &Path,
    reencrypted: bool,
    sops_key_path: Option<&Path>,
) -> String {
    let mut output = String::new();
    let _ = writeln!(
        output,
        "Created SSH host keys: {}",
        relative_to_root(paths, keys_dir)
    );
    if reencrypted {
        let _ = writeln!(
            output,
            "Updated SOPS recipients for: {}",
            relative_to_root(paths, &paths.network_secrets_file())
        );
        if let Some(path) = sops_key_path {
            let _ = writeln!(output, "Used SOPS update key: {}", path.display());
        }
    } else {
        let _ = writeln!(
            output,
            "Skipped SOPS re-encryption for: {}",
            relative_to_root(paths, &paths.network_secrets_file())
        );
    }
    let _ = writeln!(output);
    let _ = writeln!(
        output,
        "Next, review whether the host-specific keys and secret-recipient changes under {} should be kept.",
        relative_to_root(paths, keys_dir)
    );
    output
}

struct DeleteSummary<'a> {
    removed_host: bool,
    removed_keys: bool,
    reencrypted: bool,
    sops_changed: bool,
    sops_key_path: Option<&'a Path>,
}

fn print_delete_summary(paths: &RepoPaths, hostname: &str, summary: DeleteSummary<'_>) {
    println!(
        "Deleted host scaffold: {} ({})",
        relative_to_root(paths, &paths.host_dir(hostname)),
        yes_no(summary.removed_host)
    );
    println!(
        "Deleted SSH host keys: {} ({})",
        relative_to_root(paths, &paths.host_keys_dir(hostname)),
        yes_no(summary.removed_keys)
    );
    println!(
        "Refreshed generated SSH aliases at {}",
        relative_to_root(paths, &paths.ssh_managed_config_file()),
    );
    if summary.sops_changed {
        if summary.reencrypted {
            println!(
                "Updated SOPS recipients for: {}",
                relative_to_root(paths, &paths.network_secrets_file())
            );
            if let Some(path) = summary.sops_key_path {
                println!("Used SOPS update key: {}", path.display());
            }
        } else {
            println!(
                "Removed host from .sops.yaml but skipped re-encryption for: {}",
                relative_to_root(paths, &paths.network_secrets_file())
            );
        }
    } else {
        println!("No .sops.yaml recipient changes were needed for: {hostname}");
    }
}

fn relative_to_root(paths: &RepoPaths, path: &Path) -> String {
    path.strip_prefix(paths.root())
        .unwrap_or(path)
        .display()
        .to_string()
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "not present"
    }
}

#[cfg(test)]
mod tests {
    use super::{
        assert_hostname_exists_for_delete, assert_hostname_is_new, create_summary_text,
        host_presence, keys_summary_text, validate_hostname,
    };
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

[[ssh.aliases]]
name_suffix = "deploy"
user = "deploy"
identity_file = "~/.ssh/deploy_key"
"#;

    const SOPS_BASE: &str = r#"keys:
  - &genesis "PUBLIC_KEY_GENESIS"
creation_rules:
  - path_regex: secrets/network\.(yaml|json|env|ini)$
    key_groups:
      - age:
          - *genesis
"#;

    fn setup_repo() -> (tempfile::TempDir, RepoPaths) {
        let tempdir = tempdir().unwrap();
        let root = tempdir.path().to_path_buf();
        fs::create_dir_all(root.join("hosts")).unwrap();
        fs::create_dir_all(root.join("ssh_host_keys")).unwrap();
        fs::create_dir_all(root.join("secrets")).unwrap();
        fs::write(root.join("semble.toml"), SEMBLE_TOML).unwrap();
        fs::write(root.join(".sops.yaml"), SOPS_BASE).unwrap();
        (tempdir, RepoPaths::new(root).unwrap())
    }

    #[test]
    fn rejects_invalid_hostnames() {
        assert!(validate_hostname("atlas").is_ok());
        assert!(validate_hostname("atlas-01").is_ok());
        assert!(validate_hostname("Atlas").is_err());
        assert!(validate_hostname("-atlas").is_err());
        assert!(validate_hostname("").is_err());
    }

    #[test]
    fn assert_hostname_is_new_reports_conflicts() {
        let (_tempdir, paths) = setup_repo();
        let hostname = "atlas";
        fs::create_dir_all(paths.host_dir(hostname)).unwrap();
        fs::create_dir_all(paths.host_keys_dir(hostname)).unwrap();
        fs::write(
            paths.sops_config_file(),
            format!("{SOPS_BASE}  - &{hostname} \"PUBLIC_KEY_{}\"\n", hostname.to_uppercase()),
        )
        .unwrap();

        let error = assert_hostname_is_new(&paths, hostname)
            .unwrap_err()
            .to_string();
        assert!(error.contains("hostname already exists"));
        assert!(error.contains("host directory exists"));
        assert!(error.contains("SSH host keys directory exists"));
        assert!(error.contains("hostname already present in"));
    }

    #[test]
    fn assert_hostname_exists_for_delete_requires_presence() {
        let (_tempdir, paths) = setup_repo();

        let error = assert_hostname_exists_for_delete(&paths, "atlas", false)
            .unwrap_err()
            .to_string();
        assert!(error.contains("hostname was not found"));

        let error = assert_hostname_exists_for_delete(&paths, "atlas", true)
            .unwrap_err()
            .to_string();
        assert!(error.contains("no key-related entries"));
    }

    #[test]
    fn host_presence_reports_repo_state() {
        let (_tempdir, paths) = setup_repo();
        fs::create_dir_all(paths.host_dir("atlas")).unwrap();
        fs::create_dir_all(paths.host_keys_dir("atlas")).unwrap();
        let presence = host_presence(&paths, "atlas").unwrap();
        assert!(presence.host_dir);
        assert!(presence.keys_dir);
        assert!(!presence.sops);
    }

    #[test]
    fn create_summary_is_minimal_and_repo_local() {
        let (_tempdir, paths) = setup_repo();
        let hostname = "atlas";
        let output = create_summary_text(
            &paths,
            &paths.host_dir(hostname),
            &paths.host_keys_dir(hostname),
            false,
            None,
        );

        assert!(output.contains("Created host scaffold: hosts/atlas"));
        assert!(output.contains("Created SSH host keys: ssh_host_keys/atlas"));
        assert!(output.contains("Refreshed generated SSH aliases at .ssh/semble_hosts"));
        assert!(
            output.contains("Next, review and adjust the host-specific settings in hosts/atlas/.")
        );
        assert!(!output.contains("git add"));
        assert!(!output.contains("nix build"));
        assert!(!output.contains("semble host build"));
        assert!(!output.contains("semble host switch"));
    }

    #[test]
    fn keys_summary_is_minimal_and_repo_local() {
        let (_tempdir, paths) = setup_repo();
        let hostname = "atlas";
        let output = keys_summary_text(&paths, &paths.host_keys_dir(hostname), false, None);

        assert!(output.contains("Created SSH host keys: ssh_host_keys/atlas"));
        assert!(output.contains(
            "Next, review whether the host-specific keys and secret-recipient changes under ssh_host_keys/atlas should be kept."
        ));
        assert!(!output.contains("Next steps:"));
        assert!(!output.contains("git add"));
        assert!(!output.contains("  1."));
    }
}
