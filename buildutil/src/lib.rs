use std::process::{self, Command};

use eyre::WrapErr;

fn display_output(output: process::Output) -> String {
    format!(
        "Process stdout:\n\n{}\nProcess stderr:\n\n{}\n",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

pub fn run_and_check(cmd: &mut Command) -> eyre::Result<()> {
    let output = cmd.output().wrap_err_with(|| format!("{:?}", cmd))?;
    eyre::ensure!(output.status.success(), "{}", display_output(output));
    Ok(())
}
