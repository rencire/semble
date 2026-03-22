use crate::error::fail;
use anyhow::Result;
use std::io::{self, IsTerminal, Write};

pub fn require_delete_confirmation(
    hostname: &str,
    command_name: &str,
    impact_lines: &[String],
    assume_yes: bool,
) -> Result<()> {
    if assume_yes {
        return Ok(());
    }

    if !std::io::stdin().is_terminal() {
        return fail(format!(
            "refusing to run `{command_name}` without confirmation in a non-interactive shell; rerun with `--yes` if you are sure"
        ));
    }

    print!("{}", delete_confirmation_text(hostname, command_name, impact_lines));
    print!("Type the hostname `{hostname}` to confirm: ");
    io::stdout().flush()?;

    let mut typed = String::new();
    io::stdin().read_line(&mut typed)?;
    if typed.trim() != hostname {
        return fail("aborted: confirmation did not match hostname");
    }

    Ok(())
}

fn delete_confirmation_text(hostname: &str, command_name: &str, impact_lines: &[String]) -> String {
    let mut output = String::new();
    output.push_str("WARNING: destructive operation requested.\n");
    output.push_str(&format!("Command: {command_name} {hostname}\n"));
    output.push_str("This will:\n");
    for (index, line) in impact_lines.iter().enumerate() {
        output.push_str(&format!("  {}. {line}\n", index + 1));
    }
    output.push('\n');
    output
}

#[cfg(test)]
mod tests {
    use super::delete_confirmation_text;

    #[test]
    fn delete_confirmation_uses_numbered_list() {
        let output = delete_confirmation_text(
            "blah",
            "delete",
            &[
                String::from("delete host scaffold hosts/blah (including facter.json)"),
                String::from("delete SSH host keys under ssh_host_keys/blah"),
                String::from("remove `blah` recipient from .sops.yaml"),
            ],
        );

        assert!(output.contains("This will:\n"));
        assert!(output.contains("  1. delete host scaffold hosts/blah (including facter.json)\n"));
        assert!(output.contains("  2. delete SSH host keys under ssh_host_keys/blah\n"));
        assert!(output.contains("  3. remove `blah` recipient from .sops.yaml\n"));
        assert!(!output.contains("  - delete host scaffold"));
    }
}
