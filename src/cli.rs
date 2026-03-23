use clap::{Args, Parser, Subcommand};
use std::ffi::OsString;

#[derive(Debug, Parser)]
#[command(version, about = "Semble repo-aware host management CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Host(HostArgs),
    Image(ImageArgs),
}

#[derive(Debug, Args)]
pub struct HostArgs {
    #[command(subcommand)]
    pub command: HostCommand,
}

#[derive(Debug, Subcommand)]
pub enum HostCommand {
    Create(NamedHostArgs),
    Delete(DeleteHostArgs),
    Keys(KeysArgs),
    Ssh(SshArgs),
    Build(DelegatedHostArgs),
    Switch(DelegatedHostArgs),
    Provision(DelegatedHostArgs),
}

#[derive(Debug, Args)]
pub struct NamedHostArgs {
    pub hostname: String,
    #[arg(long)]
    pub force: bool,
    #[arg(long)]
    pub skip_reencrypt: bool,
    #[arg(long)]
    pub sops_key_file: Option<String>,
}

#[derive(Debug, Args)]
pub struct DeleteHostArgs {
    pub hostname: String,
    #[arg(long, short = 'y')]
    pub yes: bool,
    #[arg(long)]
    pub skip_reencrypt: bool,
    #[arg(long)]
    pub sops_key_file: Option<String>,
}

#[derive(Debug, Args)]
pub struct KeysArgs {
    #[command(subcommand)]
    pub command: KeysCommand,
}

#[derive(Debug, Subcommand)]
pub enum KeysCommand {
    Add(NamedHostArgs),
    Delete(DeleteHostArgs),
}

#[derive(Debug, Args)]
pub struct SshArgs {
    #[command(subcommand)]
    pub command: SshCommand,
}

#[derive(Debug, Subcommand)]
pub enum SshCommand {
    Add(HostnameArgs),
    Delete(HostnameArgs),
}

#[derive(Debug, Args)]
pub struct HostnameArgs {
    pub hostname: String,
}

#[derive(Debug, Args)]
pub struct DelegatedHostArgs {
    pub hostname: String,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra_args: Vec<OsString>,
}

#[derive(Debug, Args)]
pub struct ImageArgs {
    #[command(subcommand)]
    pub command: ImageCommand,
}

#[derive(Debug, Subcommand)]
pub enum ImageCommand {
    Prepare(PrepareImageArgs),
}

#[derive(Debug, Args)]
pub struct PrepareImageArgs {
    pub image_name: String,
    #[arg(long)]
    pub keys_dir: Option<String>,
    #[arg(long)]
    pub output: Option<String>,
    #[arg(long)]
    pub device: Option<String>,
    #[arg(long)]
    pub skip_inject: bool,
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::{CommandFactory, Parser};

    #[test]
    fn clap_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_local_host_commands() {
        let cases = [
            vec!["semble", "host", "create", "thor"],
            vec!["semble", "host", "delete", "thor", "--yes"],
            vec!["semble", "host", "keys", "add", "thor", "--force"],
            vec!["semble", "host", "keys", "delete", "thor", "--yes"],
            vec!["semble", "host", "ssh", "add", "thor"],
            vec!["semble", "host", "ssh", "delete", "thor"],
        ];

        for args in cases {
            let result = Cli::try_parse_from(args);
            assert!(result.is_ok(), "failed to parse host command");
        }
    }

    #[test]
    fn parses_delegated_host_commands() {
        let cases = [
            vec!["semble", "host", "build", "thor", "--ask"],
            vec!["semble", "host", "switch", "thor", "--dry-run"],
            vec![
                "semble",
                "host",
                "provision",
                "thor",
                "--debug",
                "--phases",
                "disko,install",
            ],
        ];

        for args in cases {
            let result = Cli::try_parse_from(args);
            assert!(result.is_ok(), "failed to parse delegated host command");
        }
    }

    #[test]
    fn parses_image_prepare_commands() {
        let cases = [
            vec!["semble", "image", "prepare", "vishnu"],
            vec!["semble", "image", "prepare", "vishnu", "--skip-inject"],
            vec![
                "semble",
                "image",
                "prepare",
                "vishnu",
                "--keys-dir",
                "./ssh_host_keys/vishnu",
                "--output",
                "./out/vishnu.img",
                "--device",
                "/dev/mmcblk0",
            ],
        ];

        for args in cases {
            let result = Cli::try_parse_from(args);
            assert!(result.is_ok(), "failed to parse image prepare command");
        }
    }
}
