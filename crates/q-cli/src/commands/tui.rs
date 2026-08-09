use anyhow::Result;
use q_platform::paths::queue_path;

pub fn run() -> Result<()> {
    let path = queue_path()?;
    q_tui::run(&path)?;
    Ok(())
}
