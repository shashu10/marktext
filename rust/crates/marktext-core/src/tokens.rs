/// Colors a pane can use before it reads the live GPUI appearance.
///
/// Values are `0xRRGGBB`. The GPUI window's own `Colors` stay the source of
/// truth while rendering; these tokens give tests and non-UI code a stable pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ThemeTokens {
    pub background: u32,
    pub text: u32,
    pub accent: u32,
}

pub fn theme_tokens(theme: &str) -> ThemeTokens {
    if theme.contains("light") {
        ThemeTokens {
            background: 0xFF_FF_FF,
            text: 0x1F_23_28,
            accent: 0x09_69_DA,
        }
    } else {
        ThemeTokens {
            background: 0x1E_1E_1E,
            text: 0xEE_EE_EE,
            accent: 0x4C_9A_FF,
        }
    }
}

pub fn category_label(category: &str) -> &'static str {
    match category {
        "general" => "General",
        "appearance" => "Appearance",
        "editor" => "Editor",
        "files" => "Files",
        "markdown" => "Markdown",
        "images" => "Images",
        "keybindings" => "Keybindings",
        "sidebar" => "Sidebar",
        _ => "Preferences",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_themes_use_the_light_pair() {
        assert_eq!(theme_tokens("light").background, 0xFF_FF_FF);
        assert_eq!(theme_tokens("one-dark").background, 0x1E_1E_1E);
        assert_eq!(category_label("markdown"), "Markdown");
        assert_eq!(category_label("unknown"), "Preferences");
    }
}
