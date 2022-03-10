# i3-insert-workspace
Insert a new named workspace between two other named workspaces.

Makes use of the i3 IPC protocol.

## Example
```
# Move container to a new workspace to the right
i3-insert-workspace --pivot "After me" --container-id current --name "New workspace"
```
### In i3 config file
```
set $insert_workspace ~/.config/i3/i3-insert-workspace
bindsym $mod+Control+w exec --no-startup-id $insert_workspace --before
bindsym $mod+Control+v exec --no-startup-id $insert_workspace
bindsym $mod+Control+Shift+W exec --no-startup-id $insert_workspace --before --container-id current
bindsym $mod+Control+Shift+V exec --no-startup-id $insert_workspace --container-id current
```