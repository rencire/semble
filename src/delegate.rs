use crate::cli::{DelegatedHostArgs, HostProvisionArgs, ProvisionArgs};
use crate::config::BuilderPolicyConfig;
use crate::microvm;
use crate::repo::RepoPaths;
use crate::repo::{load_host_provision_config, HostType};
use anyhow::Result;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn nh_subcommand(system: &str) -> &'static str {
    if system.ends_with("-darwin") {
        "darwin"
    } else {
        "os"
    }
}

pub fn build_host_args(
    action: &str,
    args: &DelegatedHostArgs,
    builders_override: Option<&str>,
    system: &str,
) -> Vec<OsString> {
    let mut delegated = vec![
        OsString::from(nh_subcommand(system)),
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

fn normalize_builder_policy(args: DelegatedHostArgs) -> Result<DelegatedHostArgs> {
    let mut builder_policy = args.builder_policy.clone();
    let mut extra_args = Vec::with_capacity(args.extra_args.len());
    let mut iter = args.extra_args.into_iter();

    while let Some(arg) = iter.next() {
        if arg == "--builder-policy" {
            let value = iter
                .next()
                .ok_or_else(|| anyhow::anyhow!("`--builder-policy` requires a policy name"))?;
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

pub(crate) fn normalize_switch_args(args: DelegatedHostArgs) -> DelegatedHostArgs {
    let has_target_host = args.extra_args.iter().any(|arg| arg == "--target-host");
    let has_elevation_strategy = args
        .extra_args
        .iter()
        .any(|arg| arg == "--elevation-strategy");

    if !has_target_host || has_elevation_strategy {
        return args;
    }

    let mut extra_args = Vec::with_capacity(args.extra_args.len() + 2);
    extra_args.push(OsString::from("--elevation-strategy"));
    extra_args.push(OsString::from("passwordless"));
    extra_args.extend(args.extra_args);

    DelegatedHostArgs {
        hostname: args.hostname,
        builder_policy: args.builder_policy,
        extra_args,
    }
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

fn validate_builder_policy(paths: &RepoPaths, args: &DelegatedHostArgs) -> Result<()> {
    let Some(policy_name) = args.builder_policy.as_deref() else {
        return Ok(());
    };

    paths.builder_policy(policy_name).ok_or_else(|| {
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

    Ok(())
}

fn load_host_system(paths: &RepoPaths, hostname: &str) -> Result<String> {
    let config = load_host_provision_config(paths, hostname)?;
    Ok(config.system)
}

fn run_nh(args: Vec<OsString>) -> Result<()> {
    let status = Command::new("nh")
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("nh exited with status {}", status)
    }
}

fn run_nh_host(paths: &RepoPaths, action: &str, args: DelegatedHostArgs) -> Result<()> {
    let system = load_host_system(paths, &args.hostname)?;
    let args = normalize_builder_policy(args)?;
    let builders_override = args
        .builder_policy
        .as_deref()
        .and_then(|name| paths.builder_policy(name))
        .map(serialize_builder_policy);
    validate_builder_policy(paths, &args)?;
    run_nh(build_host_args(action, &args, builders_override.as_deref(), &system))
}

// Restores NIX_BUILDERS and NIX_CONFIG to their previous values on drop.
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

fn setup_provision_builder_env(
    paths: &RepoPaths,
    policy_name: &str,
) -> Result<ProvisionEnvGuard> {
    let policy = paths.builder_policy(policy_name).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown builder policy `{policy_name}` in {}",
            paths.root().join("semble.toml").display()
        )
    })?;
    let prev_builders = env::var_os("NIX_BUILDERS");
    let prev_config = env::var_os("NIX_CONFIG");
    env::set_var("NIX_BUILDERS", serialize_builder_policy(policy));
    env::set_var(
        "NIX_CONFIG",
        merge_nix_config(prev_config.as_deref(), "max-jobs = 0"),
    );
    Ok(ProvisionEnvGuard(prev_builders, prev_config))
}

#[derive(Debug)]
struct PhysicalProvisionArgs {
    target_host: String,
    host_keys_dir: Option<String>,
    passthrough_args: Vec<OsString>,
}

fn parse_physical_provision_args(extra_args: Vec<OsString>) -> Result<PhysicalProvisionArgs> {
    let mut target_host = None;
    let mut host_keys_dir = None;
    let mut passthrough_args = Vec::new();
    let mut iter = extra_args.into_iter();

    while let Some(arg) = iter.next() {
        if arg == "--target-host" {
            let value = iter
                .next()
                .ok_or_else(|| anyhow::anyhow!("`--target-host` requires a value"))?;
            target_host = Some(
                value
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("`--target-host` value must be valid UTF-8"))?,
            );
        } else if arg == "--host-keys-dir" {
            let value = iter
                .next()
                .ok_or_else(|| anyhow::anyhow!("`--host-keys-dir` requires a value"))?;
            host_keys_dir = Some(
                value
                    .into_string()
                    .map_err(|_| anyhow::anyhow!("`--host-keys-dir` value must be valid UTF-8"))?,
            );
        } else {
            passthrough_args.push(arg);
        }
    }

    let target_host = target_host.ok_or_else(|| {
        anyhow::anyhow!(
            "physical host provision requires `--target-host <HOST>` in passthrough args"
        )
    })?;

    Ok(PhysicalProvisionArgs {
        target_host,
        host_keys_dir,
        passthrough_args,
    })
}

fn build_nixos_anywhere_args(
    flake_hostname: &str,
    target_host: &str,
    passthrough: &[OsString],
    extra_files_dir: Option<&str>,
) -> Vec<OsString> {
    let mut args = vec![
        OsString::from("--flake"),
        OsString::from(flake_hostname),
        OsString::from("--target-host"),
        OsString::from(target_host),
    ];
    if let Some(dir) = extra_files_dir {
        args.push(OsString::from("--extra-files"));
        args.push(OsString::from(dir));
    }
    args.extend_from_slice(passthrough);
    args
}

fn prepare_host_keys(host_keys_dir: &str) -> Result<TempDir> {
    let temp_dir = TempDir::new()?;
    let ssh_dir = temp_dir.path().join("etc/ssh");
    fs::create_dir_all(&ssh_dir)?;

    for entry in fs::read_dir(host_keys_dir)? {
        let entry = entry?;
        let src = entry.path();
        if !src.is_file() {
            continue;
        }
        let filename = src
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("invalid filename in host keys dir"))?;
        let dst = ssh_dir.join(filename);
        fs::copy(&src, &dst)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let is_public = src.extension().is_some_and(|ext| ext == "pub");
            let mode = if is_public { 0o644 } else { 0o600 };
            fs::set_permissions(&dst, fs::Permissions::from_mode(mode))?;
        }
    }

    Ok(temp_dir)
}

fn run_nixos_anywhere(args: Vec<OsString>) -> Result<()> {
    let binary =
        env::var("NIXOS_ANYWHERE_BIN").unwrap_or_else(|_| String::from("nixos-anywhere"));
    let status = Command::new(&binary)
        .args(&args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("nixos-anywhere exited with status {}", status)
    }
}

pub fn run_host_build(paths: &RepoPaths, args: DelegatedHostArgs) -> Result<()> {
    run_nh_host(paths, "build", args)
}

pub fn run_host_switch(paths: &RepoPaths, args: DelegatedHostArgs) -> Result<()> {
    run_nh_host(paths, "switch", normalize_switch_args(args))
}

pub fn run_host_provision(paths: &RepoPaths, args: HostProvisionArgs) -> Result<()> {
    let host = load_host_provision_config(paths, &args.hostname)?;
    match host.host_type {
        HostType::Physical => run_physical_host_provision(paths, args),
        HostType::Microvm => run_microvm_host_provision(paths, host, args),
    }
}

fn run_physical_host_provision(paths: &RepoPaths, args: HostProvisionArgs) -> Result<()> {
    if args.disk_encryption_keys.is_some()
        || args.host_keys_dir.is_some()
        || args.system_store_path.is_some()
        || args.no_encrypt
        || args.force_reformat
    {
        return Err(anyhow::anyhow!(
            "microvm-specific provisioning flags are only valid for hosts with `type = \"microvm\"`"
        ));
    }

    let delegated = DelegatedHostArgs {
        hostname: args.hostname.clone(),
        builder_policy: args.builder_policy,
        extra_args: args.extra_args,
    };
    let delegated = normalize_builder_policy(delegated)?;
    validate_builder_policy(paths, &delegated)?;

    let parsed = parse_physical_provision_args(delegated.extra_args)?;

    let host_keys_temp = parsed
        .host_keys_dir
        .as_deref()
        .map(prepare_host_keys)
        .transpose()?;
    let extra_files_dir = host_keys_temp.as_ref().and_then(|t| t.path().to_str());

    let flake_hostname = format!(".#{}", args.hostname);
    let na_args = build_nixos_anywhere_args(
        &flake_hostname,
        &parsed.target_host,
        &parsed.passthrough_args,
        extra_files_dir,
    );

    let _builder_env = delegated
        .builder_policy
        .as_deref()
        .map(|name| setup_provision_builder_env(paths, name))
        .transpose()?;

    run_nixos_anywhere(na_args)
}

fn run_microvm_host_provision(
    paths: &RepoPaths,
    host: crate::repo::HostProvisionConfig,
    args: HostProvisionArgs,
) -> Result<()> {
    if !args.extra_args.is_empty() {
        return Err(anyhow::anyhow!(
            "microvm host provisioning does not accept trailing passthrough arguments"
        ));
    }

    let parent = host.provision_target.ok_or_else(|| {
        anyhow::anyhow!(
            "missing required field `provisionTarget` for microvm host `{}`",
            args.hostname
        )
    })?;

    microvm::run_microvm_provision(
        paths,
        ProvisionArgs {
            guest: args.hostname,
            parent,
            builder_policy: args.builder_policy,
            disk_encryption_keys: args.disk_encryption_keys,
            host_keys_dir: args.host_keys_dir,
            system_store_path: args.system_store_path,
            no_encrypt: args.no_encrypt,
            force_reformat: args.force_reformat,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{
        build_host_args, build_nixos_anywhere_args, merge_nix_config, normalize_builder_policy,
        normalize_switch_args, parse_physical_provision_args, prepare_host_keys,
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

    // --- nh arg construction ---

    #[test]
    fn builds_host_build_args() {
        let args = DelegatedHostArgs {
            hostname: String::from("atlas"),
            builder_policy: None,
            extra_args: vec![OsString::from("--ask")],
        };

        assert_eq!(
            strings(&build_host_args("build", &args, None, "x86_64-linux")),
            vec!["os", "build", ".", "-H", "atlas", "--ask"]
        );
    }

    #[test]
    fn builds_darwin_host_args() {
        let args = DelegatedHostArgs {
            hostname: String::from("m1mbp"),
            builder_policy: None,
            extra_args: vec![OsString::from("--ask")],
        };

        assert_eq!(
            strings(&build_host_args("switch", &args, None, "aarch64-darwin")),
            vec!["darwin", "switch", ".", "-H", "m1mbp", "--ask"]
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
            strings(&build_host_args("switch", &args, None, "x86_64-linux")),
            vec!["os", "switch", ".", "-H", "atlas", "--dry-run", "--ask"]
        );
    }

    #[test]
    fn injects_passwordless_elevation_for_remote_switches() {
        let args = DelegatedHostArgs {
            hostname: String::from("atlas"),
            builder_policy: None,
            extra_args: vec![
                OsString::from("--target-host"),
                OsString::from("atlas-deploy"),
                OsString::from("--dry-run"),
            ],
        };

        let normalized = normalize_switch_args(args);

        assert_eq!(
            strings(&normalized.extra_args),
            vec![
                "--elevation-strategy",
                "passwordless",
                "--target-host",
                "atlas-deploy",
                "--dry-run",
            ]
        );
    }

    #[test]
    fn preserves_explicit_elevation_strategy_for_remote_switches() {
        let args = DelegatedHostArgs {
            hostname: String::from("atlas"),
            builder_policy: None,
            extra_args: vec![
                OsString::from("--target-host"),
                OsString::from("atlas-deploy"),
                OsString::from("--elevation-strategy"),
                OsString::from("ask"),
                OsString::from("--dry-run"),
            ],
        };

        let normalized = normalize_switch_args(args);

        assert_eq!(
            strings(&normalized.extra_args),
            vec![
                "--target-host",
                "atlas-deploy",
                "--elevation-strategy",
                "ask",
                "--dry-run",
            ]
        );
    }

    #[test]
    fn leaves_local_switches_unchanged() {
        let args = DelegatedHostArgs {
            hostname: String::from("atlas"),
            builder_policy: None,
            extra_args: vec![OsString::from("--dry-run"), OsString::from("--ask")],
        };

        let normalized = normalize_switch_args(args);

        assert_eq!(strings(&normalized.extra_args), vec!["--dry-run", "--ask"]);
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
                "x86_64-linux",
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
            extra_args: vec![OsString::from("--builder-policy"), OsString::from("other")],
        };

        let error = normalize_builder_policy(args).unwrap_err();
        assert!(error.to_string().contains("provided more than once"));
    }

    // --- parse_physical_provision_args ---

    #[test]
    fn parses_target_host_from_provision_extra_args() {
        let extra_args = vec![
            OsString::from("--target-host"),
            OsString::from("atlas-deploy"),
            OsString::from("--debug"),
        ];
        let parsed = parse_physical_provision_args(extra_args).unwrap();
        assert_eq!(parsed.target_host, "atlas-deploy");
        assert_eq!(parsed.host_keys_dir, None);
        assert_eq!(strings(&parsed.passthrough_args), vec!["--debug"]);
    }

    #[test]
    fn parses_host_keys_dir_from_provision_extra_args() {
        let extra_args = vec![
            OsString::from("--target-host"),
            OsString::from("root@example"),
            OsString::from("--host-keys-dir"),
            OsString::from("/tmp/keys"),
            OsString::from("-i"),
            OsString::from("/tmp/id"),
            OsString::from("--phases"),
            OsString::from("disko,install,reboot"),
        ];
        let parsed = parse_physical_provision_args(extra_args).unwrap();
        assert_eq!(parsed.target_host, "root@example");
        assert_eq!(parsed.host_keys_dir, Some(String::from("/tmp/keys")));
        assert_eq!(
            strings(&parsed.passthrough_args),
            vec!["-i", "/tmp/id", "--phases", "disko,install,reboot"]
        );
    }

    #[test]
    fn rejects_provision_extra_args_missing_target_host() {
        let extra_args = vec![OsString::from("--debug")];
        let err = parse_physical_provision_args(extra_args).unwrap_err();
        assert!(err.to_string().contains("--target-host"));
    }

    #[test]
    fn rejects_target_host_flag_without_value() {
        let extra_args = vec![OsString::from("--target-host")];
        let err = parse_physical_provision_args(extra_args).unwrap_err();
        assert!(err.to_string().contains("requires a value"));
    }

    #[test]
    fn rejects_host_keys_dir_flag_without_value() {
        let extra_args = vec![
            OsString::from("--target-host"),
            OsString::from("root@example"),
            OsString::from("--host-keys-dir"),
        ];
        let err = parse_physical_provision_args(extra_args).unwrap_err();
        assert!(err.to_string().contains("requires a value"));
    }

    #[test]
    fn provision_extra_args_without_host_keys_dir() {
        let extra_args = vec![
            OsString::from("--target-host"),
            OsString::from("root@example"),
            OsString::from("--debug"),
        ];
        let parsed = parse_physical_provision_args(extra_args).unwrap();
        assert_eq!(parsed.host_keys_dir, None);
        assert_eq!(strings(&parsed.passthrough_args), vec!["--debug"]);
    }

    // --- build_nixos_anywhere_args ---

    #[test]
    fn builds_minimal_nixos_anywhere_args() {
        let args = build_nixos_anywhere_args(".#atlas", "atlas-deploy", &[], None);
        assert_eq!(
            strings(&args),
            vec!["--flake", ".#atlas", "--target-host", "atlas-deploy"]
        );
    }

    #[test]
    fn builds_nixos_anywhere_args_with_extra_files() {
        let args =
            build_nixos_anywhere_args(".#atlas", "atlas-deploy", &[], Some("/tmp/extra-files"));
        assert_eq!(
            strings(&args),
            vec![
                "--flake",
                ".#atlas",
                "--target-host",
                "atlas-deploy",
                "--extra-files",
                "/tmp/extra-files",
            ]
        );
    }

    #[test]
    fn builds_nixos_anywhere_args_with_passthrough() {
        let passthrough = vec![
            OsString::from("-i"),
            OsString::from("/tmp/id"),
            OsString::from("--phases"),
            OsString::from("disko,install,reboot"),
        ];
        let args = build_nixos_anywhere_args(".#atlas", "root@example", &passthrough, None);
        assert_eq!(
            strings(&args),
            vec![
                "--flake",
                ".#atlas",
                "--target-host",
                "root@example",
                "-i",
                "/tmp/id",
                "--phases",
                "disko,install,reboot",
            ]
        );
    }

    #[test]
    fn builds_nixos_anywhere_args_extra_files_precede_passthrough() {
        let passthrough = vec![OsString::from("--debug")];
        let args = build_nixos_anywhere_args(
            ".#atlas",
            "root@example",
            &passthrough,
            Some("/tmp/extra-files"),
        );
        assert_eq!(
            strings(&args),
            vec![
                "--flake",
                ".#atlas",
                "--target-host",
                "root@example",
                "--extra-files",
                "/tmp/extra-files",
                "--debug",
            ]
        );
    }

    // --- prepare_host_keys ---

    #[test]
    fn prepare_host_keys_copies_all_files_in_directory() {
        use std::fs;
        use tempfile::TempDir;

        let src = TempDir::new().unwrap();
        fs::write(src.path().join("ssh_host_ed25519_key"), "private").unwrap();
        fs::write(src.path().join("ssh_host_ed25519_key.pub"), "public").unwrap();
        fs::write(src.path().join("ssh_host_initrd_ed25519_key"), "initrd-private").unwrap();
        fs::write(src.path().join("ssh_host_initrd_ed25519_key.pub"), "initrd-public").unwrap();

        let result = prepare_host_keys(src.path().to_str().unwrap()).unwrap();
        let ssh_dir = result.path().join("etc/ssh");

        assert!(ssh_dir.join("ssh_host_ed25519_key").exists());
        assert!(ssh_dir.join("ssh_host_ed25519_key.pub").exists());
        assert!(ssh_dir.join("ssh_host_initrd_ed25519_key").exists());
        assert!(ssh_dir.join("ssh_host_initrd_ed25519_key.pub").exists());
    }

    #[test]
    fn prepare_host_keys_fails_for_missing_source() {
        let err = prepare_host_keys("/nonexistent/path");
        assert!(err.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn prepare_host_keys_sets_correct_permissions() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let src = TempDir::new().unwrap();
        fs::write(src.path().join("ssh_host_ed25519_key"), "private").unwrap();
        fs::write(src.path().join("ssh_host_ed25519_key.pub"), "public").unwrap();
        fs::write(src.path().join("ssh_host_initrd_ed25519_key"), "initrd-private").unwrap();
        fs::write(src.path().join("ssh_host_initrd_ed25519_key.pub"), "initrd-public").unwrap();

        let result = prepare_host_keys(src.path().to_str().unwrap()).unwrap();
        let ssh_dir = result.path().join("etc/ssh");

        let cases = [
            ("ssh_host_ed25519_key", 0o600),
            ("ssh_host_ed25519_key.pub", 0o644),
            ("ssh_host_initrd_ed25519_key", 0o600),
            ("ssh_host_initrd_ed25519_key.pub", 0o644),
        ];
        for (filename, expected_mode) in cases {
            let mode = fs::metadata(ssh_dir.join(filename))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(mode, expected_mode, "{filename} had wrong permissions");
        }
    }
}
