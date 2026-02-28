use polodb_core::bson::{doc, oid::ObjectId, DateTime, Document};
use polodb_core::CollectionT;
use serde::{Deserialize, Serialize};

use super::Db;
use crate::error::ApiError;

const POOL_MATCHES_COLLECTION: &str = "pool_matches";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Rating {
    Apa(u8),
    Fargo(u8),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchPlayer {
    pub name: String,
    pub race_to: u8,
    pub games_won: u8,
    pub rating: Option<Rating>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMatch {
    pub player_one: MatchPlayer,
    pub player_two: MatchPlayer,
    pub start_time: DateTime,
    pub end_time: Option<DateTime>,
    pub camera_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolMatchDoc {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<ObjectId>,
    pub player_one: MatchPlayer,
    pub player_two: MatchPlayer,
    pub start_time: DateTime,
    pub end_time: Option<DateTime>,
    pub camera_name: String,
}

impl Db {
    /// List all pool matches.
    pub fn list_pool_matches(&self) -> Result<Vec<PoolMatchDoc>, ApiError> {
        let collection = self.0.collection::<PoolMatchDoc>(POOL_MATCHES_COLLECTION);
        let cursor = collection.find(doc! {}).run()?;
        Ok(cursor.collect::<Result<Vec<_>, polodb_core::Error>>()?)
    }

    /// Find pool match by id.
    pub fn find_pool_match_by_id(&self, id: &ObjectId) -> Result<Option<PoolMatchDoc>, ApiError> {
        let collection = self.0.collection::<PoolMatchDoc>(POOL_MATCHES_COLLECTION);
        Ok(collection.find_one(doc! { "_id": id })?)
    }

    /// Find the active (ongoing) pool match for a camera. Returns the match if end_time is null.
    pub fn find_active_pool_match_by_camera_name(
        &self,
        camera_name: &str,
    ) -> Result<Option<PoolMatchDoc>, ApiError> {
        let collection = self.0.collection::<PoolMatchDoc>(POOL_MATCHES_COLLECTION);
        Ok(collection.find_one(doc! {
            "camera_name": camera_name,
            "end_time": null
        })?)
    }

    /// Create a new pool match. Fails if there is already an active match for this camera.
    pub fn create_pool_match(&self, match_data: PoolMatch) -> Result<ObjectId, ApiError> {
        if self
            .find_active_pool_match_by_camera_name(&match_data.camera_name)?
            .is_some()
        {
            return Err(ApiError::BadRequest(
                "An active match already exists for this camera".to_string(),
            ));
        }
        let collection = self.0.collection::<PoolMatchDoc>(POOL_MATCHES_COLLECTION);
        let doc = PoolMatchDoc {
            id: None,
            player_one: match_data.player_one,
            player_two: match_data.player_two,
            start_time: match_data.start_time,
            end_time: match_data.end_time,
            camera_name: match_data.camera_name,
        };
        let result = collection.insert_one(doc)?;
        result
            .inserted_id
            .as_object_id()
            .ok_or_else(|| ApiError::BadRequest("Failed to get inserted id".to_string()))
    }

    /// Update games_won for a player. When games_won == race_to for that player, sets end_time.
    /// Only games_won can be updated after creation.
    /// `player` is 1 for player_one, 2 for player_two.
    pub fn update_pool_match_games_won(
        &self,
        id: &ObjectId,
        player: u8,
        games_won: u8,
    ) -> Result<PoolMatchDoc, ApiError> {
        let collection = self.0.collection::<PoolMatchDoc>(POOL_MATCHES_COLLECTION);
        let doc = collection
            .find_one(doc! { "_id": id })?
            .ok_or(ApiError::PoolMatchNotFound)?;

        log::info!("setting player {} to {} for id {}", player, games_won, id);

        let race_to = match player {
            1 => doc.player_one.race_to,
            2 => doc.player_two.race_to,
            _ => return Err(ApiError::BadRequest("player must be 1 or 2".to_string())),
        };

        if games_won > race_to {
            return Err(ApiError::BadRequest(format!(
                "games_won ({}) cannot exceed race_to ({})",
                games_won, race_to
            )));
        }

        let mut set_doc = Document::new();
        match player {
            1 => {
                let mut updated = doc.player_one.clone();
                updated.games_won = games_won;
                set_doc.insert(
                    "player_one",
                    polodb_core::bson::to_bson(&updated)
                        .map_err(|e| ApiError::Unknown(e.to_string()))?,
                );
            }
            2 => {
                let mut updated = doc.player_two.clone();
                updated.games_won = games_won;
                set_doc.insert(
                    "player_two",
                    polodb_core::bson::to_bson(&updated)
                        .map_err(|e| ApiError::Unknown(e.to_string()))?,
                );
            }
            _ => unreachable!(),
        }
        if games_won == race_to {
            set_doc.insert("end_time", DateTime::now());
        }

        let update_doc = doc! { "$set": set_doc };
        let result = collection.update_one(doc! { "_id": id }, update_doc)?;
        if result.matched_count == 0 {
            return Err(ApiError::PoolMatchNotFound);
        }

        self.find_pool_match_by_id(id)?
            .ok_or(ApiError::PoolMatchNotFound)
    }

    /// End a pool match early by setting end_time. No-op if already ended.
    pub fn end_pool_match(&self, id: &ObjectId) -> Result<PoolMatchDoc, ApiError> {
        let collection = self.0.collection::<PoolMatchDoc>(POOL_MATCHES_COLLECTION);
        let doc = collection
            .find_one(doc! { "_id": id })?
            .ok_or(ApiError::PoolMatchNotFound)?;
        if doc.end_time.is_some() {
            return Ok(doc);
        }
        let update_doc = doc! { "$set": { "end_time": DateTime::now() } };
        let result = collection.update_one(doc! { "_id": id }, update_doc)?;
        if result.matched_count == 0 {
            return Err(ApiError::PoolMatchNotFound);
        }
        self.find_pool_match_by_id(id)?
            .ok_or(ApiError::PoolMatchNotFound)
    }

    /// Delete a pool match.
    pub fn delete_pool_match(&self, id: &ObjectId) -> Result<bool, ApiError> {
        let collection = self.0.collection::<PoolMatchDoc>(POOL_MATCHES_COLLECTION);
        let result = collection.delete_one(doc! { "_id": id })?;
        Ok(result.deleted_count > 0)
    }
}
