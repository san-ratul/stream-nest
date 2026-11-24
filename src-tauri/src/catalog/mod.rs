pub mod provider;

use crate::catalog::provider::{CatalogEntry, CatalogProvider};
use serde::Serialize;
use tauri::State;

pub struct CatalogState {
    pub provider: Box<dyn CatalogProvider + Send + Sync>,
}

#[derive(Serialize)]
pub struct CatalogQueryResult {
    pub entries: Vec<CatalogEntry>,
    pub source_note: String,
}

#[tauri::command]
pub fn catalog_list(
    state: State<'_, CatalogState>,
    query: Option<String>,
    genre: Option<String>,
) -> CommandResultCatalog {
    let entries = state.provider.list(query.as_deref(), genre.as_deref());
    Ok(CatalogQueryResult {
        entries,
        source_note: state.provider.source_note().to_string(),
    })
}

#[tauri::command]
pub fn catalog_get(state: State<'_, CatalogState>, id: String) -> CommandResultCatalog {
    let entries = state
        .provider
        .list(None, None)
        .into_iter()
        .filter(|e| e.id == id)
        .collect();
    Ok(CatalogQueryResult {
        entries,
        source_note: state.provider.source_note().to_string(),
    })
}

pub type CommandResultCatalog = Result<CatalogQueryResult, CommandErrorCatalog>;

#[derive(Serialize)]
pub struct CommandErrorCatalog {
    pub message: String,
}

impl From<anyhow::Error> for CommandErrorCatalog {
    fn from(e: anyhow::Error) -> Self {
        Self {
            message: e.to_string(),
        }
    }
}
