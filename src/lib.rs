pub mod cli;
pub mod config;
pub mod confirm;
pub mod delegate;
pub mod error;
pub mod host;
pub mod keys;
pub mod repo;
pub mod sops;
pub mod ssh_config;
pub mod template;

use anyhow::Result;
use cli::{Cli, Command, HostCommand};

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Host(host) => match host.command {
            HostCommand::Build(args) => delegate::run_host_build(args),
            HostCommand::Switch(args) => delegate::run_host_switch(args),
            HostCommand::Provision(args) => delegate::run_host_provision(args),
            HostCommand::Create(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                host::validate_hostname(&args.hostname)?;
                host::run_host_create(
                    &paths,
                    &args.hostname,
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
            HostCommand::Keys(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                match args.command {
                    cli::KeysCommand::Add(args) => {
                        host::validate_hostname(&args.hostname)?;
                        host::run_host_keys_add(
                            &paths,
                            &args.hostname,
                            args.force,
                            args.skip_reencrypt,
                            args.sops_key_file.as_deref(),
                        )
                    }
                    cli::KeysCommand::Delete(args) => {
                        host::validate_hostname(&args.hostname)?;
                        host::run_host_keys_delete(
                            &paths,
                            &args.hostname,
                            args.yes,
                            args.skip_reencrypt,
                            args.sops_key_file.as_deref(),
                        )
                    }
                }
            }
            HostCommand::Ssh(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                match args.command {
                    cli::SshCommand::Add(args) => {
                        host::validate_hostname(&args.hostname)?;
                        host::run_host_ssh_add(&paths, &args.hostname)
                    }
                    cli::SshCommand::Delete(args) => {
                        host::validate_hostname(&args.hostname)?;
                        host::run_host_ssh_delete(&paths, &args.hostname)
                    }
                }
            }
        },
    }
}
