use serde::{Deserialize, Serialize};

/// An authorized distribution source for a specific title.
/// Metadata (TMDB) never implies rights: only explicitly curated entries appear here.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizedSource {
    pub provider_id: String,
    pub title: String,
    pub media_type: String,
    pub tmdb_movie_id: Option<i64>,
    pub tmdb_tv_id: Option<i64>,
    pub source_name: String,
    pub source_url: String,
    pub license: String,
    pub license_url: String,
    pub attribution: String,
    /// True only when StreamNest can hand the player a verified stream itself.
    /// Curated sources link to official pages; direct playback stays false
    /// unless a rights-verified stream source is wired by a provider.
    pub playable_in_app: bool,
}

pub trait ContentSourceProvider: Send + Sync {
    fn sources_for(&self, media_type: &str, tmdb_id: i64) -> Vec<AuthorizedSource>;
    fn all(&self) -> Vec<AuthorizedSource>;
    fn note(&self) -> &str;
}

fn parse(raw: &str) -> Vec<AuthorizedSource> {
    serde_json::from_str(raw).expect("bundled authorized_sources.json must be valid")
}

/// Curated, embedded provider. Every entry documents its license and official page.
/// Additions require verified rights AND a verified TMDB id mapping.
pub struct BundledContentSources {
    entries: Vec<AuthorizedSource>,
}

impl BundledContentSources {
    pub fn new() -> Self {
        Self {
            entries: parse(include_str!("../../resources/authorized_sources.json")),
        }
    }
}

impl ContentSourceProvider for BundledContentSources {
    fn sources_for(&self, media_type: &str, tmdb_id: i64) -> Vec<AuthorizedSource> {
        self.entries
            .iter()
            .filter(|e| {
                e.media_type == media_type
                    && match media_type {
                        "movie" => e.tmdb_movie_id == Some(tmdb_id),
                        _ => e.tmdb_tv_id == Some(tmdb_id),
                    }
            })
            .cloned()
            .collect()
    }

    fn all(&self) -> Vec<AuthorizedSource> {
        self.entries.clone()
    }

    fn note(&self) -> &str {
        "Only explicitly verified sources appear here. A TMDB record alone never grants distribution rights."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_json_parses_and_maps_by_tmdb_id() {
        let provider = BundledContentSources::new();
        assert!(!provider.all().is_empty(), "bundled sources must exist");
        for entry in provider.all() {
            assert!(!entry.license.is_empty(), "license required");
            assert!(!entry.license_url.is_empty(), "license url required");
            assert!(!entry.source_url.is_empty(), "official page required");
            assert!(!entry.playable_in_app, "bundled sources are page-level, not in-app streams");
        }
        let some_id = provider.all()[0].tmdb_movie_id.unwrap();
        let hits = provider.sources_for("movie", some_id);
        assert!(!hits.is_empty());
        assert!(provider.sources_for("movie", 999_999_999).is_empty());
    }
}
