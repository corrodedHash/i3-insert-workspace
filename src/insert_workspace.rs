/// Insert workspace before or after pivot
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum InsertionDestination {
    After { pivot: String },
    Before { pivot: String },
}

impl InsertionDestination {
    pub fn new(pivot: String, before: bool) -> Self {
        if before {
            Self::Before { pivot }
        } else {
            Self::After { pivot }
        }
    }
    fn pivot(&self) -> &str {
        match &self {
            Self::After { pivot } | Self::Before { pivot } => pivot,
        }
    }
}

/// Errors for `insert_workspace`
#[derive(Debug)]
pub enum InsertionError {
    NoPivotWorkspace(String),
    CommandError(i3ipc::MessageError),
}

impl From<i3ipc::MessageError> for InsertionError {
    fn from(e: i3ipc::MessageError) -> Self {
        Self::CommandError(e)
    }
}

impl std::error::Error for InsertionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CommandError(e) => Some(e),
            Self::NoPivotWorkspace(_) => None,
        }
    }
}

impl std::fmt::Display for InsertionError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            InsertionError::NoPivotWorkspace(pivot) => {
                write!(f, "Could not find workspace \"{}\"", pivot)
            }
            InsertionError::CommandError(error_message) => {
                write!(f, "i3 IPC error message: {}", error_message)
            }
        }
    }
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

    let creation_command = match container {
        Some(container_id) => format!("[con_id={container_id}] move container to workspace {name}"),
        None => format!("workspace \"{name}\""),
    };

    conn.run_command(&format!(
        "{creation_command}; move workspace to output {output}; {}",
        rename_commands.join("")
    ))?;
    Ok(())
}