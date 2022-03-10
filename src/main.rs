#![forbid(unsafe_code)]
#![warn(
    clippy::pedantic,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout,
)]
#![warn(clippy::cargo)]

//! Workspace enhancement for the i3 window manager
//! Insert a named workspace before or after another workspace
use clap::Parser;
mod docker_name;

/// Simple program to insert a named workspace before or after another workspace
#[derive(clap::Parser, Debug)]
struct Args {
    /// Workspace after or before which the new workspace is inserted.
    /// default: current
    #[clap(short, long)]
    pivot: Option<String>,

    /// When using pivot, should insert before or after pivot.
    /// When no pivot supplied, using focused workspace
    #[clap(short, long)]
    before: bool,

    /// Name of the new workspace
    #[clap(short, long)]
    name: Option<String>,

    /// Move container to new workspace
    /// Either provide container id, or `focused` for focused one
    #[clap(short, long)]
    container_id: Option<String>,
}

/// Insert workspace before or after pivot
#[derive(Clone, PartialEq, Eq, Debug)]
enum InsertionArg {
    After { pivot: String },
    Before { pivot: String },
}

/// Errors for `insert_workspace`
#[derive(Debug)]
enum InsertionError {
    NoPivotWorkspace,
    CommandError(i3ipc::MessageError),
}

impl std::error::Error for InsertionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CommandError(e) => Some(e),
            Self::NoPivotWorkspace => None,
        }
    }
}

impl std::fmt::Display for InsertionError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            InsertionError::NoPivotWorkspace => write!(f, "Could not find workspace"),
            InsertionError::CommandError(error_message) => {
                write!(f, "i3 IPC error message: {}", error_message)
            }
        }
    }
}

#[allow(clippy::indexing_slicing)]
fn insert_workspace(
    conn: &mut i3ipc::I3Connection,
    insertion_marker: &InsertionArg,
    name: &str,
    container: Option<i64>,
) -> Result<(), InsertionError> {
    let t = conn
        .get_workspaces()
        .map_err(InsertionError::CommandError)?;

    let (mut start_id, stop_id, output) = match &insertion_marker {
        InsertionArg::After { pivot } | InsertionArg::Before { pivot } => {
            let start_id = t
                .workspaces
                .iter()
                .position(|x| &x.name == pivot)
                .ok_or(InsertionError::NoPivotWorkspace)?;
            let output = t.workspaces[start_id].output.clone();
            let stop_id = t.workspaces[start_id..]
                .iter()
                .position(|x| x.output != output)
                .map_or(t.workspaces.len(), |x| x + start_id);
            (start_id, stop_id, output)
        }
    };
    if let InsertionArg::After { .. } = insertion_marker {
        start_id += 1;
    }
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
    ))
    .map_err(InsertionError::CommandError)
    .map(|_| ())
}

struct I3Focus {
    output: String,
    workspace: String,
    container: i64,
}

fn focused(conn: &mut i3ipc::I3Connection) -> Result<I3Focus, String> {
    let t = conn
        .get_tree()
        .map_err(|e| format!("Could not get tree: {}", e))?;

    let mut current = &t;
    let mut output = None;
    let mut workspace = None;
    while !current.focused {
        let next_focus_item = *current
            .focus
            .first()
            .ok_or_else(|| "Focus chain broken".to_owned())?;
        current = current
            .nodes
            .iter()
            .find(|x| x.id == next_focus_item)
            .ok_or_else(|| "Focus array in i3-msg reply wrong".to_owned())?;
        if current.nodetype == i3ipc::reply::NodeType::Output {
            output = Some(
                current
                    .name
                    .clone()
                    .ok_or_else(|| "output has no name".to_owned())?,
            );
        }
        if current.nodetype == i3ipc::reply::NodeType::Workspace {
            workspace = Some(
                current
                    .name
                    .clone()
                    .ok_or_else(|| "output has no name".to_owned())?,
            );
        }
    }
    Ok(I3Focus {
        output: output.ok_or_else(|| "Focused display not found".to_owned())?,
        workspace: workspace.ok_or_else(|| "Focused workspace not found".to_owned())?,
        container: current.id,
    })
}

fn generate_new_workspace_name(conn: &mut i3ipc::I3Connection) -> Result<String, String> {
    let workspace_names = conn
        .get_workspaces()
        .map_err(|x| format!("{}", x))?
        .workspaces
        .into_iter()
        .map(|x| x.name)
        .collect::<Vec<_>>();
    loop {
        let new_name = docker_name::random_name();
        if !workspace_names.iter().any(|x| x == &new_name) {
            return Ok(new_name);
        }
    }
}

fn main() {
    let args = Args::parse();

    let mut conn = match i3ipc::I3Connection::connect() {
        Ok(conn) => conn,
        Err(e) => {
            eprintln!("Could not establish connection to i3 IPC: {}", e);
            return;
        }
    };

    let focus = match focused(&mut conn) {
        Ok(res) => res,
        Err(e) => {
            eprintln!("{}", e);
            return;
        }
    };

    let pivot = if let Some(pivot) = args.pivot {
        pivot
    } else {
        focus.workspace
    };

    let x = if args.before {
        InsertionArg::Before {
            pivot: pivot.clone(),
        }
    } else {
        InsertionArg::After {
            pivot: pivot.clone(),
        }
    };

    let name = if let Some(name) = args.name {
        name
    } else {
        match generate_new_workspace_name(&mut conn) {
            Ok(name) => name,
            Err(e) => {
                eprintln!("{e}");
                return;
            }
        }
    };

    let container_id = if let Some(container_id) = args.container_id {
        if container_id.to_ascii_lowercase() == "current" {
            Some(focus.container)
        } else if let Ok(parsed_container_id) = container_id.parse::<i64>() {
            Some(parsed_container_id)
        } else {
            eprintln!("Could not parse container id {}", container_id);
            return;
        }
    } else {
        None
    };

    match insert_workspace(&mut conn, &x, &name, container_id) {
        Ok(_) => (),
        Err(e) => match e {
            InsertionError::NoPivotWorkspace => eprintln!("Could not find workspace \"{}\"", pivot),
            InsertionError::CommandError(_) => eprintln!("{}", e),
        },
    }
}
