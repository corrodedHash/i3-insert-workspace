use i3ipc::reply::Node;
use thiserror::Error;

use crate::util::InsertionDestination;

/// Errors for `insert_workspace`
#[derive(Debug, Error)]
pub enum InsertionError {
    #[error("Could not find workspace \"{0}\"")]
    NoPivotWorkspace(String),
    #[error("i3 IPC connection error: \"{0}\"")]
    ConnectionError(
        #[from]
        #[source]
        i3ipc::MessageError,
    ),
    #[error("i3 IPC command error: \"{0}\"")]
    CommandError(String),
}

/// Finds the output containing the workspace named `workspace_name`
///
/// Returns the `Output` node, and the index of the workspace in this output node
fn find_workspaces_output<'a>(
    root_node: &'a Node,
    workspace_name: &'_ str,
) -> Option<(&'a Node, usize)> {
    assert_eq!(root_node.nodetype, i3ipc::reply::NodeType::Root);

    root_node.nodes.iter().find_map(|output_node| {
        assert_eq!(output_node.nodetype, i3ipc::reply::NodeType::Output);
        output_node
            .nodes
            .iter()
            .position(|x| {
                assert_eq!(x.nodetype, i3ipc::reply::NodeType::Workspace);
                x.name.as_ref().is_some_and(|wn| wn == workspace_name)
            })
            .map(|workspace_index| (output_node, workspace_index))
    })
}

fn get_child_node_by_id(node: &Node, id: i64) -> Option<&Node> {
    node.nodes.iter().find(|x| x.id == id)
}

fn get_floating_child_node_by_id(node: &Node, id: i64) -> Option<&Node> {
    node.floating_nodes.iter().find(|x| x.id == id)
}

/// Check if `ws` is focused, or one of its children is focused
fn is_focused(ws: &Node) -> bool {
    if ws.focused {
        return true;
    }

    // Focus not only holds the path to the currently focused node, but also the history of older focused nodes.
    // Thus, we iterate through it to check if the most recently focused node is currently focused.

    ws.focus
        .first()
        .and_then(|next_focus_id| {
            get_child_node_by_id(ws, *next_focus_id)
                .or_else(|| get_floating_child_node_by_id(ws, *next_focus_id))
        })
        .is_some_and(is_focused)
}

fn move_workspace_to_end(source: &Node, container: Option<i64>) -> Vec<String> {
    let dummy_name = format!("dummy_workspace_{:#?}", std::ptr::addr_of!(source));

    let mut movings = source
        .nodes
        .iter()
        .chain(source.floating_nodes.iter())
        .filter(|x| container != Some(x.id))
        .map(|container| {
            format!(
                "[con_id={conid}] move container to workspace {dummy_name}",
                conid = container.id
            )
        })
        .collect::<Vec<_>>();
    // If we move the container somewhere, we want to stay in the current workspace
    // But this workspace should be shifted none the less
    if container.is_some() && is_focused(source) {
        movings.push(format!("workspace {dummy_name}"));
    }
    if !movings.is_empty() {
        #[allow(clippy::expect_used)]
        movings.push(format!(
            "rename workspace {dummy_name} to {conname}",
            conname = source.name.as_ref().expect("Workspace did not have a name")
        ));
    }
    movings
}

/// Insert a new workspace at the given location
pub fn insert_workspace(
    conn: &mut i3ipc::I3Connection,
    insertion_marker: &InsertionDestination,
    name: &str,
    container: Option<i64>,
) -> Result<(), InsertionError> {
    let root_node = conn.get_tree()?;
    let (output_node, workspace_id) = find_workspaces_output(&root_node, insertion_marker.pivot())
        .ok_or_else(|| InsertionError::NoPivotWorkspace(insertion_marker.pivot().to_owned()))?;

    let first_moved_workspace = match insertion_marker {
        InsertionDestination::After { .. } => workspace_id + 1,
        InsertionDestination::Before { .. } => workspace_id,
    };

    // Move to workspace {name}
    // Move everything from first-to-move ($a) to new dummy workspace
    // Rename dummy workspace to $a after $a it has been emptied

    let initial_workspace_command = container.map_or_else(
        || format!("workspace {name}"),
        |conid| format!("[con_id={conid}] move container to workspace {name}"),
    );
    let mut commands = vec![initial_workspace_command];

    let new_commands = output_node
        .nodes
        .iter()
        .skip(first_moved_workspace)
        .flat_map(|x| move_workspace_to_end(x, container).into_iter());
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
