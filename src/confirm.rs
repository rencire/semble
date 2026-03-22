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

    println!("WARNING: destructive operation requested.");
    println!("Command: {command_name} {hostname}");
    println!("This will:");
    for line in impact_lines {
        println!("  - {line}");
    }
    println!();
    print!("Type the hostname `{hostname}` to confirm: ");
    io::stdout().flush()?;

    let mut typed = String::new();
    io::stdin().read_line(&mut typed)?;
    if typed.trim() != hostname {
        return fail("aborted: confirmation did not match hostname");
    }

    Ok(())
}
