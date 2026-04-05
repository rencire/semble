use crate::cli::DelegatedHostArgs;
use crate::config::BuilderPolicyConfig;
use crate::repo::RepoPaths;
use anyhow::Result;
use std::env;
use std::ffi::{OsStr, OsString};

struct EnvGuard {
    previous_nix_builders: Option<OsString>,
    previous_nix_config: Option<OsString>,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous_nix_builders {
            Some(value) => env::set_var("NIX_BUILDERS", value),
            None => env::remove_var("NIX_BUILDERS"),
        }

        match &self.previous_nix_config {
            Some(value) => env::set_var("NIX_CONFIG", value),
            None => env::remove_var("NIX_CONFIG"),
        }
    }
}

pub fn build_host_args(action: &str, args: &DelegatedHostArgs) -> Vec<OsString> {
    let mut delegated = vec![
        OsString::from("os"),
        OsString::from(action),
        OsString::from("."),
        OsString::from("-H"),
        OsString::from(&args.hostname),
    ];
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

fn serialize_builder_policy(policy: &BuilderPolicyConfig) -> String {
    let features = if policy.supported_features.is_empty() {
        String::from("-")
    } else {
        policy.supported_features.join(",")
    };

    format!(
        "ssh://{} {} - {} {} {}",
        policy.host, policy.system, policy.max_jobs, policy.speed_factor, features
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

fn apply_builder_policy(paths: &RepoPaths, args: &DelegatedHostArgs) -> Result<Option<EnvGuard>> {
    let Some(policy_name) = args.builder_policy.as_deref() else {
        return Ok(None);
    };

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

    Ok(Some(EnvGuard {
        previous_nix_builders,
        previous_nix_config,
    }))
}

pub fn run_host_build(paths: &RepoPaths, args: DelegatedHostArgs) -> Result<()> {
    let _env_guard = apply_builder_policy(paths, &args)?;
    tianyi::run_args(build_host_args("build", &args))
}

pub fn run_host_switch(paths: &RepoPaths, args: DelegatedHostArgs) -> Result<()> {
    let _env_guard = apply_builder_policy(paths, &args)?;
    tianyi::run_args(build_host_args("switch", &args))
}

pub fn run_host_provision(paths: &RepoPaths, args: DelegatedHostArgs) -> Result<()> {
    let _env_guard = apply_builder_policy(paths, &args)?;
    tianyi::run_args(provision_host_args(&args))
}

#[cfg(test)]
mod tests {
    use super::{build_host_args, merge_nix_config, provision_host_args, serialize_builder_policy};
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
            strings(&build_host_args("build", &args)),
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
            strings(&build_host_args("switch", &args)),
            vec!["os", "switch", ".", "-H", "atlas", "--dry-run", "--ask"]
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
    fn merges_nix_config_without_dropping_existing_lines() {
        let existing = OsString::from("accept-flake-config = true");
        let merged = merge_nix_config(Some(existing.as_os_str()), "max-jobs = 0");
        assert_eq!(
            merged.to_string_lossy(),
            "accept-flake-config = true\nmax-jobs = 0"
        );
    }
}
