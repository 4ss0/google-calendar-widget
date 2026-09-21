//! Google Calendar's built-in 11-color palette.
//!
//! The API returns only an opaque color id ("1".."11") on each event; this
//! module maps ids to background/foreground hex pairs for rendering. Values
//! mirror the standard Google Calendar UI palette.

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ColorDefinition {
    pub background: String,
    pub foreground: String,
}

#[derive(Debug, Clone)]
pub struct ColorPalette {
    pub event_colors: HashMap<String, ColorDefinition>,
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self::standard()
    }
}

impl ColorPalette {
    pub fn standard() -> Self {
        let mut event_colors = HashMap::new();
        let entries: [(&str, &str, &str); 11] = [
            ("1", "#a4bdfc", "#1d1d1d"),
            ("2", "#7ae7bf", "#1d1d1d"),
            ("3", "#dbadff", "#1d1d1d"),
            ("4", "#ff887c", "#1d1d1d"),
            ("5", "#fbd75b", "#1d1d1d"),
            ("6", "#ffb878", "#1d1d1d"),
            ("7", "#46d6db", "#1d1d1d"),
            ("8", "#e1e1e1", "#1d1d1d"),
            ("9", "#5484ed", "#1d1d1d"),
            ("10", "#51b749", "#1d1d1d"),
            ("11", "#dc2127", "#ffffff"),
        ];
        for (id, bg, fg) in entries {
            event_colors.insert(
                id.to_string(),
                ColorDefinition {
                    background: bg.to_string(),
                    foreground: fg.to_string(),
                },
            );
        }
        Self { event_colors }
    }

    pub fn background_for(&self, color_id: &str) -> Option<&str> {
        self.event_colors
            .get(color_id)
            .map(|c| c.background.as_str())
    }

    pub fn foreground_for(&self, color_id: &str) -> Option<&str> {
        self.event_colors
            .get(color_id)
            .map(|c| c.foreground.as_str())
    }
}