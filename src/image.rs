use crate::cli::PrepareImageArgs;
use crate::repo::RepoPaths;
use anyhow::{anyhow, bail, Context, Result};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub fn run_image_prepare(paths: &RepoPaths, args: PrepareImageArgs) -> Result<()> {
    ensure_linux_host()?;

    let config = resolve_prepare_config(paths, args)?;
    validate_prepare_inputs(&config)?;
    build_image(paths.root(), &config.build_attr)?;

    let artifact = find_built_artifact(&paths.root().join("result"))?;
    let tmp_dir = make_temp_dir()?;
    let raw_img = tmp_dir.join(&config.raw_img_name);

    normalize_artifact_to_raw(&artifact, &raw_img)?;

    if let Some(keys_dir) = &config.keys_dir {
        inject_host_keys(&raw_img, keys_dir, &config.partition_label, &config.output_path)?;
    } else {
        if let Some(parent) = config.output_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&raw_img, &config.output_path)
            .or_else(|_| {
                fs::copy(&raw_img, &config.output_path)?;
                fs::remove_file(&raw_img)
            })
            .with_context(|| format!("failed to write {}", config.output_path.display()))?;
        println!("Skipping SSH key injection. Image written to: {}", config.output_path.display());
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

fn resolve_prepare_config(paths: &RepoPaths, args: PrepareImageArgs) -> Result<PreparedImageConfig> {
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
        paths
            .image_prepare_config(&image_name)
            .map(|cfg| cfg.partition_label.clone())
            .ok_or_else(|| anyhow!("missing image prepare config for `{image_name}` in semble.toml"))?
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

fn ensure_linux_host() -> Result<()> {
    if std::env::consts::OS != "linux" {
        bail!("this command only supports Linux hosts");
    }
    Ok(())
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
            if !metadata.file_type().is_block_device() {
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
        Command::new("nix").arg("build").arg(&full_attr).current_dir(root),
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
        if let Some(path) = candidates.iter().find(|path| path.to_string_lossy().ends_with(suffix)) {
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
    }
}

fn inject_host_keys(raw_img: &Path, keys_dir: &Path, partition_label: &str, output_path: &Path) -> Result<()> {
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

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::rename(raw_img, output_path)
        .or_else(|_| {
            fs::copy(raw_img, output_path)?;
            fs::remove_file(raw_img)
        })
        .with_context(|| format!("failed to write {}", output_path.display()))?;

    println!("Image with keys written to: {}", output_path.display());
    fs::remove_dir_all(&tmp_dir).ok();
    Ok(())
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
    let required = ["ssh_host_ed25519_key", "ssh_host_ed25519_key.pub"];
    for file in required {
        let path = keys_dir.join(file);
        if !path.is_file() {
            bail!("missing required key file: {}", path.display());
        }
    }

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

fn flash_image(image: &Path, device: &Path) -> Result<()> {
    println!("Unmounting {} partitions...", device.display());
    Command::new("sudo")
        .arg("sh")
        .arg("-c")
        .arg(format!("umount {}* 2>/dev/null || true", shell_escape(device)))
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
    use super::{find_built_artifact, resolve_prepare_config, validate_prepare_inputs};
    use crate::cli::PrepareImageArgs;
    use crate::repo::RepoPaths;
    use std::fs;
    use tempfile::tempdir;

    fn write_config(root: &std::path::Path) {
        fs::write(
            root.join("semble.toml"),
            r#"
[paths]
hosts_dir = "hosts"
host_template_dir = "hosts/_template"
ssh_host_keys_dir = "ssh_host_keys"
sops_config_file = ".sops.yaml"
network_secrets_file = "secrets/network.yaml"

[ssh]
managed_config_file = "~/.ssh/semble_hosts"
dns_suffix = "example.ts.net"

[[ssh.aliases]]
name_suffix = "admin"
user = "admin"
identity_file = "~/.ssh/id_ed25519"

[image_prepare.vishnu]
partition_label = "NIXOS_SD"

[image_prepare.genesis]
partition_label = "nixos"
"#,
        )
        .unwrap();
    }

    #[test]
    fn resolve_prepare_config_uses_repo_defaults() {
        let tempdir = tempdir().unwrap();
        write_config(tempdir.path());
        fs::create_dir_all(tempdir.path().join("ssh_host_keys").join("vishnu")).unwrap();

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let config = resolve_prepare_config(
            &paths,
            PrepareImageArgs {
                image_name: "vishnu".into(),
                keys_dir: None,
                output: None,
                device: None,
                skip_inject: false,
            },
        )
        .unwrap();

        assert_eq!(config.build_attr, "images.vishnu");
        assert_eq!(config.partition_label, "NIXOS_SD");
        assert_eq!(
            config.keys_dir.unwrap(),
            tempdir.path().join("ssh_host_keys").join("vishnu")
        );
        assert_eq!(config.output_path, tempdir.path().join("out").join("vishnu.img"));
    }

    #[test]
    fn resolve_prepare_config_allows_skip_inject_without_image_prepare_config() {
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
        assert_eq!(config.output_path, std::path::PathBuf::from("/tmp/custom.img"));
    }

    #[test]
    fn validate_prepare_inputs_rejects_missing_keys_dir() {
        let tempdir = tempdir().unwrap();
        write_config(tempdir.path());

        let paths = RepoPaths::new(tempdir.path()).unwrap();
        let config = resolve_prepare_config(
            &paths,
            PrepareImageArgs {
                image_name: "vishnu".into(),
                keys_dir: None,
                output: None,
                device: None,
                skip_inject: false,
            },
        )
        .unwrap();

        let error = validate_prepare_inputs(&config).unwrap_err();
        assert!(error.to_string().contains("keys directory not found"));
    }

    #[test]
    fn find_built_artifact_prefers_compressed_img() {
        let tempdir = tempdir().unwrap();
        let result_dir = tempdir.path().join("result").join("nested");
        fs::create_dir_all(&result_dir).unwrap();
        fs::write(result_dir.join("disk.img"), b"img").unwrap();
        fs::write(result_dir.join("disk.img.zst"), b"zst").unwrap();
        fs::write(result_dir.join("disk.raw"), b"raw").unwrap();

        let artifact = find_built_artifact(&tempdir.path().join("result")).unwrap();
        assert!(artifact.ends_with("disk.img.zst"));
    }
}
