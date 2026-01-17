//! Broadcast routes.

use askama::Template;
use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::error::Result;
use crate::state::AppState;

/// Broadcast page template.
#[derive(Template)]
#[template(path = "broadcast.html")]
pub struct BroadcastTemplate {
    pub topics: Vec<TopicInfo>,
}

/// Topic information for the broadcast form.
#[derive(Clone, Serialize)]
pub struct TopicInfo {
    pub slug: String,
    pub subscriber_count: i64,
}

/// Request to preview broadcast recipients.
#[derive(Deserialize)]
pub struct PreviewRequest {
    pub topics: Vec<String>,
}

/// Preview response showing recipient count.
#[derive(Serialize)]
pub struct PreviewResponse {
    pub recipient_count: usize,
    pub recipients: Vec<RecipientInfo>,
}

/// Information about a recipient.
#[derive(Serialize)]
pub struct RecipientInfo {
    pub id: String,
    pub name: String,
}

/// Request to send a broadcast.
#[derive(Deserialize)]
pub struct BroadcastRequest {
    pub topics: Vec<String>,
    pub message: String,
}

/// Broadcast send result.
#[derive(Serialize)]
pub struct BroadcastResponse {
    pub sent: usize,
    pub failed: usize,
    pub errors: Vec<String>,
}

/// Render the broadcast page.
pub async fn broadcast_page(State(state): State<AppState>) -> Result<BroadcastTemplate> {
    let topics = get_topics(&state).await?;
    Ok(BroadcastTemplate { topics })
}

/// Get topics with subscriber counts as JSON.
pub async fn topics_api(State(state): State<AppState>) -> Result<Json<Vec<TopicInfo>>> {
    let topics = get_topics(&state).await?;
    Ok(Json(topics))
}

/// Preview recipients for selected topics.
pub async fn preview_api(
    State(state): State<AppState>,
    Json(req): Json<PreviewRequest>,
) -> Result<Json<PreviewResponse>> {
    let pool = state.db.pool();
    let mut seen = std::collections::HashSet::new();
    let mut recipients = Vec::new();

    for topic_slug in &req.topics {
        let subscribers = database::notification::get_topic_subscribers(pool, topic_slug).await?;
        for user in subscribers {
            if seen.insert(user.id.clone()) {
                recipients.push(RecipientInfo {
                    id: user.id,
                    name: user.name,
                });
            }
        }
    }

    Ok(Json(PreviewResponse {
        recipient_count: recipients.len(),
        recipients,
    }))
}

/// Send broadcast to subscribers of selected topics.
pub async fn send_api(
    State(state): State<AppState>,
    Json(req): Json<BroadcastRequest>,
) -> Result<Json<BroadcastResponse>> {
    let pool = state.db.pool();
    let mut seen = std::collections::HashSet::new();
    let mut recipient_ids = Vec::new();

    // Collect unique recipients across all topics
    for topic_slug in &req.topics {
        let subscribers = database::notification::get_topic_subscribers(pool, topic_slug).await?;
        for user in subscribers {
            if seen.insert(user.id.clone()) {
                recipient_ids.push(user.id);
            }
        }
    }

    info!(
        topics = ?req.topics,
        recipient_count = recipient_ids.len(),
        "Sending broadcast"
    );

    let mut sent = 0;
    let mut failed = 0;
    let mut errors = Vec::new();

    for recipient_id in recipient_ids {
        match state.broadcaster.send_text(&recipient_id, &req.message).await {
            Ok(_) => {
                sent += 1;
                info!(recipient = %recipient_id, "Broadcast sent");
            }
            Err(err) => {
                failed += 1;
                let error_msg = format!("{}: {}", recipient_id, err);
                errors.push(error_msg.clone());
                tracing::warn!(recipient = %recipient_id, error = %err, "Broadcast failed");
            }
        }
    }

    info!(sent, failed, "Broadcast complete");

    Ok(Json(BroadcastResponse { sent, failed, errors }))
}

/// Fetch topics with subscriber counts.
async fn get_topics(state: &AppState) -> Result<Vec<TopicInfo>> {
    let pool = state.db.pool();
    let topics_with_counts = database::topic::list_topics_with_subscriber_counts(pool).await?;

    Ok(topics_with_counts
        .into_iter()
        .map(|(slug, count)| TopicInfo {
            slug,
            subscriber_count: count,
        })
        .collect())
}
