use std::fs;
use std::path::Path;
use std::process::Command;
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
initrd_ssh_host_keys_dir = "initrd_ssh_host_keys"
luks_root_keys_dir = "luks_root_keys"
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

fn create_template(root: &Path, name: &str, body: &str) {
    let dir = root.join("hosts/templates").join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("default.nix"), body).unwrap();
    fs::write(dir.join("facter.json"), "{}\n").unwrap();
}

fn run_semble(root: &Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_semble"));
    cmd.current_dir(root).args(args);
    cmd
}

// Verify the fallback template is used when `--template` is omitted.
#[test]
fn host_create_uses_default_template_when_omitted() {
    let tempdir = tempdir().unwrap();
    write_repo_config(tempdir.path());
    write_minimal_repo_files(tempdir.path());
    create_template(tempdir.path(), "default", "host = TEMPLATE_HOSTNAME;\n");

    let status = run_semble(
        tempdir.path(),
        &["host", "create", "atlas", "--skip-reencrypt"],
    )
        .status()
        .unwrap();
    assert!(status.success());

    let host_file = tempdir.path().join("hosts/atlas/default.nix");
    assert_eq!(fs::read_to_string(host_file).unwrap(), "host = atlas;\n");
}

// Verify a named template overrides the default template at create time.
#[test]
fn host_create_uses_named_template_when_requested() {
    let tempdir = tempdir().unwrap();
    write_repo_config(tempdir.path());
    write_minimal_repo_files(tempdir.path());
    create_template(tempdir.path(), "default", "host = TEMPLATE_HOSTNAME;\n");
    create_template(tempdir.path(), "microvm", "template = TEMPLATE_HOSTNAME;\n");

    let status = run_semble(
        tempdir.path(),
        &[
            "host",
            "create",
            "atlas",
            "--template",
            "microvm",
            "--skip-reencrypt",
        ],
    )
    .status()
    .unwrap();
    assert!(status.success());

    let host_file = tempdir.path().join("hosts/atlas/default.nix");
    assert_eq!(fs::read_to_string(host_file).unwrap(), "template = atlas;\n");
}

// Verify missing templates fail before any host directory is created.
#[test]
fn host_create_rejects_missing_named_template_before_copying() {
    let tempdir = tempdir().unwrap();
    write_repo_config(tempdir.path());
    write_minimal_repo_files(tempdir.path());
    create_template(tempdir.path(), "default", "host = TEMPLATE_HOSTNAME;\n");

    let output = run_semble(
        tempdir.path(),
        &[
            "host",
            "create",
            "atlas",
            "--template",
            "missing",
            "--skip-reencrypt",
        ],
    )
    .output()
    .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("host template does not exist"));
    assert!(!tempdir.path().join("hosts/atlas").exists());
}
