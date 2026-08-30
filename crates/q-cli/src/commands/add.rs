use std::io::Read;
use std::path::Path;

use anyhow::{bail, Result};
use q_core::Prompt;
use q_platform::external_document::{absolute_path, read_utf8};

use super::with_workspace_mut;

pub fn run(
    text: Option<String>,
    literal: bool,
    pin: bool,
    tab: &str,
    workspace: Option<&str>,
) -> Result<()> {
    let mut prompt = match text {
        Some(text) if !literal && is_markdown_path(&text) => {
            let path = absolute_path(Path::new(&text))?;
            let _ = read_utf8(&path)?;
            Prompt::from_external_markdown(path)?
        }
        Some(text) => Prompt::new(text)?,
        None => {
            let mut text = String::new();
            std::io::stdin().read_to_string(&mut text)?;
            Prompt::new(text)?
        }
    };
    prompt.set_pinned(pin);
    prompt.created_by = q_platform::github::cached_login().ok().flatten();

    if prompt.external_markdown_path().is_some() {
        let dir = super::resolve_workspace_dir(workspace)?;
        if q_platform::git::is_repo(&dir) {
            bail!(
                "team workspaces accept inline prompts only; pass --text to add the literal text"
            );
        }
    }

    let id = with_workspace_mut(workspace, |workspace| {
        let tab_id = workspace.resolve_tab(tab)?;
        Ok(workspace.add_prompt(tab_id, prompt)?)
    })?;
    println!("added {id}");
    Ok(())
}

fn is_markdown_path(value: &str) -> bool {
    Path::new(value)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
        })
}

#[cfg(test)]
#[path = "../../tests/unit/commands/add.rs"]
mod tests;
