use anyhow::Result;

use super::with_workspace_mut;

pub fn run(id: &str, pinned: bool) -> Result<()> {
    let resolved = with_workspace_mut(|workspace| {
        let resolved = workspace.resolve_prompt(id)?;
        workspace.set_prompt_pinned(resolved, pinned)?;
        Ok(resolved)
    })?;
    println!("{} {resolved}", if pinned { "pinned" } else { "unpinned" });
    Ok(())
}
