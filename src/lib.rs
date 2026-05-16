pub mod cli;
pub mod config;
pub mod confirm;
pub mod delegate;
pub mod error;
pub mod host;
pub mod image;
pub mod keys;
pub mod microvm;
pub mod repo;
pub mod sops;
pub mod ssh;
pub mod template;

use anyhow::Result;
use cli::{
    Cli, Command, HostCommand, ImageCommand, KeyActionCommand, KeysCommand, SshKeyActionCommand,
    UnlockRootArgs, SshCommand,
};

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Host(host) => match host.command {
            HostCommand::Build(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                delegate::run_host_build(&paths, args)
            }
            HostCommand::Switch(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                delegate::run_host_switch(&paths, args)
            }
            HostCommand::Provision(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                delegate::run_host_provision(&paths, args)
            }
            HostCommand::Create(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                host::validate_hostname(&args.hostname)?;
                host::run_host_create(
                    &paths,
                    &args.hostname,
                    args.template.as_deref(),
                    args.force,
                    args.skip_reencrypt,
                    args.sops_key_file.as_deref(),
                )
            }
            HostCommand::Delete(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                host::validate_hostname(&args.hostname)?;
                host::run_host_delete(
                    &paths,
                    &args.hostname,
                    args.yes,
                    args.skip_reencrypt,
                    args.sops_key_file.as_deref(),
                )
            }
            HostCommand::UnlockRoot(UnlockRootArgs { hostname, jump, identity }) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                host::validate_hostname(&hostname)?;
                host::run_host_unlock_root(&paths, &hostname, jump.as_deref(), identity.as_deref())
            }
            HostCommand::Keys(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                match args.command {
                    KeysCommand::Ssh(args) => run_ssh_keys(&paths, args.command),
                    KeysCommand::InitrdSsh(args) => {
                        run_typed_keys(&paths, host::KeyKind::InitrdSsh, args.command)
                    }
                    KeysCommand::Luks(args) => {
                        run_typed_keys(&paths, host::KeyKind::Luks, args.command)
                    }
                }
            }
            HostCommand::Ssh(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                match args.command {
                    SshCommand::Generate => ssh::run_host_ssh_generate(&paths),
                }
            }
        },
        Command::Image(image) => match image.command {
            ImageCommand::Prepare(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                image::run_image_prepare(&paths, args)
            }
        },
    }
}

fn run_ssh_keys(paths: &repo::RepoPaths, command: SshKeyActionCommand) -> Result<()> {
    match command {
        SshKeyActionCommand::Add(args) => run_validated_key_action(args.hostname, |hostname| {
            host::run_host_keys_add(
                paths,
                hostname,
                args.force,
                args.skip_reencrypt,
                args.sops_key_file.as_deref(),
            )
        }),
        SshKeyActionCommand::Delete(args) => run_validated_key_action(args.hostname, |hostname| {
            host::run_host_keys_delete(
                paths,
                hostname,
                args.yes,
                args.skip_reencrypt,
                args.sops_key_file.as_deref(),
            )
        }),
    }
}

fn run_typed_keys(
    paths: &repo::RepoPaths,
    kind: host::KeyKind,
    command: KeyActionCommand,
) -> Result<()> {
    match command {
        KeyActionCommand::Add(args) => run_validated_key_action(args.hostname, |hostname| {
            host::run_host_key_add(paths, hostname, kind, args.force)
        }),
        KeyActionCommand::Delete(args) => run_validated_key_action(args.hostname, |hostname| {
            host::run_host_key_delete(paths, hostname, kind, args.yes)
        }),
    }
}

fn run_validated_key_action<F>(hostname: String, action: F) -> Result<()>
where
    F: FnOnce(&str) -> Result<()>,
{
    host::validate_hostname(&hostname)?;
    action(&hostname)
}
