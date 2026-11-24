//! Preference category list — mirrors `prefComponents/categories.ts`
//! and English labels from `preferences.categories.*` in `en.json`.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreferenceCategory {
    pub id: &'static str,
    /// English UI label (Liberation Sans).
    pub name: &'static str,
    /// Single-letter stand-in for the Element Plus icon.
    pub glyph: &'static str,
}

/// Same order as `preferenceCategories` in the Vue sidebar.
pub const PREFERENCE_CATEGORIES: &[PreferenceCategory] = &[
    PreferenceCategory {
        id: "general",
        name: "General",
        glyph: "G",
    },
    PreferenceCategory {
        id: "appearance",
        name: "Appearance",
        glyph: "A",
    },
    PreferenceCategory {
        id: "editor",
        name: "Editing",
        glyph: "E",
    },
    PreferenceCategory {
        id: "files",
        name: "Files & saving",
        glyph: "F",
    },
    PreferenceCategory {
        id: "markdown",
        name: "Markdown",
        glyph: "M",
    },
    PreferenceCategory {
        id: "images",
        name: "Images & export",
        glyph: "I",
    },
    PreferenceCategory {
        id: "keybindings",
        name: "Keyboard shortcuts",
        glyph: "K",
    },
];

pub fn category_by_id(id: &str) -> Option<&'static PreferenceCategory> {
    PREFERENCE_CATEGORIES.iter().find(|category| category.id == id)
}

pub fn category_path(id: &str) -> String {
    format!("/preference/{id}")
}

/// Title-case English used in search `categoryEn` (legacy capitalizes the id).
pub fn category_en(id: &str) -> String {
    let mut chars = id.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seven_categories_match_legacy_order() {
        let ids: Vec<_> = PREFERENCE_CATEGORIES
            .iter()
            .map(|category| category.id)
            .collect();
        assert_eq!(
            ids,
            [
                "general",
                "appearance",
                "editor",
                "files",
                "markdown",
                "images",
                "keybindings"
            ]
        );
        assert_eq!(category_by_id("editor").unwrap().name, "Editing");
        assert_eq!(category_path("markdown"), "/preference/markdown");
        assert_eq!(category_en("general"), "General");
    }
}
