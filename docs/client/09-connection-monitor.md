# Connection Monitor

This guide covers the Connection Monitor panel for viewing and managing active server connections.

## Overview

The Connection Monitor shows all users currently connected to the server. It's primarily used by administrators to:

- See who is online and from where
- View connection duration
- Quickly access user actions (Info, Kick, Ban)

## Accessing the Connection Monitor

Click the **Connection Monitor** icon in the toolbar (monitor icon). You need the `connection_monitor` permission to access this feature.

## Connection Table

The panel displays a sortable table with the following columns:

| Column | Description |
|--------|-------------|
| **Nickname** | Display name (colored for admins and shared accounts) |
| **Username** | Account name (same as nickname for regular accounts) |
| **IP Address** | Remote IP address (IPv4 or IPv6) |
| **Time** | Connection duration (e.g., "5m", "2h", "3d") |

### Sorting

Click any column header to sort by that column. Click again to reverse the sort order. A sort indicator (▲/▼) shows the current sort column and direction.

### Color Coding

- **Red** — Administrator accounts
- **Muted** — Shared account sessions
- **Default** — Regular accounts

## Context Menu

Right-click any cell in a row to access the context menu:

| Action | Description | Permission Required |
|--------|-------------|---------------------|
| **Info** | Open the User Info panel for this user | `user_info` |
| **Copy** | Copy the cell value to clipboard | None |
| **Kick** | Open disconnect dialog with Kick selected | `user_kick` |
| **Ban** | Open disconnect dialog with Ban selected | `ban_create` |

### Menu Visibility

- **Copy** is always available
- **Info**, **Kick**, and **Ban** are hidden if you lack the required permission
- **Kick** and **Ban** are hidden for administrator rows (admins cannot be kicked/banned)

### Kick vs Ban

- **Kick** — Disconnects the user immediately. They can reconnect.
- **Ban** — Disconnects the user and blocks their IP address for a specified duration.

Both actions open the disconnect dialog where you can optionally provide a reason.

## Refreshing

Click the **refresh** button (circular arrow icon) to reload the connection list. The data is not auto-refreshed; use this button to see recent connections.

## Shared Accounts

When a shared account has multiple sessions, each session appears as a separate row:

| Nickname | Username | IP Address | Time |
|----------|----------|------------|------|
| visitor1 | guests | 192.168.1.50 | 5m |
| visitor2 | guests | 192.168.1.51 | 3m |

This lets you manage individual sessions of shared accounts.

## Permissions

| Permission | Allows |
|------------|--------|
| `connection_monitor` | View the Connection Monitor panel |
| `user_info` | Use Info action from context menu |
| `user_kick` | Use Kick action from context menu |
| `ban_create` | Use Ban action from context menu |

Admins automatically have all permissions.

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Escape` | Close the Connection Monitor panel |

## Troubleshooting

### Can't see the Connection Monitor icon

You need the `connection_monitor` permission. Contact the server admin.

### Can't kick or ban a user

- You need `user_kick` or `ban_create` permission
- You cannot kick or ban administrators

### IP addresses show IPv6 format

IPv4 addresses are displayed in IPv4-mapped IPv6 format (e.g., `::ffff:192.168.1.1`). This is normal behavior.

## Next Steps

- [Commands](04-commands.md) — Use `/kick` and `/ban` commands
- [Settings](07-settings.md) — Configure notifications