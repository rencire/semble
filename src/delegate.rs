use crate::cli::DelegatedHostArgs;
use anyhow::Result;
use std::ffi::OsString;

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

pub fn run_host_build(args: DelegatedHostArgs) -> Result<()> {
    tianyi::run_args(build_host_args("build", &args))
}

pub fn run_host_switch(args: DelegatedHostArgs) -> Result<()> {
    tianyi::run_args(build_host_args("switch", &args))
}

pub fn run_host_provision(args: DelegatedHostArgs) -> Result<()> {
    tianyi::run_args(provision_host_args(&args))
}

#[cfg(test)]
mod tests {
    use super::{build_host_args, provision_host_args};
    use crate::cli::DelegatedHostArgs;
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
}
