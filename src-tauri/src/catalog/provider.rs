use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogEntry {
    pub id: String,
    pub title: String,
    pub year: u16,
    pub description: String,
    pub genres: Vec<String>,
    pub duration_minutes: u16,
    pub source_name: String,
    pub source_url: String,
    pub license: String,
    pub license_url: String,
}

pub trait CatalogProvider {
    fn list(&self, query: Option<&str>, genre: Option<&str>) -> Vec<CatalogEntry>;
    fn source_note(&self) -> &str;
}

/// Curated, embedded catalog of documented legal content.
/// Entries reference official project pages only; no torrent indexes.
pub struct BundledProvider {
    entries: Vec<CatalogEntry>,
}

impl BundledProvider {
    pub fn new() -> Self {
        let raw = include_str!("../../resources/catalog.json");
        let entries: Vec<CatalogEntry> =
            serde_json::from_str(raw).expect("bundled catalog.json must be valid");
        Self { entries }
    }
}

impl CatalogProvider for BundledProvider {
    fn list(&self, query: Option<&str>, genre: Option<&str>) -> Vec<CatalogEntry> {
        self.entries
            .iter()
            .filter(|e| match query {
                Some(q) if !q.is_empty() => {
                    let q = q.to_lowercase();
                    e.title.to_lowercase().contains(&q)
                        || e.description.to_lowercase().contains(&q)
                        || e.genres
                            .iter()
                            .any(|g| g.to_lowercase().contains(&q))
                }
                _ => true,
            })
            .filter(|e| match genre {
                Some(g) if !g.is_empty() && g != "all" => {
                    e.genres.iter().any(|eg| eg.eq_ignore_ascii_case(g))
                }
                _ => true,
            })
            .cloned()
            .collect()
    }

    fn source_note(&self) -> &str {
        "Curated legal content. Entries link to official project pages with documented licenses. StreamNest does not index third-party torrents."
    }
}
