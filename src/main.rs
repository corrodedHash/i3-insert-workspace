#![forbid(unsafe_code)]
#![warn(
    clippy::pedantic,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::print_stdout
)]
#![warn(clippy::cargo)]

//! Workspace enhancement for the i3 window manager
//! Insert a named workspace before or after another named workspace
use clap::Parser;
mod docker_name;
mod insert_workspace;

use insert_workspace::{insert_workspace, InsertionDestination, InsertionError};

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

/// Get the currently focused output, workspace and container
fn focused(conn: &mut i3ipc::I3Connection) -> Result<I3ConLocation, String> {
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

        match current.nodetype {
            i3ipc::reply::NodeType::Output => {
                output = Some(
                    current
                        .name
                        .clone()
                        .ok_or_else(|| "Output has no name".to_owned())?,
                );
            }
            i3ipc::reply::NodeType::Workspace => {
                workspace = Some(
                    current
                        .name
                        .clone()
                        .ok_or_else(|| "Workspace has no name".to_owned())?,
                );
            }
            _ => (),
        }
    }
    Ok(I3ConLocation {
        output: output.ok_or_else(|| "Focused display not found".to_owned())?,
        workspace: workspace.ok_or_else(|| "Focused workspace not found".to_owned())?,
        container: current.id,
    })
}

/// Generate a random name, make sure no workspace with this name exists already
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

#[derive(Debug)]
enum MainError {
    Insertion(InsertionError),
    Connection(i3ipc::EstablishError),
    Communication(String),
    ParseCointainerID(std::num::ParseIntError),
}

impl From<InsertionError> for MainError {
    fn from(e: InsertionError) -> Self {
        Self::Insertion(e)
    }
}

impl From<i3ipc::EstablishError> for MainError {
    fn from(e: i3ipc::EstablishError) -> Self {
        Self::Connection(e)
    }
}

impl std::fmt::Display for MainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MainError::Insertion(i) => write!(f, "Error during insertion: {}", i),
            MainError::Connection(c) => write!(f, "Could not connect to i3 IPC: {}", c),
            MainError::Communication(c) => write!(f, "IPC communication error: {}", c),
            MainError::ParseCointainerID(p) => write!(f, "Non-numeric container id: {}", p),
        }
    }
}

fn handle() -> Result<(), MainError> {
    let args = Args::parse();

    let mut conn = i3ipc::I3Connection::connect()?;

    let focus = focused(&mut conn).map_err(MainError::Communication)?;

    let pivot = args.pivot.unwrap_or(focus.workspace);

    let destination = InsertionDestination::new(pivot, args.before);

    let name = args.name.map_or_else(
        || generate_new_workspace_name(&mut conn).map_err(MainError::Communication),
        Ok,
    )?;

    let parse_container_id = |container_id: String| {
        if container_id.to_ascii_lowercase() == "focused" {
            Ok(focus.container)
        } else {
            container_id
                .parse::<i64>()
                .map_err(MainError::ParseCointainerID)
        }
    };

    let container_id = args.container_id.map(parse_container_id).transpose()?;

    insert_workspace(&mut conn, &destination, &name, container_id)?;
    Ok(())
}

fn main() {
    if let Err(e) = handle() {
        eprintln!("{}", e);
    }
}
