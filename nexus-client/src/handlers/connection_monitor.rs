//! Connection Monitor panel handlers

use iced::Task;
use nexus_common::protocol::ClientMessage;

use crate::NexusApp;
use crate::i18n::t;
use crate::types::{ActivePanel, ConnectionMonitorSortColumn, Message};

impl NexusApp {
    /// Toggle the Connection Monitor panel
    ///
    /// When opening, requests connection data from the server.
    pub fn handle_toggle_connection_monitor(&mut self) -> Task<Message> {
        if self.active_panel() == ActivePanel::ConnectionMonitor {
            return Task::none();
        }

        self.set_active_panel(ActivePanel::ConnectionMonitor);

        let Some(conn_id) = self.active_connection else {
            return Task::none();
        };
        let Some(conn) = self.connections.get_mut(&conn_id) else {
            return Task::none();
        };

        // Reset state and set loading
        conn.connection_monitor.reset();
        conn.connection_monitor.loading = true;

        // Request connection list from server
        if let Err(e) = conn.send(ClientMessage::ConnectionMonitor) {
            conn.connection_monitor.loading = false;
            conn.connection_monitor.connections =
                Some(Err(format!("{}: {}", t("err-send-failed"), e)));
        }

        Task::none()
    }

    /// Close the Connection Monitor panel
    pub fn handle_close_connection_monitor(&mut self) -> Task<Message> {
        self.handle_show_chat_view()
    }

    /// Refresh the Connection Monitor data
    pub fn handle_refresh_connection_monitor(&mut self) -> Task<Message> {
        let Some(conn_id) = self.active_connection else {
            return Task::none();
        };
        let Some(conn) = self.connections.get_mut(&conn_id) else {
            return Task::none();
        };

        // Set loading state (keep existing data visible during refresh)
        conn.connection_monitor.loading = true;

        // Request connection list from server
        if let Err(e) = conn.send(ClientMessage::ConnectionMonitor) {
            conn.connection_monitor.loading = false;
            conn.connection_monitor.connections =
                Some(Err(format!("{}: {}", t("err-send-failed"), e)));
        }

        Task::none()
    }

    /// Handle Connection Monitor response from server
    pub fn handle_connection_monitor_response(
        &mut self,
        connection_id: usize,
        success: bool,
        error: Option<String>,
        connections: Option<Vec<nexus_common::protocol::ConnectionInfo>>,
    ) -> Task<Message> {
        let Some(conn) = self.connections.get_mut(&connection_id) else {
            return Task::none();
        };

        conn.connection_monitor.loading = false;

        if success {
            conn.connection_monitor.connections = Some(Ok(connections.unwrap_or_default()));
        } else {
            conn.connection_monitor.connections =
                Some(Err(error.unwrap_or_else(|| t("err-unknown").to_string())));
        }

        Task::none()
    }

    /// Copy a value to the clipboard
    pub fn handle_connection_monitor_copy(&mut self, value: String) -> Task<Message> {
        iced::clipboard::write(value)
    }

    /// Handle sort column change
    pub fn handle_connection_monitor_sort_by(
        &mut self,
        column: ConnectionMonitorSortColumn,
    ) -> Task<Message> {
        let Some(conn_id) = self.active_connection else {
            return Task::none();
        };
        let Some(conn) = self.connections.get_mut(&conn_id) else {
            return Task::none();
        };

        // If clicking same column, toggle direction; otherwise set new column ascending
        if conn.connection_monitor.sort_column == column {
            conn.connection_monitor.sort_ascending = !conn.connection_monitor.sort_ascending;
        } else {
            conn.connection_monitor.sort_column = column;
            conn.connection_monitor.sort_ascending = true;
        }

        Task::none()
    }
}
