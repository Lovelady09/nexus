//! Files settings tab (download location, transfer queue)

use iced::widget::button as btn;
use iced::widget::{Column, Space, button, checkbox, container, row};
use iced::{Center, Element, Fill};
use iced_aw::NumberInput;

use crate::config::settings::default_download_path;
use crate::i18n::t;
use crate::style::{
    BUTTON_PADDING, ELEMENT_SPACING, INPUT_PADDING, PATH_DISPLAY_PADDING, SPACER_SIZE_MEDIUM,
    SPACER_SIZE_SMALL, TEXT_SIZE, shaped_text,
};
use crate::types::Message;

/// Build the Files tab content (download location, transfer queue settings)
pub(super) fn files_tab_content(
    download_path: Option<&str>,
    queue_transfers: bool,
    download_limit: u8,
    upload_limit: u8,
) -> Element<'static, Message> {
    let mut items: Vec<Element<'_, Message>> = Vec::new();

    // Space between tab bar and first content
    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // Download location label
    let download_label = shaped_text(t("label-download-location")).size(TEXT_SIZE);
    items.push(download_label.into());

    // Get current download path (from live config or system default)
    // Only call default_download_path() if no path is configured
    let path_display = match download_path {
        Some(path) => path.to_string(),
        None => default_download_path().unwrap_or_else(|| t("placeholder-download-location")),
    };

    // Path display (read-only style, no left padding to align with label) with Browse button
    let path_text = shaped_text(path_display).size(TEXT_SIZE);
    let path_container = container(path_text)
        .padding(PATH_DISPLAY_PADDING)
        .width(Fill);

    let browse_button = button(shaped_text(t("button-browse")).size(TEXT_SIZE))
        .on_press(Message::BrowseDownloadPathPressed)
        .padding(BUTTON_PADDING)
        .style(btn::secondary);

    let path_row = row![path_container, browse_button]
        .spacing(ELEMENT_SPACING)
        .align_y(Center);
    items.push(path_row.into());

    // Spacer before queue settings
    items.push(Space::new().height(SPACER_SIZE_SMALL).into());

    // Queue transfers checkbox
    let queue_checkbox = checkbox(queue_transfers)
        .label(t("label-queue-transfers"))
        .on_toggle(Message::QueueTransfersToggled)
        .text_size(TEXT_SIZE);
    items.push(queue_checkbox.into());

    // Download limit (disabled when queue_transfers is off)
    let download_limit_label = shaped_text(t("label-download-limit")).size(TEXT_SIZE);
    let download_limit_input: Element<'_, Message> = if queue_transfers {
        NumberInput::new(&download_limit, 0..=u8::MAX, Message::DownloadLimitChanged)
            .padding(INPUT_PADDING)
            .into()
    } else {
        NumberInput::new(&download_limit, 0..=u8::MAX, Message::DownloadLimitChanged)
            .on_input_maybe(None::<fn(u8) -> Message>)
            .padding(INPUT_PADDING)
            .into()
    };
    let download_limit_row = row![download_limit_label, download_limit_input]
        .spacing(ELEMENT_SPACING)
        .align_y(Center);
    items.push(download_limit_row.into());

    // Upload limit (disabled when queue_transfers is off)
    let upload_limit_label = shaped_text(t("label-upload-limit")).size(TEXT_SIZE);
    let upload_limit_input: Element<'_, Message> = if queue_transfers {
        NumberInput::new(&upload_limit, 0..=u8::MAX, Message::UploadLimitChanged)
            .padding(INPUT_PADDING)
            .into()
    } else {
        NumberInput::new(&upload_limit, 0..=u8::MAX, Message::UploadLimitChanged)
            .on_input_maybe(None::<fn(u8) -> Message>)
            .padding(INPUT_PADDING)
            .into()
    };
    let upload_limit_row = row![upload_limit_label, upload_limit_input]
        .spacing(ELEMENT_SPACING)
        .align_y(Center);
    items.push(upload_limit_row.into());

    Column::with_children(items)
        .spacing(ELEMENT_SPACING)
        .width(Fill)
        .into()
}
