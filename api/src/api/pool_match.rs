use axum::{
    extract::{Path, Query, State},
    routing::{get, patch},
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::api::auth::AuthenticatedUser;
use crate::api::AppState;
use crate::db::pool_match::{MatchPlayer, PoolMatch, PoolMatchDoc, Rating};
use crate::error::ApiError;
use crate::video;

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

#[derive(Serialize, Deserialize, Debug)]
pub struct RatingDto {
    #[serde(rename = "type")]
    pub rating_type: String,
    pub value: u16,
}

impl From<Rating> for RatingDto {
    fn from(r: Rating) -> Self {
        match r {
            Rating::Apa(v) => RatingDto {
                rating_type: "Apa".to_string(),
                value: v as u16,
            },
            Rating::Fargo(v) => RatingDto {
                rating_type: "Fargo".to_string(),
                value: v,
            },
        }
    }
}

impl TryFrom<RatingDto> for Rating {
    type Error = ApiError;
    fn try_from(d: RatingDto) -> Result<Self, ApiError> {
        match d.rating_type.as_str() {
            "Apa" => {
                if d.value > u8::MAX as u16 {
                    return Err(ApiError::BadRequest("APA rating must be 0-255".to_string()));
                }
                Ok(Rating::Apa(d.value as u8))
            }
            "Fargo" => Ok(Rating::Fargo(d.value)),
            _ => Err(ApiError::BadRequest(
                "rating type must be Apa or Fargo".to_string(),
            )),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MatchPlayerDto {
    pub name: String,
    pub race_to: u8,
    pub games_won: u8,
    pub rating: Option<RatingDto>,
}

impl From<MatchPlayer> for MatchPlayerDto {
    fn from(p: MatchPlayer) -> Self {
        Self {
            name: p.name,
            race_to: p.race_to,
            games_won: p.games_won,
            rating: p.rating.map(Into::into),
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ScoreHistoryEntryDto {
    pub player_one_games_won: u8,
    pub player_two_games_won: u8,
    pub timestamp: i64,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PoolMatchResponse {
    pub id: String,
    pub player_one: MatchPlayerDto,
    pub player_two: MatchPlayerDto,
    pub start_time: i64,
    pub end_time: Option<i64>,
    pub camera_id: String,
    pub camera_name: String,
    /// Display name of the user who started the match.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_by: Option<String>,
    /// Match description (supports newlines), used in live video post.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// History of score adjustments with timestamps, newest last.
    #[serde(default)]
    pub score_history: Vec<ScoreHistoryEntryDto>,
}

impl PoolMatchResponse {
    fn from_doc(doc: PoolMatchDoc, camera_name: String) -> Option<Self> {
        let id = doc.id?;
        let camera_id = doc.camera_id.as_ref().cloned().unwrap_or_default();
        let score_history = doc
            .score_history
            .into_iter()
            .map(|e| ScoreHistoryEntryDto {
                player_one_games_won: e.player_one_games_won,
                player_two_games_won: e.player_two_games_won,
                timestamp: e.timestamp.timestamp_millis(),
            })
            .collect();
        Some(Self {
            id: id.clone(),
            player_one: doc.player_one.into(),
            player_two: doc.player_two.into(),
            start_time: doc.start_time.timestamp_millis(),
            end_time: doc.end_time.map(|dt| dt.timestamp_millis()),
            camera_id,
            camera_name,
            started_by: doc.started_by_name,
            description: doc.description,
            score_history,
        })
    }
}

#[derive(Deserialize)]
pub struct MatchPlayerCreateDto {
    pub name: String,
    pub race_to: u8,
    pub rating: Option<RatingDto>,
}

#[derive(Deserialize)]
pub struct PoolMatchCreateRequest {
    pub player_one: MatchPlayerCreateDto,
    pub player_two: MatchPlayerCreateDto,
    pub camera_id: String,
    /// Optional match description (supports newlines), included in live video post.
    pub description: Option<String>,
}

#[derive(Deserialize)]
pub struct PoolMatchUpdateScoreRequest {
    pub player: u8,
    pub games_won: u8,
}

#[derive(Deserialize)]
pub struct ActiveMatchQuery {
    pub camera_id: String,
}

/// GET /api/pool-matches/active?camera_id=X - Get the active (ongoing) match for a camera.
pub async fn pool_matches_active(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Query(q): Query<ActiveMatchQuery>,
) -> Result<Json<Option<PoolMatchResponse>>, ApiError> {
    if !valid_id(&q.camera_id) {
        return Err(ApiError::BadRequest("Invalid camera_id".to_string()));
    }
    let m = app.db.find_active_pool_match_by_camera_id(&q.camera_id)?;
    let resp = m.and_then(|doc| {
        let camera_name = doc
            .camera_id
            .as_ref()
            .and_then(|cid| app.db.find_camera_by_id(cid).ok().flatten())
            .map(|c| c.name)
            .unwrap_or_default();
        PoolMatchResponse::from_doc(doc, camera_name)
    });
    Ok(Json(resp))
}

/// GET /api/pool-matches - List all pool matches. Public (no auth required).
pub async fn pool_matches_list(
    State(app): State<AppState>,
) -> Result<Json<Vec<PoolMatchResponse>>, ApiError> {
    let matches = app.db.list_pool_matches()?;
    let responses: Vec<PoolMatchResponse> = matches
        .into_iter()
        .filter_map(|doc| {
            let camera_name = doc
                .camera_id
                .as_ref()
                .and_then(|cid| app.db.find_camera_by_id(cid).ok().flatten())
                .map(|c| c.name)
                .unwrap_or_default();
            PoolMatchResponse::from_doc(doc, camera_name)
        })
        .collect();
    Ok(Json(responses))
}

/// GET /api/pool-matches/:id - Get a pool match by id.
pub async fn pool_matches_get(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PoolMatchResponse>, ApiError> {
    if !valid_id(&id) {
        return Err(ApiError::BadRequest("Invalid pool match id".to_string()));
    }
    let m = app
        .db
        .find_pool_match_by_id(&id)?
        .ok_or(ApiError::PoolMatchNotFound)?;
    let camera_name = m
        .camera_id
        .as_ref()
        .and_then(|cid| app.db.find_camera_by_id(cid).ok().flatten())
        .map(|c| c.name)
        .unwrap_or_default();
    PoolMatchResponse::from_doc(m, camera_name)
        .ok_or(ApiError::PoolMatchNotFound)
        .map(Json)
}

/// POST /api/pool-matches - Create a new pool match.
pub async fn pool_matches_create(
    auth: AuthenticatedUser,
    State(app): State<AppState>,
    Json(req): Json<PoolMatchCreateRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if req.player_one.name.is_empty() || req.player_two.name.is_empty() {
        return Err(ApiError::BadRequest(
            "player names are required".to_string(),
        ));
    }
    if !valid_id(&req.camera_id) {
        return Err(ApiError::BadRequest("Invalid camera_id".to_string()));
    }
    let _camera = app
        .db
        .find_camera_by_id(&req.camera_id)?
        .ok_or(ApiError::CameraNotFound)?;
    if req.player_one.race_to == 0 || req.player_two.race_to == 0 {
        return Err(ApiError::BadRequest(
            "race_to must be greater than 0".to_string(),
        ));
    }

    let player_one = MatchPlayer {
        name: req.player_one.name,
        race_to: req.player_one.race_to,
        games_won: 0,
        rating: req.player_one.rating.map(|r| r.try_into()).transpose()?,
    };
    let player_two = MatchPlayer {
        name: req.player_two.name,
        race_to: req.player_two.race_to,
        games_won: 0,
        rating: req.player_two.rating.map(|r| r.try_into()).transpose()?,
    };

    let match_data = PoolMatch {
        player_one,
        player_two,
        start_time: Utc::now(),
        end_time: None,
        camera_id: req.camera_id.clone(),
        started_by_sub: Some(auth.sub),
        started_by_name: Some(auth.name),
        description: req.description.filter(|s| !s.trim().is_empty()),
    };

    let id = app.db.create_pool_match(match_data)?;
    video::update_overlay(
        &app.db,
        &app.overlay,
        &req.camera_id,
        &app.rtmp_processes,
        None,
    );
    Ok(Json(serde_json::json!({ "id": id })))
}

/// PATCH /api/pool-matches/:id/score - Update games_won for a player. Sets end_time when games_won == race_to.
/// Only the user who created the match can update it.
pub async fn pool_matches_update_score(
    auth: AuthenticatedUser,
    State(app): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<PoolMatchUpdateScoreRequest>,
) -> Result<Json<PoolMatchResponse>, ApiError> {
    tracing::debug!(match_id = %id, player = req.player, games_won = req.games_won, "update score: entry");
    if !valid_id(&id) {
        return Err(ApiError::BadRequest("Invalid pool match id".to_string()));
    }
    tracing::debug!(match_id = %id, "update score: fetching match doc");
    let doc = app
        .db
        .find_pool_match_by_id(&id)?
        .ok_or(ApiError::PoolMatchNotFound)?;
    tracing::debug!(match_id = %id, "update score: got match doc, checking end_time and auth");
    if doc.end_time.is_some() {
        return Err(ApiError::BadRequest(
            "Cannot update an ended match".to_string(),
        ));
    }
    let can_update = doc
        .started_by_sub
        .as_ref()
        .map(|sub| sub == &auth.sub)
        .unwrap_or(false);
    if !can_update {
        return Err(ApiError::Forbidden(
            "Only the person who created the match can update it".to_string(),
        ));
    }
    tracing::debug!(match_id = %id, "update score: calling update_pool_match_games_won");
    let updated = app
        .db
        .update_pool_match_games_won(&id, req.player, req.games_won)?;
    tracing::debug!(match_id = %id, "update score: db update done, updating overlay");
    if let Some(ref cid) = updated.camera_id {
        if updated.end_time.is_some() {
            video::clear_overlay(&app.db, &app.overlay, cid, &app.rtmp_processes);
        } else {
            video::update_overlay(
                &app.db,
                &app.overlay,
                cid,
                &app.rtmp_processes,
                Some(video::MatchOverlay {
                    player_one: video::OverlayPlayer::from_match_player(&updated.player_one),
                    player_two: video::OverlayPlayer::from_match_player(&updated.player_two),
                }),
            );
        }
    }
    tracing::debug!(match_id = %id, "update score: overlay done, fetching camera name");
    let camera_name = updated
        .camera_id
        .as_ref()
        .and_then(|cid| app.db.find_camera_by_id(cid).ok().flatten())
        .map(|c| c.name)
        .unwrap_or_default();
    tracing::debug!(match_id = %id, "update score: building response");
    PoolMatchResponse::from_doc(updated, camera_name)
        .ok_or(ApiError::PoolMatchNotFound)
        .map(Json)
}

/// PATCH /api/pool-matches/:id/end - End the match early (set end_time).
/// The match creator or an admin can end a match.
pub async fn pool_matches_end(
    auth: AuthenticatedUser,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PoolMatchResponse>, ApiError> {
    tracing::debug!(match_id = %id, "end match: entry");
    if !valid_id(&id) {
        return Err(ApiError::BadRequest("Invalid pool match id".to_string()));
    }
    tracing::debug!(match_id = %id, "end match: fetching match doc");
    let doc = app
        .db
        .find_pool_match_by_id(&id)?
        .ok_or(ApiError::PoolMatchNotFound)?;
    tracing::debug!(match_id = %id, "end match: got match doc, checking auth");
    let can_end = auth.is_admin
        || doc
            .started_by_sub
            .as_ref()
            .map(|sub| sub == &auth.sub)
            .unwrap_or(false);
    if !can_end {
        return Err(ApiError::Forbidden(
            "Only the match creator or an admin can end the match".to_string(),
        ));
    }
    tracing::debug!(match_id = %id, "end match: calling end_pool_match");
    let updated = app.db.end_pool_match(&id)?;
    tracing::debug!(match_id = %id, "end match: db update done, clearing overlay");
    if let Some(ref cid) = updated.camera_id {
        video::clear_overlay(&app.db, &app.overlay, cid, &app.rtmp_processes);
    }
    tracing::debug!(match_id = %id, "end match: overlay done, fetching camera name");
    let camera_name = updated
        .camera_id
        .as_ref()
        .and_then(|cid| app.db.find_camera_by_id(cid).ok().flatten())
        .map(|c| c.name)
        .unwrap_or_default();
    tracing::debug!(match_id = %id, "end match: building response");
    PoolMatchResponse::from_doc(updated, camera_name)
        .ok_or(ApiError::PoolMatchNotFound)
        .map(Json)
}

/// DELETE /api/pool-matches/:id - Delete a pool match.
pub async fn pool_matches_delete(
    _auth: AuthenticatedUser,
    State(app): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    if !valid_id(&id) {
        return Err(ApiError::BadRequest("Invalid pool match id".to_string()));
    }
    let match_doc = app.db.find_pool_match_by_id(&id)?;
    let camera_id = match_doc.as_ref().and_then(|m| m.camera_id.clone());
    let deleted = app.db.delete_pool_match(&id)?;
    if !deleted {
        return Err(ApiError::PoolMatchNotFound);
    }
    if let Some(ref cid) = camera_id {
        video::clear_overlay(&app.db, &app.overlay, cid, &app.rtmp_processes);
    }
    Ok(Json(serde_json::json!({ "ok": true })))
}

pub fn routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/pool-matches/active", get(pool_matches_active))
        .route(
            "/api/pool-matches",
            get(pool_matches_list).post(pool_matches_create),
        )
        .route(
            "/api/pool-matches/:id",
            get(pool_matches_get).delete(pool_matches_delete),
        )
        .route(
            "/api/pool-matches/:id/score",
            patch(pool_matches_update_score),
        )
        .route("/api/pool-matches/:id/end", patch(pool_matches_end))
}
