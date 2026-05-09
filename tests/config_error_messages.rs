use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn run_semble(root: &std::path::Path, args: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_semble"));
    cmd.current_dir(root).args(args);
    cmd
}

// Verify that a missing required field surfaces the field name in stderr.
#[test]
fn parse_error_names_missing_field() {
    let tempdir = tempdir().unwrap();
    fs::write(
        tempdir.path().join("semble.toml"),
        r#"[paths]
hosts_dir = "hosts"
host_template_dir = "hosts/templates"
ssh_host_keys_dir = "ssh_host_keys"
initrd_ssh_host_keys_dir = "initrd_ssh_host_keys"
sops_config_file = ".sops.yaml"
network_secrets_file = "secrets/network.yaml"
"#,
    )
    .unwrap();

    let output = run_semble(tempdir.path(), &["host", "create", "test-host"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("missing field"),
        "expected 'missing field' in stderr, got: {stderr}"
    );
}
