use crate::cli::DelegatedHostArgs;
use crate::config::BuilderPolicyConfig;
use crate::repo::RepoPaths;
use anyhow::Result;
use std::env;
use std::ffi::{OsStr, OsString};

pub fn build_host_args(
    action: &str,
    args: &DelegatedHostArgs,
    builders_override: Option<&str>,
) -> Vec<OsString> {
    let mut delegated = vec![
        OsString::from("os"),
        OsString::from(action),
        OsString::from("."),
        OsString::from("-H"),
        OsString::from(&args.hostname),
    ];
    if let Some(builders) = builders_override {
        delegated.push(OsString::from("--builders"));
        delegated.push(OsString::from(builders));
        delegated.push(OsString::from("--max-jobs"));
        delegated.push(OsString::from("0"));
    }
    delegated.extend(args.extra_args.iter().cloned());
    delegated
}

pub fn provision_host_args(args: &DelegatedHostArgs) -> Vec<OsString> {
    let mut delegated = vec![
        OsString::from("provision"),
        OsString::from("."),
        OsString::from("-H"),
        OsString::from(&args.hostname),
    ];
    delegated.extend(args.extra_args.iter().cloned());
    delegated
}

fn normalize_builder_policy(args: DelegatedHostArgs) -> Result<DelegatedHostArgs> {
    let mut builder_policy = args.builder_policy.clone();
    let mut extra_args = Vec::with_capacity(args.extra_args.len());
    let mut iter = args.extra_args.into_iter();

    while let Some(arg) = iter.next() {
        if arg == "--builder-policy" {
            let value = iter.next().ok_or_else(|| {
                anyhow::anyhow!("`--builder-policy` requires a policy name")
            })?;
            let value = value
                .into_string()
                .map_err(|_| anyhow::anyhow!("`--builder-policy` value must be valid UTF-8"))?;

            if builder_policy.is_some() {
                return Err(anyhow::anyhow!(
                    "`--builder-policy` was provided more than once"
                ));
            }

            builder_policy = Some(value);
            continue;
        }

        extra_args.push(arg);
    }

    Ok(DelegatedHostArgs {
        hostname: args.hostname,
        builder_policy,
        extra_args,
    })
}

fn serialize_builder_policy(policy: &BuilderPolicyConfig) -> String {
    let ssh_key = policy.ssh_key.as_deref().unwrap_or("-");
    let features = if policy.supported_features.is_empty() {
        String::from("-")
    } else {
        policy.supported_features.join(",")
    };

    format!(
        "ssh://{} {} {} {} {} {}",
        policy.host, policy.system, ssh_key, policy.max_jobs, policy.speed_factor, features
    )
}

fn merge_nix_config(existing: Option<&OsStr>, extra_line: &str) -> OsString {
    match existing {
        Some(existing) if !existing.is_empty() => {
            let mut merged = existing.to_os_string();
            merged.push("\n");
            merged.push(extra_line);
            merged
        }
        _ => OsString::from(extra_line),
    }
}

fn apply_builder_policy(paths: &RepoPaths, args: &DelegatedHostArgs) -> Result<Option<()>> {
    let Some(policy_name) = args.builder_policy.as_deref() else {
        return Ok(None);
    };

    let _policy = paths.builder_policy(policy_name).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown builder policy `{policy_name}` in {}",
            paths.root().join("semble.toml").display()
        )
    })?;

    if args.extra_args.iter().any(|arg| arg == "--builders") {
        return Err(anyhow::anyhow!(
            "`--builder-policy` cannot be combined with an explicit `--builders` argument"
        ));
    }

    Ok(Some(()))
}

pub fn run_host_build(paths: &RepoPaths, args: DelegatedHostArgs) -> Result<()> {
    let args = normalize_builder_policy(args)?;
    let builders_override = args
        .builder_policy
        .as_deref()
        .and_then(|policy_name| paths.builder_policy(policy_name))
        .map(serialize_builder_policy);
    let _env_guard = apply_builder_policy(paths, &args)?;
    tianyi::run_args(build_host_args(
        "build",
        &args,
        builders_override.as_deref(),
    ))
}

pub fn run_host_switch(paths: &RepoPaths, args: DelegatedHostArgs) -> Result<()> {
    let args = normalize_builder_policy(args)?;
    let builders_override = args
        .builder_policy
        .as_deref()
        .and_then(|policy_name| paths.builder_policy(policy_name))
        .map(serialize_builder_policy);
    let _env_guard = apply_builder_policy(paths, &args)?;
    tianyi::run_args(build_host_args(
        "switch",
        &args,
        builders_override.as_deref(),
    ))
}

pub fn run_host_provision(paths: &RepoPaths, args: DelegatedHostArgs) -> Result<()> {
    let args = normalize_builder_policy(args)?;
    let _env_guard = apply_builder_policy(paths, &args)?;
    if let Some(policy_name) = args.builder_policy.as_deref() {
        let policy = paths.builder_policy(policy_name).ok_or_else(|| {
            anyhow::anyhow!(
                "unknown builder policy `{policy_name}` in {}",
                paths.root().join("semble.toml").display()
            )
        })?;

        let previous_nix_builders = env::var_os("NIX_BUILDERS");
        let previous_nix_config = env::var_os("NIX_CONFIG");
        env::set_var("NIX_BUILDERS", serialize_builder_policy(policy));
        env::set_var(
            "NIX_CONFIG",
            merge_nix_config(previous_nix_config.as_deref(), "max-jobs = 0"),
        );

        struct ProvisionEnvGuard(Option<OsString>, Option<OsString>);
        impl Drop for ProvisionEnvGuard {
            fn drop(&mut self) {
                match &self.0 {
                    Some(value) => env::set_var("NIX_BUILDERS", value),
                    None => env::remove_var("NIX_BUILDERS"),
                }
                match &self.1 {
                    Some(value) => env::set_var("NIX_CONFIG", value),
                    None => env::remove_var("NIX_CONFIG"),
                }
            }
        }
        let _guard = ProvisionEnvGuard(previous_nix_builders, previous_nix_config);
        return tianyi::run_args(provision_host_args(&args));
    }
    tianyi::run_args(provision_host_args(&args))
}

#[cfg(test)]
mod tests {
    use super::{
        build_host_args, merge_nix_config, normalize_builder_policy, provision_host_args,
        serialize_builder_policy,
    };
    use crate::cli::DelegatedHostArgs;
    use crate::config::BuilderPolicyConfig;
    use std::ffi::OsString;

    fn strings(values: &[OsString]) -> Vec<String> {
        values
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn builds_host_build_args() {
        let args = DelegatedHostArgs {
            hostname: String::from("atlas"),
            builder_policy: None,
            extra_args: vec![OsString::from("--ask")],
        };

        assert_eq!(
            strings(&build_host_args("build", &args, None)),
            vec!["os", "build", ".", "-H", "atlas", "--ask"]
        );
    }

    #[test]
    fn builds_host_switch_args_without_inserting_double_dash() {
        let args = DelegatedHostArgs {
            hostname: String::from("atlas"),
            builder_policy: None,
            extra_args: vec![OsString::from("--dry-run"), OsString::from("--ask")],
        };

        assert_eq!(
            strings(&build_host_args("switch", &args, None)),
            vec!["os", "switch", ".", "-H", "atlas", "--dry-run", "--ask"]
        );
    }

    #[test]
    fn injects_builders_override_before_extra_args() {
        let args = DelegatedHostArgs {
            hostname: String::from("atlas"),
            builder_policy: Some(String::from("l380y")),
            extra_args: vec![OsString::from("--dry-run")],
        };

        assert_eq!(
            strings(&build_host_args(
                "switch",
                &args,
                Some("ssh://l380y-deploy x86_64-linux - 6 1 benchmark,big-parallel"),
            )),
            vec![
                "os",
                "switch",
                ".",
                "-H",
                "atlas",
                "--builders",
                "ssh://l380y-deploy x86_64-linux - 6 1 benchmark,big-parallel",
                "--max-jobs",
                "0",
                "--dry-run",
            ]
        );
    }

    #[test]
    fn builds_host_provision_args() {
        let args = DelegatedHostArgs {
            hostname: String::from("atlas"),
            builder_policy: None,
            extra_args: vec![
                OsString::from("--target-host"),
                OsString::from("atlas-deploy"),
                OsString::from("--debug"),
            ],
        };

        assert_eq!(
            strings(&provision_host_args(&args)),
            vec![
                "provision",
                ".",
                "-H",
                "atlas",
                "--target-host",
                "atlas-deploy",
                "--debug",
            ]
        );
    }

    #[test]
    fn serializes_builder_policy_to_nix_builders_entry() {
        let policy = BuilderPolicyConfig {
            name: "l380y".into(),
            host: "l380y-deploy".into(),
            ssh_key: None,
            system: "x86_64-linux".into(),
            max_jobs: 6,
            speed_factor: 1,
            supported_features: vec!["benchmark".into(), "big-parallel".into()],
        };

        assert_eq!(
            serialize_builder_policy(&policy),
            "ssh://l380y-deploy x86_64-linux - 6 1 benchmark,big-parallel"
        );
    }

    #[test]
    fn serializes_builder_policy_with_explicit_ssh_key() {
        let policy = BuilderPolicyConfig {
            name: "l380y".into(),
            host: "deploy@l380y.baiji-carat.ts.net".into(),
            ssh_key: Some("/Users/ren/.ssh/homelab_deploy".into()),
            system: "x86_64-linux".into(),
            max_jobs: 6,
            speed_factor: 1,
            supported_features: vec!["benchmark".into(), "big-parallel".into()],
        };

        assert_eq!(
            serialize_builder_policy(&policy),
            "ssh://deploy@l380y.baiji-carat.ts.net x86_64-linux /Users/ren/.ssh/homelab_deploy 6 1 benchmark,big-parallel"
        );
    }

    #[test]
    fn merges_nix_config_without_dropping_existing_lines() {
        let existing = OsString::from("accept-flake-config = true");
        let merged = merge_nix_config(Some(existing.as_os_str()), "max-jobs = 0");
        assert_eq!(
            merged.to_string_lossy(),
            "accept-flake-config = true\nmax-jobs = 0"
        );
    }

    #[test]
    fn normalizes_builder_policy_from_trailing_args() {
        let args = DelegatedHostArgs {
            hostname: String::from("thor"),
            builder_policy: None,
            extra_args: vec![
                OsString::from("--target-host"),
                OsString::from("thor-deploy"),
                OsString::from("--builder-policy"),
                OsString::from("l380y"),
                OsString::from("--dry"),
            ],
        };

        let normalized = normalize_builder_policy(args).unwrap();
        assert_eq!(normalized.builder_policy.as_deref(), Some("l380y"));
        assert_eq!(
            strings(&normalized.extra_args),
            vec!["--target-host", "thor-deploy", "--dry"]
        );
    }

    #[test]
    fn rejects_duplicate_builder_policy_sources() {
        let args = DelegatedHostArgs {
            hostname: String::from("thor"),
            builder_policy: Some(String::from("l380y")),
            extra_args: vec![
                OsString::from("--builder-policy"),
                OsString::from("other"),
            ],
        };

        let error = normalize_builder_policy(args).unwrap_err();
        assert!(error.to_string().contains("provided more than once"));
    }
}
