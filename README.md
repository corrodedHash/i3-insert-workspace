# i3-insert-workspace

[![crates.io](https://img.shields.io/crates/v/i3-insert-workspace.svg)](https://crates.io/crates/i3-insert-workspace)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

Insert a new named workspace between two other named workspaces.

Makes use of the i3 IPC protocol.

## Usage

```
i3-insert-workspace [OPTIONS]

Options:
  -p, --pivot <PIVOT>
          Workspace before or after which the new workspace is inserted.

          If no pivot given, using focused workspaces

  -b, --before
          Insert before the pivot instead of after it

  -n, --name <NAME>
          Name of the new workspace

  -m, --mode <MODE>
          Method to insert workspace is handled differently for i3 and sway

          [default: i3]
          [possible values: i3, sway]

  -c, --container-id <CONTAINER_ID>
          Move container to the new workspace.

          Either provide container id, or `focused` for focused one

  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

## Example

### From the commandline

```
# Move container to a new workspace to the right
i3-insert-workspace --pivot "After me" --container-id focused --name "New workspace"
```

### In i3 config file

```
set $insert_workspace ~/.config/i3/i3-insert-workspace
bindsym $mod+Control+w exec --no-startup-id $insert_workspace --before
bindsym $mod+Control+v exec --no-startup-id $insert_workspace
bindsym $mod+Control+Shift+W exec --no-startup-id $insert_workspace --before --container-id focused
bindsym $mod+Control+Shift+V exec --no-startup-id $insert_workspace --container-id focused
```