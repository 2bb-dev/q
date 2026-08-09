use std::io::Read;

use anyhow::Result;
use qcli_core::Prompt;

use super::with_workspace_mut;

pub fn run(text: Option<String>, pin: bool, tab: Option<String>) -> Result<()> {
    let text = match text {
        Some(t) => t,
        None => {
            let mut buf = String::new();
            std::io::stdin().read_to_string(&mut buf)?;
            buf
        }
    };
    let id = with_workspace_mut(|workspace| {
        let tab_id = workspace.resolve_context_tab(tab.as_deref())?;
        let mut prompt = Prompt::new(text)?;
        prompt.pinned = pin;
        Ok(workspace.add_prompt(tab_id, prompt)?)
    })?;
    println!("added {id}");
    Ok(())
}
