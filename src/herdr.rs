//! Thin wrapper around the Herdr CLI (`HERDR_BIN_PATH`).

use anyhow::{anyhow, Context, Result};
use std::process::{Command, Output};

pub fn herdr_bin() -> String {
    std::env::var("HERDR_BIN_PATH").unwrap_or_else(|_| "herdr".to_string())
}

pub fn run_herdr(args: &[&str]) -> Result<Output> {
    let bin = herdr_bin();
    Command::new(&bin)
        .args(args)
        .output()
        .with_context(|| format!("spawn `{bin} {}`", args.join(" ")))
}

pub fn run_herdr_ok(args: &[&str]) -> Result<String> {
    let out = run_herdr(args)?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        return Err(anyhow!(
            "herdr {} failed ({}):\n{stdout}\n{stderr}",
            args.join(" "),
            out.status
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn run_capture(bin: &str, args: &[&str]) -> Result<Output> {
    Command::new(bin)
        .args(args)
        .output()
        .with_context(|| format!("spawn `{bin} {}`", args.join(" ")))
}

pub fn which_exists(name: &str) -> bool {
    #[cfg(windows)]
    {
        Command::new("where")
            .arg(name)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        Command::new("sh")
            .args(["-c", &format!("command -v {name} >/dev/null 2>&1")])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}
