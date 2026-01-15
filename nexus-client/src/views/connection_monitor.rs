//! Connection Monitor panel view
//!
//! Displays a table of active connections with columns for nickname, username,
//! IP address, and connection time. Supports right-click context menu for copying values.

use std::hash::{Hash, Hasher};

use iced::widget::text::Wrapping;
use iced::widget::{Space, button, column, container, lazy, row, scrollable, table, tooltip};
use iced::{Center, Element, Fill, Right, Theme, alignment};
use iced_aw::ContextMenu;
use nexus_common::protocol::ConnectionInfo;

use crate::i18n::t;
use crate::i18n::t_args;
use crate::icon;
use crate::style::{
    CONTENT_MAX_WIDTH, CONTENT_PADDING, CONTEXT_MENU_ITEM_PADDING, CONTEXT_MENU_MIN_WIDTH,
    CONTEXT_MENU_PADDING, ICON_BUTTON_PADDING, NO_SPACING, SCROLLBAR_PADDING, SEPARATOR_HEIGHT,
    SIDEBAR_ACTION_ICON_SIZE, SORT_ICON_LEFT_MARGIN, SORT_ICON_RIGHT_MARGIN, SORT_ICON_SIZE,
    SPACER_SIZE_LARGE, SPACER_SIZE_MEDIUM, SPACER_SIZE_SMALL, TEXT_SIZE, TITLE_SIZE,
    TOOLTIP_BACKGROUND_PADDING, TOOLTIP_GAP, TOOLTIP_PADDING, TOOLTIP_TEXT_SIZE, chat,
    content_background_style, context_menu_button_style, context_menu_container_style,
    error_text_style, muted_text_style, shaped_text, shaped_text_wrapped, tooltip_container_style,
    transparent_icon_button_style,
};
use crate::types::{ConnectionMonitorSortColumn, ConnectionMonitorState, Message};

// Column width for fixed-size connected time column
const CONNECTED_COLUMN_WIDTH: f32 = 80.0;

/// Dependencies for lazy table rendering
#[derive(Clone)]
struct ConnectionTableDeps {
    connections: Vec<ConnectionInfo>,
    sort_column: ConnectionMonitorSortColumn,
    sort_ascending: bool,
    admin_color: iced::Color,
    shared_color: iced::Color,
}

impl Hash for ConnectionTableDeps {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.connections.len().hash(state);
        for conn in &self.connections {
            conn.nickname.hash(state);
            conn.username.hash(state);
            conn.ip.hash(state);
            conn.login_time.hash(state);
            conn.is_admin.hash(state);
            conn.is_shared.hash(state);
        }
        self.sort_column.hash(state);
        self.sort_ascending.hash(state);
        // Colors don't need hashing - they're derived from theme which doesn't change per-render
    }
}

/// Format a Unix timestamp as relative time (e.g., "5m", "2h", "3d")
fn format_connected_time(login_time: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let elapsed_secs = now.saturating_sub(login_time);

    if elapsed_secs < 60 {
        format!("{}s", elapsed_secs)
    } else if elapsed_secs < 3600 {
        format!("{}m", elapsed_secs / 60)
    } else if elapsed_secs < 86400 {
        format!("{}h", elapsed_secs / 3600)
    } else {
        format!("{}d", elapsed_secs / 86400)
    }
}

/// Build a context menu with a single copy action for a value
fn build_copy_menu(value: String) -> Element<'static, Message> {
    container(
        button(shaped_text(t("menu-copy")).size(TEXT_SIZE))
            .padding(CONTEXT_MENU_ITEM_PADDING)
            .width(Fill)
            .style(context_menu_button_style)
            .on_press(Message::ConnectionMonitorCopy(value)),
    )
    .width(CONTEXT_MENU_MIN_WIDTH)
    .padding(CONTEXT_MENU_PADDING)
    .style(context_menu_container_style)
    .into()
}

/// Build the lazy connection table using table widget
fn lazy_connection_table(deps: ConnectionTableDeps) -> Element<'static, Message> {
    lazy(deps, |deps| {
        // Nickname column header
        let nickname_header_content: Element<'static, Message> =
            if deps.sort_column == ConnectionMonitorSortColumn::Nickname {
                let sort_icon = if deps.sort_ascending {
                    icon::down_dir()
                } else {
                    icon::up_dir()
                };
                row![
                    shaped_text(t("col-nickname"))
                        .size(TEXT_SIZE)
                        .style(muted_text_style),
                    Space::new().width(Fill),
                    Space::new().width(SORT_ICON_LEFT_MARGIN),
                    sort_icon.size(SORT_ICON_SIZE).style(muted_text_style),
                    Space::new().width(SORT_ICON_RIGHT_MARGIN),
                ]
                .align_y(Center)
                .into()
            } else {
                shaped_text(t("col-nickname"))
                    .size(TEXT_SIZE)
                    .style(muted_text_style)
                    .into()
            };
        let nickname_header: Element<'static, Message> = button(nickname_header_content)
            .padding(NO_SPACING)
            .width(Fill)
            .style(transparent_icon_button_style)
            .on_press(Message::ConnectionMonitorSortBy(
                ConnectionMonitorSortColumn::Nickname,
            ))
            .into();

        // Nickname column - with admin/shared coloring
        let admin_color = deps.admin_color;
        let shared_color = deps.shared_color;
        let nickname_column = table::column(nickname_header, move |conn: ConnectionInfo| {
            let nickname_for_menu = conn.nickname.clone();

            // Apply color based on user type: admin (red), shared (muted), regular (default)
            let content: Element<'static, Message> = if conn.is_admin {
                shaped_text(conn.nickname)
                    .size(TEXT_SIZE)
                    .wrapping(Wrapping::Word)
                    .color(admin_color)
                    .into()
            } else if conn.is_shared {
                shaped_text(conn.nickname)
                    .size(TEXT_SIZE)
                    .wrapping(Wrapping::Word)
                    .color(shared_color)
                    .into()
            } else {
                shaped_text(conn.nickname)
                    .size(TEXT_SIZE)
                    .wrapping(Wrapping::Word)
                    .into()
            };

            // Wrap in context menu
            ContextMenu::new(content, move || build_copy_menu(nickname_for_menu.clone()))
        })
        .width(Fill);

        // Username column header
        let username_header_content: Element<'static, Message> =
            if deps.sort_column == ConnectionMonitorSortColumn::Username {
                let sort_icon = if deps.sort_ascending {
                    icon::down_dir()
                } else {
                    icon::up_dir()
                };
                row![
                    shaped_text(t("col-username"))
                        .size(TEXT_SIZE)
                        .style(muted_text_style),
                    Space::new().width(Fill),
                    Space::new().width(SORT_ICON_LEFT_MARGIN),
                    sort_icon.size(SORT_ICON_SIZE).style(muted_text_style),
                    Space::new().width(SORT_ICON_RIGHT_MARGIN),
                ]
                .align_y(Center)
                .into()
            } else {
                shaped_text(t("col-username"))
                    .size(TEXT_SIZE)
                    .style(muted_text_style)
                    .into()
            };
        let username_header: Element<'static, Message> = button(username_header_content)
            .padding(NO_SPACING)
            .width(Fill)
            .style(transparent_icon_button_style)
            .on_press(Message::ConnectionMonitorSortBy(
                ConnectionMonitorSortColumn::Username,
            ))
            .into();

        // Username column - with admin/shared coloring
        let username_column = table::column(username_header, move |conn: ConnectionInfo| {
            let username_for_menu = conn.username.clone();

            // Apply color based on user type: admin (red), shared (muted), regular (muted)
            let content: Element<'static, Message> = if conn.is_admin {
                shaped_text(conn.username)
                    .size(TEXT_SIZE)
                    .wrapping(Wrapping::Word)
                    .color(admin_color)
                    .into()
            } else if conn.is_shared {
                shaped_text(conn.username)
                    .size(TEXT_SIZE)
                    .wrapping(Wrapping::Word)
                    .color(shared_color)
                    .into()
            } else {
                shaped_text(conn.username)
                    .size(TEXT_SIZE)
                    .wrapping(Wrapping::Word)
                    .into()
            };

            ContextMenu::new(content, move || build_copy_menu(username_for_menu.clone()))
        })
        .width(Fill);

        // IP Address column header
        let ip_header_content: Element<'static, Message> =
            if deps.sort_column == ConnectionMonitorSortColumn::Ip {
                let sort_icon = if deps.sort_ascending {
                    icon::down_dir()
                } else {
                    icon::up_dir()
                };
                row![
                    shaped_text(t("col-ip-address"))
                        .size(TEXT_SIZE)
                        .style(muted_text_style),
                    Space::new().width(Fill),
                    Space::new().width(SORT_ICON_LEFT_MARGIN),
                    sort_icon.size(SORT_ICON_SIZE).style(muted_text_style),
                    Space::new().width(SORT_ICON_RIGHT_MARGIN),
                ]
                .align_y(Center)
                .into()
            } else {
                shaped_text(t("col-ip-address"))
                    .size(TEXT_SIZE)
                    .style(muted_text_style)
                    .into()
            };
        let ip_header: Element<'static, Message> = button(ip_header_content)
            .padding(NO_SPACING)
            .width(Fill)
            .style(transparent_icon_button_style)
            .on_press(Message::ConnectionMonitorSortBy(
                ConnectionMonitorSortColumn::Ip,
            ))
            .into();

        // IP Address column
        let ip_column = table::column(ip_header, |conn: ConnectionInfo| {
            let ip_for_menu = conn.ip.clone();
            let content: Element<'static, Message> = shaped_text(conn.ip)
                .size(TEXT_SIZE)
                .style(muted_text_style)
                .wrapping(Wrapping::WordOrGlyph)
                .into();

            ContextMenu::new(content, move || build_copy_menu(ip_for_menu.clone()))
        })
        .width(Fill);

        // Connected time column header
        let connected_header_content: Element<'static, Message> =
            if deps.sort_column == ConnectionMonitorSortColumn::Connected {
                let sort_icon = if deps.sort_ascending {
                    icon::down_dir()
                } else {
                    icon::up_dir()
                };
                row![
                    shaped_text(t("col-time"))
                        .size(TEXT_SIZE)
                        .style(muted_text_style),
                    Space::new().width(Fill),
                    Space::new().width(SORT_ICON_LEFT_MARGIN),
                    sort_icon.size(SORT_ICON_SIZE).style(muted_text_style),
                    Space::new().width(SORT_ICON_RIGHT_MARGIN),
                ]
                .align_y(Center)
                .into()
            } else {
                shaped_text(t("col-time"))
                    .size(TEXT_SIZE)
                    .style(muted_text_style)
                    .into()
            };
        let connected_header: Element<'static, Message> = button(connected_header_content)
            .padding(NO_SPACING)
            .width(Fill)
            .style(transparent_icon_button_style)
            .on_press(Message::ConnectionMonitorSortBy(
                ConnectionMonitorSortColumn::Connected,
            ))
            .into();

        // Connected time column
        let connected_column = table::column(connected_header, |conn: ConnectionInfo| {
            let time_str = format_connected_time(conn.login_time);
            shaped_text(time_str)
                .size(TEXT_SIZE)
                .style(muted_text_style)
        })
        .width(CONNECTED_COLUMN_WIDTH)
        .align_x(Right);

        // Build the table
        let columns = [
            nickname_column,
            username_column,
            ip_column,
            connected_column,
        ];

        table(columns, deps.connections.clone())
            .width(Fill)
            .padding_x(SPACER_SIZE_SMALL)
            .padding_y(SPACER_SIZE_SMALL)
            .separator_x(NO_SPACING)
            .separator_y(SEPARATOR_HEIGHT)
    })
    .into()
}

/// Sort connections based on column and direction
fn sort_connections(
    connections: &mut [ConnectionInfo],
    column: ConnectionMonitorSortColumn,
    ascending: bool,
) {
    connections.sort_by(|a, b| {
        let cmp = match column {
            ConnectionMonitorSortColumn::Nickname => {
                a.nickname.to_lowercase().cmp(&b.nickname.to_lowercase())
            }
            ConnectionMonitorSortColumn::Username => {
                a.username.to_lowercase().cmp(&b.username.to_lowercase())
            }
            ConnectionMonitorSortColumn::Ip => a.ip.cmp(&b.ip),
            ConnectionMonitorSortColumn::Connected => a.login_time.cmp(&b.login_time),
        };
        if ascending { cmp } else { cmp.reverse() }
    });
}

/// Render the Connection Monitor panel
pub fn connection_monitor_view(
    state: &ConnectionMonitorState,
    theme: Theme,
) -> Element<'_, Message> {
    // Refresh button with tooltip
    let refresh_btn: Element<'_, Message> = {
        let refresh_icon = container(icon::refresh().size(SIDEBAR_ACTION_ICON_SIZE))
            .width(SIDEBAR_ACTION_ICON_SIZE)
            .height(SIDEBAR_ACTION_ICON_SIZE)
            .align_x(alignment::Horizontal::Center)
            .align_y(alignment::Vertical::Center);

        tooltip(
            button(refresh_icon)
                .on_press(Message::RefreshConnectionMonitor)
                .padding(ICON_BUTTON_PADDING)
                .style(transparent_icon_button_style),
            container(shaped_text(t("tooltip-files-refresh")).size(TOOLTIP_TEXT_SIZE))
                .padding(TOOLTIP_BACKGROUND_PADDING)
                .style(tooltip_container_style),
            tooltip::Position::Top,
        )
        .gap(TOOLTIP_GAP)
        .padding(TOOLTIP_PADDING)
        .into()
    };

    // Build subtitle with connection count
    let count = match &state.connections {
        Some(Ok(connections)) => connections.len().to_string(),
        _ => "…".to_string(),
    };
    let subtitle_text = t_args("panel-active-connections", &[("count", &count)]);

    // Title row with refresh button on the right
    // Add invisible spacer on the left to balance the button width for proper centering
    let button_width =
        SIDEBAR_ACTION_ICON_SIZE + ICON_BUTTON_PADDING.left + ICON_BUTTON_PADDING.right;
    let title_row: Element<'_, Message> = row![
        Space::new().width(SCROLLBAR_PADDING),
        Space::new().width(button_width), // Balance the refresh button on the right
        shaped_text(t("panel-connection-monitor"))
            .size(TITLE_SIZE)
            .width(Fill)
            .align_x(Center),
        refresh_btn,
        Space::new().width(SCROLLBAR_PADDING),
    ]
    .align_y(Center)
    .into();

    // Subtitle row with connection count (muted, like breadcrumbs in Files)
    let subtitle_row: Element<'_, Message> = shaped_text(subtitle_text)
        .size(TEXT_SIZE)
        .width(Fill)
        .align_x(Center)
        .style(muted_text_style)
        .into();

    // Build content based on state
    let content: Element<'_, Message> = match &state.connections {
        None => {
            // Loading state
            container(
                shaped_text(t("connection-monitor-loading"))
                    .size(TEXT_SIZE)
                    .style(muted_text_style),
            )
            .width(Fill)
            .center_x(Fill)
            .padding(SPACER_SIZE_SMALL)
            .into()
        }
        Some(Err(error)) => {
            // Error state
            container(
                shaped_text_wrapped(error)
                    .size(TEXT_SIZE)
                    .style(error_text_style),
            )
            .width(Fill)
            .center_x(Fill)
            .padding(SPACER_SIZE_SMALL)
            .into()
        }
        Some(Ok(connections)) => {
            if connections.is_empty() {
                // Empty state (shouldn't happen since requester is connected)
                container(
                    shaped_text(t("connection-monitor-no-connections"))
                        .size(TEXT_SIZE)
                        .style(muted_text_style),
                )
                .width(Fill)
                .center_x(Fill)
                .padding(SPACER_SIZE_SMALL)
                .into()
            } else {
                // Sort connections based on current settings
                let mut sorted_connections = connections.clone();
                sort_connections(
                    &mut sorted_connections,
                    state.sort_column,
                    state.sort_ascending,
                );

                let deps = ConnectionTableDeps {
                    connections: sorted_connections,
                    sort_column: state.sort_column,
                    sort_ascending: state.sort_ascending,
                    admin_color: chat::admin(&theme),
                    shared_color: chat::shared(&theme),
                };

                lazy_connection_table(deps)
            }
        }
    };

    // Build the form with max_width constraint
    let form = column![
        title_row,
        subtitle_row,
        Space::new().height(SPACER_SIZE_LARGE - SPACER_SIZE_MEDIUM),
        container(scrollable(content)).height(Fill),
    ]
    .spacing(SPACER_SIZE_MEDIUM)
    .align_x(Center)
    .padding(CONTENT_PADDING)
    .max_width(CONTENT_MAX_WIDTH)
    .height(Fill);

    // Center the form horizontally
    let centered_form = container(form).width(Fill).center_x(Fill);

    // Wrap everything in content background
    container(centered_form)
        .width(Fill)
        .height(Fill)
        .style(content_background_style)
        .into()
}
