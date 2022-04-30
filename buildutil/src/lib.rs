use std::process::{self, Command};

use anyhow::{self, Error};

fn display_output(output: process::Output) -> String {
    format!(
        "Process stdout:\n\n{}\nProcess stderr:\n\n{}\n",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

pub fn run_and_check(cmd: &mut Command) -> anyhow::Result<()> {
    let output = cmd
        .output()
        .map_err(|e| Error::new(e).context(format!("{:?}", cmd)))?;
    anyhow::ensure!(
        output.status.success(),
        "Command failed: {:?}\n{}",
        cmd,
        display_output(output)
    );
    Ok(())
}
