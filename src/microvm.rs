use crate::cli::{DelegatedHostArgs, ProvisionArgs};
use crate::delegate;
use crate::error::fail;
use crate::host::validate_hostname;
use crate::repo::RepoPaths;
use anyhow::{Context, Result};
use serde::Deserialize;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct MicrovmVolumeSpec {
    image: String,
    size: u64,
    #[serde(rename = "mountPoint")]
    mount_point: Option<String>,
    #[serde(default, rename = "autoCreate")]
    auto_create: bool,
}

pub fn run_microvm_provision(paths: &RepoPaths, args: ProvisionArgs) -> Result<()> {
    validate_hostname(&args.guest)?;

    let volumes = load_microvm_volumes(paths, &args.guest)?;
    let volume = resolve_volume_spec(&args.guest, volumes)?;
    let image_path = volume.image;
    let volume_mount_point = volume.mount_point;
    let auto_create = volume.auto_create;
    let mapper_name = "cryptroot".to_string();

    if auto_create {
        println!(
            "Skipping provisioning for {}: autoCreate=true, so the microVM service will create it at boot.",
            image_path
        );
        return Ok(());
    }

    require_cmd("ssh")?;
    require_cmd("scp")?;
    require_cmd("nix")?;

    let encrypt_root = !args.no_encrypt;
    let disk_encryption_keys = args.disk_encryption_keys.map(PathBuf::from);
    if encrypt_root {
        let Some(disk_encryption_keys) = disk_encryption_keys.as_ref() else {
            return fail("--disk-encryption-keys is required unless --no-encrypt is set");
        };
        if !disk_encryption_keys.is_file() {
            return fail(format!("Key file not found: {}", disk_encryption_keys.display()));
        }
    }

    let host_keys_dir = args.host_keys_dir.map(PathBuf::from);
    if let Some(keys_dir) = host_keys_dir.as_ref() {
        validate_ssh_host_keys_dir(keys_dir)?;
    }

    if encrypt_root {
        if volume_mount_point.is_some() {
            return fail(format!(
                "This command expects mountPoint=null for encrypted provisioning so the guest root is mounted from the LUKS mapper: {}",
                volume_mount_point.as_deref().unwrap()
            ));
        }
    }

    let mount_point = format!("/mnt/{}-root", args.guest);
    let remote_key_path = format!("/tmp/{}-root.key", args.guest);
    let remote_ssh_host_key_path = format!("/tmp/{}-ssh_host_ed25519_key", args.guest);
    let remote_ssh_host_key_pub_path = format!("{remote_ssh_host_key_path}.pub");

    let mut cleanup = ProvisioningCleanup::new(
        args.parent.clone(),
        mount_point.clone(),
        mapper_name.clone(),
        encrypt_root,
        remote_key_path.clone(),
        remote_ssh_host_key_path.clone(),
        remote_ssh_host_key_pub_path.clone(),
    );

    let system_store_path = resolve_system_store_path(
        paths,
        &args.guest,
        args.builder_policy.as_deref(),
        args.system_store_path.as_deref(),
    )?;
    if !system_store_path.starts_with("/nix/store/") {
        return fail(format!("Invalid system store path: {system_store_path}"));
    }

    ensure_remote_microvm_owner(&args.parent)?;

    println!("Checking remote image state...");
    if encrypt_root
        && run_remote_status(
            &args.parent,
            &format!("test -e /dev/mapper/{}", shell_quote(&mapper_name)),
        )?
    {
        return fail(format!(
            "Refusing to proceed while mapper is already open: /dev/mapper/{}",
            mapper_name
        ));
    }

    let image_size = format!("{}M", volume.size);
    if encrypt_root {
        println!("Copying root unlock key to parent host...");
        upload_file(&disk_encryption_keys.unwrap(), &args.parent, &remote_key_path)?;
        run_remote(
            &args.parent,
            &format!("chmod 600 {}", shell_quote(&remote_key_path)),
        )?;

        println!("Formatting encrypted root image on parent host...");
        run_remote(
            &args.parent,
            &format!(
                "sudo nix shell nixpkgs#cryptsetup -c cryptsetup luksFormat --batch-mode {} {}",
                shell_quote(&image_path),
                shell_quote(&remote_key_path)
            ),
        )?;
        log_remote_parent_dir_owner(&args.parent, &image_path)?;
        ensure_remote_image_owner(&args.parent, &image_path)?;
        log_remote_image_owner(&args.parent, &image_path)?;

        println!("Opening LUKS mapping and creating filesystem...");
        run_remote(
            &args.parent,
            &format!(
                "sudo nix shell nixpkgs#cryptsetup -c cryptsetup open {} {} --key-file {}",
                shell_quote(&image_path),
                shell_quote(&mapper_name),
                shell_quote(&remote_key_path)
            ),
        )?;
        run_remote(
            &args.parent,
            &format!(
                "sudo nix shell nixpkgs#e2fsprogs -c mkfs.ext4 /dev/mapper/{}",
                shell_quote(&mapper_name)
            ),
        )?;

        println!("Mounting encrypted root...");
        run_remote(
            &args.parent,
            &format!("sudo mkdir -p {}", shell_quote(&mount_point)),
        )?;
        run_remote(
            &args.parent,
            &format!(
                "sudo mount /dev/mapper/{} {}",
                shell_quote(&mapper_name),
                shell_quote(&mount_point)
            ),
        )?;
    } else {
        println!("Creating plain ext4 root image on parent host...");
        ensure_remote_parent_dir(&args.parent, &image_path)?;
        run_remote(
            &args.parent,
            &format!(
                "sudo truncate -s {} {}",
                shell_quote(&image_size),
                shell_quote(&image_path)
            ),
        )?;
        run_remote(
            &args.parent,
            &format!(
                "sudo nix shell nixpkgs#e2fsprogs -c mkfs.ext4 -F {}",
                shell_quote(&image_path)
            ),
        )?;
        log_remote_parent_dir_owner(&args.parent, &image_path)?;
        ensure_remote_image_owner(&args.parent, &image_path)?;
        log_remote_image_owner(&args.parent, &image_path)?;
        run_remote(
            &args.parent,
            &format!("sudo mkdir -p {}", shell_quote(&mount_point)),
        )?;
        run_remote(
            &args.parent,
            &format!(
                "sudo mount -o loop {} {}",
                shell_quote(&image_path),
                shell_quote(&mount_point)
            ),
        )?;
    }

    println!("Copying built system closure to parent host...");
    run_command(
        Command::new("nix")
            .arg("copy")
            .arg("--to")
            .arg(format!("ssh-ng://{}", args.parent))
            .arg(&system_store_path),
        "failed to copy built system closure to parent host",
    )?;

    println!("Installing built system into guest root...");
    run_remote(
        &args.parent,
        &format!(
            "sudo nix shell nixpkgs#nixos-install-tools -c nixos-install --root {} --system {} --no-root-passwd --no-bootloader",
            shell_quote(&mount_point),
            shell_quote(&system_store_path)
        ),
    )?;

    if let Some(keys_dir) = host_keys_dir.as_ref() {
        println!("Copying SSH host keys into guest root...");
        upload_file(
            &keys_dir.join("ssh_host_ed25519_key"),
            &args.parent,
            &remote_ssh_host_key_path,
        )?;
        upload_file(
            &keys_dir.join("ssh_host_ed25519_key.pub"),
            &args.parent,
            &remote_ssh_host_key_pub_path,
        )?;
        run_remote(
            &args.parent,
            &format!(
                "sudo install -d -m 0755 {}/etc/ssh",
                shell_quote(&mount_point)
            ),
        )?;
        run_remote(
            &args.parent,
            &format!(
                "sudo install -m 0600 {} {}/etc/ssh/ssh_host_ed25519_key",
                shell_quote(&remote_ssh_host_key_path),
                shell_quote(&mount_point)
            ),
        )?;
        run_remote(
            &args.parent,
            &format!(
                "sudo install -m 0644 {} {}/etc/ssh/ssh_host_ed25519_key.pub",
                shell_quote(&remote_ssh_host_key_pub_path),
                shell_quote(&mount_point)
            ),
        )?;
    }

    println!("Validating installed system profile...");
    run_remote(
        &args.parent,
        &format!(
            "sudo test -L {}/nix/var/nix/profiles/system",
            shell_quote(&mount_point)
        ),
    )?;
    run_remote(
        &args.parent,
        &format!("sudo test -e {}/etc/NIXOS", shell_quote(&mount_point)),
    )?;
    if host_keys_dir.is_some() {
        run_remote(
            &args.parent,
            &format!(
                "sudo test -e {}/etc/ssh/ssh_host_ed25519_key",
                shell_quote(&mount_point)
            ),
        )?;
        run_remote(
            &args.parent,
            &format!(
                "sudo test -e {}/etc/ssh/ssh_host_ed25519_key.pub",
                shell_quote(&mount_point)
            ),
        )?;
    }

    cleanup.cleanup();
    println!("Provisioning complete for {}", args.guest);
    Ok(())
}

fn resolve_system_store_path(
    paths: &RepoPaths,
    guest: &str,
    builder_policy: Option<&str>,
    provided: Option<&str>,
) -> Result<String> {
    if let Some(system_store_path) = provided {
        return Ok(system_store_path.trim().to_string());
    }

    build_guest_system(paths, guest, builder_policy)?;
    let output = run_command_capture(
        Command::new("nix")
            .arg("path-info")
            .arg(format!(
                ".#nixosConfigurations.{guest}.config.system.build.toplevel"
            ))
            .current_dir(paths.root()),
        "failed to resolve built guest system store path",
    )?;
    Ok(output.trim().to_string())
}

fn build_guest_system(paths: &RepoPaths, guest: &str, builder_policy: Option<&str>) -> Result<()> {
    let args = DelegatedHostArgs {
        hostname: guest.to_string(),
        builder_policy: builder_policy.map(ToOwned::to_owned),
        extra_args: vec![
            OsString::from("--no-nom"),
            OsString::from("--accept-flake-config"),
        ],
    };
    delegate::run_host_build(paths, args)
}

fn load_microvm_volumes(paths: &RepoPaths, guest: &str) -> Result<Vec<MicrovmVolumeSpec>> {
    load_microvm_volumes_with(paths, guest, run_nix_eval_microvm_volumes)
}

fn load_microvm_volumes_with(
    paths: &RepoPaths,
    guest: &str,
    eval: impl FnOnce(&RepoPaths, &str) -> Result<Option<Vec<u8>>>,
) -> Result<Vec<MicrovmVolumeSpec>> {
    let output = eval(paths, guest)?
        .ok_or_else(|| anyhow::anyhow!("failed to evaluate microVM volume metadata"))?;
    let volumes: Vec<MicrovmVolumeSpec> = serde_json::from_slice(&output)
        .context("failed to parse microVM volume metadata from `nix eval`")?;
    Ok(volumes)
}

fn run_nix_eval_microvm_volumes(paths: &RepoPaths, guest: &str) -> Result<Option<Vec<u8>>> {
    let output = run_command_capture_bytes(
        Command::new("nix")
            .args([
                "eval",
                "--extra-experimental-features",
                "nix-command flakes",
                "--impure",
                "--json",
            ])
            .arg("--expr")
            .arg(format!(
                "let flake = builtins.getFlake (toString {}); in flake.nixosConfigurations.\"{}\".config.microvm.volumes",
                paths.root().canonicalize()?.display(),
                guest
            ))
            .current_dir(paths.root()),
        "failed to evaluate microVM volume metadata",
    )?;

    Ok(Some(output))
}

fn resolve_volume_spec(guest: &str, volumes: Vec<MicrovmVolumeSpec>) -> Result<MicrovmVolumeSpec> {
    if volumes.is_empty() {
        return fail(format!("No microvm.volumes entries found for {guest}"));
    }

    if volumes.len() > 1 {
        return fail(format!(
            "This command currently supports exactly one microvm.volumes entry for {guest}; found {}.",
            volumes.len()
        ));
    }

    Ok(volumes
        .into_iter()
        .next()
        .expect("volume count checked above"))
}

fn validate_ssh_host_keys_dir(keys_dir: &Path) -> Result<()> {
    if !keys_dir.is_dir() {
        return fail(format!(
            "SSH host key directory not found: {}",
            keys_dir.display()
        ));
    }

    let private_key = keys_dir.join("ssh_host_ed25519_key");
    let public_key = keys_dir.join("ssh_host_ed25519_key.pub");
    if !private_key.is_file() {
        return fail(format!(
            "SSH host private key not found: {}",
            private_key.display()
        ));
    }
    if !public_key.is_file() {
        return fail(format!(
            "SSH host public key not found: {}",
            public_key.display()
        ));
    }

    Ok(())
}

fn ensure_remote_parent_dir(parent: &str, image_path: &str) -> Result<()> {
    if let Some(parent_dir) = Path::new(image_path).parent() {
        run_remote(
            parent,
            &format!(
                "sudo mkdir -p {}",
                shell_quote(parent_dir.to_string_lossy().as_ref())
            ),
        )?;
        run_remote(
            parent,
            &format!(
                "sudo chown microvm:kvm {} && sudo chmod 2775 {}",
                shell_quote(parent_dir.to_string_lossy().as_ref()),
                shell_quote(parent_dir.to_string_lossy().as_ref())
            ),
        )?;
    }
    Ok(())
}

fn ensure_remote_microvm_owner(parent: &str) -> Result<()> {
    let status = run_remote_status(
        parent,
        "getent passwd microvm >/dev/null 2>&1 && getent group kvm >/dev/null 2>&1",
    )?;
    if status {
        return Ok(());
    }

    fail(format!(
        "parent host {parent} must provide the `microvm` user and `kvm` group so Semble can hand off the guest image with microvm:kvm ownership"
    ))
}

fn ensure_remote_image_owner(parent: &str, image_path: &str) -> Result<()> {
    run_remote(
        parent,
        &format!(
            "sudo chown microvm:kvm {} && sudo chmod 0660 {}",
            shell_quote(image_path),
            shell_quote(image_path)
        ),
    )
}

fn log_remote_image_owner(parent: &str, image_path: &str) -> Result<()> {
    let output = run_command_capture(
        Command::new("ssh")
            .arg(parent)
            .arg(format!("stat -c '%U:%G %a %n' {}", shell_quote(image_path))),
        "failed to inspect remote image ownership",
    )?;
    println!("Image ownership on parent host: {}", output.trim());
    Ok(())
}

fn log_remote_parent_dir_owner(parent: &str, image_path: &str) -> Result<()> {
    if let Some(parent_dir) = Path::new(image_path).parent() {
        let output = run_command_capture(
            Command::new("ssh").arg(parent).arg(format!(
                "stat -c '%U:%G %a %n' {}",
                shell_quote(parent_dir.to_string_lossy().as_ref())
            )),
            "failed to inspect remote parent directory ownership",
        )?;
        println!("Parent directory ownership on host: {}", output.trim());
    }
    Ok(())
}

fn upload_file(source: &Path, target_host: &str, remote_path: &str) -> Result<()> {
    if !source.is_file() {
        return fail(format!("missing file: {}", source.display()));
    }

    let status = Command::new("scp")
        .arg(source)
        .arg(format!("{target_host}:{remote_path}"))
        .status()
        .with_context(|| format!("failed to upload {}", source.display()))?;

    if !status.success() {
        return fail(format!(
            "upload of {} to {target_host}:{remote_path} exited with status {status}",
            source.display()
        ));
    }

    Ok(())
}

fn require_cmd(cmd: &str) -> Result<()> {
    if Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {cmd} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
    {
        return Ok(());
    }

    fail(format!("Missing required command: {cmd}"))
}

fn run_remote(target_host: &str, remote_command: &str) -> Result<()> {
    if run_remote_status(target_host, remote_command)? {
        Ok(())
    } else {
        fail(format!(
            "ssh command on {target_host} exited nonzero: {remote_command}"
        ))
    }
}

fn run_remote_status(target_host: &str, remote_command: &str) -> Result<bool> {
    let status = Command::new("ssh")
        .arg(target_host)
        .arg(remote_command)
        .status()
        .with_context(|| format!("failed to run ssh for {target_host}"))?;

    Ok(status.success())
}

fn run_command(command: &mut Command, context: &str) -> Result<()> {
    let status = command.status().with_context(|| context.to_string())?;
    if status.success() {
        Ok(())
    } else {
        fail(context)
    }
}

fn run_command_capture(command: &mut Command, context: &str) -> Result<String> {
    let output = command.output().with_context(|| context.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        fail(format!("{context}: {}", stderr.trim()))
    }
}

fn run_command_capture_bytes(command: &mut Command, context: &str) -> Result<Vec<u8>> {
    let output = command.output().with_context(|| context.to_string())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        fail(format!("{context}: {}", stderr.trim()))
    }
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

struct ProvisioningCleanup {
    target_host: String,
    mount_point: String,
    mapper_name: String,
    encrypt_root: bool,
    remote_key_path: String,
    remote_ssh_host_key_path: String,
    remote_ssh_host_key_pub_path: String,
    active: bool,
}

impl ProvisioningCleanup {
    fn new(
        target_host: String,
        mount_point: String,
        mapper_name: String,
        encrypt_root: bool,
        remote_key_path: String,
        remote_ssh_host_key_path: String,
        remote_ssh_host_key_pub_path: String,
    ) -> Self {
        Self {
            target_host,
            mount_point,
            mapper_name,
            encrypt_root,
            remote_key_path,
            remote_ssh_host_key_path,
            remote_ssh_host_key_pub_path,
            active: true,
        }
    }

    fn cleanup(&mut self) {
        let _ = run_remote(
            &self.target_host,
            &format!(
                "sudo umount {} >/dev/null 2>&1 || true",
                shell_quote(&self.mount_point)
            ),
        );
        if self.encrypt_root {
            let _ = run_remote(
                &self.target_host,
                &format!(
                    "sudo nix shell nixpkgs#cryptsetup -c cryptsetup close {} >/dev/null 2>&1 || true",
                    shell_quote(&self.mapper_name)
                ),
            );
        }
        let _ = run_remote(
            &self.target_host,
            &format!(
                "rm -f {} >/dev/null 2>&1 || true",
                shell_quote(&self.remote_key_path)
            ),
        );
        let _ = run_remote(
            &self.target_host,
            &format!(
                "rm -f {} {} >/dev/null 2>&1 || true",
                shell_quote(&self.remote_ssh_host_key_path),
                shell_quote(&self.remote_ssh_host_key_pub_path)
            ),
        );

        self.active = false;
    }
}

impl Drop for ProvisioningCleanup {
    fn drop(&mut self) {
        if self.active {
            self.cleanup();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{load_microvm_volumes_with, resolve_volume_spec, shell_quote, MicrovmVolumeSpec};
    use crate::repo::RepoPaths;
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_semble_toml(root: &Path) {
        fs::write(
            root.join("semble.toml"),
            r#"
[paths]
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
    }

    #[test]
    fn quotes_shell_values() {
        assert_eq!(shell_quote("/run/example"), "'/run/example'");
        assert_eq!(shell_quote("value'with quote"), "'value'\\''with quote'");
    }

    #[test]
    fn resolves_single_volume_spec() {
        let spec = resolve_volume_spec(
            "claw",
            vec![MicrovmVolumeSpec {
                image: "/var/lib/microvms/claw/root.img".into(),
                size: 4096,
                mount_point: None,
                auto_create: false,
            }],
        )
        .unwrap();

        assert_eq!(spec.image, "/var/lib/microvms/claw/root.img");
        assert_eq!(spec.size, 4096);
    }

    #[test]
    fn rejects_multiple_volume_specs() {
        let error = resolve_volume_spec(
            "claw",
            vec![
                MicrovmVolumeSpec {
                    image: "/var/lib/microvms/claw/root.img".into(),
                    size: 4096,
                    mount_point: None,
                    auto_create: false,
                },
                MicrovmVolumeSpec {
                    image: "/var/lib/microvms/claw/data.img".into(),
                    size: 1024,
                    mount_point: None,
                    auto_create: false,
                },
            ],
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("supports exactly one microvm.volumes entry"));
    }

    #[test]
    fn loads_microvm_volumes_from_eval_output() {
        let tempdir = tempdir().unwrap();
        write_semble_toml(tempdir.path());
        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let volumes = load_microvm_volumes_with(&paths, "claw", |_paths, _guest| {
            Ok(Some(
                br#"[{"image":"/var/lib/microvms/claw/root.img","size":4096,"mountPoint":null,"autoCreate":false}]"#
                    .to_vec(),
            ))
        })
        .unwrap();

        assert_eq!(volumes.len(), 1);
        assert_eq!(volumes[0].image, "/var/lib/microvms/claw/root.img");
        assert_eq!(volumes[0].size, 4096);
    }
}
