use crate::cli::PrepareImageArgs;
use crate::repo::{load_image_prepare_config, RepoPaths};
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run_image_prepare(paths: &RepoPaths, args: PrepareImageArgs) -> Result<()> {
    let config = resolve_prepare_config(paths, args)?;
    validate_prepare_inputs(&config)?;
    build_image(paths.root(), &config.build_attr)?;

    let artifact = find_built_artifact(&paths.root().join("result"))?;
    let tmp_dir = make_temp_dir()?;
    let raw_img = tmp_dir.join(&config.raw_img_name);

    normalize_artifact_to_raw(&artifact, &raw_img)?;

    if let Some(keys_dir) = &config.keys_dir {
        inject_host_keys(
            &raw_img,
            keys_dir,
            &config.partition_label,
            &config.output_path,
        )?;
    } else {
        write_output_image(&raw_img, &config.output_path)?;
        println!(
            "Skipping SSH key injection. Image written to: {}",
            config.output_path.display()
        );
    }

    if let Some(device) = &config.device {
        flash_image(&config.output_path, device)?;
    }

    fs::remove_dir_all(&tmp_dir).ok();
    Ok(())
}

#[derive(Debug)]
struct PreparedImageConfig {
    build_attr: String,
    partition_label: String,
    keys_dir: Option<PathBuf>,
    output_path: PathBuf,
    device: Option<PathBuf>,
    raw_img_name: String,
}

fn resolve_prepare_config(
    paths: &RepoPaths,
    args: PrepareImageArgs,
) -> Result<PreparedImageConfig> {
    resolve_prepare_config_with(paths, args, |paths, image_name| {
        load_image_prepare_config(paths, image_name)
    })
}

fn resolve_prepare_config_with(
    paths: &RepoPaths,
    args: PrepareImageArgs,
    load_prepare_config: impl FnOnce(&RepoPaths, &str) -> Result<crate::repo::ImagePrepareConfig>,
) -> Result<PreparedImageConfig> {
    let image_name = args.image_name;
    let build_attr = format!("images.{image_name}");
    let output_path = args
        .output
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.root().join("out").join(format!("{image_name}.img")));
    let keys_dir = if args.skip_inject {
        None
    } else {
        Some(
            args.keys_dir
                .map(PathBuf::from)
                .unwrap_or_else(|| paths.host_keys_dir(&image_name)),
        )
    };
    let partition_label = if args.skip_inject {
        String::new()
    } else {
        load_prepare_config(paths, &image_name)?.partition_label
    };

    Ok(PreparedImageConfig {
        raw_img_name: format!("{image_name}.img"),
        build_attr,
        partition_label,
        keys_dir,
        output_path,
        device: args.device.map(PathBuf::from),
    })
}

fn validate_prepare_inputs(config: &PreparedImageConfig) -> Result<()> {
    if let Some(keys_dir) = &config.keys_dir {
        if !keys_dir.is_dir() {
            bail!("keys directory not found: {}", keys_dir.display());
        }
    }

    if let Some(device) = &config.device {
        let metadata = fs::metadata(device)
            .with_context(|| format!("device not found or unreadable: {}", device.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            let file_type = metadata.file_type();
            let is_supported_device = file_type.is_block_device()
                || (std::env::consts::OS == "macos" && file_type.is_char_device());
            if !is_supported_device {
                bail!("device is not a block device: {}", device.display());
            }
        }
    }

    Ok(())
}

fn build_image(root: &Path, attr: &str) -> Result<()> {
    let full_attr = format!(".#{attr}");
    run_command(
        Command::new("nix")
            .arg("eval")
            .arg("--raw")
            .arg(format!("{full_attr}.drvPath"))
            .current_dir(root),
        &format!("failed to evaluate {full_attr}"),
    )?;

    println!("Building image attr: {attr}");
    run_command(
        Command::new("nix")
            .arg("build")
            .arg(&full_attr)
            .current_dir(root),
        &format!("failed to build {full_attr}"),
    )
}

fn find_built_artifact(result_path: &Path) -> Result<PathBuf> {
    if !result_path.exists() {
        bail!("could not find built image artifact under ./result");
    }

    let mut candidates = Vec::new();
    collect_artifacts(result_path, &mut candidates)?;

    for suffix in [".img.zst", ".img.xz", ".img", ".raw"] {
        if let Some(path) = candidates
            .iter()
            .find(|path| path.to_string_lossy().ends_with(suffix))
        {
            return Ok(path.clone());
        }
    }

    bail!("could not find supported image artifact under ./result");
}

fn collect_artifacts(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    let metadata = fs::metadata(dir)?;
    if metadata.is_file() {
        out.push(dir.to_path_buf());
        return Ok(());
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        if metadata.is_dir() {
            collect_artifacts(&path, out)?;
        } else if metadata.is_file() {
            out.push(path);
        }
    }
    Ok(())
}

fn normalize_artifact_to_raw(artifact: &Path, raw_img: &Path) -> Result<()> {
    println!("Found image artifact: {}", artifact.display());
    match artifact.extension().and_then(OsStr::to_str) {
        Some("zst") => run_command_capture_to_file(
            Command::new("zstd").arg("-d").arg("--stdout").arg(artifact),
            raw_img,
            "failed to decompress zstd image",
        ),
        Some("xz") => run_command_capture_to_file(
            Command::new("xz").arg("-d").arg("--stdout").arg(artifact),
            raw_img,
            "failed to decompress xz image",
        ),
        Some("img") | Some("raw") => fs::copy(artifact, raw_img)
            .map(|_| ())
            .with_context(|| format!("failed to copy image artifact to {}", raw_img.display())),
        _ => bail!("unsupported image format: {}", artifact.display()),
    }?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        // The raw image becomes a temporary container for injected private keys.
        fs::set_permissions(raw_img, fs::Permissions::from_mode(0o600))
            .with_context(|| format!("failed to make raw image writable: {}", raw_img.display()))?;
    }

    Ok(())
}

fn inject_host_keys(
    raw_img: &Path,
    keys_dir: &Path,
    partition_label: &str,
    output_path: &Path,
) -> Result<()> {
    match injection_backend_for_os(std::env::consts::OS)? {
        HostKeyInjectionBackend::LinuxMount => {
            inject_host_keys_linux_mount(raw_img, keys_dir, partition_label, output_path)
        }
        HostKeyInjectionBackend::Debugfs => {
            inject_host_keys_debugfs(raw_img, keys_dir, partition_label, output_path)
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
enum HostKeyInjectionBackend {
    LinuxMount,
    Debugfs,
}

fn injection_backend_for_os(os: &str) -> Result<HostKeyInjectionBackend> {
    match os {
        "linux" => Ok(HostKeyInjectionBackend::LinuxMount),
        "macos" => Ok(HostKeyInjectionBackend::Debugfs),
        _ => bail!("SSH host key injection is not supported on {os}"),
    }
}

fn inject_host_keys_linux_mount(
    raw_img: &Path,
    keys_dir: &Path,
    partition_label: &str,
    output_path: &Path,
) -> Result<()> {
    let ssh_dir_rel = Path::new("etc/ssh");
    let loopdev = run_command_capture(
        Command::new("sudo")
            .arg("losetup")
            .arg("--find")
            .arg("--show")
            .arg("--partscan")
            .arg(raw_img),
        "failed to attach loop device",
    )?;
    let loopdev = loopdev.trim().to_string();

    let tmp_dir = make_temp_dir()?;
    let mount_dir = tmp_dir.join("mnt");
    fs::create_dir_all(&mount_dir)?;

    let cleanup = LoopCleanup {
        loopdev: loopdev.clone(),
        mount_dir: mount_dir.clone(),
    };

    let root_part = find_root_partition(&loopdev, partition_label)?;
    run_command(
        Command::new("sudo")
            .arg("mount")
            .arg(&root_part)
            .arg(&mount_dir),
        &format!("failed to mount {root_part}"),
    )?;

    install_ssh_host_keys(keys_dir, &mount_dir.join(ssh_dir_rel))?;
    run_command(&mut Command::new("sync"), "failed to sync image writes")?;

    drop(cleanup);

    write_output_image(raw_img, output_path)?;

    println!("Image with keys written to: {}", output_path.display());
    fs::remove_dir_all(&tmp_dir).ok();
    Ok(())
}

fn inject_host_keys_debugfs(
    raw_img: &Path,
    keys_dir: &Path,
    partition_label: &str,
    output_path: &Path,
) -> Result<()> {
    validate_host_key_files(keys_dir)?;

    let tmp_dir = make_temp_dir()?;
    let partition = find_ext_partition_by_label(raw_img, partition_label, &tmp_dir)?;
    let rootfs_img = tmp_dir.join("rootfs.img");

    extract_partition(raw_img, &rootfs_img, &partition)?;
    inject_host_keys_with_debugfs(&rootfs_img, keys_dir)?;
    run_command(
        Command::new("e2fsck").arg("-fn").arg(&rootfs_img),
        "modified root filesystem failed e2fsck",
    )?;
    splice_partition(raw_img, &rootfs_img, &partition)?;

    write_output_image(raw_img, output_path)?;

    println!("Image with keys written to: {}", output_path.display());
    fs::remove_dir_all(&tmp_dir).ok();
    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
struct PartitionTable {
    sector_size: u64,
    partitions: Vec<ImagePartition>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ImagePartition {
    start: u64,
    size: u64,
    sector_size: u64,
}

#[derive(Deserialize)]
struct SfdiskOutput {
    partitiontable: SfdiskPartitionTable,
}

#[derive(Deserialize)]
struct SfdiskPartitionTable {
    sectorsize: Option<u64>,
    partitions: Option<Vec<SfdiskPartition>>,
}

#[derive(Deserialize)]
struct SfdiskPartition {
    start: Option<u64>,
    size: Option<u64>,
}

fn parse_partition_table(json: &str) -> Result<PartitionTable> {
    let output: SfdiskOutput = serde_json::from_str(json).context("failed to parse sfdisk JSON")?;
    let sector_size = output.partitiontable.sectorsize.unwrap_or(512);
    let partitions = output
        .partitiontable
        .partitions
        .unwrap_or_default()
        .into_iter()
        .map(|partition| {
            let start = partition
                .start
                .context("sfdisk partition is missing start sector")?;
            let size = partition.size.context("sfdisk partition is missing size")?;
            Ok(ImagePartition {
                start,
                size,
                sector_size,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(PartitionTable {
        sector_size,
        partitions,
    })
}

fn find_ext_partition_by_label(
    image: &Path,
    partition_label: &str,
    tmp_dir: &Path,
) -> Result<ImagePartition> {
    let output = run_command_capture(
        Command::new("sfdisk").arg("--json").arg(image),
        "failed to inspect image partition table",
    )?;
    let table = parse_partition_table(&output)?;

    for (index, partition) in table.partitions.iter().enumerate() {
        let partition_img = tmp_dir.join(format!("partition-{index}.img"));
        extract_partition(image, &partition_img, partition)?;
        if ext_partition_has_label(&partition_img, partition_label)? {
            return Ok(partition.clone());
        }
    }

    bail!(
        "could not find partition with label `{partition_label}` in {}",
        image.display()
    )
}

fn ext_partition_has_label(partition_img: &Path, partition_label: &str) -> Result<bool> {
    let output = Command::new("dumpe2fs")
        .arg("-h")
        .arg(partition_img)
        .output();
    let Ok(output) = output else {
        return Ok(false);
    };
    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().any(|line| {
        line.trim_start()
            .strip_prefix("Filesystem volume name:")
            .map(|label| label.trim() == partition_label)
            .unwrap_or(false)
    }))
}

fn extract_partition(image: &Path, partition_img: &Path, partition: &ImagePartition) -> Result<()> {
    run_command(
        Command::new("dd")
            .arg(format!("if={}", image.display()))
            .arg(format!("of={}", partition_img.display()))
            .arg(format!("bs={}", partition.sector_size))
            .arg(format!("skip={}", partition.start))
            .arg(format!("count={}", partition.size))
            .arg("status=none"),
        "failed to extract root partition from image",
    )
}

fn splice_partition(image: &Path, partition_img: &Path, partition: &ImagePartition) -> Result<()> {
    run_command(
        Command::new("dd")
            .arg(format!("if={}", partition_img.display()))
            .arg(format!("of={}", image.display()))
            .arg(format!("bs={}", partition.sector_size))
            .arg(format!("seek={}", partition.start))
            .arg("conv=notrunc")
            .arg("status=none"),
        "failed to splice root partition back into image",
    )
}

fn inject_host_keys_with_debugfs(rootfs_img: &Path, keys_dir: &Path) -> Result<()> {
    ensure_debugfs_dir(rootfs_img, "/etc")?;
    ensure_debugfs_dir(rootfs_img, "/etc/ssh")?;

    run_command_allow_failure(
        Command::new("debugfs")
            .arg("-w")
            .arg("-R")
            .arg("rm /etc/ssh/ssh_host_ed25519_key")
            .arg(rootfs_img),
    );
    run_command_allow_failure(
        Command::new("debugfs")
            .arg("-w")
            .arg("-R")
            .arg("rm /etc/ssh/ssh_host_ed25519_key.pub")
            .arg(rootfs_img),
    );

    let command_file = rootfs_img.with_extension("debugfs.commands");
    fs::write(
        &command_file,
        debugfs_host_key_commands(keys_dir).join("\n") + "\n",
    )
    .with_context(|| format!("failed to write {}", command_file.display()))?;
    run_command(
        Command::new("debugfs")
            .arg("-w")
            .arg("-f")
            .arg(&command_file)
            .arg(rootfs_img),
        "failed to inject SSH host keys with debugfs",
    )
}

fn ensure_debugfs_dir(rootfs_img: &Path, path: &str) -> Result<()> {
    if debugfs_path_exists(rootfs_img, path) {
        return Ok(());
    }

    run_command(
        Command::new("debugfs")
            .arg("-w")
            .arg("-R")
            .arg(format!("mkdir {path}"))
            .arg(rootfs_img),
        &format!("failed to create {path} in root filesystem"),
    )
}

fn debugfs_path_exists(rootfs_img: &Path, path: &str) -> bool {
    let Ok(output) = Command::new("debugfs")
        .arg("-R")
        .arg(format!("stat {path}"))
        .arg(rootfs_img)
        .output()
    else {
        return false;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    output.status.success()
        && stdout.contains("Inode:")
        && !stdout.contains("File not found")
        && !stderr.contains("File not found")
}

fn debugfs_host_key_commands(keys_dir: &Path) -> Vec<String> {
    let private_key = keys_dir.join("ssh_host_ed25519_key");
    let public_key = keys_dir.join("ssh_host_ed25519_key.pub");
    vec![
        format!(
            "write {} /etc/ssh/ssh_host_ed25519_key",
            private_key.display()
        ),
        format!(
            "write {} /etc/ssh/ssh_host_ed25519_key.pub",
            public_key.display()
        ),
        "set_inode_field /etc/ssh mode 040755".to_string(),
        "set_inode_field /etc/ssh uid 0".to_string(),
        "set_inode_field /etc/ssh gid 0".to_string(),
        "set_inode_field /etc/ssh/ssh_host_ed25519_key mode 0100600".to_string(),
        "set_inode_field /etc/ssh/ssh_host_ed25519_key uid 0".to_string(),
        "set_inode_field /etc/ssh/ssh_host_ed25519_key gid 0".to_string(),
        "set_inode_field /etc/ssh/ssh_host_ed25519_key.pub mode 0100644".to_string(),
        "set_inode_field /etc/ssh/ssh_host_ed25519_key.pub uid 0".to_string(),
        "set_inode_field /etc/ssh/ssh_host_ed25519_key.pub gid 0".to_string(),
    ]
}

fn find_root_partition(loopdev: &str, partition_label: &str) -> Result<String> {
    let output = run_command_capture(
        Command::new("sudo")
            .arg("lsblk")
            .arg("-nrpo")
            .arg("NAME,LABEL")
            .arg(loopdev),
        "failed to inspect loop device partitions",
    )?;

    for line in output.lines() {
        let mut parts = line.split_whitespace();
        let name = parts.next();
        let label = parts.next();
        if let (Some(name), Some(label)) = (name, label) {
            if label == partition_label {
                return Ok(name.to_string());
            }
        }
    }

    let fallback = format!("{loopdev}p2");
    if Path::new(&fallback).exists() {
        return Ok(fallback);
    }

    bail!("could not find partition with label `{partition_label}` on {loopdev}")
}

fn install_ssh_host_keys(keys_dir: &Path, ssh_dir: &Path) -> Result<()> {
    validate_host_key_files(keys_dir)?;

    run_command(
        Command::new("sudo")
            .arg("install")
            .arg("-d")
            .arg("-m")
            .arg("0755")
            .arg(ssh_dir),
        "failed to create target ssh directory",
    )?;
    run_command(
        Command::new("sudo")
            .arg("install")
            .arg("-m")
            .arg("0600")
            .arg(keys_dir.join("ssh_host_ed25519_key"))
            .arg(ssh_dir.join("ssh_host_ed25519_key")),
        "failed to install private host key",
    )?;
    run_command(
        Command::new("sudo")
            .arg("install")
            .arg("-m")
            .arg("0644")
            .arg(keys_dir.join("ssh_host_ed25519_key.pub"))
            .arg(ssh_dir.join("ssh_host_ed25519_key.pub")),
        "failed to install public host key",
    )?;
    run_command(
        Command::new("sudo")
            .arg("chown")
            .arg("root:root")
            .arg(ssh_dir.join("ssh_host_ed25519_key"))
            .arg(ssh_dir.join("ssh_host_ed25519_key.pub")),
        "failed to set host key ownership",
    )
}

fn validate_host_key_files(keys_dir: &Path) -> Result<()> {
    let required = ["ssh_host_ed25519_key", "ssh_host_ed25519_key.pub"];
    for file in required {
        let path = keys_dir.join(file);
        if !path.is_file() {
            bail!("missing required key file: {}", path.display());
        }
    }
    Ok(())
}

fn flash_image(image: &Path, device: &Path) -> Result<()> {
    match std::env::consts::OS {
        "linux" => flash_image_linux(image, device),
        "macos" => flash_image_macos(image, device),
        os => bail!("image flashing is not supported on {os}"),
    }
}

fn write_output_image(raw_img: &Path, output_path: &Path) -> Result<()> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(raw_img, output_path)
        .or_else(|_| {
            fs::copy(raw_img, output_path)?;
            fs::remove_file(raw_img)
        })
        .with_context(|| format!("failed to write {}", output_path.display()))
}

fn flash_image_linux(image: &Path, device: &Path) -> Result<()> {
    println!("Unmounting {} partitions...", device.display());
    Command::new("sudo")
        .arg("sh")
        .arg("-c")
        .arg(format!(
            "umount {}* 2>/dev/null || true",
            shell_escape(device)
        ))
        .status()
        .ok();

    println!(
        "Flashing {} to {} (sync may take awhile after 100%)...",
        image.display(),
        device.display()
    );

    let pipeline = format!(
        "pv {} | sudo dd of={} bs=8M conv=fsync status=none",
        shell_escape(image),
        shell_escape(device)
    );
    run_command(
        Command::new("sh").arg("-c").arg(&pipeline),
        "failed to flash image to device",
    )?;

    println!("Done. Image flashed to {}", device.display());
    Ok(())
}

fn flash_image_macos(image: &Path, device: &Path) -> Result<()> {
    println!("Unmounting {}...", device.display());
    run_command(
        Command::new("diskutil").arg("unmountDisk").arg(device),
        "failed to unmount target disk",
    )?;

    let raw_device = darwin_raw_device_path(device);
    println!(
        "Flashing {} to {} (sync may take awhile after 100%)...",
        image.display(),
        raw_device.display()
    );
    run_command(
        Command::new("sudo")
            .arg("dd")
            .arg(format!("if={}", image.display()))
            .arg(format!("of={}", raw_device.display()))
            .arg("bs=4M")
            .arg("conv=fsync")
            .arg("status=progress"),
        "failed to flash image to device",
    )?;
    run_command(
        Command::new("diskutil").arg("eject").arg(device),
        "failed to eject target disk",
    )?;

    println!("Done. Image flashed to {}", device.display());
    Ok(())
}

fn darwin_raw_device_path(device: &Path) -> PathBuf {
    let raw = device.to_string_lossy();
    if raw.starts_with("/dev/rdisk") {
        return device.to_path_buf();
    }
    if let Some(name) = device.file_name().and_then(OsStr::to_str) {
        if let Some(disk) = name.strip_prefix("disk") {
            return PathBuf::from(format!("/dev/rdisk{disk}"));
        }
    }
    device.to_path_buf()
}

fn make_temp_dir() -> Result<PathBuf> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("semble-image-{}-{}", std::process::id(), unique));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn run_command(cmd: &mut Command, context: &str) -> Result<()> {
    let status = cmd.status().with_context(|| context.to_string())?;
    if status.success() {
        Ok(())
    } else {
        bail!("{context}");
    }
}

fn run_command_allow_failure(cmd: &mut Command) {
    cmd.status().ok();
}

fn run_command_capture(cmd: &mut Command, context: &str) -> Result<String> {
    let output = cmd.output().with_context(|| context.to_string())?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{context}: {}", stderr.trim());
    }
}

fn run_command_capture_to_file(cmd: &mut Command, output_path: &Path, context: &str) -> Result<()> {
    let output = cmd.output().with_context(|| context.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("{context}: {}", stderr.trim());
    }
    fs::write(output_path, output.stdout)
        .with_context(|| format!("failed to write {}", output_path.display()))
}

fn shell_escape(path: &Path) -> String {
    let raw = path.to_string_lossy();
    format!("'{}'", raw.replace('\'', "'\"'\"'"))
}

struct LoopCleanup {
    loopdev: String,
    mount_dir: PathBuf,
}

impl Drop for LoopCleanup {
    fn drop(&mut self) {
        let _ = Command::new("sudo")
            .arg("umount")
            .arg(&self.mount_dir)
            .status();
        let _ = Command::new("sudo")
            .arg("losetup")
            .arg("-d")
            .arg(&self.loopdev)
            .status();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        darwin_raw_device_path, debugfs_host_key_commands, extract_partition, find_built_artifact,
        find_ext_partition_by_label, inject_host_keys_debugfs, injection_backend_for_os,
        normalize_artifact_to_raw, parse_partition_table, resolve_prepare_config, resolve_prepare_config_with,
        run_command_capture, splice_partition, validate_prepare_inputs, HostKeyInjectionBackend,
    };
    use crate::cli::PrepareImageArgs;
    use crate::repo::{ImagePrepareConfig, RepoPaths};
    use anyhow::anyhow;
    use std::fs;
    use std::io::Write;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};
    use tempfile::tempdir;

    fn write_config(root: &std::path::Path) {
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
    fn resolve_prepare_config_uses_repo_defaults() {
        // Verify the default build attr, partition label, keys dir, and output path.
        let tempdir = tempdir().unwrap();
        write_config(tempdir.path());
        fs::create_dir_all(tempdir.path().join("ssh_host_keys").join("vishnu")).unwrap();

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let config = resolve_prepare_config_with(
            &paths,
            PrepareImageArgs {
                image_name: "vishnu".into(),
                keys_dir: None,
                output: None,
                device: None,
                skip_inject: false,
            },
            |_paths, image_name| {
                assert_eq!(image_name, "vishnu");
                Ok(ImagePrepareConfig {
                    partition_label: "NIXOS_SD".into(),
                })
            },
        )
        .unwrap();

        assert_eq!(config.build_attr, "images.vishnu");
        assert_eq!(config.partition_label, "NIXOS_SD");
        assert_eq!(
            config.keys_dir.unwrap(),
            tempdir.path().join("ssh_host_keys").join("vishnu")
        );
        assert_eq!(
            config.output_path,
            tempdir.path().join("out").join("vishnu.img")
        );
    }

    #[test]
    fn resolve_prepare_config_reads_prepare_metadata_for_multiple_images() {
        // Verify prepare resolution uses the requested image name, not a hardcoded one.
        let tempdir = tempdir().unwrap();
        write_config(tempdir.path());
        fs::create_dir_all(tempdir.path().join("ssh_host_keys").join("genesis")).unwrap();

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let config = resolve_prepare_config_with(
            &paths,
            PrepareImageArgs {
                image_name: "genesis".into(),
                keys_dir: None,
                output: None,
                device: None,
                skip_inject: false,
            },
            |_paths, image_name| {
                assert_eq!(image_name, "genesis");
                Ok(ImagePrepareConfig {
                    partition_label: "nixos".into(),
                })
            },
        )
        .unwrap();

        assert_eq!(config.build_attr, "images.genesis");
        assert_eq!(config.partition_label, "nixos");
    }

    #[test]
    fn resolve_prepare_config_allows_skip_inject_without_image_prepare_config() {
        // Verify skip-inject bypasses prepare metadata and keys-dir requirements.
        let tempdir = tempdir().unwrap();
        write_config(tempdir.path());

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let config = resolve_prepare_config(
            &paths,
            PrepareImageArgs {
                image_name: "unknown".into(),
                keys_dir: None,
                output: Some("/tmp/custom.img".into()),
                device: None,
                skip_inject: true,
            },
        )
        .unwrap();

        assert!(config.keys_dir.is_none());
        assert_eq!(
            config.output_path,
            std::path::PathBuf::from("/tmp/custom.img")
        );
    }

    #[test]
    fn validate_prepare_inputs_rejects_missing_keys_dir() {
        // Verify injection mode fails fast when the default keys dir does not exist.
        let tempdir = tempdir().unwrap();
        write_config(tempdir.path());

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let config = resolve_prepare_config_with(
            &paths,
            PrepareImageArgs {
                image_name: "vishnu".into(),
                keys_dir: None,
                output: None,
                device: None,
                skip_inject: false,
            },
            |_paths, _image_name| {
                Ok(ImagePrepareConfig {
                    partition_label: "NIXOS_SD".into(),
                })
            },
        )
        .unwrap();

        let error = validate_prepare_inputs(&config).unwrap_err();
        assert!(error.to_string().contains("keys directory not found"));
    }

    #[test]
    fn resolve_prepare_config_propagates_prepare_lookup_failures() {
        // Verify prepare metadata lookup failures are surfaced to the caller.
        let tempdir = tempdir().unwrap();
        write_config(tempdir.path());

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let error = resolve_prepare_config_with(
            &paths,
            PrepareImageArgs {
                image_name: "vishnu".into(),
                keys_dir: None,
                output: None,
                device: None,
                skip_inject: false,
            },
            |_paths, _image_name| Err(anyhow!("metadata lookup failed")),
        )
        .unwrap_err();

        assert!(error.to_string().contains("metadata lookup failed"));
    }

    #[test]
    fn find_built_artifact_prefers_compressed_img() {
        // Verify artifact discovery prefers compressed images over plain img/raw files.
        let tempdir = tempdir().unwrap();
        let result_dir = tempdir.path().join("result").join("nested");
        fs::create_dir_all(&result_dir).unwrap();
        fs::write(result_dir.join("disk.img"), b"img").unwrap();
        fs::write(result_dir.join("disk.img.zst"), b"zst").unwrap();
        fs::write(result_dir.join("disk.raw"), b"raw").unwrap();

        let artifact = find_built_artifact(&tempdir.path().join("result")).unwrap();
        assert!(artifact.ends_with("disk.img.zst"));
    }

    #[cfg(unix)]
    #[test]
    fn normalize_raw_image_makes_copy_private_and_writable() {
        let tempdir = tempdir().unwrap();
        let artifact = tempdir.path().join("disk.img");
        let raw_img = tempdir.path().join("prepared.img");
        fs::write(&artifact, b"raw image").unwrap();
        fs::set_permissions(&artifact, fs::Permissions::from_mode(0o444)).unwrap();

        normalize_artifact_to_raw(&artifact, &raw_img).unwrap();

        assert_eq!(fs::metadata(&raw_img).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn injection_backend_uses_debugfs_on_macos() {
        assert_eq!(
            injection_backend_for_os("macos").unwrap(),
            HostKeyInjectionBackend::Debugfs
        );
    }

    #[test]
    fn parse_partition_table_reads_sector_offsets() {
        let table = parse_partition_table(
            r#"
            {
              "partitiontable": {
                "label": "dos",
                "sectorsize": 512,
                "partitions": [
                  { "node": "image.img1", "start": 8192, "size": 1048576, "type": "c" },
                  { "node": "image.img2", "start": 1056768, "size": 2097152, "type": "83" }
                ]
              }
            }
            "#,
        )
        .unwrap();

        assert_eq!(table.sector_size, 512);
        assert_eq!(table.partitions.len(), 2);
        assert_eq!(table.partitions[1].start, 1056768);
        assert_eq!(table.partitions[1].size, 2097152);
    }

    #[test]
    fn debugfs_host_key_commands_write_keys_with_root_permissions() {
        let tempdir = tempdir().unwrap();
        let keys_dir = tempdir.path().join("keys");

        let commands = debugfs_host_key_commands(&keys_dir);

        assert!(commands.contains(&format!(
            "write {} /etc/ssh/ssh_host_ed25519_key",
            keys_dir.join("ssh_host_ed25519_key").display()
        )));
        assert!(commands.contains(&format!(
            "write {} /etc/ssh/ssh_host_ed25519_key.pub",
            keys_dir.join("ssh_host_ed25519_key.pub").display()
        )));
        assert!(commands
            .contains(&"set_inode_field /etc/ssh/ssh_host_ed25519_key mode 0100600".to_string()));
        assert!(commands.contains(
            &"set_inode_field /etc/ssh/ssh_host_ed25519_key.pub mode 0100644".to_string()
        ));
        assert!(commands.contains(&"set_inode_field /etc/ssh uid 0".to_string()));
        assert!(commands.contains(&"set_inode_field /etc/ssh gid 0".to_string()));
    }

    #[test]
    fn debugfs_injection_writes_keys_into_partitioned_ext4_image() {
        let tempdir = tempdir().unwrap();
        let keys_dir = tempdir.path().join("keys");
        fs::create_dir_all(&keys_dir).unwrap();
        fs::write(keys_dir.join("ssh_host_ed25519_key"), b"private-key\n").unwrap();
        fs::write(keys_dir.join("ssh_host_ed25519_key.pub"), b"public-key\n").unwrap();

        let disk_img = tempdir.path().join("disk.img");
        let output_img = tempdir.path().join("output.img");
        fs::File::create(&disk_img)
            .unwrap()
            .set_len(64 * 1024 * 1024)
            .unwrap();

        let mut sfdisk = Command::new("sfdisk")
            .arg(&disk_img)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        sfdisk
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b",8M,c\n,32M,83\n")
            .unwrap();
        assert!(sfdisk.wait().unwrap().success());

        let table_json = run_command_capture(
            Command::new("sfdisk").arg("--json").arg(&disk_img),
            "failed to inspect synthetic image",
        )
        .unwrap();
        let partition = parse_partition_table(&table_json).unwrap().partitions[1].clone();
        let rootfs_img = tempdir.path().join("rootfs.img");
        fs::File::create(&rootfs_img)
            .unwrap()
            .set_len(partition.size * partition.sector_size)
            .unwrap();
        assert!(Command::new("mkfs.ext4")
            .arg("-F")
            .arg("-L")
            .arg("NIXOS_SD")
            .arg(&rootfs_img)
            .status()
            .unwrap()
            .success());
        splice_partition(&disk_img, &rootfs_img, &partition).unwrap();

        inject_host_keys_debugfs(&disk_img, &keys_dir, "NIXOS_SD", &output_img).unwrap();

        let modified_rootfs = tempdir.path().join("modified-rootfs.img");
        extract_partition(&output_img, &modified_rootfs, &partition).unwrap();
        let private_key = run_command_capture(
            Command::new("debugfs")
                .arg("-R")
                .arg("cat /etc/ssh/ssh_host_ed25519_key")
                .arg(&modified_rootfs),
            "failed to read injected private key",
        )
        .unwrap();
        let public_key = run_command_capture(
            Command::new("debugfs")
                .arg("-R")
                .arg("cat /etc/ssh/ssh_host_ed25519_key.pub")
                .arg(&modified_rootfs),
            "failed to read injected public key",
        )
        .unwrap();

        assert_eq!(private_key, "private-key\n");
        assert_eq!(public_key, "public-key\n");
    }

    #[test]
    fn debugfs_injection_rejects_images_without_requested_root_label() {
        let tempdir = tempdir().unwrap();
        let disk_img = tempdir.path().join("disk.img");
        fs::File::create(&disk_img)
            .unwrap()
            .set_len(64 * 1024 * 1024)
            .unwrap();

        let mut sfdisk = Command::new("sfdisk")
            .arg(&disk_img)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();
        sfdisk
            .stdin
            .as_mut()
            .unwrap()
            .write_all(b",8M,c\n,32M,83\n")
            .unwrap();
        assert!(sfdisk.wait().unwrap().success());

        let table_json = run_command_capture(
            Command::new("sfdisk").arg("--json").arg(&disk_img),
            "failed to inspect synthetic image",
        )
        .unwrap();
        let partition = parse_partition_table(&table_json).unwrap().partitions[1].clone();
        let rootfs_img = tempdir.path().join("rootfs.img");
        fs::File::create(&rootfs_img)
            .unwrap()
            .set_len(partition.size * partition.sector_size)
            .unwrap();
        assert!(Command::new("mkfs.ext4")
            .arg("-F")
            .arg("-L")
            .arg("OTHER_LABEL")
            .arg(&rootfs_img)
            .status()
            .unwrap()
            .success());
        splice_partition(&disk_img, &rootfs_img, &partition).unwrap();

        let error = find_ext_partition_by_label(&disk_img, "NIXOS_SD", tempdir.path()).unwrap_err();

        assert!(error
            .to_string()
            .contains("could not find partition with label `NIXOS_SD`"));
    }

    #[test]
    fn darwin_raw_device_path_prefers_rdisk_for_disk_devices() {
        assert_eq!(
            darwin_raw_device_path(std::path::Path::new("/dev/disk4")),
            std::path::PathBuf::from("/dev/rdisk4")
        );
        assert_eq!(
            darwin_raw_device_path(std::path::Path::new("/dev/rdisk4")),
            std::path::PathBuf::from("/dev/rdisk4")
        );
    }
}
