//! Helper functions for the files view (icons, formatting, breadcrumb parsing)

use chrono::{DateTime, Local, TimeZone, Utc};

use crate::icon;

// =============================================================================
// Icon Helpers
// =============================================================================

/// Get the appropriate icon for a file based on its extension
pub(super) fn file_icon_for_extension(filename: &str) -> iced::widget::Text<'static> {
    // Extract extension (lowercase for comparison)
    let ext = filename
        .rsplit('.')
        .next()
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    match ext.as_str() {
        // PDF
        "pdf" => icon::file_pdf(),

        // Word processing
        "doc" | "docx" | "odt" | "rtf" => icon::file_word(),

        // Spreadsheets
        "xls" | "xlsx" | "ods" | "csv" => icon::file_excel(),

        // Presentations
        "ppt" | "pptx" | "odp" => icon::file_powerpoint(),

        // Images
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" | "ico" => icon::file_image(),

        // Archives
        "zip" | "tar" | "gz" | "bz2" | "7z" | "rar" | "xz" | "zst" => icon::file_archive(),

        // Audio
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "wma" => icon::file_audio(),

        // Video
        "mp4" | "mkv" | "avi" | "mov" | "wmv" | "webm" | "flv" => icon::file_video(),

        // Code
        "rs" | "py" | "js" | "ts" | "c" | "cpp" | "h" | "java" | "go" | "rb" | "php" | "html"
        | "css" | "json" | "xml" | "yaml" | "yml" | "toml" | "sh" | "bash" => icon::file_code(),

        // Text
        "txt" | "md" | "log" | "cfg" | "conf" | "ini" | "nfo" => icon::file_text(),

        // Default
        _ => icon::file(),
    }
}

// =============================================================================
// Formatting Helpers
// =============================================================================

/// Format a Unix timestamp for display
pub(super) fn format_timestamp(timestamp: i64) -> String {
    if timestamp == 0 {
        return String::new();
    }

    // Convert Unix timestamp to local time
    if let Some(utc_time) = Utc.timestamp_opt(timestamp, 0).single() {
        let local_time: DateTime<Local> = utc_time.with_timezone(&Local);
        // Format as "Jan 15, 2025 10:30"
        local_time.format("%b %d, %Y %H:%M").to_string()
    } else {
        String::new()
    }
}

/// Format a file size for display (human-readable)
pub(super) fn format_size(size: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    if size >= TB {
        format!("{:.1} TB", size as f64 / TB as f64)
    } else if size >= GB {
        format!("{:.1} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.1} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.1} KB", size as f64 / KB as f64)
    } else {
        format!("{size} B")
    }
}

// =============================================================================
// Breadcrumb Helpers
// =============================================================================

/// Truncate a segment name to a maximum length, adding ellipsis if needed
pub(super) fn truncate_segment(name: &str, max_len: usize) -> String {
    if name.chars().count() <= max_len {
        name.to_string()
    } else {
        // Leave room for "…" (1 character)
        let truncated: String = name.chars().take(max_len - 1).collect();
        format!("{truncated}…")
    }
}

/// Build a navigation path by appending a segment to the current path.
///
/// This is public so the file info handler can use it to build full paths.
pub fn build_navigate_path(current_path: &str, segment: &str) -> String {
    if current_path.is_empty() || current_path == "/" {
        segment.to_string()
    } else {
        format!("{current_path}/{segment}")
    }
}

/// Parse breadcrumb segments from a path
pub(super) fn parse_breadcrumbs(path: &str) -> Vec<(&str, String)> {
    if path.is_empty() || path == "/" {
        return Vec::new();
    }

    let mut result = Vec::new();
    let mut accumulated_path = String::new();

    for segment in path.split('/').filter(|s| !s.is_empty()) {
        if accumulated_path.is_empty() {
            accumulated_path = segment.to_string();
        } else {
            accumulated_path = format!("{accumulated_path}/{segment}");
        }
        result.push((segment, accumulated_path.clone()));
    }

    result
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // format_timestamp Tests
    // =========================================================================

    #[test]
    fn test_format_timestamp_zero() {
        assert_eq!(format_timestamp(0), "");
    }

    #[test]
    fn test_format_timestamp_valid() {
        // 2025-01-15 10:30:00 UTC = 1736935800
        let result = format_timestamp(1736935800);
        // Just check it's non-empty and contains expected parts
        assert!(!result.is_empty());
        assert!(result.contains("2025"));
    }

    #[test]
    fn test_format_timestamp_negative() {
        // Negative timestamps (before 1970) are valid - they represent dates before Unix epoch
        // -1 = Dec 31, 1969 23:59:59 UTC
        let result = format_timestamp(-1);
        assert!(!result.is_empty());
        assert!(result.contains("1969"));
    }

    // =========================================================================
    // format_size Tests
    // =========================================================================

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(1), "1 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0 KB");
        assert_eq!(format_size(1536), "1.5 KB");
        assert_eq!(format_size(10240), "10.0 KB");
        assert_eq!(format_size(1048575), "1024.0 KB"); // Just under 1 MB
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1048576), "1.0 MB"); // Exactly 1 MB
        assert_eq!(format_size(1572864), "1.5 MB");
        assert_eq!(format_size(104857600), "100.0 MB");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1073741824), "1.0 GB"); // Exactly 1 GB
        assert_eq!(format_size(1610612736), "1.5 GB");
        assert_eq!(format_size(107374182400), "100.0 GB");
    }

    #[test]
    fn test_format_size_terabytes() {
        assert_eq!(format_size(1099511627776), "1.0 TB"); // Exactly 1 TB
        assert_eq!(format_size(1649267441664), "1.5 TB");
    }

    // =========================================================================
    // parse_breadcrumbs Tests
    // =========================================================================

    #[test]
    fn test_parse_breadcrumbs_empty() {
        assert!(parse_breadcrumbs("").is_empty());
    }

    #[test]
    fn test_parse_breadcrumbs_root_slash() {
        assert!(parse_breadcrumbs("/").is_empty());
    }

    #[test]
    fn test_parse_breadcrumbs_single_segment() {
        let result = parse_breadcrumbs("Documents");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "Documents");
        assert_eq!(result[0].1, "Documents");
    }

    #[test]
    fn test_parse_breadcrumbs_multiple_segments() {
        let result = parse_breadcrumbs("Documents/Photos/2024");
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, "Documents");
        assert_eq!(result[0].1, "Documents");
        assert_eq!(result[1].0, "Photos");
        assert_eq!(result[1].1, "Documents/Photos");
        assert_eq!(result[2].0, "2024");
        assert_eq!(result[2].1, "Documents/Photos/2024");
    }

    #[test]
    fn test_parse_breadcrumbs_with_leading_slash() {
        let result = parse_breadcrumbs("/Documents/Photos");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "Documents");
        assert_eq!(result[0].1, "Documents");
        assert_eq!(result[1].0, "Photos");
        assert_eq!(result[1].1, "Documents/Photos");
    }

    #[test]
    fn test_parse_breadcrumbs_with_trailing_slash() {
        let result = parse_breadcrumbs("Documents/Photos/");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "Documents");
        assert_eq!(result[1].0, "Photos");
    }

    #[test]
    fn test_parse_breadcrumbs_with_suffix() {
        // Suffix should be preserved in segment (display_name strips it later)
        let result = parse_breadcrumbs("Uploads [NEXUS-UL]/Photos");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].0, "Uploads [NEXUS-UL]");
        assert_eq!(result[0].1, "Uploads [NEXUS-UL]");
        assert_eq!(result[1].0, "Photos");
        assert_eq!(result[1].1, "Uploads [NEXUS-UL]/Photos");
    }

    // =========================================================================
    // build_navigate_path Tests
    // =========================================================================

    #[test]
    fn test_build_navigate_path_from_empty() {
        assert_eq!(build_navigate_path("", "Documents"), "Documents");
    }

    #[test]
    fn test_build_navigate_path_from_root_slash() {
        assert_eq!(build_navigate_path("/", "Documents"), "Documents");
    }

    #[test]
    fn test_build_navigate_path_from_existing() {
        assert_eq!(
            build_navigate_path("Documents", "Photos"),
            "Documents/Photos"
        );
    }

    #[test]
    fn test_build_navigate_path_nested() {
        assert_eq!(
            build_navigate_path("Documents/Photos", "2024"),
            "Documents/Photos/2024"
        );
    }

    #[test]
    fn test_build_navigate_path_with_suffix() {
        assert_eq!(
            build_navigate_path("Files", "Uploads [NEXUS-UL]"),
            "Files/Uploads [NEXUS-UL]"
        );
    }

    // =========================================================================
    // truncate_segment Tests
    // =========================================================================

    #[test]
    fn test_truncate_segment_short_name() {
        // Names shorter than max should be unchanged
        assert_eq!(truncate_segment("Documents", 32), "Documents");
        assert_eq!(truncate_segment("A", 32), "A");
        assert_eq!(truncate_segment("", 32), "");
    }

    #[test]
    fn test_truncate_segment_exact_length() {
        // Name exactly at max length should be unchanged
        let name = "a".repeat(32);
        assert_eq!(truncate_segment(&name, 32), name);
    }

    #[test]
    fn test_truncate_segment_too_long() {
        // Name longer than max should be truncated with ellipsis
        let name = "a".repeat(40);
        let result = truncate_segment(&name, 32);
        assert_eq!(result.chars().count(), 32);
        assert!(result.ends_with('…'));
        assert_eq!(result, format!("{}…", "a".repeat(31)));
    }

    #[test]
    fn test_truncate_segment_unicode() {
        // Unicode characters should be handled correctly (count chars, not bytes)
        let name = "日本語フォルダ名テスト長い名前";
        assert_eq!(name.chars().count(), 15);
        assert_eq!(truncate_segment(name, 32), name);

        // Truncate unicode
        let long_name = "日".repeat(40);
        let result = truncate_segment(&long_name, 32);
        assert_eq!(result.chars().count(), 32);
        assert!(result.ends_with('…'));
    }

    #[test]
    fn test_truncate_segment_one_over() {
        // Name one character over should truncate
        let name = "a".repeat(33);
        let result = truncate_segment(&name, 32);
        assert_eq!(result.chars().count(), 32);
        assert_eq!(result, format!("{}…", "a".repeat(31)));
    }

    // =========================================================================
    // file_icon_for_extension Tests
    // =========================================================================

    // Note: We can't directly compare Text widgets, so we test that the function
    // doesn't panic and returns something for each category. The actual icon
    // correctness is verified by visual inspection.

    #[test]
    fn test_file_icon_pdf() {
        // Should not panic
        let _ = file_icon_for_extension("document.pdf");
        let _ = file_icon_for_extension("DOCUMENT.PDF");
    }

    #[test]
    fn test_file_icon_word() {
        let _ = file_icon_for_extension("report.doc");
        let _ = file_icon_for_extension("report.docx");
        let _ = file_icon_for_extension("report.odt");
        let _ = file_icon_for_extension("report.rtf");
    }

    #[test]
    fn test_file_icon_excel() {
        let _ = file_icon_for_extension("data.xls");
        let _ = file_icon_for_extension("data.xlsx");
        let _ = file_icon_for_extension("data.ods");
        let _ = file_icon_for_extension("data.csv");
    }

    #[test]
    fn test_file_icon_powerpoint() {
        let _ = file_icon_for_extension("slides.ppt");
        let _ = file_icon_for_extension("slides.pptx");
        let _ = file_icon_for_extension("slides.odp");
    }

    #[test]
    fn test_file_icon_image() {
        let _ = file_icon_for_extension("photo.png");
        let _ = file_icon_for_extension("photo.jpg");
        let _ = file_icon_for_extension("photo.jpeg");
        let _ = file_icon_for_extension("photo.gif");
        let _ = file_icon_for_extension("photo.bmp");
        let _ = file_icon_for_extension("photo.svg");
        let _ = file_icon_for_extension("photo.webp");
        let _ = file_icon_for_extension("photo.ico");
    }

    #[test]
    fn test_file_icon_archive() {
        let _ = file_icon_for_extension("archive.zip");
        let _ = file_icon_for_extension("archive.tar");
        let _ = file_icon_for_extension("archive.gz");
        let _ = file_icon_for_extension("archive.bz2");
        let _ = file_icon_for_extension("archive.7z");
        let _ = file_icon_for_extension("archive.rar");
        let _ = file_icon_for_extension("archive.xz");
        let _ = file_icon_for_extension("archive.zst");
    }

    #[test]
    fn test_file_icon_audio() {
        let _ = file_icon_for_extension("song.mp3");
        let _ = file_icon_for_extension("song.wav");
        let _ = file_icon_for_extension("song.flac");
        let _ = file_icon_for_extension("song.ogg");
        let _ = file_icon_for_extension("song.m4a");
        let _ = file_icon_for_extension("song.aac");
        let _ = file_icon_for_extension("song.wma");
    }

    #[test]
    fn test_file_icon_video() {
        let _ = file_icon_for_extension("movie.mp4");
        let _ = file_icon_for_extension("movie.mkv");
        let _ = file_icon_for_extension("movie.avi");
        let _ = file_icon_for_extension("movie.mov");
        let _ = file_icon_for_extension("movie.wmv");
        let _ = file_icon_for_extension("movie.webm");
        let _ = file_icon_for_extension("movie.flv");
    }

    #[test]
    fn test_file_icon_code() {
        let _ = file_icon_for_extension("main.rs");
        let _ = file_icon_for_extension("script.py");
        let _ = file_icon_for_extension("app.js");
        let _ = file_icon_for_extension("app.ts");
        let _ = file_icon_for_extension("main.c");
        let _ = file_icon_for_extension("main.cpp");
        let _ = file_icon_for_extension("header.h");
        let _ = file_icon_for_extension("Main.java");
        let _ = file_icon_for_extension("main.go");
        let _ = file_icon_for_extension("script.rb");
        let _ = file_icon_for_extension("index.php");
        let _ = file_icon_for_extension("index.html");
        let _ = file_icon_for_extension("style.css");
        let _ = file_icon_for_extension("config.json");
        let _ = file_icon_for_extension("data.xml");
        let _ = file_icon_for_extension("config.yaml");
        let _ = file_icon_for_extension("config.yml");
        let _ = file_icon_for_extension("Cargo.toml");
        let _ = file_icon_for_extension("script.sh");
        let _ = file_icon_for_extension("script.bash");
    }

    #[test]
    fn test_file_icon_text() {
        let _ = file_icon_for_extension("readme.txt");
        let _ = file_icon_for_extension("README.md");
        let _ = file_icon_for_extension("server.log");
        let _ = file_icon_for_extension("app.cfg");
        let _ = file_icon_for_extension("nginx.conf");
        let _ = file_icon_for_extension("config.ini");
        let _ = file_icon_for_extension("release.nfo");
    }

    #[test]
    fn test_file_icon_default() {
        // Unknown extensions should return generic file icon
        let _ = file_icon_for_extension("unknown.xyz");
        let _ = file_icon_for_extension("noextension");
        let _ = file_icon_for_extension(".hidden");
    }

    #[test]
    fn test_file_icon_case_insensitive() {
        // Extensions should be case-insensitive
        let _ = file_icon_for_extension("PHOTO.PNG");
        let _ = file_icon_for_extension("Photo.Png");
        let _ = file_icon_for_extension("photo.PNG");
    }
}
