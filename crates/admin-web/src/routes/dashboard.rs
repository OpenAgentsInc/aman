//! Dashboard routes.

use askama::Template;
use axum::extract::State;
use axum::Json;
use proton_proxy::ImapClient;
use serde::Serialize;
use tracing::warn;

use crate::error::Result;
use crate::state::AppState;

/// Dashboard page template.
#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub stats: Stats,
}

/// Dashboard statistics.
#[derive(Clone, Serialize)]
pub struct Stats {
    pub user_count: i64,
    pub languages: Vec<LanguageStats>,
    pub proton: Option<ProtonStats>,
}

/// Statistics for a language.
#[derive(Clone, Serialize)]
pub struct LanguageStats {
    pub language: String,
    pub user_count: i64,
}

/// Proton Mail statistics.
#[derive(Clone, Serialize)]
pub struct ProtonStats {
    pub unread_count: usize,
    pub total_count: u32,
    pub email: String,
    pub error: Option<String>,
}

/// Render the dashboard page.
pub async fn dashboard_page(State(state): State<AppState>) -> Result<DashboardTemplate> {
    let stats = get_stats(&state).await?;
    Ok(DashboardTemplate { stats })
}

/// Get dashboard statistics as JSON.
pub async fn stats_api(State(state): State<AppState>) -> Result<Json<Stats>> {
    let stats = get_stats(&state).await?;
    Ok(Json(stats))
}

/// Fetch statistics from the database.
async fn get_stats(state: &AppState) -> Result<Stats> {
    let pool = state.db.pool();

    let user_count = database::user::count_users(pool).await?;
    let languages = database::user::count_users_by_language(pool).await?;

    let languages = languages
        .into_iter()
        .map(|(language, count)| LanguageStats {
            language,
            user_count: count,
        })
        .collect();

    // Fetch Proton Mail stats if configured
    let proton = if let Some(config) = &state.proton_config {
        Some(get_proton_stats(config).await)
    } else {
        None
    };

    Ok(Stats {
        user_count,
        languages,
        proton,
    })
}

/// Fetch Proton Mail statistics via IMAP.
async fn get_proton_stats(config: &proton_proxy::ProtonConfig) -> ProtonStats {
    let email = config.username.clone();

    match ImapClient::connect(config).await {
        Ok(mut client) => {
            // Select INBOX and get total count
            let total_count = match client.select_folder("INBOX").await {
                Ok(count) => count,
                Err(e) => {
                    warn!("Failed to select INBOX: {}", e);
                    return ProtonStats {
                        unread_count: 0,
                        total_count: 0,
                        email,
                        error: Some(format!("Failed to select INBOX: {}", e)),
                    };
                }
            };

            // Search for unread messages
            let unread_count = match client.search_unread().await {
                Ok(uids) => uids.len(),
                Err(e) => {
                    warn!("Failed to search unread: {}", e);
                    0
                }
            };

            // Logout (ignore errors)
            let _ = client.logout().await;

            ProtonStats {
                unread_count,
                total_count,
                email,
                error: None,
            }
        }
        Err(e) => {
            warn!("Failed to connect to Proton IMAP: {}", e);
            ProtonStats {
                unread_count: 0,
                total_count: 0,
                email,
                error: Some(format!("Connection failed: {}", e)),
            }
        }
    }
}
