use streamnest_lib::catalog::provider::{BundledProvider, CatalogProvider};

#[test]
fn bundled_catalog_lists_all_entries() {
    let p = BundledProvider::new();
    let all = p.list(None, None);
    assert_eq!(all.len(), 4, "bundled catalog must contain exactly 4 entries");
    assert!(all.iter().all(|e| !e.source_url.is_empty()));
    assert!(all.iter().all(|e| e.license.to_lowercase().contains("cc-by")));
}

#[test]
fn catalog_search_matches_title_and_description() {
    let p = BundledProvider::new();
    let by_title = p.list(Some("sintel"), None);
    assert_eq!(by_title.len(), 1);
    assert_eq!(by_title[0].title, "Sintel");
    let by_desc = p.list(Some("dragon"), None);
    assert_eq!(by_desc.len(), 1);
    assert_eq!(by_desc[0].id, "sintel-2010");
}

#[test]
fn catalog_genre_filter_works() {
    let p = BundledProvider::new();
    let sci_fi = p.list(None, Some("sci-fi"));
    assert_eq!(sci_fi.len(), 1);
    assert_eq!(sci_fi[0].id, "tos-2012");
    let none = p.list(None, Some("horror"));
    assert!(none.is_empty());
}
