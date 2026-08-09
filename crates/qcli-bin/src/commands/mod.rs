pub mod add;
pub mod copy;
pub mod list;
pub mod pin;
pub mod pop;
pub mod tui;

use anyhow::Result;
use qcli_core::Workspace;
use qcli_platform::lock::FileLock;
use qcli_platform::paths;

pub(crate) fn with_workspace<T>(action: impl FnOnce(&Workspace) -> Result<T>) -> Result<T> {
    let path = paths::queue_path()?;
    let mut lock = FileLock::open(&path.with_extension("lock"))?;
    let _guard = lock.write()?;
    let workspace = qcli_core::storage::load(&path)?;
    action(&workspace)
}

pub(crate) fn with_workspace_mut<T>(action: impl FnOnce(&mut Workspace) -> Result<T>) -> Result<T> {
    let path = paths::queue_path()?;
    let mut lock = FileLock::open(&path.with_extension("lock"))?;
    let _guard = lock.write()?;
    let mut workspace = qcli_core::storage::load(&path)?;
    let result = action(&mut workspace)?;
    qcli_core::storage::save(&path, &workspace)?;
    Ok(result)
}
