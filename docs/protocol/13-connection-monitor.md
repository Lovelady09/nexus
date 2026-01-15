# Connection Monitor

The Connection Monitor feature allows administrators to view all active connections to the server.

## Overview

Users with the `connection_monitor` permission can request a list of all currently connected sessions. This provides visibility into who is connected, from where, and for how long.

## Flow

```
Client                                        Server
   │                                             │
   │  ConnectionMonitor                          │
   │ ───────────────────────────────────────►    │
   │                                             │
   │         ConnectionMonitorResponse           │
   │ ◄───────────────────────────────────────    │
   │                                             │
```

## Messages

### ConnectionMonitor (Client → Server)

Request the list of active connections.

**Payload:**

```json
{
  "type": "ConnectionMonitor"
}
```

No additional fields are required.

**Required Permission:** `connection_monitor`

### ConnectionMonitorResponse (Server → Client)

Response containing all active connections.

**Success Response:**

```json
{
  "type": "ConnectionMonitorResponse",
  "success": true,
  "connections": [
    {
      "nickname": "alice",
      "username": "alice",
      "ip": "::ffff:127.0.0.1",
      "login_time": 1704067200,
      "is_admin": false,
      "is_shared": false
    },
    {
      "nickname": "bob",
      "username": "bob",
      "ip": "::ffff:192.168.1.100",
      "login_time": 1704067500,
      "is_admin": false,
      "is_shared": false
    },
    {
      "nickname": "admin",
      "username": "admin",
      "ip": "::ffff:10.0.0.1",
      "login_time": 1704066000,
      "is_admin": true,
      "is_shared": false
    }
  ]
}
```

**Error Response:**

```json
{
  "type": "ConnectionMonitorResponse",
  "success": false,
  "error": "Permission denied"
}
```

## Connection Info Fields

| Field | Type | Description |
|-------|------|-------------|
| `nickname` | `string` | Display name (equals username for regular accounts) |
| `username` | `string` | Account username (database key) |
| `ip` | `string` | Remote IP address (IPv4 or IPv6) |
| `login_time` | `i64` | Unix timestamp when session logged in |
| `is_admin` | `bool` | Whether the user has admin privileges |
| `is_shared` | `bool` | Whether this is a shared account session |

## Sorting

The server returns connections sorted alphabetically by nickname (case-insensitive). The client may re-sort by any column.

## Shared Accounts

For shared accounts, each session appears as a separate entry. The `nickname` field shows the session's display name, while `username` shows the underlying account name.

Example with a shared account "guests" having two sessions:

```json
{
  "connections": [
    {
      "nickname": "visitor1",
      "username": "guests",
      "ip": "::ffff:192.168.1.50",
      "login_time": 1704067200,
      "is_admin": false,
      "is_shared": true
    },
    {
      "nickname": "visitor2",
      "username": "guests",
      "ip": "::ffff:192.168.1.51",
      "login_time": 1704067300,
      "is_admin": false,
      "is_shared": true
    }
  ]
}
```

## Error Handling

| Error | Cause |
|-------|-------|
| Not logged in | Request sent without valid session |
| Permission denied | User lacks `connection_monitor` permission |

## Notes

- Admin users automatically have all permissions, including `connection_monitor`
- The requesting user's own session is included in the results
- IP addresses are shown in their canonical form (IPv4-mapped IPv6 for IPv4 addresses)
- The `login_time` field can be used to calculate connection duration

## Next Step

See [Errors](10-errors.md) for general error handling.