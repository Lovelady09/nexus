//! Chat settings tab (history, font size, timestamps, notifications)

use iced::widget::{Column, Space, checkbox, pick_list, row};
use iced::{Center, Element, Fill};
use iced_aw::NumberInput;

use crate::config::settings::{CHAT_FONT_SIZES, ChatHistoryRetention};
use crate::i18n::t;
use crate::style::{
    CHECKBOX_INDENT, ELEMENT_SPACING, INPUT_PADDING, SPACER_SIZE_MEDIUM, TEXT_SIZE, shaped_text,
};
use crate::types::Message;
use crate::views::chat::TimestampSettings;

/// Build the Chat tab content (font size, notifications, timestamps)
pub(super) fn chat_tab_content(
    chat_history_retention: ChatHistoryRetention,
    max_scrollback: usize,
    chat_font_size: u8,
    show_connection_events: bool,
    show_join_leave_events: bool,
    timestamp_settings: TimestampSettings,
) -> Element<'static, Message> {
    let mut items: Vec<Element<'_, Message>> = Vec::new();

    // Space between tab bar and first content
    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // Chat history retention picker row
    let retention_label = shaped_text(t("label-chat-history-retention")).size(TEXT_SIZE);
    let retention_picker = pick_list(
        ChatHistoryRetention::ALL,
        Some(chat_history_retention),
        Message::ChatHistoryRetentionSelected,
    )
    .text_size(TEXT_SIZE);
    let retention_row = row![retention_label, retention_picker]
        .spacing(ELEMENT_SPACING)
        .align_y(Center);
    items.push(retention_row.into());

    // Max scrollback input row (0 = unlimited)
    let scrollback_label = shaped_text(t("label-max-scrollback")).size(TEXT_SIZE);
    let scrollback_input: Element<'_, Message> = NumberInput::new(
        &max_scrollback,
        0..=usize::MAX,
        Message::MaxScrollbackChanged,
    )
    .padding(INPUT_PADDING)
    .into();
    let scrollback_row = row![scrollback_label, scrollback_input]
        .spacing(ELEMENT_SPACING)
        .align_y(Center);
    items.push(scrollback_row.into());

    // Chat font size picker row
    let font_size_label = shaped_text(t("label-chat-font-size")).size(TEXT_SIZE);
    let font_size_picker = pick_list(
        CHAT_FONT_SIZES,
        Some(chat_font_size),
        Message::ChatFontSizeSelected,
    )
    .text_size(TEXT_SIZE);
    let font_size_row = row![font_size_label, font_size_picker]
        .spacing(ELEMENT_SPACING)
        .align_y(Center);
    items.push(font_size_row.into());

    // Connection events checkbox
    let connection_events_checkbox = checkbox(show_connection_events)
        .label(t("label-show-connection-events"))
        .on_toggle(Message::ConnectionNotificationsToggled)
        .text_size(TEXT_SIZE);
    items.push(connection_events_checkbox.into());

    // Channel join/leave events checkbox
    let join_leave_events_checkbox = checkbox(show_join_leave_events)
        .label(t("label-show-channel-events"))
        .on_toggle(Message::ChannelNotificationsToggled)
        .text_size(TEXT_SIZE);
    items.push(join_leave_events_checkbox.into());

    // Timestamp settings
    let timestamps_checkbox = checkbox(timestamp_settings.show_timestamps)
        .label(t("label-show-timestamps"))
        .on_toggle(Message::ShowTimestampsToggled)
        .text_size(TEXT_SIZE);
    items.push(timestamps_checkbox.into());

    // 24-hour time checkbox (disabled if timestamps are hidden, indented)
    let time_format_checkbox = if timestamp_settings.show_timestamps {
        checkbox(timestamp_settings.use_24_hour_time)
            .label(t("label-use-24-hour-time"))
            .on_toggle(Message::Use24HourTimeToggled)
            .text_size(TEXT_SIZE)
    } else {
        checkbox(timestamp_settings.use_24_hour_time)
            .label(t("label-use-24-hour-time"))
            .text_size(TEXT_SIZE)
    };
    let time_format_row = row![Space::new().width(CHECKBOX_INDENT), time_format_checkbox];
    items.push(time_format_row.into());

    // Show seconds checkbox (disabled if timestamps are hidden, indented)
    let seconds_checkbox = if timestamp_settings.show_timestamps {
        checkbox(timestamp_settings.show_seconds)
            .label(t("label-show-seconds"))
            .on_toggle(Message::ShowSecondsToggled)
            .text_size(TEXT_SIZE)
    } else {
        checkbox(timestamp_settings.show_seconds)
            .label(t("label-show-seconds"))
            .text_size(TEXT_SIZE)
    };
    let seconds_row = row![Space::new().width(CHECKBOX_INDENT), seconds_checkbox];
    items.push(seconds_row.into());

    Column::with_children(items)
        .spacing(ELEMENT_SPACING)
        .width(Fill)
        .into()
}
