use crate::keys::SSH_PUBLIC_KEY_FILENAME;
use crate::repo::{load_host_manifests, HostManifestConfig, HostOperatorAlias, RepoPaths};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;

const DEFAULT_ALIAS_TEMPLATES: &[&str] = &["admin", "deploy"];

#[derive(Debug, Clone, PartialEq, Eq)]
struct AliasTemplate {
    suffix: &'static str,
    user: &'static str,
    identity_file: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedAlias {
    source_host: String,
    name: String,
    host_name: String,
    user: String,
    identity_file: String,
}

fn template_definition(name: &str) -> Option<AliasTemplate> {
    match name {
        "admin" => Some(AliasTemplate {
            suffix: "admin",
            user: "admin",
            identity_file: "%d/.ssh/homelab_admin",
        }),
        "deploy" => Some(AliasTemplate {
            suffix: "deploy",
            user: "deploy",
            identity_file: "%d/.ssh/homelab_deploy",
        }),
        "builder" => Some(AliasTemplate {
            suffix: "builder",
            user: "builder",
            identity_file: "%d/.ssh/homelab_builder",
        }),
        _ => None,
    }
}

pub fn run_host_ssh_generate(paths: &RepoPaths) -> Result<()> {
    let manifests = load_host_manifests(paths)?;
    let aliases = collect_server_aliases(&manifests)?;
    let alias_config = render_alias_config(&aliases);
    let known_hosts = render_known_hosts(paths, &aliases)?;

    fs::create_dir_all(paths.ssh_cache_dir()).with_context(|| {
        format!(
            "failed to create SSH cache directory {}",
            paths.ssh_cache_dir().display()
        )
    })?;
    fs::write(paths.ssh_alias_config_file(), alias_config).with_context(|| {
        format!(
            "failed to write {}",
            paths.ssh_alias_config_file().display()
        )
    })?;
    fs::write(paths.ssh_known_hosts_file(), known_hosts)
        .with_context(|| format!("failed to write {}", paths.ssh_known_hosts_file().display()))?;

    println!(
        "Generated SSH config: {}",
        paths.ssh_alias_config_file().display()
    );
    println!(
        "Generated known hosts: {}",
        paths.ssh_known_hosts_file().display()
    );
    Ok(())
}

pub fn ensure_host_ssh_artifacts_for_client(paths: &RepoPaths, hostname: &str) -> Result<()> {
    let manifests = load_host_manifests(paths)?;
    let Some(manifest) = manifests.get(hostname) else {
        return Ok(());
    };
    if manifest
        .operator
        .as_ref()
        .and_then(|operator| operator.role.as_deref())
        == Some("client")
    {
        run_host_ssh_generate(paths)?;
    }
    Ok(())
}

fn collect_server_aliases(
    manifests: &BTreeMap<String, HostManifestConfig>,
) -> Result<Vec<RenderedAlias>> {
    let mut aliases = Vec::new();
    for (host_key, manifest) in manifests {
        let Some(operator) = &manifest.operator else {
            continue;
        };
        if operator.role.as_deref() != Some("server") {
            continue;
        }

        let host_name = operator
            .host_name
            .as_deref()
            .or(manifest.host_name.as_deref())
            .unwrap_or(host_key);
        let mut host_aliases = BTreeMap::new();
        let template_names = operator
            .alias_templates
            .clone()
            .unwrap_or_else(|| {
                DEFAULT_ALIAS_TEMPLATES
                    .iter()
                    .map(|name| name.to_string())
                    .collect()
            })
            .into_iter()
            .chain(operator.extra_alias_templates.iter().cloned());

        for template_name in template_names {
            let template = template_definition(&template_name).ok_or_else(|| {
                anyhow::anyhow!(
                    "unknown SSH alias template `{template_name}` for host `{host_key}`"
                )
            })?;
            let alias = RenderedAlias {
                source_host: host_key.to_string(),
                name: format!("{host_key}-{}", template.suffix),
                host_name: host_name.to_string(),
                user: template.user.to_string(),
                identity_file: template.identity_file.to_string(),
            };
            host_aliases.insert(alias.name.clone(), alias);
        }

        for explicit in &operator.aliases {
            let alias = explicit_alias(host_key, host_name, explicit)?;
            host_aliases.insert(alias.name.clone(), alias);
        }

        aliases.extend(host_aliases.into_values());
    }

    Ok(aliases)
}

fn explicit_alias(
    host_key: &str,
    default_host_name: &str,
    alias: &HostOperatorAlias,
) -> Result<RenderedAlias> {
    Ok(RenderedAlias {
        source_host: host_key.to_string(),
        name: alias.name.clone().unwrap_or_else(|| host_key.to_string()),
        host_name: alias
            .host_name
            .clone()
            .unwrap_or_else(|| default_host_name.to_string()),
        user: alias.user.clone(),
        identity_file: alias.identity_file.clone(),
    })
}

fn render_alias_config(aliases: &[RenderedAlias]) -> String {
    let mut output = String::new();
    for alias in aliases {
        output.push_str(&format!(
            "Host {}\n  HostName {}\n  User {}\n  IdentityFile {}\n  IdentitiesOnly yes\n\n",
            alias.name, alias.host_name, alias.user, alias.identity_file
        ));
    }
    output
}

fn render_known_hosts(paths: &RepoPaths, aliases: &[RenderedAlias]) -> Result<String> {
    let mut by_host = BTreeMap::<String, Vec<&RenderedAlias>>::new();
    for alias in aliases {
        by_host
            .entry(alias.host_name.clone())
            .or_default()
            .push(alias);
    }

    let mut output = String::new();
    for (host_name, aliases) in by_host {
        let public_key = read_public_host_key(paths, &aliases[0].source_host)?;
        let mut names = vec![host_name];
        names.extend(aliases.into_iter().map(|alias| alias.name.clone()));
        names.sort();
        names.dedup();
        output.push_str(&format!("{} {}\n", names.join(","), public_key.trim()));
    }
    Ok(output)
}

fn read_public_host_key(paths: &RepoPaths, host_key: &str) -> Result<String> {
    let path = paths.host_keys_dir(host_key).join(SSH_PUBLIC_KEY_FILENAME);
    fs::read_to_string(&path).with_context(|| {
        format!(
            "failed to read SSH public host key for `{host_key}` at {}",
            path.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::{HostOperatorConfig, RepoPaths};
    use std::fs;
    use tempfile::tempdir;

    fn manifest(role: &str) -> HostManifestConfig {
        HostManifestConfig {
            host_name: Some("server.example".into()),
            operator: Some(HostOperatorConfig {
                role: Some(role.into()),
                host_name: None,
                alias_templates: None,
                extra_alias_templates: Vec::new(),
                aliases: Vec::new(),
            }),
        }
    }

    #[test]
    fn renders_default_server_aliases() {
        let manifests = BTreeMap::from([("server-a".into(), manifest("server"))]);

        let aliases = collect_server_aliases(&manifests).unwrap();
        let config = render_alias_config(&aliases);

        assert!(config.contains("Host server-a-admin\n"));
        assert!(config.contains("User admin\n"));
        assert!(config.contains("Host server-a-deploy\n"));
        assert!(config.contains("User deploy\n"));
    }

    #[test]
    fn supports_builder_template_opt_in() {
        let mut server = manifest("server");
        server.operator.as_mut().unwrap().extra_alias_templates = vec!["builder".into()];
        let manifests = BTreeMap::from([("builder-host".into(), server)]);

        let aliases = collect_server_aliases(&manifests).unwrap();
        let names = aliases
            .iter()
            .map(|alias| alias.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"builder-host-admin"));
        assert!(names.contains(&"builder-host-deploy"));
        assert!(names.contains(&"builder-host-builder"));
    }

    #[test]
    fn empty_alias_templates_disables_defaults_but_keeps_explicit_aliases() {
        let mut server = manifest("server");
        let operator = server.operator.as_mut().unwrap();
        operator.alias_templates = Some(Vec::new());
        operator.aliases = vec![HostOperatorAlias {
            name: Some("bootstrap-host".into()),
            user: "root".into(),
            identity_file: "%d/.ssh/bootstrap_installer".into(),
            host_name: None,
        }];
        let manifests = BTreeMap::from([("genesis".into(), server)]);

        let aliases = collect_server_aliases(&manifests).unwrap();
        let config = render_alias_config(&aliases);

        assert!(!config.contains("Host genesis-admin\n"));
        assert!(!config.contains("Host genesis-deploy\n"));
        assert!(config.contains("Host bootstrap-host\n"));
    }

    #[test]
    fn explicit_alias_overrides_template_alias() {
        let mut server = manifest("server");
        server.operator.as_mut().unwrap().aliases = vec![HostOperatorAlias {
            name: Some("server-a-admin".into()),
            user: "root".into(),
            identity_file: "%d/.ssh/root".into(),
            host_name: None,
        }];
        let manifests = BTreeMap::from([("server-a".into(), server)]);

        let aliases = collect_server_aliases(&manifests).unwrap();
        let admin = aliases
            .iter()
            .find(|alias| alias.name == "server-a-admin")
            .unwrap();

        assert_eq!(admin.user, "root");
        assert_eq!(
            aliases
                .iter()
                .filter(|alias| alias.name == "server-a-admin")
                .count(),
            1
        );
    }

    #[test]
    fn renders_plain_known_hosts_from_public_keys() {
        let tempdir = tempdir().unwrap();
        fs::write(
            tempdir.path().join("semble.toml"),
            r#"[paths]
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
        fs::create_dir_all(tempdir.path().join("ssh_host_keys/server-a")).unwrap();
        fs::write(
            tempdir
                .path()
                .join("ssh_host_keys/server-a/ssh_host_ed25519_key.pub"),
            "ssh-ed25519 AAAATEST server-a\n",
        )
        .unwrap();
        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let aliases = vec![RenderedAlias {
            source_host: "server-a".into(),
            name: "server-a-admin".into(),
            host_name: "server.example".into(),
            user: "admin".into(),
            identity_file: "%d/.ssh/admin".into(),
        }];

        let known_hosts = render_known_hosts(&paths, &aliases).unwrap();

        assert_eq!(
            known_hosts,
            "server-a-admin,server.example ssh-ed25519 AAAATEST server-a\n"
        );
    }
}
