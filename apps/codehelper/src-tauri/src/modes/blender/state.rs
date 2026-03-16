use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

const SCENE_STALE_THRESHOLD_SECS: u64 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModifierData {
    pub name: String,
    #[serde(rename = "type")]
    pub modifier_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SceneObject {
    pub name: String,
    #[serde(rename = "type")]
    pub object_type: String,
    #[serde(default)]
    pub modifiers: Vec<ModifierData>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SceneData {
    pub object_count: usize,
    pub active_object: Option<String>,
    pub mode: String,
    pub render_engine: Option<String>,
    #[serde(default)]
    pub objects: Vec<SceneObject>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SceneSnapshot {
    pub connected: bool,
    pub scene_data: Option<SceneData>,
    pub message: Option<String>,
    pub last_update: Option<u64>,
}

#[derive(Debug, Default)]
pub struct SceneCache {
    scene_data: Option<SceneData>,
    last_update: Option<u64>,
}

impl SceneCache {
    pub fn update(&mut self, scene_data: SceneData) {
        self.scene_data = Some(scene_data);
        self.last_update = Some(now_unix_seconds());
    }

    pub fn snapshot(&self) -> SceneSnapshot {
        match (&self.scene_data, self.last_update) {
            (Some(scene_data), Some(last_update)) => {
                let age = now_unix_seconds().saturating_sub(last_update);
                if age > SCENE_STALE_THRESHOLD_SECS {
                    SceneSnapshot {
                        connected: false,
                        scene_data: None,
                        message: Some(
                            "Scene data is stale. Blender may not be connected right now."
                                .to_string(),
                        ),
                        last_update: Some(last_update),
                    }
                } else {
                    SceneSnapshot {
                        connected: true,
                        scene_data: Some(scene_data.clone()),
                        message: None,
                        last_update: Some(last_update),
                    }
                }
            }
            _ => SceneSnapshot {
                connected: false,
                scene_data: None,
                message: Some(
                    "No live Blender scene data is available yet. Open Blender with the addon enabled to share scene context."
                        .to_string(),
                ),
                last_update: None,
            },
        }
    }
}

pub fn shared_scene_cache() -> Arc<Mutex<SceneCache>> {
    Arc::new(Mutex::new(SceneCache::default()))
}

pub fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{now_unix_seconds, SceneCache, SceneData};

    #[test]
    fn scene_cache_returns_no_scene_message_by_default() {
        let cache = SceneCache::default();
        let snapshot = cache.snapshot();

        assert!(!snapshot.connected);
        assert!(snapshot.scene_data.is_none());
        assert!(snapshot.message.expect("message").contains("No live Blender scene data"));
    }

    #[test]
    fn scene_cache_returns_fresh_scene_when_recent() {
        let mut cache = SceneCache::default();
        cache.update(SceneData {
            object_count: 1,
            active_object: Some("Cube".to_string()),
            mode: "OBJECT".to_string(),
            render_engine: Some("BLENDER_EEVEE".to_string()),
            objects: Vec::new(),
        });

        let snapshot = cache.snapshot();
        assert!(snapshot.connected);
        assert_eq!(snapshot.last_update, Some(now_unix_seconds()));
    }
}
