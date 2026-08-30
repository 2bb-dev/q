use anyhow::Result;

use super::with_workspace_mut;

pub fn run(id: &str) -> Result<()> {
    let removed = with_workspace_mut(|workspace| {
        let resolved = workspace.resolve_prompt(id)?;
        Ok(workspace.remove_prompt(resolved)?)
    })?;
    println!("removed {}", removed.id);
    Ok(())
}
