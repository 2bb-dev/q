use anyhow::Result;
use qcli_platform::paths::queue_path;

pub fn run() -> Result<()> {
    let path = queue_path()?;
    qcli_tui::run(&path)?;
    Ok(())
}
