use anyhow::Result;

pub fn run() -> Result<()> {
    let dir = super::active_workspace_dir()?;
    q_tui::run(&dir)?;
    Ok(())
}
