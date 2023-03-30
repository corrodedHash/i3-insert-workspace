use thiserror::Error;

use crate::util::InsertionDestination;

/// Errors for `insert_workspace`
#[derive(Debug, Error)]
pub enum InsertionError {
    #[error("Could not find workspace \"{0}\"")]
    NoPivotWorkspace(String),
    #[error("i3 IPC error message: \"{0}\"")]
    CommandError(
        #[from]
        #[source]
        i3ipc::MessageError,
    ),
}

/// Insert a new workspace at the given location
#[allow(clippy::indexing_slicing)]
pub fn insert_workspace(
    conn: &mut i3ipc::I3Connection,
    insertion_marker: &InsertionDestination,
    name: &str,
    container: Option<i64>,
) -> Result<(), InsertionError> {
    let t = conn.get_workspaces()?;

    let pivot_id = t
        .workspaces
        .iter()
        .position(|x| x.name == insertion_marker.pivot())
        .ok_or_else(|| InsertionError::NoPivotWorkspace(insertion_marker.pivot().to_owned()))?;

    let output = &t.workspaces[pivot_id].output;

    let stop_id = t.workspaces[pivot_id..]
        .iter()
        .position(|x| &x.output != output)
        .map_or(t.workspaces.len(), |x| x + pivot_id);

    let start_id = match insertion_marker {
        InsertionDestination::After { .. } => pivot_id + 1,
        InsertionDestination::Before { .. } => pivot_id,
    };

    // Renaming moves the workspace to the end of list of workspaces in the output
    let rename_commands: Vec<_> = t.workspaces[start_id..stop_id]
        .iter()
        .filter(|x| x.name != name)
        .map(|x| format!("rename workspace \"{0}\" to \"{0}\";", x.name.clone()))
        .collect();

    let creation_command = if let Some(container_id) = container {
        format!("[con_id={container_id}] move container to workspace {name}")
    } else {
        format!("workspace \"{name}\"")
    };

    conn.run_command(&format!(
        "{creation_command}; move workspace to output {output}; {}",
        rename_commands.join("")
    ))?;
    Ok(())
}
