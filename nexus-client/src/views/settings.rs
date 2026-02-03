//! Settings panel view

use crate::config::settings::ChatHistoryRetention;

use iced::widget::button as btn;
use iced::widget::{
    Column, Id, Space, button, checkbox, container, pick_list, row, scrollable, slider, text_input,
};
use iced::{Center, Element, Fill, Theme};
use iced_aw::NumberInput;
use iced_aw::TabLabel;
use iced_aw::Tabs;
use nexus_common::voice::VoiceQuality;

use super::chat::TimestampSettings;
use super::voice::build_vu_meter;
use crate::config::audio::{LocalizedVoiceQuality, PttMode, PttReleaseDelay};
use crate::config::events::{EventSettings, EventType, NotificationContent, SoundChoice};
use crate::config::settings::{
    CHAT_FONT_SIZES, ProxySettings, SOUND_VOLUME_MAX, SOUND_VOLUME_MIN, default_download_path,
};
use crate::config::theme::all_themes;
use crate::i18n::t;
use crate::image::CachedImage;
use crate::style::SPACER_SIZE_LARGE;
use crate::style::{
    AVATAR_PREVIEW_SIZE, BUTTON_PADDING, CHECKBOX_INDENT, CONTENT_MAX_WIDTH, CONTENT_PADDING,
    ELEMENT_SPACING, INPUT_PADDING, PATH_DISPLAY_PADDING, SPACER_SIZE_MEDIUM, SPACER_SIZE_SMALL,
    TAB_LABEL_PADDING, TEXT_SIZE, content_background_style, error_text_style, panel_title,
    shaped_text, shaped_text_wrapped,
};
use crate::types::{InputId, Message, SettingsFormState, SettingsTab};
use crate::voice::audio::AudioDevice;
use crate::voice::ptt::{hotkey_to_string, parse_hotkey};

/// Data needed to render the audio settings tab
pub struct AudioTabData<'a> {
    /// Available output devices (borrowed from SettingsFormState cache)
    pub output_devices: &'a [AudioDevice],
    /// Selected output device
    pub selected_output_device: AudioDevice,
    /// Available input devices (borrowed from SettingsFormState cache)
    pub input_devices: &'a [AudioDevice],
    /// Selected input device
    pub selected_input_device: AudioDevice,
    /// Voice quality setting
    pub voice_quality: VoiceQuality,
    /// Push-to-talk key binding
    pub ptt_key: &'a str,
    /// Whether PTT key capture is active
    pub ptt_capturing: bool,
    /// Push-to-talk mode
    pub ptt_mode: PttMode,
    /// Push-to-talk release delay
    pub ptt_release_delay: PttReleaseDelay,
    /// Whether microphone test is active
    pub mic_testing: bool,
    /// Current microphone input level (0.0 - 1.0)
    pub mic_level: f32,
    /// Error message from microphone test (e.g., device not found)
    pub mic_error: Option<&'a str>,
    /// Enable noise suppression
    pub noise_suppression: bool,
    /// Enable echo cancellation
    pub echo_cancellation: bool,
    /// Enable automatic gain control
    pub agc: bool,
    /// Enable transient suppression (keyboard/click noise reduction)
    pub transient_suppression: bool,
    /// Current theme for styling
    pub theme: Theme,
}

// ============================================================================
// Settings View Data
// ============================================================================

/// Data needed to render the settings panel
pub struct SettingsViewData<'a> {
    /// Current theme for styling
    pub current_theme: Theme,
    /// Show user connect/disconnect events in chat
    pub show_connection_events: bool,
    /// Show channel join/leave events in chat
    pub show_join_leave_events: bool,
    /// Chat history retention policy for user message conversations
    pub chat_history_retention: ChatHistoryRetention,
    /// Maximum scrollback lines per chat tab (0 = unlimited)
    pub max_scrollback: usize,
    /// Font size for chat messages
    pub chat_font_size: u8,
    /// Timestamp display settings
    pub timestamp_settings: TimestampSettings,
    /// Settings form state (present when panel is open)
    pub settings_form: Option<&'a SettingsFormState>,
    /// Default nickname for shared accounts
    pub nickname: &'a str,
    /// SOCKS5 proxy settings
    pub proxy: &'a ProxySettings,
    /// Download path for file transfers
    pub download_path: Option<&'a str>,
    /// Whether to queue transfers (limit concurrent transfers per server)
    pub queue_transfers: bool,
    /// Maximum concurrent downloads per server (0 = unlimited)
    pub download_limit: u8,
    /// Maximum concurrent uploads per server (0 = unlimited)
    pub upload_limit: u8,
    /// Event notification settings
    pub event_settings: &'a EventSettings,
    /// Currently selected event type in the Events tab
    pub selected_event_type: EventType,
    /// Global toggle for desktop notifications
    pub notifications_enabled: bool,
    /// Global toggle for sound notifications
    pub sound_enabled: bool,
    /// Master volume for sounds (0.0 - 1.0)
    pub sound_volume: f32,
    // ==================== Audio Settings ====================
    /// Available output devices (borrowed from SettingsFormState cache)
    pub output_devices: &'a [AudioDevice],
    /// Selected output device
    pub selected_output_device: AudioDevice,
    /// Available input devices (borrowed from SettingsFormState cache)
    pub input_devices: &'a [AudioDevice],
    /// Selected input device
    pub selected_input_device: AudioDevice,
    /// Voice quality setting
    pub voice_quality: VoiceQuality,
    /// Push-to-talk key binding
    pub ptt_key: &'a str,
    /// Whether PTT key capture is active
    pub ptt_capturing: bool,
    /// Push-to-talk mode
    pub ptt_mode: PttMode,
    /// Push-to-talk release delay
    pub ptt_release_delay: PttReleaseDelay,
    /// Whether microphone test is active
    pub mic_testing: bool,
    /// Current microphone input level (0.0 - 1.0)
    pub mic_level: f32,
    /// Error message from microphone test (e.g., device not found)
    pub mic_error: Option<&'a str>,
    /// Enable noise suppression
    pub noise_suppression: bool,
    /// Enable echo cancellation
    pub echo_cancellation: bool,
    /// Enable automatic gain control
    pub agc: bool,
    /// Enable transient suppression (keyboard/click noise reduction)
    pub transient_suppression: bool,
    // ==================== System Tray (Windows/Linux only) ====================
    /// Show system tray icon
    pub show_tray_icon: bool,
    /// Minimize to tray instead of closing
    pub minimize_to_tray: bool,
}

// ============================================================================
// Settings View
// ============================================================================

/// Render the settings panel with tabbed layout
///
/// Shows application settings organized into tabs:
/// - General: Theme, avatar, nickname
/// - Chat: Font size, timestamps, notifications
/// - Files: Download location
/// - Network: Proxy configuration
///
/// Cancel restores original settings, Save persists changes.
pub fn settings_view<'a>(data: SettingsViewData<'a>) -> Element<'a, Message> {
    // Extract state from settings form (only present when panel is open)
    let (avatar, default_avatar, error, active_tab): (
        Option<&CachedImage>,
        Option<&CachedImage>,
        Option<&str>,
        SettingsTab,
    ) = data
        .settings_form
        .map(|f| {
            (
                f.cached_avatar.as_ref(),
                Some(&f.default_avatar),
                f.error.as_deref(),
                f.active_tab,
            )
        })
        .unwrap_or((None, None, None, SettingsTab::General));

    // Build tab content
    let theme = data.current_theme.clone();
    let general_content = general_tab_content(
        data.current_theme,
        avatar,
        default_avatar,
        data.nickname,
        data.show_tray_icon,
        data.minimize_to_tray,
    );
    let chat_content = chat_tab_content(
        data.chat_history_retention,
        data.max_scrollback,
        data.chat_font_size,
        data.show_connection_events,
        data.show_join_leave_events,
        data.timestamp_settings,
    );
    let network_content = network_tab_content(data.proxy);

    let files_content = files_tab_content(
        data.download_path,
        data.queue_transfers,
        data.download_limit,
        data.upload_limit,
    );

    let events_content = events_tab_content(
        data.event_settings,
        data.selected_event_type,
        data.notifications_enabled,
        data.sound_enabled,
        data.sound_volume,
    );

    let audio_content = audio_tab_content(AudioTabData {
        output_devices: data.output_devices,
        selected_output_device: data.selected_output_device.clone(),
        input_devices: data.input_devices,
        selected_input_device: data.selected_input_device.clone(),
        voice_quality: data.voice_quality,
        ptt_key: data.ptt_key,
        ptt_capturing: data.ptt_capturing,
        ptt_mode: data.ptt_mode,
        ptt_release_delay: data.ptt_release_delay,
        mic_testing: data.mic_testing,
        mic_level: data.mic_level,
        mic_error: data.mic_error,
        noise_suppression: data.noise_suppression,
        echo_cancellation: data.echo_cancellation,
        agc: data.agc,
        transient_suppression: data.transient_suppression,
        theme,
    });

    // Create tabs widget with compact styling
    let tabs = Tabs::new(Message::SettingsTabSelected)
        .push(
            SettingsTab::General,
            TabLabel::Text(t("tab-general")),
            general_content,
        )
        .push(
            SettingsTab::Chat,
            TabLabel::Text(t("tab-chat")),
            chat_content,
        )
        .push(
            SettingsTab::Files,
            TabLabel::Text(t("tab-files")),
            files_content,
        )
        .push(
            SettingsTab::Network,
            TabLabel::Text(t("tab-network")),
            network_content,
        )
        .push(
            SettingsTab::Events,
            TabLabel::Text(t("settings-tab-events")),
            events_content,
        )
        .push(
            SettingsTab::Audio,
            TabLabel::Text(t("tab-audio")),
            audio_content,
        )
        .set_active_tab(&active_tab)
        .tab_bar_position(iced_aw::TabBarPosition::Top)
        .text_size(TEXT_SIZE)
        .tab_label_padding(TAB_LABEL_PADDING);

    // Buttons row
    let buttons = row![
        Space::new().width(Fill),
        button(shaped_text(t("button-cancel")).size(TEXT_SIZE))
            .on_press(Message::CancelSettings)
            .padding(BUTTON_PADDING)
            .style(btn::secondary),
        button(shaped_text(t("button-save")).size(TEXT_SIZE))
            .on_press(Message::SaveSettings)
            .padding(BUTTON_PADDING),
    ]
    .spacing(ELEMENT_SPACING);

    // Build the form
    let mut content_items: Vec<Element<'_, Message>> = Vec::new();

    content_items.push(panel_title(t("title-settings")).into());

    // Show error if present
    if let Some(error) = error {
        content_items.push(
            shaped_text_wrapped(error)
                .size(TEXT_SIZE)
                .width(Fill)
                .align_x(Center)
                .style(error_text_style)
                .into(),
        );
        content_items.push(Space::new().height(SPACER_SIZE_SMALL).into());
    } else {
        content_items.push(Space::new().height(SPACER_SIZE_SMALL).into());
    }

    content_items.push(tabs.into());
    content_items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());
    content_items.push(buttons.into());

    let content = Column::with_children(content_items)
        .spacing(ELEMENT_SPACING)
        .padding(CONTENT_PADDING)
        .max_width(CONTENT_MAX_WIDTH);

    // Scrollable, top-aligned, with background
    let scrollable_content = scrollable(container(content).width(Fill).center_x(Fill)).width(Fill);

    container(scrollable_content)
        .width(Fill)
        .height(Fill)
        .style(content_background_style)
        .into()
}

// ============================================================================
// Tab Content Builders
// ============================================================================

/// Build the General tab content (theme, avatar, nickname, tray settings)
fn general_tab_content<'a>(
    current_theme: Theme,
    avatar: Option<&'a crate::image::CachedImage>,
    default_avatar: Option<&'a crate::image::CachedImage>,
    nickname: &'a str,
    show_tray_icon: bool,
    minimize_to_tray: bool,
) -> Element<'a, Message> {
    let mut items: Vec<Element<'_, Message>> = Vec::new();

    // Space between tab bar and first content
    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // Theme picker row
    let theme_label = shaped_text(t("label-theme")).size(TEXT_SIZE);
    let theme_picker =
        pick_list(all_themes(), Some(current_theme), Message::ThemeSelected).text_size(TEXT_SIZE);
    let theme_row = row![theme_label, theme_picker]
        .spacing(ELEMENT_SPACING)
        .align_y(Center);
    items.push(theme_row.into());

    // Avatar section
    let avatar_preview: Element<'_, Message> = if let Some(av) = avatar {
        av.render(AVATAR_PREVIEW_SIZE)
    } else if let Some(default) = default_avatar {
        default.render(AVATAR_PREVIEW_SIZE)
    } else {
        Space::new()
            .width(AVATAR_PREVIEW_SIZE)
            .height(AVATAR_PREVIEW_SIZE)
            .into()
    };

    let pick_avatar_button = button(shaped_text(t("button-choose-avatar")).size(TEXT_SIZE))
        .on_press(Message::PickAvatarPressed)
        .padding(BUTTON_PADDING)
        .style(btn::secondary);

    let clear_avatar_button = if avatar.is_some() {
        button(shaped_text(t("button-clear-avatar")).size(TEXT_SIZE))
            .on_press(Message::ClearAvatarPressed)
            .padding(BUTTON_PADDING)
            .style(btn::secondary)
    } else {
        button(shaped_text(t("button-clear-avatar")).size(TEXT_SIZE))
            .padding(BUTTON_PADDING)
            .style(btn::secondary)
    };

    let avatar_buttons = row![pick_avatar_button, clear_avatar_button].spacing(ELEMENT_SPACING);
    let avatar_row = row![avatar_preview, avatar_buttons]
        .spacing(ELEMENT_SPACING)
        .align_y(Center);
    items.push(avatar_row.into());

    // Nickname input (placeholder guides the user)
    let nickname_input = text_input(&t("placeholder-nickname-optional"), nickname)
        .on_input(Message::SettingsNicknameChanged)
        .on_submit(Message::SaveSettings)
        .id(Id::from(InputId::SettingsNickname))
        .padding(INPUT_PADDING)
        .size(TEXT_SIZE);
    items.push(nickname_input.into());

    // System tray settings (Windows/Linux only)
    #[cfg(not(target_os = "macos"))]
    {
        // Add some spacing before tray section
        items.push(Space::new().height(SPACER_SIZE_SMALL).into());

        // Show tray icon checkbox
        let tray_icon_checkbox = checkbox(show_tray_icon)
            .label(t("settings-show-tray-icon"))
            .on_toggle(Message::ShowTrayIconToggled)
            .text_size(TEXT_SIZE);
        items.push(tray_icon_checkbox.into());

        // Minimize to tray checkbox (only enabled when tray icon is shown)
        let minimize_checkbox = checkbox(minimize_to_tray)
            .label(t("settings-minimize-to-tray"))
            .text_size(TEXT_SIZE);
        let minimize_checkbox = if show_tray_icon {
            minimize_checkbox.on_toggle(Message::MinimizeToTrayToggled)
        } else {
            minimize_checkbox
        };
        let minimize_row = row![Space::new().width(CHECKBOX_INDENT), minimize_checkbox];
        items.push(minimize_row.into());
    }

    // Suppress unused variable warnings on macOS
    #[cfg(target_os = "macos")]
    {
        let _ = show_tray_icon;
        let _ = minimize_to_tray;
    }

    Column::with_children(items)
        .spacing(ELEMENT_SPACING)
        .width(Fill)
        .into()
}

/// Build the Chat tab content (font size, notifications, timestamps)
fn chat_tab_content(
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

/// Build the Files tab content (download location, transfer queue settings)
fn files_tab_content(
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

/// Build the Network tab content (proxy configuration)
fn network_tab_content(proxy: &ProxySettings) -> Element<'_, Message> {
    let mut items: Vec<Element<'_, Message>> = Vec::new();

    // Space between tab bar and first content
    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // Proxy enabled checkbox
    let proxy_enabled_checkbox = checkbox(proxy.enabled)
        .label(t("label-use-socks5-proxy"))
        .on_toggle(Message::ProxyEnabledToggled)
        .text_size(TEXT_SIZE);
    items.push(proxy_enabled_checkbox.into());

    // Proxy address input (disabled when proxy is disabled)
    let proxy_address_input = if proxy.enabled {
        text_input(&t("placeholder-proxy-address"), &proxy.address)
            .on_input(Message::ProxyAddressChanged)
            .on_submit(Message::SaveSettings)
            .id(Id::from(InputId::ProxyAddress))
            .padding(INPUT_PADDING)
            .size(TEXT_SIZE)
    } else {
        text_input(&t("placeholder-proxy-address"), &proxy.address)
            .id(Id::from(InputId::ProxyAddress))
            .padding(INPUT_PADDING)
            .size(TEXT_SIZE)
    };
    items.push(proxy_address_input.into());

    // Proxy port input with label (disabled when proxy is disabled)
    let proxy_port_label = shaped_text(t("label-proxy-port")).size(TEXT_SIZE);
    let proxy_port_input: Element<'_, Message> = if proxy.enabled {
        NumberInput::new(&proxy.port, 1..=65535, Message::ProxyPortChanged)
            .id(Id::from(InputId::ProxyPort))
            .padding(INPUT_PADDING)
            .into()
    } else {
        NumberInput::new(&proxy.port, 1..=65535, Message::ProxyPortChanged)
            .on_input_maybe(None::<fn(u16) -> Message>)
            .id(Id::from(InputId::ProxyPort))
            .padding(INPUT_PADDING)
            .into()
    };
    let proxy_port_row = row![proxy_port_label, proxy_port_input]
        .spacing(ELEMENT_SPACING)
        .align_y(Center);
    items.push(proxy_port_row.into());

    // Proxy username input (optional, disabled when proxy is disabled)
    let proxy_username_value = proxy.username.as_deref().unwrap_or("");
    let proxy_username_input = if proxy.enabled {
        text_input(&t("placeholder-proxy-username"), proxy_username_value)
            .on_input(Message::ProxyUsernameChanged)
            .on_submit(Message::SaveSettings)
            .id(Id::from(InputId::ProxyUsername))
            .padding(INPUT_PADDING)
            .size(TEXT_SIZE)
    } else {
        text_input(&t("placeholder-proxy-username"), proxy_username_value)
            .id(Id::from(InputId::ProxyUsername))
            .padding(INPUT_PADDING)
            .size(TEXT_SIZE)
    };
    items.push(proxy_username_input.into());

    // Proxy password input (optional, disabled when proxy is disabled)
    let proxy_password_value = proxy.password.as_deref().unwrap_or("");
    let proxy_password_input = if proxy.enabled {
        text_input(&t("placeholder-proxy-password"), proxy_password_value)
            .on_input(Message::ProxyPasswordChanged)
            .on_submit(Message::SaveSettings)
            .id(Id::from(InputId::ProxyPassword))
            .padding(INPUT_PADDING)
            .size(TEXT_SIZE)
            .secure(true)
    } else {
        text_input(&t("placeholder-proxy-password"), proxy_password_value)
            .id(Id::from(InputId::ProxyPassword))
            .padding(INPUT_PADDING)
            .size(TEXT_SIZE)
            .secure(true)
    };
    items.push(proxy_password_input.into());

    Column::with_children(items)
        .spacing(ELEMENT_SPACING)
        .width(Fill)
        .into()
}

// ============================================================================
// Events Tab
// ============================================================================

/// Build the events tab content
fn audio_tab_content(data: AudioTabData<'_>) -> Element<'_, Message> {
    let mut items: Vec<Element<'_, Message>> = Vec::new();

    // Space between tab bar and first content
    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // Output device picker
    let output_label = shaped_text(t("audio-output-device")).size(TEXT_SIZE);
    let output_picker = pick_list(
        data.output_devices,
        Some(data.selected_output_device),
        Message::AudioOutputDeviceSelected,
    )
    .text_size(TEXT_SIZE);

    let output_row = row![
        output_label,
        Space::new().width(ELEMENT_SPACING),
        output_picker,
    ]
    .align_y(iced::Alignment::Center);
    items.push(output_row.into());

    items.push(Space::new().height(SPACER_SIZE_SMALL).into());

    // Input device picker
    let input_label = shaped_text(t("audio-input-device")).size(TEXT_SIZE);
    let input_picker = pick_list(
        data.input_devices,
        Some(data.selected_input_device),
        Message::AudioInputDeviceSelected,
    )
    .text_size(TEXT_SIZE);

    let input_row = row![
        input_label,
        Space::new().width(ELEMENT_SPACING),
        input_picker,
    ]
    .align_y(iced::Alignment::Center);
    items.push(input_row.into());

    items.push(Space::new().height(SPACER_SIZE_SMALL).into());

    // Refresh devices button
    let refresh_button = button(shaped_text(t("audio-refresh-devices")).size(TEXT_SIZE))
        .on_press(Message::AudioRefreshDevices)
        .padding(BUTTON_PADDING);
    items.push(refresh_button.into());

    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // Voice quality picker
    let quality_label = shaped_text(t("audio-voice-quality")).size(TEXT_SIZE);
    let quality_options = LocalizedVoiceQuality::all();
    let quality_picker = pick_list(
        quality_options,
        Some(LocalizedVoiceQuality::from(data.voice_quality)),
        |lq| Message::AudioQualitySelected(lq.into()),
    )
    .text_size(TEXT_SIZE);

    let quality_row = row![
        quality_label,
        Space::new().width(ELEMENT_SPACING),
        quality_picker,
    ]
    .align_y(iced::Alignment::Center);
    items.push(quality_row.into());

    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // PTT key capture
    let ptt_key_label = shaped_text(t("audio-ptt-key")).size(TEXT_SIZE);
    let ptt_key_display = if data.ptt_capturing {
        t("audio-ptt-key-hint")
    } else {
        // Parse and re-format for platform-aware display (e.g., "Cmd" on macOS, "Super" on Linux)
        match parse_hotkey(data.ptt_key) {
            Ok((modifiers, code)) => hotkey_to_string(modifiers, code),
            Err(_) => data.ptt_key.to_string(), // Fallback to raw string if parse fails
        }
    };
    let ptt_key_button = button(shaped_text(ptt_key_display).size(TEXT_SIZE))
        .on_press(Message::AudioPttKeyCapture)
        .padding(INPUT_PADDING)
        .style(btn::secondary);

    let ptt_key_row = row![
        ptt_key_label,
        Space::new().width(ELEMENT_SPACING),
        ptt_key_button,
    ]
    .align_y(iced::Alignment::Center);
    items.push(ptt_key_row.into());

    items.push(Space::new().height(SPACER_SIZE_SMALL).into());

    // PTT mode picker
    let ptt_mode_label = shaped_text(t("audio-ptt-mode")).size(TEXT_SIZE);
    let ptt_modes: Vec<PttMode> = PttMode::ALL.to_vec();
    let ptt_mode_picker = pick_list(
        ptt_modes,
        Some(data.ptt_mode),
        Message::AudioPttModeSelected,
    )
    .text_size(TEXT_SIZE);

    let ptt_mode_row = row![
        ptt_mode_label,
        Space::new().width(ELEMENT_SPACING),
        ptt_mode_picker,
    ]
    .align_y(iced::Alignment::Center);
    items.push(ptt_mode_row.into());

    items.push(Space::new().height(SPACER_SIZE_SMALL).into());

    // PTT release delay picker
    let ptt_delay_label = shaped_text(t("audio-ptt-release-delay")).size(TEXT_SIZE);
    let ptt_delays: Vec<PttReleaseDelay> = PttReleaseDelay::ALL.to_vec();
    let ptt_delay_picker = pick_list(
        ptt_delays,
        Some(data.ptt_release_delay),
        Message::AudioPttReleaseDelaySelected,
    )
    .text_size(TEXT_SIZE);

    let ptt_delay_row = row![
        ptt_delay_label,
        Space::new().width(ELEMENT_SPACING),
        ptt_delay_picker,
    ]
    .align_y(iced::Alignment::Center);
    items.push(ptt_delay_row.into());

    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // Microphone test section
    let mic_test_label = shaped_text(t("audio-input-level")).size(TEXT_SIZE);

    // VU meter for mic level (larger size for settings)
    let mic_meter = build_vu_meter(data.mic_level, &data.theme, 8.0, 16.0);

    let mic_test_button = if data.mic_testing {
        button(shaped_text(t("audio-stop-test")).size(TEXT_SIZE))
            .on_press(Message::AudioTestMicStop)
            .padding(INPUT_PADDING)
            .style(btn::secondary)
    } else {
        button(shaped_text(t("audio-test-mic")).size(TEXT_SIZE))
            .on_press(Message::AudioTestMicStart)
            .padding(INPUT_PADDING)
            .style(btn::secondary)
    };

    let mic_test_row = row![
        mic_test_label,
        Space::new().width(ELEMENT_SPACING),
        mic_meter,
        Space::new().width(ELEMENT_SPACING),
        mic_test_button,
    ]
    .align_y(iced::Alignment::Center)
    .spacing(ELEMENT_SPACING);
    items.push(mic_test_row.into());

    // Show mic error if present
    if let Some(error) = data.mic_error {
        items.push(
            shaped_text_wrapped(error)
                .size(TEXT_SIZE)
                .style(error_text_style)
                .into(),
        );
    }

    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // Noise suppression toggle
    let noise_suppression_checkbox = checkbox(data.noise_suppression)
        .label(t("audio-noise-suppression"))
        .on_toggle(Message::AudioNoiseSuppression)
        .text_size(TEXT_SIZE);
    items.push(noise_suppression_checkbox.into());

    items.push(Space::new().height(SPACER_SIZE_SMALL).into());

    // Echo cancellation toggle
    let echo_cancellation_checkbox = checkbox(data.echo_cancellation)
        .label(t("audio-echo-cancellation"))
        .on_toggle(Message::AudioEchoCancellation)
        .text_size(TEXT_SIZE);
    items.push(echo_cancellation_checkbox.into());

    items.push(Space::new().height(SPACER_SIZE_SMALL).into());

    // Automatic gain control toggle
    let agc_checkbox = checkbox(data.agc)
        .label(t("audio-agc"))
        .on_toggle(Message::AudioAgc)
        .text_size(TEXT_SIZE);
    items.push(agc_checkbox.into());

    items.push(Space::new().height(SPACER_SIZE_SMALL).into());

    // Transient suppression toggle (keyboard/click noise reduction)
    let transient_suppression_checkbox = checkbox(data.transient_suppression)
        .label(t("audio-transient-suppression"))
        .on_toggle(Message::AudioTransientSuppression)
        .text_size(TEXT_SIZE);
    items.push(transient_suppression_checkbox.into());

    Column::with_children(items)
        .spacing(ELEMENT_SPACING)
        .width(Fill)
        .into()
}

fn events_tab_content<'a>(
    event_settings: &'a EventSettings,
    selected_event_type: EventType,
    notifications_enabled: bool,
    sound_enabled: bool,
    sound_volume: f32,
) -> Element<'a, Message> {
    let mut items: Vec<Element<'_, Message>> = Vec::new();

    // Space between tab bar and first content
    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // Global toggles for notifications and sound
    let notifications_checkbox = checkbox(notifications_enabled)
        .label(t("settings-notifications-enabled"))
        .on_toggle(Message::ToggleNotificationsEnabled)
        .text_size(TEXT_SIZE)
        .spacing(ELEMENT_SPACING);

    let sound_checkbox = checkbox(sound_enabled)
        .label(t("settings-sound-enabled"))
        .on_toggle(Message::ToggleSoundEnabled)
        .text_size(TEXT_SIZE)
        .spacing(ELEMENT_SPACING);

    let global_toggles_row = row![
        notifications_checkbox,
        Space::new().width(SPACER_SIZE_LARGE),
        sound_checkbox,
    ]
    .align_y(iced::Alignment::Center);
    items.push(global_toggles_row.into());

    items.push(Space::new().height(SPACER_SIZE_SMALL).into());

    // Volume slider with label and percentage
    let volume_percent = (sound_volume * 100.0).round() as u8;
    let volume_label = shaped_text(t("settings-sound-volume")).size(TEXT_SIZE);
    let volume_value = shaped_text(format!("{}%", volume_percent)).size(TEXT_SIZE);

    // Create the slider (always interactive - disable is handled at handler level)
    let volume_slider = slider(
        SOUND_VOLUME_MIN..=SOUND_VOLUME_MAX,
        sound_volume,
        Message::SoundVolumeChanged,
    )
    .step(0.01);

    let volume_row = row![
        volume_label,
        Space::new().width(ELEMENT_SPACING),
        volume_slider,
        Space::new().width(ELEMENT_SPACING),
        volume_value,
    ]
    .spacing(ELEMENT_SPACING)
    .align_y(iced::Alignment::Center);

    items.push(volume_row.into());

    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // Event type picker with label on same row
    let event_label = shaped_text(t("event-settings-event")).size(TEXT_SIZE);
    let event_types: Vec<EventType> = EventType::all().to_vec();
    let event_picker = pick_list(
        event_types,
        Some(selected_event_type),
        Message::EventTypeSelected,
    )
    .text_size(TEXT_SIZE);

    let event_row = row![
        event_label,
        Space::new().width(ELEMENT_SPACING),
        event_picker,
    ]
    .align_y(iced::Alignment::Center);
    items.push(event_row.into());

    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // Get config for selected event
    let event_config = event_settings.get(selected_event_type);

    // Show notification checkbox - disabled when global notifications are off
    let show_notification_checkbox = if notifications_enabled {
        checkbox(event_config.show_notification)
            .label(t("event-settings-show-notification"))
            .on_toggle(Message::EventShowNotificationToggled)
            .text_size(TEXT_SIZE)
            .spacing(ELEMENT_SPACING)
    } else {
        checkbox(event_config.show_notification)
            .label(t("event-settings-show-notification"))
            .text_size(TEXT_SIZE)
            .spacing(ELEMENT_SPACING)
    };
    items.push(show_notification_checkbox.into());

    // Content level picker and test button on same row
    let content_enabled = notifications_enabled && event_config.show_notification;
    let content_levels: Vec<NotificationContent> = NotificationContent::all().to_vec();
    let content_picker = if content_enabled {
        pick_list(
            content_levels,
            Some(event_config.notification_content),
            Message::EventNotificationContentSelected,
        )
        .text_size(TEXT_SIZE)
    } else {
        pick_list(
            content_levels,
            Some(event_config.notification_content),
            |_| Message::EventNotificationContentSelected(event_config.notification_content),
        )
        .text_size(TEXT_SIZE)
    };

    let test_notification_button = if notifications_enabled && event_config.show_notification {
        button(shaped_text(t("settings-notification-test")).size(TEXT_SIZE))
            .on_press(Message::TestNotification)
            .padding(INPUT_PADDING)
            .style(btn::secondary)
    } else {
        button(shaped_text(t("settings-notification-test")).size(TEXT_SIZE))
            .padding(INPUT_PADDING)
            .style(btn::secondary)
    };

    let notification_row = row![
        content_picker,
        Space::new().width(ELEMENT_SPACING),
        test_notification_button,
    ]
    .align_y(iced::Alignment::Center);
    items.push(notification_row.into());

    items.push(Space::new().height(SPACER_SIZE_MEDIUM).into());

    // Play sound checkbox with Always Play inline
    let play_sound_enabled = sound_enabled;
    let always_play_enabled = sound_enabled && event_config.play_sound;

    let play_sound_checkbox = if play_sound_enabled {
        checkbox(event_config.play_sound)
            .label(t("settings-sound-play"))
            .on_toggle(Message::EventPlaySoundToggled)
            .text_size(TEXT_SIZE)
            .spacing(ELEMENT_SPACING)
    } else {
        checkbox(event_config.play_sound)
            .label(t("settings-sound-play"))
            .text_size(TEXT_SIZE)
            .spacing(ELEMENT_SPACING)
    };

    let always_play_checkbox = if always_play_enabled {
        checkbox(event_config.always_play_sound)
            .label(t("settings-sound-always-play"))
            .on_toggle(Message::EventAlwaysPlaySoundToggled)
            .text_size(TEXT_SIZE)
            .spacing(ELEMENT_SPACING)
    } else {
        checkbox(event_config.always_play_sound)
            .label(t("settings-sound-always-play"))
            .text_size(TEXT_SIZE)
            .spacing(ELEMENT_SPACING)
    };

    let sound_checkboxes_row = row![
        play_sound_checkbox,
        Space::new().width(SPACER_SIZE_LARGE),
        always_play_checkbox,
    ]
    .align_y(iced::Alignment::Center);
    items.push(sound_checkboxes_row.into());

    // Sound picker and test button on same row
    let sound_picker_enabled = sound_enabled && event_config.play_sound;
    let sound_choices: Vec<SoundChoice> = SoundChoice::all().to_vec();
    let sound_picker = if sound_picker_enabled {
        pick_list(
            sound_choices,
            Some(event_config.sound.clone()),
            Message::EventSoundSelected,
        )
        .text_size(TEXT_SIZE)
    } else {
        pick_list(sound_choices, Some(event_config.sound.clone()), |_| {
            Message::EventSoundSelected(event_config.sound.clone())
        })
        .text_size(TEXT_SIZE)
    };

    let test_sound_button = if sound_enabled && event_config.play_sound {
        button(shaped_text(t("settings-sound-test")).size(TEXT_SIZE))
            .on_press(Message::TestSound)
            .padding(INPUT_PADDING)
            .style(btn::secondary)
    } else {
        button(shaped_text(t("settings-sound-test")).size(TEXT_SIZE))
            .padding(INPUT_PADDING)
            .style(btn::secondary)
    };

    let sound_row = row![
        sound_picker,
        Space::new().width(ELEMENT_SPACING),
        test_sound_button,
    ]
    .align_y(iced::Alignment::Center);
    items.push(sound_row.into());

    Column::with_children(items)
        .spacing(ELEMENT_SPACING)
        .width(Fill)
        .into()
}
