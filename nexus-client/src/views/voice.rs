//! Voice chat UI components
//!
//! This module provides UI elements for voice chat:
//! - Voice bar: Shows above the input when in a voice session
//! - Voice button: Join/leave toggle in the input row

use iced::widget::{Row, button, container, row, tooltip};
use iced::{Element, Fill};

use crate::i18n::t;
use crate::icon;
use crate::style::{
    INPUT_PADDING, SMALL_SPACING, TOOLTIP_BACKGROUND_PADDING, TOOLTIP_GAP, TOOLTIP_PADDING,
    TOOLTIP_TEXT_SIZE, shaped_text, tooltip_container_style, voice_bar_style,
};
use crate::types::{Message, ServerConnection, VoiceSession};

// =============================================================================
// Constants
// =============================================================================

/// Font size for the voice bar text
const VOICE_BAR_FONT_SIZE: f32 = 13.0;

/// Spacing within the voice bar
const VOICE_BAR_SPACING: f32 = 8.0;

/// Padding for the voice bar container
const VOICE_BAR_PADDING: f32 = 6.0;

/// Icon size for voice bar
const VOICE_BAR_ICON_SIZE: f32 = 14.0;

// =============================================================================
// Voice Bar
// =============================================================================

/// Build the voice bar that appears above the chat input when in a voice session
///
/// Shows:
/// - Headphones icon
/// - Target name (channel or other user)
/// - Participant count
pub fn build_voice_bar(session: &VoiceSession) -> Element<'static, Message> {
    // Icon
    let headphones_icon = icon::headphones().size(VOICE_BAR_ICON_SIZE);

    // Target name (channel like "#general" or other user's nickname)
    let target_text = shaped_text(&session.target).size(VOICE_BAR_FONT_SIZE);

    // Participant count
    let count = session.participant_count();

    // Build the bar content
    let mut bar_row = Row::new().spacing(VOICE_BAR_SPACING).align_y(iced::Center);

    bar_row = bar_row.push(headphones_icon);
    bar_row = bar_row.push(target_text);

    if count > 0 {
        let count_text =
            shaped_text(t("voice-bar-participants").replace("{ $count }", &count.to_string()))
                .size(VOICE_BAR_FONT_SIZE);
        bar_row = bar_row.push(count_text);
    }

    container(bar_row)
        .width(Fill)
        .padding(VOICE_BAR_PADDING)
        .style(voice_bar_style)
        .into()
}

// =============================================================================
// Voice Button
// =============================================================================

/// Build the voice join/leave button for the input row
///
/// - When not in voice: Shows mic icon, clicking joins voice for current tab
/// - When in voice: Shows mic icon, clicking leaves voice
pub fn build_voice_button<'a>(
    conn: &'a ServerConnection,
    has_voice_permission: bool,
    voice_target: Option<String>,
    font_size: f32,
) -> Element<'a, Message> {
    let is_in_voice = conn.voice_session.is_some();

    if is_in_voice {
        // In voice - click to leave
        let btn = button(icon::mic().size(font_size))
            .on_press(Message::VoiceLeavePressed)
            .padding(INPUT_PADDING);

        tooltip(
            btn,
            container(shaped_text(t("voice-leave-tooltip")).size(TOOLTIP_TEXT_SIZE))
                .padding(TOOLTIP_BACKGROUND_PADDING)
                .style(tooltip_container_style),
            tooltip::Position::Top,
        )
        .gap(TOOLTIP_GAP)
        .padding(TOOLTIP_PADDING)
        .into()
    } else if has_voice_permission && voice_target.is_some() {
        // Join voice button (enabled)
        let target = voice_target.unwrap_or_default();
        let btn = button(icon::mic().size(font_size))
            .on_press(Message::VoiceJoinPressed(target))
            .padding(INPUT_PADDING);

        tooltip(
            btn,
            container(shaped_text(t("voice-join-tooltip")).size(TOOLTIP_TEXT_SIZE))
                .padding(TOOLTIP_BACKGROUND_PADDING)
                .style(tooltip_container_style),
            tooltip::Position::Top,
        )
        .gap(TOOLTIP_GAP)
        .padding(TOOLTIP_PADDING)
        .into()
    } else {
        // Join voice button (disabled - no permission or on console tab or no target)
        let btn: iced::widget::Button<'_, Message> =
            button(icon::mic().size(font_size)).padding(INPUT_PADDING);

        btn.into()
    }
}

// =============================================================================
// Voice Input Row
// =============================================================================

/// Build the input row with voice button
///
/// This extends the standard input row with a voice join/leave button.
pub fn build_input_row_with_voice<'a>(
    message_input: &'a str,
    font_size: f32,
    conn: &'a ServerConnection,
    has_voice_permission: bool,
    voice_target: Option<String>,
) -> Row<'a, Message> {
    use iced::widget::Id;
    use iced::widget::text_input;

    use crate::style::MONOSPACE_FONT;
    use crate::types::InputId;

    let text_field = text_input(&t("placeholder-message"), message_input)
        .on_input(Message::ChatInputChanged)
        .on_submit(Message::SendMessagePressed)
        .id(Id::from(InputId::ChatInput))
        .padding(INPUT_PADDING)
        .size(font_size)
        .font(MONOSPACE_FONT)
        .width(Fill);

    let send_button = button(shaped_text(t("button-send")).size(font_size))
        .on_press(Message::SendMessagePressed)
        .padding(INPUT_PADDING);

    let voice_button = build_voice_button(conn, has_voice_permission, voice_target, font_size);

    row![voice_button, text_field, send_button]
        .spacing(SMALL_SPACING)
        .width(Fill)
}
