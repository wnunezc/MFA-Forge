use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

pub fn proxy_agent_session() -> Result<()> {
    proxy_local_bridge("mfa-forge-agent", "agent session")
}

pub fn proxy_mcp_server() -> Result<()> {
    proxy_local_bridge("mfa-forge-mcp", "MCP server")
}

fn proxy_local_bridge(executable_stem: &str, label: &str) -> Result<()> {
    #[cfg(not(target_os = "windows"))]
    bail!(
        "The local {label} bridge is only supported on Windows in this MFA-Forge line. Use the human CLI on this platform or move the agent/MCP flow to Windows."
    );

    let current_exe =
        std::env::current_exe().context("unable to resolve the CLI executable path")?;
    let executable_name = if cfg!(windows) {
        format!("{executable_stem}.exe")
    } else {
        executable_stem.to_owned()
    };
    let agent_path = current_exe
        .parent()
        .context("unable to resolve the CLI executable directory")?
        .join(&executable_name);

    if !agent_path.exists() {
        bail!(
            "{label} binary not found at {}. Reinstall MFA-Forge to include the local bridge.",
            agent_path.display(),
        );
    }

    let status = Command::new(&agent_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("unable to launch {}", agent_path.display()))?;

    if status.success() {
        return Ok(());
    }

    std::process::exit(status.code().unwrap_or(1));
}
