use crate::cli::ProvisionIdentityArgs;
use crate::error::fail;
use crate::host::validate_hostname;
use crate::repo::RepoPaths;
use anyhow::{Context, Result};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub fn run_microvm_provision_identity(
    paths: &RepoPaths,
    args: ProvisionIdentityArgs,
) -> Result<()> {
    validate_hostname(&args.name)?;

    let target_host = args
        .target_host
        .unwrap_or_else(|| format!("{}-admin", args.parent));
    let host_keys_dir = args
        .host_keys_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| paths.host_keys_dir(&args.name));

    let private_key = host_keys_dir.join("ssh_host_ed25519_key");
    let public_key = host_keys_dir.join("ssh_host_ed25519_key.pub");
    if !private_key.is_file() || !public_key.is_file() {
        return fail(format!(
            "missing SSH host key pair under {}; expected ssh_host_ed25519_key and ssh_host_ed25519_key.pub",
            host_keys_dir.display()
        ));
    }

    let staging_dir = format!("/run/microvm-provisioning/{}", args.name);

    println!("Creating runtime staging directory on {target_host}:{staging_dir}");
    run_ssh(
        &target_host,
        &format!(
            "sudo install -d -m 0700 -o root -g root '{staging_dir}' && sudo rm -f '{staging_dir}'/.provisioned '{staging_dir}'/.replace"
        ),
    )?;
    let mut cleanup = RemoteProvisioningCleanup::new(target_host.clone(), staging_dir.clone());

    println!("Uploading host keys to runtime staging");
    upload_file(
        &private_key,
        &target_host,
        &format!("{staging_dir}/ssh_host_ed25519_key"),
        "0600",
    )?;
    upload_file(
        &public_key,
        &target_host,
        &format!("{staging_dir}/ssh_host_ed25519_key.pub"),
        "0644",
    )?;

    if args.replace {
        run_ssh(
            &target_host,
            &format!("sudo touch '{staging_dir}/.replace'"),
        )?;
    }

    println!("Restarting microVM {} on {target_host}", args.name);
    run_ssh(
        &target_host,
        &format!(
            "sudo systemctl restart 'microvm-virtiofsd@{}.service' 'microvm@{}.service'",
            args.name, args.name
        ),
    )?;

    println!("Waiting for guest to persist host keys");
    wait_for_provisioning(&target_host, &staging_dir, args.timeout)?;

    println!("Removing staged private key from {target_host}");
    cleanup.run()?;

    println!("Provisioned SSH host identity for {}", args.name);
    Ok(())
}

fn wait_for_provisioning(target_host: &str, staging_dir: &str, timeout_seconds: u64) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    loop {
        if run_ssh_status(
            target_host,
            &format!("sudo test -e '{staging_dir}/.provisioned'"),
        )? {
            return Ok(());
        }

        if Instant::now() >= deadline {
            return fail("timed out waiting for guest to acknowledge host key provisioning");
        }

        thread::sleep(Duration::from_secs(2));
    }
}

fn upload_file(source: &Path, target_host: &str, remote_path: &str, mode: &str) -> Result<()> {
    let source_file =
        File::open(source).with_context(|| format!("failed to open {}", source.display()))?;
    let status = Command::new("ssh")
        .arg(target_host)
        .arg(format!(
            "sudo -n sh -c 'cat > \"$1\" && chown root:root \"$1\" && chmod \"$2\" \"$1\"' sh {} {}",
            shell_quote(remote_path),
            shell_quote(mode)
        ))
        .stdin(Stdio::from(source_file))
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

fn run_ssh(target_host: &str, remote_command: &str) -> Result<()> {
    if run_ssh_status(target_host, remote_command)? {
        Ok(())
    } else {
        fail(format!(
            "ssh command on {target_host} exited nonzero: {remote_command}"
        ))
    }
}

fn run_ssh_status(target_host: &str, remote_command: &str) -> Result<bool> {
    let status = Command::new("ssh")
        .arg(target_host)
        .arg(remote_command)
        .status()
        .with_context(|| format!("failed to run ssh for {target_host}"))?;

    Ok(status.success())
}

fn cleanup_command(staging_dir: &str) -> String {
    format!(
        "sudo rm -f {} {} {} {}",
        shell_quote(&format!("{staging_dir}/ssh_host_ed25519_key")),
        shell_quote(&format!("{staging_dir}/ssh_host_ed25519_key.pub")),
        shell_quote(&format!("{staging_dir}/.provisioned")),
        shell_quote(&format!("{staging_dir}/.replace")),
    )
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

struct RemoteProvisioningCleanup {
    target_host: String,
    staging_dir: String,
    active: bool,
}

impl RemoteProvisioningCleanup {
    fn new(target_host: String, staging_dir: String) -> Self {
        Self {
            target_host,
            staging_dir,
            active: true,
        }
    }

    fn run(&mut self) -> Result<()> {
        let command = cleanup_command(&self.staging_dir);
        run_ssh(&self.target_host, &command)?;
        self.active = false;
        Ok(())
    }
}

impl Drop for RemoteProvisioningCleanup {
    fn drop(&mut self) {
        if !self.active {
            return;
        }

        let command = cleanup_command(&self.staging_dir);
        if let Err(err) = run_ssh(&self.target_host, &command) {
            eprintln!(
                "warning: failed to clean remote MicroVM provisioning files on {}: {err:#}",
                self.target_host
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{cleanup_command, shell_quote};

    #[test]
    fn quotes_shell_values() {
        assert_eq!(shell_quote("/run/example"), "'/run/example'");
        assert_eq!(shell_quote("value'with quote"), "'value'\\''with quote'");
    }

    #[test]
    fn builds_cleanup_command() {
        assert_eq!(
            cleanup_command("/run/microvm-provisioning/claw"),
            "sudo rm -f '/run/microvm-provisioning/claw/ssh_host_ed25519_key' '/run/microvm-provisioning/claw/ssh_host_ed25519_key.pub' '/run/microvm-provisioning/claw/.provisioned' '/run/microvm-provisioning/claw/.replace'"
        );
    }
}
