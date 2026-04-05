// src/theme.rs

use crate::models::AppConfig;
use owo_colors::OwoColorize;

/// Returns the value formatted and ANSI-colored for speed (Mbps).
pub fn color_speed(mbps: f64, config: &AppConfig) -> String {
    let s = format!("{:.2} Mbps", mbps);
    if !config.color {
        return s;
    }

    if mbps >= 100.0 {
        s.green().to_string()
    } else if mbps >= 25.0 {
        s.yellow().to_string()
    } else {
        s.red().to_string()
    }
}

/// Returns the value formatted and ANSI-colored for ping (ms).
pub fn color_ping(ms: f64, config: &AppConfig) -> String {
    let s = format!("{:.1} ms", ms);
    if !config.color {
        return s;
    }

    if ms <= 20.0 {
        s.green().to_string()
    } else if ms <= 80.0 {
        s.yellow().to_string()
    } else {
        s.red().to_string()
    }
}

/// Returns the value formatted and ANSI-colored for jitter (ms).
pub fn color_jitter(ms: f64, config: &AppConfig) -> String {
    let s = format!("{:.2} ms", ms);
    if !config.color {
        return s;
    }

    if ms <= 5.0 {
        s.green().to_string()
    } else if ms <= 20.0 {
        s.yellow().to_string()
    } else {
        s.red().to_string()
    }
}

/// Returns the value formatted and ANSI-colored for packet loss (%).
pub fn color_loss(pct: f64, config: &AppConfig) -> String {
    let s = format!("{:.1}%", pct);
    if !config.color {
        return s;
    }

    if pct == 0.0 {
        s.green().to_string()
    } else {
        s.red().to_string()
    }
}

/// Returns a short rating label for a given Mbps value.
pub fn speed_rating(mbps: f64, config: &AppConfig) -> String {
    let label = if mbps >= 500.0 {
        "Excellent"
    } else if mbps >= 100.0 {
        "Great"
    } else if mbps >= 25.0 {
        "Good"
    } else if mbps >= 5.0 {
        "Fair"
    } else {
        "Poor"
    };

    if !config.color {
        return label.to_string();
    }

    if mbps >= 100.0 {
        label.green().to_string()
    } else if mbps >= 25.0 {
        label.yellow().to_string()
    } else {
        label.red().to_string()
    }
}

/// Returns the visible (printed) length of a string by stripping ANSI codes first.
/// Uses console::strip_ansi_codes which handles all standard SGR sequences.
pub fn visible_len(s: &str) -> usize {
    console::strip_ansi_codes(s).chars().count()
}

/// Right-pads `s` with spaces so its *visible* width equals `width`.
/// If the visible length already meets or exceeds `width`, returns `s` unchanged.
pub fn pad_to(s: &str, width: usize) -> String {
    let vlen = visible_len(s);
    if vlen >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - vlen))
    }
}

/// Truncates a string to a max length, appending an ellipsis if truncated.
pub fn truncate_to(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mock_config(color: bool) -> AppConfig {
        AppConfig {
            quiet: false,
            color,
        }
    }

    #[test]
    fn color_speed_green() {
        let res = color_speed(150.0, &mock_config(true));
        assert!(res.contains("\x1b[32m")); // ANSI green
        assert!(res.contains("150.00 Mbps"));
    }

    #[test]
    fn color_speed_plain_when_no_color() {
        let res = color_speed(150.0, &mock_config(false));
        assert!(!res.contains("\x1b"));
        assert_eq!(res, "150.00 Mbps");
    }

    #[test]
    fn speed_rating_boundaries() {
        let c = mock_config(false);
        assert_eq!(speed_rating(500.0, &c), "Excellent");
        assert_eq!(speed_rating(100.0, &c), "Great");
        assert_eq!(speed_rating(25.0, &c), "Good");
        assert_eq!(speed_rating(5.0, &c), "Fair");
        assert_eq!(speed_rating(4.9, &c), "Poor");
    }

    #[test]
    fn truncate_to_short_string() {
        assert_eq!(truncate_to("short", 10), "short");
        assert_eq!(truncate_to("exact", 5), "exact");
    }

    #[test]
    fn truncate_to_long_string() {
        // "long string" (11 chars) truncated to 5 -> "long…"
        assert_eq!(truncate_to("long string", 5), "long…");
    }

    #[test]
    fn visible_len_plain_string() {
        assert_eq!(visible_len("hello"), 5);
    }

    #[test]
    fn visible_len_colored_string() {
        assert_eq!(visible_len("\x1b[32m401.74\x1b[0m"), 6);
    }

    #[test]
    fn pad_to_short_string_pads_correctly() {
        assert_eq!(pad_to("hi", 5), "hi   ");
    }

    #[test]
    fn pad_to_colored_string_pads_to_visible_width() {
        let colored = "\x1b[32m401.74\x1b[0m";
        let padded = pad_to(colored, 10);
        assert_eq!(visible_len(&padded), 10);
        assert!(padded.starts_with(colored));
    }

    #[test]
    fn pad_to_already_at_width_unchanged() {
        assert_eq!(pad_to("hello", 5), "hello");
    }

    #[test]
    fn pad_to_over_width_unchanged() {
        assert_eq!(pad_to("toolong", 4), "toolong");
    }
}
