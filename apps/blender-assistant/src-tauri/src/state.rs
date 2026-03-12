use crate::rag::index::RagIndex;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

const SCENE_STALE_THRESHOLD_SECS: u64 = 30;
const ALLOW_OLLAMA_FALLBACK_ENV: &str = "BLENDER_HELPER_ALLOW_OLLAMA_FALLBACK";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GenerationBackend {
    Ollama,
    SharedEngine,
}

impl GenerationBackend {
    pub fn from_str(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "ollama" => Some(Self::Ollama),
            "shared_engine" | "sharedengine" | "engine" => Some(Self::SharedEngine),
            _ => None,
        }
    }

    pub fn from_env() -> Self {
        std::env::var("BLENDER_HELPER_BACKEND")
            .ok()
            .and_then(|value| Self::from_str(&value))
            .unwrap_or(Self::SharedEngine)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ollama => "ollama",
            Self::SharedEngine => "shared_engine",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModifierData {
    pub name: String,
    #[serde(rename = "type")]
    pub modifier_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneObject {
    pub name: String,
    #[serde(rename = "type")]
    pub object_type: String,
    #[serde(default)]
    pub modifiers: Vec<ModifierData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneData {
    pub object_count: usize,
    pub active_object: Option<String>,
    pub mode: String,
    pub render_engine: Option<String>,
    #[serde(default)]
    pub objects: Vec<SceneObject>,
}

#[derive(Debug, Clone, Serialize)]
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
                        message: Some("Scene data is stale. Blender may not be connected.".to_string()),
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
                message: Some("No scene data available. Make sure Blender addon is active.".to_string()),
                last_update: None,
            },
        }
    }

    pub fn latest_scene(&self) -> Option<SceneData> {
        self.snapshot().scene_data
    }
}

#[derive(Clone)]
pub struct BackendState {
    pub scene_cache: Arc<Mutex<SceneCache>>,
    pub rag_index: Arc<Mutex<RagIndex>>,
    generation_backend: Arc<RwLock<GenerationBackend>>,
    loaded_model_id: Arc<RwLock<Option<String>>>,
}

impl BackendState {
    pub fn new(rag_index: RagIndex) -> Self {
        Self {
            scene_cache: Arc::new(Mutex::new(SceneCache::default())),
            rag_index: Arc::new(Mutex::new(rag_index)),
            generation_backend: Arc::new(RwLock::new(GenerationBackend::from_env())),
            loaded_model_id: Arc::new(RwLock::new(None)),
        }
    }

    pub fn get_generation_backend(&self) -> GenerationBackend {
        match self.generation_backend.read() {
            Ok(guard) => *guard,
            Err(poisoned) => *poisoned.into_inner(),
        }
    }

    pub fn set_generation_backend(&self, backend: GenerationBackend) {
        match self.generation_backend.write() {
            Ok(mut guard) => *guard = backend,
            Err(poisoned) => *poisoned.into_inner() = backend,
        }
    }

    pub fn get_loaded_model_id(&self) -> Option<String> {
        match self.loaded_model_id.read() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }

    pub fn set_loaded_model_id(&self, model_id: Option<String>) {
        match self.loaded_model_id.write() {
            Ok(mut guard) => *guard = model_id,
            Err(poisoned) => *poisoned.into_inner() = model_id,
        }
    }
}

pub fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn allow_ollama_fallback() -> bool {
    std::env::var(ALLOW_OLLAMA_FALLBACK_ENV)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}
