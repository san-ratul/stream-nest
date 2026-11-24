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
    load_tmdb_api_key_from(&env_candidates())
}

/// Ordered `.env` candidates for a given app-data directory (installed apps):
/// app data first, then CWD/parent and exe-dir/parent.
pub fn candidate_paths(data_dir: Option<&std::path::Path>) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Some(dir) = data_dir {
        out.push(dir.join(".env"));
    }
    out.extend(env_candidates());
    out
}

pub fn load_tmdb_api_key_from(paths: &[std::path::PathBuf]) -> Option<String> {
    // Environment variable takes precedence (set by wrapper scripts, never committed).
    if let Ok(v) = std::env::var("TMDB_API_KEY") {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Some(v);
        }
    }
    read_env_file_value(paths, "TMDB_API_KEY")
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

    #[test]
    fn candidate_paths_put_app_data_first() {
        let paths = candidate_paths(Some(std::path::Path::new("C:\\appdata\\streamnest")));
        assert_eq!(
            paths[0],
            std::path::PathBuf::from("C:\\appdata\\streamnest\\.env")
        );
        assert!(paths.len() > 1, "fallback candidates remain");
    }

    #[test]
    fn loads_from_explicit_path_list() {
        let dir = std::env::temp_dir().join(format!("streamnest_env_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".env");
        std::fs::write(&path, "TMDB_API_KEY=file-key\n").unwrap();
        let loaded = load_tmdb_api_key_from(&[path.clone()]);
        std::fs::remove_file(&path).ok();
        assert_eq!(loaded, Some("file-key".to_string()));
    }
}
