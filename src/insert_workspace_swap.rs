use i3ipc::reply::Node;
use thiserror::Error;

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
#[derive(Debug, Error)]
pub enum InsertionError {
    #[error("Could not find workspace \"{0}\"")]
    NoPivotWorkspace(String),
    // #[error("Unnamed workspace \"{0}\"")]
    // UnnamedWorkspace(String),
    #[error("i3 IPC connection error: \"{0}\"")]
    ConnectionError(
        #[from]
        #[source]
        i3ipc::MessageError,
    ),
    #[error("i3 IPC command error: \"{0}\"")]
    CommandError(String),
}

fn find_workspaces_output<'a>(
    root_node: &'a Node,
    workspace_name: &'_ str,
) -> Option<(&'a Node, usize)> {
    assert!(root_node.nodetype == i3ipc::reply::NodeType::Root);

    for output_node in &root_node.nodes {
        assert!(output_node.nodetype == i3ipc::reply::NodeType::Output);
        let candidate = output_node.nodes.iter().position(|x| {
            assert!(x.nodetype == i3ipc::reply::NodeType::Workspace);
            if let Some(wn) = &x.name {
                wn == workspace_name
            } else {
                false
            }
        });
        if let Some(workspace_id) = candidate {
            return Some((output_node, workspace_id));
        }
    }
    None
}

fn is_focused(ws: &Node) -> bool {
    if ws.focused {
        return true;
    }
    if let Some(next_focus_id) = ws.focus.first() {
        if let Some(next_focus) = ws.nodes.iter().find(|x| (**x).id == *next_focus_id) {
            return is_focused(next_focus);
        }
    }
    false
}

/// Insert a new workspace at the given location
pub fn insert_workspace_swap(
    conn: &mut i3ipc::I3Connection,
    insertion_marker: &InsertionDestination,
    name: &str,
    container: Option<i64>,
) -> Result<(), InsertionError> {
    let root_node = conn.get_tree()?;
    let (output_node, workspace_id) = find_workspaces_output(&root_node, insertion_marker.pivot())
        .ok_or_else(|| InsertionError::NoPivotWorkspace(insertion_marker.pivot().to_owned()))?;

    let first_moved_workspace = match insertion_marker {
        InsertionDestination::After { pivot: _ } => workspace_id + 1,
        InsertionDestination::Before { pivot: _ } => workspace_id,
    };

    // Move to workspace {name}
    // Move everything from first-to-move ($a) to new dummy workspace
    // Rename dummy workspace to $a after $a it has been emptied

    let mut commands = vec![if let Some(conid) = container {
        format!("[con_id={conid}] move container to workspace {name}")
    } else {
        format!("workspace {name}")
    }];
    let dummy_name = format!("dummy_workspace_{:#?}", std::ptr::addr_of!(root_node));

    let empty_workspace = |source: &Node| {
        let mut movings = source
            .nodes
            .iter()
            .filter(|x| container.map_or(true, |cid| x.id != cid))
            .map(|container| {
                format!(
                    "[con_id={conid}] move container to workspace {dummy_name}",
                    conid = container.id
                )
            })
            .collect::<Vec<_>>();
        // If we move the container somewhere, we want to stay in the current workspace
        // But this workspace should be shifted none the less
        if container.is_some() && (is_focused(source)) {
            movings.push(format!("workspace {dummy_name}"));
        }
        if !movings.is_empty() {
            movings.push(format!(
                "rename workspace {dummy_name} to {conname}",
                conname = source.name.as_ref().expect("Workspace did not have a name")
            ));
        }
        movings
    };
    let new_commands = output_node
        .nodes
        .iter()
        .skip(first_moved_workspace)
        .flat_map(|x| empty_workspace(x).into_iter());
    commands.extend(new_commands);
    let joined_commands = commands.join("; ");
    let replies = &conn.run_command(&joined_commands)?;
    let errored_command = replies.outcomes.iter().find(|x| !x.success);
    if let Some(ec) = errored_command {
        return Err(InsertionError::CommandError(
            ec.error
                .clone()
                .unwrap_or_else(|| "No error message, but errored".to_string()),
        ));
    }
    Ok(())
}
