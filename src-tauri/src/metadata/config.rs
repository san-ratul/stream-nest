/// Loads the TMDB API key from local `.env` files.
/// The value is never logged and never leaves the backend.
fn parse_env_key(content: &str, wanted: &str) -> Option<String> {
    for raw in content.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != wanted {
            continue;
        }
        let mut value = value.trim().to_string();
        if (value.starts_with('"') && value.ends_with('"') && value.len() >= 2)
            || (value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2)
        {
            value = value[1..value.len() - 1].to_string();
        }
        if value.is_empty() {
            return None;
        }
        return Some(value);
    }
    None
}

pub fn read_env_file_value(paths: &[std::path::PathBuf], wanted: &str) -> Option<String> {
    for path in paths {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Some(v) = parse_env_key(&content, wanted) {
                return Some(v);
            }
        }
    }
    None
}

/// Candidate `.env` locations: process CWD, parent (dev: src-tauri -> repo root),
/// and the executable directory (installed app support).
pub fn env_candidates() -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        out.push(cwd.join(".env"));
        if let Some(parent) = cwd.parent() {
            out.push(parent.join(".env"));
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            out.push(dir.join(".env"));
            if let Some(parent) = dir.parent() {
                out.push(parent.join(".env"));
            }
        }
    }
    out
}

pub fn load_tmdb_api_key() -> Option<String> {
    // Environment variable takes precedence (set by wrapper scripts, never committed).
    if let Ok(v) = std::env::var("TMDB_API_KEY") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }
    read_env_file_value(&env_candidates(), "TMDB_API_KEY")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_value() {
        assert_eq!(
            parse_env_key("TMDB_API_KEY=abc123", "TMDB_API_KEY"),
            Some("abc123".to_string())
        );
    }

    #[test]
    fn parses_quoted_value_and_skips_comments() {
        let content = "# comment\nTMDB_API_KEY=\"quoted key\"\nOTHER=x";
        assert_eq!(
            parse_env_key(content, "TMDB_API_KEY"),
            Some("quoted key".to_string())
        );
    }

    #[test]
    fn empty_value_is_none() {
        assert_eq!(parse_env_key("TMDB_API_KEY=", "TMDB_API_KEY"), None);
        assert_eq!(parse_env_key("# TMDB_API_KEY=x", "TMDB_API_KEY"), None);
        assert_eq!(parse_env_key("", "TMDB_API_KEY"), None);
    }

    #[test]
    fn missing_key_is_none() {
        assert_eq!(parse_env_key("OTHER=y", "TMDB_API_KEY"), None);
    }
}
