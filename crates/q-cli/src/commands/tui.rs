use anyhow::Result;

pub fn run(workspace: Option<&str>) -> Result<()> {
    let dir = super::resolve_workspace_dir(workspace)?;
    q_tui::run(&dir)?;
    Ok(())
}
