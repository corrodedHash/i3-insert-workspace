#![forbid(unsafe_code)]
// Classes
#![warn(clippy::pedantic, clippy::cargo, rustdoc::all)]
// Stability
#![warn(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]
// Nice code
#![warn(clippy::unneeded_field_pattern, clippy::unneeded_wildcard_pattern)]
// Debug remains
#![warn(
    clippy::use_debug,
    clippy::print_stdout,
    clippy::todo,
    clippy::unimplemented,
    clippy::dbg_macro
)]

//! Workspace enhancement for the i3 window manager
//! Insert a named workspace before or after another named workspace
use clap::Parser;
mod docker_name;
// mod insert_workspace_rename;
mod insert_workspace_swap;
use insert_workspace_swap::{
    insert_workspace as insert_workspace_swap, InsertionError as SwapInsertionError,
};
mod insert_workspace_rename;
use insert_workspace_rename::{
    insert_workspace as insert_workspace_rename, InsertionError as RenameInsertionError,
};
mod util;
use thiserror::Error;
use util::InsertionDestination;
#[derive(clap::ArgEnum, Clone, Debug)]
enum InsertMode {
    I3,
    Sway,
}

/// Simple program to insert a named workspace before or after another workspace
#[derive(clap::Parser, Debug)]
#[clap(version)]
struct Args {
    /// Workspace before or after which the new workspace is inserted.
    ///
    /// If no pivot given, using focused workspaces
    #[clap(short, long)]
    pivot: Option<String>,

    /// Insert before the pivot instead of after it.
    #[clap(short, long)]
    before: bool,

    /// Name of the new workspace
    #[clap(short, long)]
    name: Option<String>,

    /// Method to insert workspace is handled differently for i3 and sway
    #[clap(short, long, arg_enum, default_value_t=InsertMode::I3)]
    mode: InsertMode,

    /// Move container to the new workspace.
    ///
    /// Either provide container id, or `focused` for focused one
    #[clap(short, long)]
    container_id: Option<String>,
}

/// The location of a container, given by the output and workspace that contains it
struct I3ConLocation {
    #[allow(dead_code)]
    output: String,
    workspace: String,
    container: i64,
}

#[derive(Debug, Error)]
enum FocusError {
    #[error("Could not get tree: {0}")]
    IPCCommunication(#[from] i3ipc::MessageError),
    #[error("Focus chain in tree incorrect")]
    BrokenFocusChain,
    #[error("Focus entry incorrect")]
    IncorrectFocusEntry,
    #[error("Focused output unnamed")]
    UnnamedOutput,
    #[error("Focused workspace unnamed")]
    UnnamedWorkspace,
    #[error("No focused output found")]
    OutputNameNotFound,
    #[error("No focused workspace found")]
    WorkspaceNameNotFound,
}

/// Get the currently focused output, workspace and container
fn focused(conn: &mut i3ipc::I3Connection) -> Result<I3ConLocation, FocusError> {
    let t = conn.get_tree().map_err(FocusError::IPCCommunication)?;

    let mut current = &t;
    let mut output = None;
    let mut workspace = None;
    while !current.focused {
        let next_focus_item = *current.focus.first().ok_or(FocusError::BrokenFocusChain)?;
        current = current
            .nodes
            .iter()
            .chain(current.floating_nodes.iter())
            .find(|x| x.id == next_focus_item)
            .ok_or(FocusError::IncorrectFocusEntry)?;

        match current.nodetype {
            i3ipc::reply::NodeType::Output => {
                output = Some(current.name.as_ref().ok_or(FocusError::UnnamedOutput)?);
            }
            i3ipc::reply::NodeType::Workspace => {
                workspace = Some(current.name.as_ref().ok_or(FocusError::UnnamedWorkspace)?);
            }
            _ => (),
        }
    }
    Ok(I3ConLocation {
        output: output.ok_or(FocusError::OutputNameNotFound)?.clone(),
        workspace: workspace.ok_or(FocusError::WorkspaceNameNotFound)?.clone(),
        container: current.id,
    })
}

/// Generate a random name, make sure no workspace with this name exists already
fn generate_new_workspace_name(
    conn: &mut i3ipc::I3Connection,
) -> Result<String, i3ipc::MessageError> {
    let workspace_names = conn
        .get_workspaces()?
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

#[derive(Debug, Error)]
enum MainError {
    #[error("Error during sway insertion: {0}")]
    SwapInsertion(
        #[from]
        #[source]
        SwapInsertionError,
    ),
    #[error("Error during i3 insertion: {0}")]
    RenameInsertion(
        #[from]
        #[source]
        RenameInsertionError,
    ),
    #[error("Could not connect to i3 IPC: {0}")]
    Connection(
        #[from]
        #[source]
        i3ipc::EstablishError,
    ),
    #[error("Container tree error: {0}")]
    TreeError(#[from] FocusError),
    #[error("Communication error: {0}")]
    GenWorkspaceName(i3ipc::MessageError),
    #[error("Non-numeric container id: {0}")]
    ParseCointainerID(
        #[from]
        #[source]
        std::num::ParseIntError,
    ),
}

fn handle() -> Result<(), MainError> {
    let args = Args::parse();

    let mut conn = i3ipc::I3Connection::connect()?;

    let focus = focused(&mut conn)?;

    let pivot = args.pivot.unwrap_or(focus.workspace);

    let destination = InsertionDestination::new(pivot, args.before);

    let name = args.name.map_or_else(
        || generate_new_workspace_name(&mut conn).map_err(MainError::GenWorkspaceName),
        Ok,
    )?;

    let parse_container_id = |container_id: String| {
        if container_id.to_ascii_lowercase() == "focused" {
            Ok(focus.container)
        } else {
            container_id.parse::<i64>()
        }
    };

    let container_id = args.container_id.map(parse_container_id).transpose()?;

    match args.mode {
        InsertMode::I3 => insert_workspace_rename(&mut conn, &destination, &name, container_id)?,
        InsertMode::Sway => insert_workspace_swap(&mut conn, &destination, &name, container_id)?,
    }
    Ok(())
}

fn main() {
    if let Err(e) = handle() {
        eprintln!("{}", e);
    }
}
