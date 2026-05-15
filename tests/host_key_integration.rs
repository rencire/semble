use std::fs;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

fn write_repo_config(root: &Path) {
    fs::write(
        root.join("semble.toml"),
        r#"
[paths]
hosts_dir = "hosts"
host_template_dir = "hosts/templates"
default_host_template = "default"
ssh_host_keys_dir = "ssh_host_keys"
disk_keys_dir = "disk_keys"
sops_config_file = ".sops.yaml"
network_secrets_file = "secrets/network.yaml"
"#,
    )
    .unwrap();
}

fn write_minimal_repo_files(root: &Path) {
    fs::create_dir_all(root.join("secrets")).unwrap();
    fs::write(
        root.join(".sops.yaml"),
        "keys:\n  - &genesis \"PUBLIC_KEY_GENESIS\"\ncreation_rules:\n  - path_regex: secrets/network\\.(yaml|json|env|ini)$\n    key_groups:\n      - age:\n          - *genesis\n",
    )
    .unwrap();
    fs::write(root.join("secrets/network.yaml"), "keys: []\n").unwrap();
}

fn run_semble(root: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_semble"));
    cmd.current_dir(root).args(args);
    cmd
}

#[test]
fn host_key_commands_preserve_shared_directory_contents_and_target_only_matching_files() {
    let tempdir = tempdir().unwrap();
    write_repo_config(tempdir.path());
    write_minimal_repo_files(tempdir.path());

    let ssh_add = run_semble(
        tempdir.path(),
        &["host", "keys", "ssh", "add", "atlas", "--force", "--skip-reencrypt"],
    )
    .output()
    .unwrap();
    assert!(
        ssh_add.status.success(),
        "ssh add failed: {}",
        String::from_utf8_lossy(&ssh_add.stderr)
    );

    let keys_dir = tempdir.path().join("ssh_host_keys/atlas");
    let ssh_private = keys_dir.join("ssh_host_ed25519_key");
    let ssh_public = keys_dir.join("ssh_host_ed25519_key.pub");
    let initrd_private = keys_dir.join("initrd_ssh_host_ed25519_key");
    let initrd_public = keys_dir.join("initrd_ssh_host_ed25519_key.pub");
    let sentinel = keys_dir.join("keep.txt");

    assert!(ssh_private.exists());
    assert!(ssh_public.exists());
    assert!(!initrd_private.exists());
    fs::write(&sentinel, "preserve me\n").unwrap();

    let initrd_add = run_semble(
        tempdir.path(),
        &["host", "keys", "initrd-ssh", "add", "atlas", "--force"],
    )
    .output()
    .unwrap();
    assert!(
        initrd_add.status.success(),
        "initrd add failed: {}",
        String::from_utf8_lossy(&initrd_add.stderr)
    );

    assert!(ssh_private.exists());
    assert!(ssh_public.exists());
    assert!(initrd_private.exists());
    assert!(initrd_public.exists());
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "preserve me\n");

    let ssh_private_before = fs::read(&ssh_private).unwrap();
    let ssh_public_before = fs::read_to_string(&ssh_public).unwrap();
    let initrd_private_before = fs::read(&initrd_private).unwrap();
    let initrd_public_before = fs::read_to_string(&initrd_public).unwrap();

    thread::sleep(Duration::from_millis(10));
    let ssh_force = run_semble(
        tempdir.path(),
        &["host", "keys", "ssh", "add", "atlas", "--force", "--skip-reencrypt"],
    )
    .output()
    .unwrap();
    assert!(
        ssh_force.status.success(),
        "ssh force add failed: {}",
        String::from_utf8_lossy(&ssh_force.stderr)
    );

    assert_ne!(fs::read(&ssh_private).unwrap(), ssh_private_before);
    assert_ne!(fs::read_to_string(&ssh_public).unwrap(), ssh_public_before);
    assert_eq!(fs::read(&initrd_private).unwrap(), initrd_private_before);
    assert_eq!(fs::read_to_string(&initrd_public).unwrap(), initrd_public_before);
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "preserve me\n");

    let ssh_private_after_ssh_force = fs::read(&ssh_private).unwrap();
    let ssh_public_after_ssh_force = fs::read_to_string(&ssh_public).unwrap();
    let initrd_private_after_ssh_force = fs::read(&initrd_private).unwrap();
    let initrd_public_after_ssh_force = fs::read_to_string(&initrd_public).unwrap();

    thread::sleep(Duration::from_millis(10));
    let initrd_force = run_semble(
        tempdir.path(),
        &["host", "keys", "initrd-ssh", "add", "atlas", "--force"],
    )
    .output()
    .unwrap();
    assert!(
        initrd_force.status.success(),
        "initrd force add failed: {}",
        String::from_utf8_lossy(&initrd_force.stderr)
    );

    assert_eq!(fs::read(&ssh_private).unwrap(), ssh_private_after_ssh_force);
    assert_eq!(
        fs::read_to_string(&ssh_public).unwrap(),
        ssh_public_after_ssh_force
    );
    assert_ne!(fs::read(&initrd_private).unwrap(), initrd_private_after_ssh_force);
    assert_ne!(
        fs::read_to_string(&initrd_public).unwrap(),
        initrd_public_after_ssh_force
    );
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "preserve me\n");

    let initrd_delete = run_semble(
        tempdir.path(),
        &["host", "keys", "initrd-ssh", "delete", "atlas", "--yes"],
    )
    .output()
    .unwrap();
    assert!(
        initrd_delete.status.success(),
        "initrd delete failed: {}",
        String::from_utf8_lossy(&initrd_delete.stderr)
    );

    assert!(keys_dir.exists());
    assert!(ssh_private.exists());
    assert!(ssh_public.exists());
    assert!(!initrd_private.exists());
    assert!(!initrd_public.exists());
    assert_eq!(fs::read_to_string(&sentinel).unwrap(), "preserve me\n");
}
