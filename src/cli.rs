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
    Build(DelegatedHostArgs),
    Switch(DelegatedHostArgs),
    Provision(HostProvisionArgs),
}

#[derive(Debug, Args)]
pub struct NamedHostArgs {
    pub hostname: String,
    #[arg(long)]
    pub template: Option<String>,
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
    Ssh(SshKeysArgs),
    InitrdSsh(KeyTypeArgs),
    Luks(KeyTypeArgs),
}

#[derive(Debug, Args)]
pub struct SshKeysArgs {
    #[command(subcommand)]
    pub command: SshKeyActionCommand,
}

#[derive(Debug, Subcommand)]
pub enum SshKeyActionCommand {
    Add(NamedHostArgs),
    Delete(DeleteHostArgs),
}

#[derive(Debug, Args)]
pub struct KeyTypeArgs {
    #[command(subcommand)]
    pub command: KeyActionCommand,
}

#[derive(Debug, Subcommand)]
pub enum KeyActionCommand {
    Add(TypedKeyAddArgs),
    Delete(TypedKeyDeleteArgs),
}

#[derive(Debug, Args)]
pub struct TypedKeyAddArgs {
    pub hostname: String,
    #[arg(long)]
    pub force: bool,
}

#[derive(Debug, Args)]
pub struct TypedKeyDeleteArgs {
    pub hostname: String,
    #[arg(long, short = 'y')]
    pub yes: bool,
}

#[derive(Debug, Args)]
pub struct DelegatedHostArgs {
    pub hostname: String,
    #[arg(long)]
    pub builder_policy: Option<String>,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra_args: Vec<OsString>,
}

#[derive(Debug, Args)]
#[command(after_help = "Examples:\n  semble host provision my-vm --key-file ./secrets/my-vm-root.key\n  semble host provision thor --target-host genesis-nixos -- --disk-encryption-keys ./secrets/thor/luks-root.key /tmp/luks-root.key")]
pub struct HostProvisionArgs {
    pub hostname: String,
    /// Optional builder policy used for the build/install invocation.
    #[arg(long)]
    pub builder_policy: Option<String>,
    /// Physical-host passthrough args forwarded to `tianyi provision` and then `nixos-anywhere`.
    /// Use these for `nixos-anywhere` flags such as `--disk-encryption-keys`.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub extra_args: Vec<OsString>,
    /// MicroVM-only: root unlock key staged for encrypted guest provisioning.
    /// This is separate from `nixos-anywhere --disk-encryption-keys`.
    #[arg(long)]
    pub key_file: Option<String>,
    /// MicroVM-only: copy SSH host keys into the guest root.
    #[arg(long)]
    pub install_ssh_host_keys: Option<String>,
    /// MicroVM-only: use an existing Nix store path instead of building.
    #[arg(long)]
    pub system_store_path: Option<String>,
    /// MicroVM-only: create a plain ext4 root image instead of LUKS.
    #[arg(long)]
    pub no_encrypt: bool,
    /// MicroVM-only: overwrite an existing guest image.
    #[arg(long)]
    pub force_reformat: bool,
}

#[derive(Debug, Clone)]
pub struct ProvisionArgs {
    pub guest: String,
    pub parent: String,
    pub builder_policy: Option<String>,
    pub key_file: Option<String>,
    pub install_ssh_host_keys: Option<String>,
    pub system_store_path: Option<String>,
    pub no_encrypt: bool,
    pub force_reformat: bool,
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
            vec!["semble", "host", "create", "atlas"],
            vec!["semble", "host", "create", "atlas", "--template", "microvm"],
            vec!["semble", "host", "delete", "atlas", "--yes"],
            vec!["semble", "host", "keys", "ssh", "add", "atlas", "--force"],
            vec!["semble", "host", "keys", "ssh", "delete", "atlas", "--yes"],
            vec![
                "semble",
                "host",
                "keys",
                "initrd-ssh",
                "add",
                "atlas",
                "--force",
            ],
            vec![
                "semble",
                "host",
                "keys",
                "initrd-ssh",
                "delete",
                "atlas",
                "--yes",
            ],
            vec!["semble", "host", "keys", "luks", "add", "atlas", "--force"],
            vec!["semble", "host", "keys", "luks", "delete", "atlas", "--yes"],
        ];

        for args in cases {
            let result = Cli::try_parse_from(args);
            assert!(result.is_ok(), "failed to parse host command");
        }
    }

    #[test]
    fn parses_delegated_host_commands() {
        let cases = [
            vec!["semble", "host", "build", "atlas", "--ask"],
            vec!["semble", "host", "switch", "atlas", "--dry-run"],
            vec![
                "semble",
                "host",
                "switch",
                "atlas",
                "--builder-policy",
                "l380y",
                "--dry-run",
            ],
            vec![
                "semble",
                "host",
                "provision",
                "atlas",
                "--builder-policy",
                "l380y",
                "--key-file",
                "secrets/disk_keys/atlas/luks-root.key",
                "--install-ssh-host-keys",
                "ssh_host_keys/atlas",
                "--system-store-path",
                "/nix/store/test-system",
                "--no-encrypt",
                "--force-reformat",
                "--",
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
