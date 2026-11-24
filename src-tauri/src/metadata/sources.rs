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
    pub playable_in_app: bool,
    /// "official_torrent" (rights-verified torrent published by the rights holder),
    /// "official_page" (link to official download page), or "dev_fixture"
    /// (local deterministic-seeder fixture, clearly labeled, for QA only).
    #[serde(default)]
    pub source_kind: String,
    /// Official magnet URI published by the rights holder.
    #[serde(default)]
    pub magnet_uri: Option<String>,
    /// Development fixture: path to a locally generated .torrent file.
    #[serde(default)]
    pub torrent_file: Option<String>,
    /// Peers to seed from (dev fixtures use the local deterministic seeder).
    #[serde(default)]
    pub initial_peers: Vec<String>,
    #[serde(default)]
    pub quality: Option<String>,
    #[serde(default)]
    pub size_label: Option<String>,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    /// Runtime availability: dev fixtures require their .torrent file to exist.
    #[serde(default)]
    pub available: bool,
}

pub trait ContentSourceProvider: Send + Sync {
    fn sources_for(&self, media_type: &str, tmdb_id: i64) -> Vec<AuthorizedSource>;
    fn all(&self) -> Vec<AuthorizedSource>;
    fn note(&self) -> &str;
}

fn parse(raw: &str) -> Vec<AuthorizedSource> {
    serde_json::from_str(raw).expect("bundled authorized_sources.json must be valid")
}

fn check_available(entry: &AuthorizedSource) -> bool {
    if !entry.playable_in_app {
        // Page-level sources are always "available" as links.
        return true;
    }
    if entry.source_kind == "dev_fixture" {
        // Fixture torrent must exist locally; otherwise the source is hidden.
        return entry
            .torrent_file
            .as_ref()
            .map(|p| std::path::Path::new(p).is_file())
            .unwrap_or(false);
    }
    // Official torrents are always offered (public swarm availability varies;
    // the streaming workflow surfaces real engine errors honestly).
    true
}

/// Curated, embedded provider. Every entry documents its license and official page.
/// Additions require verified rights AND a verified TMDB id mapping.
pub struct BundledContentSources {
    entries: Vec<AuthorizedSource>,
}

impl BundledContentSources {
    pub fn new() -> Self {
        let mut entries: Vec<AuthorizedSource> =
            parse(include_str!("../../resources/authorized_sources.json"));
        for e in &mut entries {
            e.available = check_available(e);
        }
        Self { entries }
    }
}

impl ContentSourceProvider for BundledContentSources {
    fn sources_for(&self, media_type: &str, tmdb_id: i64) -> Vec<AuthorizedSource> {
        self.entries
            .iter()
            .filter(|e| {
                e.available
                    && e.media_type == media_type
                    && match media_type {
                        "movie" => e.tmdb_movie_id == Some(tmdb_id),
                        _ => e.tmdb_tv_id == Some(tmdb_id),
                    }
            })
            .cloned()
            .collect()
    }

    fn all(&self) -> Vec<AuthorizedSource> {
        self.entries.iter().filter(|e| e.available).cloned().collect()
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
            match entry.source_kind.as_str() {
                "official_torrent" => {
                    assert!(entry.playable_in_app, "official torrents are playable");
                    let magnet = entry.magnet_uri.as_ref().expect("magnet required");
                    assert!(
                        magnet.starts_with("magnet:?xt=urn:btih:"),
                        "magnet must carry a btih hash"
                    );
                }
                "dev_fixture" => {
                    assert!(entry.playable_in_app, "fixtures are playable");
                    assert!(entry.torrent_file.is_some(), "fixture needs a torrent file");
                }
                _ => assert!(!entry.playable_in_app, "page sources are not playable"),
            }
        }
        let some_id = provider.all()[0].tmdb_movie_id.unwrap();
        let hits = provider.sources_for("movie", some_id);
        assert!(!hits.is_empty());
        assert!(provider.sources_for("movie", 999_999_999).is_empty());
    }

    #[test]
    fn official_torrent_sources_exist_for_blender_films() {
        let provider = BundledContentSources::new();
        for tmdb_id in [10378, 45745, 133701] {
            let hits = provider.sources_for("movie", tmdb_id);
            assert!(
                hits.iter().any(|h| h.source_kind == "official_torrent" && h.playable_in_app),
                "tmdb {tmdb_id} must have a playable official torrent source"
            );
        }
    }

    #[test]
    fn unavailable_dev_fixtures_are_hidden() {
        let provider = BundledContentSources::new();
        let all = provider.all();
        for entry in all.iter().filter(|e| e.source_kind == "dev_fixture") {
            // On this machine the fixture either exists (test media present) or the
            // entry is filtered out entirely.
            let file_exists = entry
                .torrent_file
                .as_ref()
                .map(|p| std::path::Path::new(p).is_file())
                .unwrap_or(false);
            assert!(file_exists, "available fixture must have its torrent file");
        }
    }

    #[test]
    fn page_level_sources_stay_present() {
        let provider = BundledContentSources::new();
        // Cosmos Laundromat has no verified in-app torrent: page-level only.
        let hits = provider.sources_for("movie", 358332);
        assert!(
            hits.iter().any(|h| h.source_kind == "official_page" && !h.playable_in_app),
            "page-level fallback must remain"
        );
    }
}
