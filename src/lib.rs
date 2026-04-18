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
pub mod template;

use anyhow::Result;
use cli::{Cli, Command, HostCommand, ImageCommand, MicrovmCommand};

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
        },
        Command::Image(image) => match image.command {
            ImageCommand::Prepare(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                image::run_image_prepare(&paths, args)
            }
        },
        Command::Microvm(microvm) => match microvm.command {
            MicrovmCommand::ProvisionIdentity(args) => {
                let paths = repo::RepoPaths::new(std::env::current_dir()?)?;
                microvm::run_microvm_provision_identity(&paths, args)
            }
        },
    }
}
