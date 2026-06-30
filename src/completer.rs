use std::path::PathBuf;
use inquire::autocompletion::Replacement;
use crate::serve::expand_tilde;

#[derive(Clone, Default)]
pub struct FilePathCompleter;

impl inquire::Autocomplete for FilePathCompleter {
    fn get_suggestions(&mut self, input: &str) -> Result<Vec<String>, inquire::CustomUserError> {
        if input.is_empty() {
            return Ok(vec![]);
        }

        let expanded = expand_tilde(PathBuf::from(input));
        let (dir, prefix) = if expanded.is_dir() {
            (expanded.clone(), String::new())
        } else {
            let parent = expanded
                .parent()
                .unwrap_or(std::path::Path::new("."))
                .to_path_buf();
            let prefix = expanded
                .file_name()
                .map(|f| f.to_string_lossy().to_string())
                .unwrap_or_default();
            (parent, prefix)
        };

        let mut suggestions = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') && !prefix.starts_with('.') {
                    continue;
                }
                if prefix.is_empty() || name.to_lowercase().starts_with(&prefix.to_lowercase()) {
                    let full = entry.path();
                    let display = to_display_path(input, &full);
                    if full.is_dir() {
                        suggestions.push(format!("{}/", display));
                    } else {
                        suggestions.push(display);
                    }
                }
            }
        }
        suggestions.sort();
        if suggestions.len() > 20 {
            suggestions.truncate(20);
        }
        Ok(suggestions)
    }

    fn get_completion(
        &mut self,
        input: &str,
        highlighted_suggestion: Option<String>,
    ) -> Result<Replacement, inquire::CustomUserError> {
        if let Some(suggestion) = highlighted_suggestion {
            return Ok(Some(suggestion));
        }

        let suggestions = self.get_suggestions(input)?;
        if suggestions.is_empty() {
            return Ok(None);
        }
        if suggestions.len() == 1 {
            return Ok(Some(suggestions[0].clone()));
        }

        let first = &suggestions[0];
        let mut common_len = first.len();
        for s in &suggestions[1..] {
            common_len = first
                .chars()
                .zip(s.chars())
                .take_while(|(a, b)| a == b)
                .count()
                .min(common_len);
        }

        let prefix: String = first.chars().take(common_len).collect();
        if prefix.len() > input.len() {
            Ok(Some(prefix))
        } else {
            Ok(None)
        }
    }
}

fn to_display_path(input: &str, full: &std::path::Path) -> String {
    if input.starts_with("~/") || input == "~" {
        if let Some(home) = dirs_next::home_dir() {
            if let Ok(stripped) = full.strip_prefix(&home) {
                return format!("~/{}", stripped.display());
            }
        }
    }
    full.display().to_string()
}
