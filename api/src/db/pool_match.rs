use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{new_id, Db, Id};
use crate::error::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Rating {
    Apa(u8),
    Fargo(u16),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchPlayer {
    pub name: String,
    pub race_to: u8,
    pub games_won: u8,
    pub rating: Option<Rating>,
}

/// A single score adjustment event. Stores the score state after the change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreHistoryEntry {
    pub player_one_games_won: u8,
    pub player_two_games_won: u8,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMatch {
    pub player_one: MatchPlayer,
    pub player_two: MatchPlayer,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub camera_id: Id,
    #[serde(default)]
    pub started_by_sub: Option<String>,
    #[serde(default)]
    pub started_by_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub match_type: MatchType,
}

/// Match type: standard (two players) or practice (single player, racks count).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MatchType {
    #[default]
    Standard,
    Practice,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMatchDoc {
    pub id: Option<Id>,
    pub player_one: MatchPlayer,
    pub player_two: MatchPlayer,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    #[serde(default)]
    pub camera_id: Option<Id>,
    #[serde(default)]
    pub started_by_sub: Option<String>,
    #[serde(default)]
    pub started_by_name: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub score_history: Vec<ScoreHistoryEntry>,
    #[serde(default)]
    pub match_type: MatchType,
}

fn parse_match_doc(
    id: String,
    player_one: String,
    player_two: String,
    start_time: String,
    end_time: Option<String>,
    camera_id: Option<String>,
    started_by_sub: Option<String>,
    started_by_name: Option<String>,
    description: Option<String>,
    score_history: String,
    match_type: Option<String>,
) -> Result<PoolMatchDoc, ApiError> {
    let player_one: MatchPlayer =
        serde_json::from_str(&player_one).map_err(|e| ApiError::Unknown(e.to_string()))?;
    let player_two: MatchPlayer =
        serde_json::from_str(&player_two).map_err(|e| ApiError::Unknown(e.to_string()))?;
    let start_time: DateTime<Utc> = chrono::DateTime::parse_from_rfc3339(&start_time)
        .map_err(|e| ApiError::Unknown(e.to_string()))?
        .with_timezone(&Utc);
    let end_time = end_time
        .map(|s| {
            chrono::DateTime::parse_from_rfc3339(&s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ApiError::Unknown(e.to_string()))
        })
        .transpose()?;
    let score_history: Vec<ScoreHistoryEntry> =
        serde_json::from_str(&score_history).unwrap_or_default();
    let match_type = match match_type.as_deref() {
        Some("practice") => MatchType::Practice,
        _ => MatchType::Standard,
    };
    Ok(PoolMatchDoc {
        id: Some(id),
        player_one,
        player_two,
        start_time,
        end_time,
        camera_id,
        started_by_sub,
        started_by_name,
        description,
        score_history,
        match_type,
    })
}

impl Db {
    /// List all pool matches.
    pub fn list_pool_matches(&self) -> Result<Vec<PoolMatchDoc>, ApiError> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, player_one, player_two, start_time, end_time, camera_id, started_by_sub, started_by_name, description, score_history, match_type FROM pool_matches",
            )?;
            let rows = stmt.query_map([], |row| {
                parse_match_doc(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get::<_, String>(9).unwrap_or_else(|_| "[]".to_string()),
                    row.get::<_, Option<String>>(10).ok().flatten(),
                )
                .map_err(|e| rusqlite::Error::InvalidParameterName(e.to_string()))
            })?;
            Ok(rows.collect::<Result<Vec<_>, _>>()?)
        })
    }

    /// Find pool match by id.
    pub fn find_pool_match_by_id(&self, id: &str) -> Result<Option<PoolMatchDoc>, ApiError> {
        tracing::debug!(match_id = %id, "find_pool_match_by_id: acquiring db lock");
        self.execute(|conn| {
            tracing::debug!(match_id = %id, "find_pool_match_by_id: db lock acquired");
            let mut stmt = conn.prepare(
                "SELECT id, player_one, player_two, start_time, end_time, camera_id, started_by_sub, started_by_name, description, score_history, match_type FROM pool_matches WHERE id = ?1",
            )?;
            let mut rows = stmt.query([id])?;
            if let Some(row) = rows.next()? {
                let doc = parse_match_doc(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get::<_, String>(9).unwrap_or_else(|_| "[]".to_string()),
                    row.get::<_, Option<String>>(10).ok().flatten(),
                )?;
                Ok(Some(doc))
            } else {
                Ok(None)
            }
        })
    }

    /// Find the active (ongoing) pool match for a camera. Returns the match if end_time is null.
    pub fn find_active_pool_match_by_camera_id(
        &self,
        camera_id: &str,
    ) -> Result<Option<PoolMatchDoc>, ApiError> {
        self.execute(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, player_one, player_two, start_time, end_time, camera_id, started_by_sub, started_by_name, description, score_history, match_type FROM pool_matches WHERE camera_id = ?1 AND end_time IS NULL",
            )?;
            let mut rows = stmt.query([camera_id])?;
            if let Some(row) = rows.next()? {
                let doc = parse_match_doc(
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                    row.get::<_, String>(9).unwrap_or_else(|_| "[]".to_string()),
                    row.get::<_, Option<String>>(10).ok().flatten(),
                )?;
                Ok(Some(doc))
            } else {
                Ok(None)
            }
        })
    }

    /// Create a new pool match. Fails if there is already an active match for this camera.
    pub fn create_pool_match(&self, match_data: PoolMatch) -> Result<Id, ApiError> {
        if self
            .find_active_pool_match_by_camera_id(&match_data.camera_id)?
            .is_some()
        {
            return Err(ApiError::BadRequest(
                "An active match already exists for this camera".to_string(),
            ));
        }
        let id = new_id();
        let player_one = serde_json::to_string(&match_data.player_one)
            .map_err(|e| ApiError::Unknown(e.to_string()))?;
        let player_two = serde_json::to_string(&match_data.player_two)
            .map_err(|e| ApiError::Unknown(e.to_string()))?;
        let start_time = match_data.start_time.to_rfc3339();
        let end_time = match_data.end_time.map(|dt| dt.to_rfc3339());
        let match_type = match match_data.match_type {
            MatchType::Practice => "practice",
            MatchType::Standard => "standard",
        };
        self.execute(|conn| {
            conn.execute(
                "INSERT INTO pool_matches (id, player_one, player_two, start_time, end_time, camera_id, started_by_sub, started_by_name, description, score_history, match_type) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '[]', ?10)",
                rusqlite::params![
                    id,
                    player_one,
                    player_two,
                    start_time,
                    end_time,
                    match_data.camera_id,
                    match_data.started_by_sub,
                    match_data.started_by_name,
                    match_data.description,
                    match_type,
                ],
            )?;
            Ok(id)
        })
    }

    /// Update games_won for a player. When games_won == race_to for that player, sets end_time.
    pub fn update_pool_match_games_won(
        &self,
        id: &str,
        player: u8,
        games_won: u8,
    ) -> Result<PoolMatchDoc, ApiError> {
        tracing::debug!(match_id = %id, "update_pool_match_games_won: entry");
        let doc = self
            .find_pool_match_by_id(id)?
            .ok_or(ApiError::PoolMatchNotFound)?;

        tracing::info!("setting player {} to {} for id {}", player, games_won, id);

        let race_to = match player {
            1 => doc.player_one.race_to,
            2 => doc.player_two.race_to,
            _ => return Err(ApiError::BadRequest("player must be 1 or 2".to_string())),
        };

        // For practice (race_to=0) there is no limit; for standard, enforce race_to
        if race_to > 0 && games_won > race_to {
            return Err(ApiError::BadRequest(format!(
                "games_won ({}) cannot exceed race_to ({})",
                games_won, race_to
            )));
        }

        let current_games_won = match player {
            1 => doc.player_one.games_won,
            2 => doc.player_two.games_won,
            _ => unreachable!(),
        };
        let is_correction = games_won < current_games_won;

        let (player_one_games_won, player_two_games_won) = match player {
            1 => (games_won, doc.player_two.games_won),
            2 => (doc.player_one.games_won, games_won),
            _ => unreachable!(),
        };

        let history_entry = ScoreHistoryEntry {
            player_one_games_won,
            player_two_games_won,
            timestamp: Utc::now(),
        };

        let (mut player_one, mut player_two) = (doc.player_one.clone(), doc.player_two.clone());
        match player {
            1 => player_one.games_won = games_won,
            2 => player_two.games_won = games_won,
            _ => unreachable!(),
        }

        let mut score_history = doc.score_history.clone();
        if is_correction {
            if player_one_games_won == 0 && player_two_games_won == 0 {
                score_history.clear();
            } else {
                score_history.pop();
            }
        } else {
            score_history.push(history_entry);
        }

        // Practice (race_to=0) never auto-ends; standard ends when games_won == race_to
        let end_time = if race_to > 0 && games_won == race_to {
            Some(Utc::now())
        } else if doc.end_time.is_some() && is_correction {
            None
        } else {
            doc.end_time
        };

        let player_one_json =
            serde_json::to_string(&player_one).map_err(|e| ApiError::Unknown(e.to_string()))?;
        let player_two_json =
            serde_json::to_string(&player_two).map_err(|e| ApiError::Unknown(e.to_string()))?;
        let score_history_json =
            serde_json::to_string(&score_history).map_err(|e| ApiError::Unknown(e.to_string()))?;
        let end_time_str = end_time.as_ref().map(|dt| dt.to_rfc3339());

        tracing::debug!(match_id = %id, "update_pool_match_games_won: executing UPDATE");
        self.execute(|conn| {
            let changed = conn.execute(
                "UPDATE pool_matches SET player_one = ?1, player_two = ?2, end_time = ?3, score_history = ?4 WHERE id = ?5",
                rusqlite::params![
                    player_one_json,
                    player_two_json,
                    end_time_str,
                    score_history_json,
                    id,
                ],
            )?;
            if changed == 0 {
                return Err(ApiError::PoolMatchNotFound);
            }
            Ok(())
        })?;

        tracing::debug!(match_id = %id, "update_pool_match_games_won: UPDATE done, re-fetching match");
        self.find_pool_match_by_id(id)?
            .ok_or(ApiError::PoolMatchNotFound)
    }

    /// End a pool match early by setting end_time. No-op if already ended.
    pub fn end_pool_match(&self, id: &str) -> Result<PoolMatchDoc, ApiError> {
        let doc = self
            .find_pool_match_by_id(id)?
            .ok_or(ApiError::PoolMatchNotFound)?;
        if doc.end_time.is_some() {
            return Ok(doc);
        }
        let end_time = Utc::now().to_rfc3339();
        self.execute(|conn| {
            let changed = conn.execute(
                "UPDATE pool_matches SET end_time = ?1 WHERE id = ?2",
                rusqlite::params![end_time, id],
            )?;
            if changed == 0 {
                return Err(ApiError::PoolMatchNotFound);
            }
            Ok(())
        })?;
        self.find_pool_match_by_id(id)?
            .ok_or(ApiError::PoolMatchNotFound)
    }

    /// Delete a pool match.
    pub fn delete_pool_match(&self, id: &str) -> Result<bool, ApiError> {
        self.execute(|conn| {
            let changed = conn.execute("DELETE FROM pool_matches WHERE id = ?1", [id])?;
            Ok(changed > 0)
        })
    }
}
