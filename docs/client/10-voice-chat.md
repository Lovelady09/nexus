# Voice Chat

This guide covers push-to-talk voice chat for channels and user messages.

## Overview

Nexus BBS supports real-time voice communication using:

- **Opus codec** — High-quality audio at low bandwidth
- **DTLS encryption** — Secure UDP transport
- **Push-to-talk** — Hold or toggle a key to transmit
- **WebRTC audio processing** — Noise suppression, echo cancellation, and automatic gain control

Voice chat works in both channels (group voice) and user messages (1-on-1 voice).

## Requirements

### Permissions

| Permission | Required For |
|------------|--------------|
| `voice_listen` | Joining voice chat (required) |
| `voice_talk` | Transmitting audio (optional) |

You must have `voice_listen` to join a voice session. Without `voice_talk`, you can listen but not speak.

### Audio Devices

- **Microphone** — Required to transmit (if you have `voice_talk`)
- **Speakers/Headphones** — Required to hear others

Configure audio devices in **Settings > Audio** before joining voice.

**Note:** Nexus automatically handles audio devices that don't natively support 48kHz (the sample rate required by the Opus codec). If your device uses a different sample rate (e.g., 44.1kHz or 96kHz), audio is automatically resampled with minimal latency impact.

## Joining Voice

### From a Channel Tab

1. Switch to a channel tab (e.g., `#general`)
2. Click the **microphone icon** (🎤) in the input bar
3. The voice bar appears above the input area when connected

### From a User Message Tab

1. Switch to a user message tab
2. Click the **microphone icon** (🎤) in the input bar
3. Voice starts when the other user also joins

**Note:** You cannot join voice from the Console tab.

### One Session at a Time

You can only be in one voice session at a time, even if connected to multiple servers. If you try to join voice while already in a session:

- You'll see an error message
- Leave the current voice session first

## Voice Bar

When in voice, a bar appears above the input area showing:

```
🎧 #general (3 in voice) │ 🎤 [▮▮▮▮░░░░] Alice
```

- **Headphones icon** — Indicates you're in voice
- **Target name** — Channel name or user nickname
- **Participant count** — How many people are in this voice session
- **Mic icon + VU meter** — When you're transmitting, shows your input level
- **Speaking names** — Names of others currently speaking

### VU Meter

When transmitting, a segmented VU meter appears next to the mic icon showing your input level in real-time:

- **Green segments** (0-60%) — Normal speaking level
- **Yellow segments** (60-80%) — Getting loud
- **Red segments** (80-100%) — Too hot / clipping

The meter updates at 60fps for smooth visual feedback. If you see red frequently, move back from your microphone or lower your system input volume.

The voice bar only appears on the connection with an active voice session.

## Push-to-Talk (PTT)

Voice transmission uses push-to-talk—you must press a key to transmit.

### PTT Modes

Configure in **Settings > Audio**:

| Mode | Behavior |
|------|----------|
| **Hold** | Press and hold the key to talk; release to stop |
| **Toggle** | Press once to enable voice-activated transmission; press again to stop |

**Toggle mode with Voice Activity Detection (VAD):** When you toggle on, your microphone becomes "hot" but only transmits when you're actually speaking. Background noise and silence are automatically filtered out. This gives you hands-free operation while preventing constant transmission of ambient sound. Toggle off to fully mute.

### Default Key

The default PTT key is **backtick** (`` ` ``), also known as the grave or tilde key.

### Changing the PTT Key

1. Open **Settings > Audio**
2. Click the **PTT Key** field
3. Press your desired key
4. Click **Save**

Supported keys include:
- Letter keys (A-Z)
- Number keys (0-9)
- Function keys (F1-F12)
- Special keys (Space, Tab, Backtick, etc.)

### When PTT is Active

The key only activates PTT when:
- You're in a voice session
- The Nexus window doesn't need to be focused (global hotkey)

When not in voice, the key types normally.

## Speaking Indicators

### Your Own Status

When you're transmitting:
- Your PTT key is pressed (hold mode) or toggled on (toggle mode)
- Others in the session hear your audio

### Others Speaking

When someone else is speaking:
- Their name appears in the speaking indicator
- Audio plays through your speakers/headphones

## Mute All

You can mute all incoming voice audio while staying in the voice session:

1. Look for the **speaker icon** (🔊) on the right side of the voice bar
2. Click it to mute all incoming audio
3. The icon changes to **muted** (🔇) when active
4. Click again to unmute

This is useful when you need to temporarily stop hearing everyone without leaving the voice session. You can still transmit with PTT while muted.

## Muting Individual Users

You can mute individual users so you don't hear them:

1. Find the user in the user list
2. Click their name to open the action bar
3. Click the **mute** button

This is client-side only—they can still hear you, and others can still hear them.

To unmute, click the mute button again.

## Leaving Voice

### Click the Mic Button

Click the **microphone icon** (🎤) again to leave voice.

### Leave the Channel

If you leave a channel while in voice for that channel, you automatically leave voice too. You'll see a "You have left voice chat" message.

### Automatic Leave

Voice automatically ends when:
- You disconnect from the server
- The server restarts
- Your `voice_listen` permission is revoked
- You close the client

**Note:** If only your `voice_talk` permission is revoked, you remain in voice but can no longer transmit.

## Audio Settings

Configure voice in **Settings > Audio**:

| Setting | Description |
|---------|-------------|
| **Output Device** | Speakers/headphones for voice and notification sounds |
| **Input Device** | Microphone for voice transmission |
| **Voice Quality** | Audio quality/bandwidth tradeoff |
| **PTT Key** | Key to press for push-to-talk |
| **PTT Mode** | Hold or Toggle |
| **Noise Suppression** | Reduce background noise from your microphone |
| **Echo Cancellation** | Remove speaker audio from your microphone signal |
| **Automatic Gain Control** | Automatically adjust microphone volume |

### Voice Quality Levels

| Level | Bitrate | Best For |
|-------|---------|----------|
| Low | 16 kbps | Poor connections |
| Medium | 32 kbps | Moderate connections |
| High | 64 kbps | Good connections (default) |
| Very High | 96 kbps | Excellent connections |

Higher quality uses more bandwidth but sounds better.

**Note:** Quality changes apply immediately—you don't need to leave and rejoin voice. If you're experiencing audio issues, try lowering the quality while in the call.

### Audio Processing

Nexus uses the same audio processing technology as Discord, Google Meet, and other professional voice applications (WebRTC AudioProcessing).

| Feature | Default | Description |
|---------|---------|-------------|
| **Noise Suppression** | On | Filters out background noise (fans, AC, ambient noise) |
| **Echo Cancellation** | Off | Removes speaker audio picked up by your microphone |
| **Automatic Gain Control** | On | Normalizes your volume so you're not too quiet or too loud |
| **Keyboard Noise Reduction** | Off | Suppresses transient sounds like keyboard clicks and mouse clicks |

**Why is echo cancellation off by default?** Most users wear headphones, which don't cause echo. Echo cancellation adds processing overhead and is only needed when using speakers. Enable it if others hear themselves echoing back.

**Why is keyboard noise reduction off by default?** Transient suppression can occasionally clip the start of words. Enable it if you type while talking and want to reduce keyboard noise for others.

All audio processing settings apply immediately—you don't need to leave and rejoin voice.

**Voice Activity Detection (VAD):** The processor also includes VAD, which is used automatically in Toggle PTT mode to detect when you're speaking and only transmit voice (not background noise).

### Testing Your Microphone

1. Open **Settings > Audio**
2. Select your input device
3. Click **Test Microphone**
4. The VU meter shows your microphone input level in real-time (green/yellow/red segments)
5. Speak to verify the meter responds

The same VU meter style is used in both the settings mic test and the voice bar during transmission.

## Troubleshooting

### Can't Join Voice

**"You don't have permission"**
- Contact the server admin to grant `voice_listen` or `voice_talk`

**"Already in voice on another connection"**
- Leave voice on your other server connection first

**"Not in channel"**
- Join the channel before trying to join voice

### No Audio Output

1. Check **Settings > Audio > Output Device** is correct
2. Check your system volume isn't muted
3. Try selecting a different output device
4. Restart the client if you changed devices while in voice

### Microphone Not Working

1. Check **Settings > Audio > Input Device** is correct
2. Verify the mic level meter responds when you speak
3. Check your operating system's microphone permissions
4. Ensure no other application is using the microphone exclusively

### Audio Quality Issues

**Choppy or robotic audio:**
- Lower the voice quality setting
- Check your network connection
- The speaker may have a poor connection

**Echo or feedback:**
- Enable **Echo Cancellation** in Settings > Audio
- Use headphones instead of speakers
- Move microphone away from speakers

**Too quiet or too loud:**
- Enable **Automatic Gain Control** in Settings > Audio (on by default)
- Adjust your system microphone volume
- Ask others to adjust their system volume

**Background noise:**
- Enable **Noise Suppression** in Settings > Audio (on by default)
- Move away from noise sources (fans, AC, keyboards)
- Use a directional microphone or headset

### PTT Key Not Working

1. Verify you're in a voice session (voice bar is visible)
2. Check **Settings > Audio > PTT Key** is set correctly
3. Try a different key (some keys may be captured by other applications)
4. On Linux, ensure your display server allows global hotkeys

### Connection Failed

**"DTLS handshake failed"**
- The server may not support voice chat
- Check your firewall allows UDP on the server's port
- Try reconnecting

**"Connection timeout"**
- Network issues between you and the server
- Try again or check your connection

## Technical Details

### Protocol

- **Signaling:** TCP (same connection as chat)
- **Audio:** UDP with DTLS encryption
- **Codec:** Opus at 48kHz mono
- **Frame size:** 10ms (480 samples per frame)
- **Audio processing:** WebRTC AudioProcessing (same as Discord, Chrome, Meet)
- **Resampling:** Automatic via rubato (FFT-based) for non-48kHz devices

### Bandwidth Usage

Approximate bandwidth per direction:

| Quality | Bandwidth |
|---------|-----------|
| Low | ~20 kbps |
| Medium | ~40 kbps |
| High | ~75 kbps |
| Very High | ~110 kbps |

Actual usage includes packet overhead.

### Latency

Typical voice latency: 40-100ms depending on:
- Network latency to server
- Jitter buffer size (20-200ms adaptive, reduced from 40-200ms in v0.5.7)
- Audio device latency
- Resampling (adds ~10-20ms if device doesn't support 48kHz)

## Next Steps

- [Settings](07-settings.md) — Configure audio and other preferences
- [Chat](03-chat.md) — Text chat in channels and user messages