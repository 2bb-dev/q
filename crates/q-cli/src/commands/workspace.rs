use std::io::{BufRead, Write};

use anyhow::{bail, Result};
use q_platform::paths;
use serde::Serialize;

use super::{available_names, ensure_workspaces, find_by_name, read_state, set_active_workspace};

#[derive(Serialize)]
struct WorkspaceOutput {
    id: String,
    name: String,
    active: bool,
}

pub fn list(json: bool) -> Result<()> {
    let workspaces = ensure_workspaces()?;
    let active_dir = super::resolve_workspace_dir(None)?;
    let outputs: Vec<WorkspaceOutput> = workspaces
        .iter()
        .map(|(dir, meta)| WorkspaceOutput {
            id: meta.id.to_string(),
            name: meta.name.clone(),
            active: *dir == active_dir,
        })
        .collect();
    if json {
        println!("{}", serde_json::to_string_pretty(&outputs)?);
        return Ok(());
    }
    for output in outputs {
        let marker = if output.active { "*" } else { " " };
        println!("{marker} {}", output.name);
    }
    Ok(())
}

pub fn create(name: &str) -> Result<()> {
    let name = validate_name(name)?;
    let workspaces = ensure_workspaces()?;
    if find_by_name(&workspaces, &name).is_some() {
        bail!("workspace name already exists: {name}");
    }
    let root = paths::workspaces_dir()?;
    let dir = q_core::storage::init_dir(&root, &name)?;
    set_active_workspace(&dir)?;
    println!("created workspace {name}");
    Ok(())
}

pub fn rename(name: &str, new_name: &str) -> Result<()> {
    let new_name = validate_name(new_name)?;
    let workspaces = ensure_workspaces()?;
    let Some((dir, meta)) = find_by_name(&workspaces, name) else {
        bail!(
            "workspace not found: {} (available: {})",
            name.trim(),
            available_names(&workspaces)
        );
    };
    if let Some((_, existing)) = find_by_name(&workspaces, &new_name) {
        if existing.id != meta.id {
            bail!("workspace name already exists: {new_name}");
        }
    }
    q_core::storage::rename_dir(dir, &new_name)?;
    println!("renamed workspace {} to {new_name}", meta.name);
    Ok(())
}

pub fn switch(name: &str) -> Result<()> {
    let workspaces = ensure_workspaces()?;
    let Some((dir, meta)) = find_by_name(&workspaces, name) else {
        bail!(
            "workspace not found: {} (available: {})",
            name.trim(),
            available_names(&workspaces)
        );
    };
    set_active_workspace(dir)?;
    println!("switched to workspace {}", meta.name);
    Ok(())
}

pub fn delete(name: &str, yes: bool) -> Result<()> {
    let workspaces = ensure_workspaces()?;
    let Some((dir, meta)) = find_by_name(&workspaces, name) else {
        bail!(
            "workspace not found: {} (available: {})",
            name.trim(),
            available_names(&workspaces)
        );
    };
    if workspaces.len() == 1 {
        bail!("cannot delete the last workspace");
    }
    if !yes && !confirm(&format!("delete workspace '{}'?", meta.name))? {
        println!("aborted");
        return Ok(());
    }
    let dir = dir.clone();
    let deleted_name = meta.name.clone();
    std::fs::remove_dir_all(&dir)?;

    let deleted_id = dir.file_name().and_then(|n| n.to_str()).map(str::to_string);
    if read_state()?.active_workspace == deleted_id {
        let remaining = ensure_workspaces()?;
        set_active_workspace(&remaining[0].0)?;
    }
    println!("deleted workspace {deleted_name}");
    Ok(())
}

fn validate_name(name: &str) -> Result<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        bail!("workspace name is empty");
    }
    Ok(trimmed.to_string())
}

fn confirm(question: &str) -> Result<bool> {
    print!("{question} [y/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes"))
}
