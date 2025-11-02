use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{error::ApiError, repository::DashboardRepository, Result};
use models::Snapshot;

pub type RepositoryState = Arc<dyn DashboardRepository>;

/// Query parameters for dashboard summary endpoint
#[derive(Debug, Deserialize)]
pub struct SummaryQuery {
    #[serde(default)]
    pub view: Option<String>, // "real_purchasing_power" or "inflation_adjusted"
}

/// Response for the summary endpoint
#[derive(Debug, Serialize)]
pub struct SummaryResponse {
    pub generated_at: String,
    pub base_currency: String,
    pub latest: Snapshot,
}

/// GET /api/dashboard
/// Returns the complete dashboard with all snapshots
pub async fn get_dashboard(
    State(repo): State<RepositoryState>,
) -> Result<impl IntoResponse> {
    let dashboard = repo.fetch_dashboard().await?;
    let dashboard = dashboard.rounded(); // Round all values to 2 decimals
    
    let generated_at = dashboard.generated_at.clone();
    let etag = format!("\"{}\"", generated_at);
    
    let mut headers = HeaderMap::new();
    headers.insert(header::ETAG, etag.parse().unwrap());
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=60".parse().unwrap(),
    );

    Ok((StatusCode::OK, headers, Json(dashboard)))
}

/// GET /api/dashboard/latest
/// Returns only the latest snapshot
pub async fn get_latest_snapshot(
    State(repo): State<RepositoryState>,
) -> Result<impl IntoResponse> {
    let snapshot = repo.fetch_latest_snapshot().await?;
    let snapshot = snapshot.rounded();
    
    Ok(Json(snapshot))
}

/// GET /api/dashboard/summary?view=real_purchasing_power
/// Returns a reduced payload optimized for quick cards/charts
pub async fn get_summary(
    State(repo): State<RepositoryState>,
    Query(params): Query<SummaryQuery>,
) -> Result<impl IntoResponse> {
    let dashboard = repo.fetch_dashboard().await?;
    let latest = dashboard
        .latest
        .or_else(|| dashboard.snapshots.last().cloned())
        .ok_or(ApiError::DashboardNotFound)?;

    let response = SummaryResponse {
        generated_at: dashboard.generated_at,
        base_currency: dashboard.base_currency,
        latest: match params.view.as_deref() {
            Some("real_purchasing_power") | Some("inflation_adjusted") => latest.rounded(),
            _ => latest.rounded(),
        },
    };

    Ok(Json(response))
}

/// GET /api/snapshots/:date/entries
/// Returns raw entry-level data for a specific snapshot date
/// Date format: YYYY-MM-DD
pub async fn get_snapshot_entries(
    State(repo): State<RepositoryState>,
    Path(date_str): Path<String>,
) -> Result<impl IntoResponse> {
    let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
        .map_err(|_| ApiError::InvalidDateFormat(date_str.clone()))?;

    let document = repo.fetch_entries_by_date(date).await?;
    
    Ok(Json(document))
}

/// Response for the entries endpoint with enriched data
#[derive(Debug, Serialize)]
pub struct EntriesResponse {
    pub date: String,
    pub base_currency: String,
    pub entries: Vec<EnrichedEntry>,
    pub metadata: EntryMetadata,
}

#[derive(Debug, Serialize)]
pub struct EnrichedEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub currency: String,
    pub balance: f64,
    pub balance_in_base: f64, // Converted to base currency
    pub comment: String,
}

#[derive(Debug, Serialize)]
pub struct EntryMetadata {
    pub reference_month: Option<String>,
    pub fx_rates: Option<std::collections::HashMap<String, f64>>,
    pub hicp: Option<f64>,
}

/// GET /api/snapshots/:date/entries/enriched
/// Returns enriched entry-level data with FX conversion
pub async fn get_snapshot_entries_enriched(
    State(repo): State<RepositoryState>,
    Path(date_str): Path<String>,
) -> Result<impl IntoResponse> {
    let date = NaiveDate::parse_from_str(&date_str, "%Y-%m-%d")
        .map_err(|_| ApiError::InvalidDateFormat(date_str.clone()))?;

    let document = repo.fetch_entries_by_date(date).await?;
    let fx_rates = document.get_fx_rates();
    let base_currency = document.metadata.base_currency
        .clone()
        .unwrap_or_else(|| "EUR".to_string());

    let enriched_entries: Vec<EnrichedEntry> = document
        .net_worth_entries
        .iter()
        .map(|entry| {
            let fx_rate = if entry.currency == base_currency {
                1.0
            } else {
                *fx_rates.get(&entry.currency).unwrap_or(&1.0)
            };
            
            EnrichedEntry {
                name: entry.name.clone(),
                kind: entry.kind.clone(),
                currency: entry.currency.clone(),
                balance: entry.balance,
                balance_in_base: entry.balance * fx_rate,
                comment: entry.comment.clone(),
            }
        })
        .collect();

    let response = EntriesResponse {
        date: document.metadata.date.clone(),
        base_currency,
        entries: enriched_entries,
        metadata: EntryMetadata {
            reference_month: document.metadata.reference_month.clone(),
            fx_rates: document.metadata.fx_rates.clone(),
            hicp: document.metadata.hicp,
        },
    };

    Ok(Json(response))
}

/// GET /health
/// Health check endpoint
pub async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "net-worth-api"
    }))
}
