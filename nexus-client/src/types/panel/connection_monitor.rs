//! Connection monitor panel state

use nexus_common::protocol::{ConnectionInfo, TransferInfo};

// =============================================================================
// Connection Monitor State
// =============================================================================

/// Tab selection for Connection Monitor panel
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum ConnectionMonitorTab {
    /// Connections tab (default)
    #[default]
    Connections,
    /// Transfers tab
    Transfers,
}

/// Column to sort connections by
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum ConnectionMonitorSortColumn {
    /// Sort by nickname (default)
    #[default]
    Nickname,
    /// Sort by username
    Username,
    /// Sort by IP address
    Ip,
    /// Sort by connection time
    Connected,
}

/// Column to sort transfers by
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub enum TransferSortColumn {
    /// Sort by user (nickname) - default
    #[default]
    User,
    /// Sort by IP address
    Ip,
    /// Sort by direction (download/upload)
    Direction,
    /// Sort by file path
    Path,
    /// Sort by progress (bytes transferred)
    Progress,
    /// Sort by time (started_at)
    Time,
}

#[derive(Debug, Clone)]
pub struct ConnectionMonitorState {
    /// Active connections (None = not loaded, Some(Ok) = loaded, Some(Err) = error)
    pub connections: Option<Result<Vec<ConnectionInfo>, String>>,
    /// Active transfers (None = not loaded, Some(Ok) = loaded, Some(Err) = error)
    pub transfers: Option<Result<Vec<TransferInfo>, String>>,
    /// Whether a refresh is in progress
    pub loading: bool,
    /// Currently active tab
    pub active_tab: ConnectionMonitorTab,
    /// Current sort column for connections
    pub sort_column: ConnectionMonitorSortColumn,
    /// Sort ascending (true) or descending (false) for connections
    pub sort_ascending: bool,
    /// Current sort column for transfers
    pub transfer_sort_column: TransferSortColumn,
    /// Sort ascending (true) or descending (false) for transfers
    pub transfer_sort_ascending: bool,
}

impl Default for ConnectionMonitorState {
    fn default() -> Self {
        Self {
            connections: None,
            transfers: None,
            loading: false,
            active_tab: ConnectionMonitorTab::Connections,
            sort_column: ConnectionMonitorSortColumn::Nickname,
            sort_ascending: true,
            transfer_sort_column: TransferSortColumn::User,
            transfer_sort_ascending: true,
        }
    }
}

impl ConnectionMonitorState {
    /// Reset to initial state
    pub fn reset(&mut self) {
        self.connections = None;
        self.transfers = None;
        self.loading = false;
        // Keep tab and sort settings across refreshes
    }
}
