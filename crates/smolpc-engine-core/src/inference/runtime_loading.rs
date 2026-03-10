use libloading::{Library, Symbol};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::{DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeFamily {
    Ort,
    OpenVino,
}

impl RuntimeFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ort => "ort",
            Self::OpenVino => "openvino",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequiredRuntimeFile {
    pub logical_name: String,
    pub path: PathBuf,
    pub exists: bool,
}

impl RequiredRuntimeFile {
    pub fn new(logical_name: impl Into<String>, path: PathBuf) -> Self {
        Self {
            logical_name: logical_name.into(),
            exists: path.exists(),
            path,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeVersionMetadata {
    pub component: String,
    pub version: String,
}

impl RuntimeVersionMetadata {
    pub fn new(component: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            component: component.into(),
            version: version.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BundleValidationFailureClass {
    MissingRoot,
    NonAbsoluteRoot,
    CanonicalizationFailed,
    OrtCoreMissing,
    OrtProvidersSharedMissing,
    OrtGenAiMissing,
    DirectMlMissing,
    OpenVinoRuntimeMissing,
    OpenVinoCapiMissing,
    OpenVinoNpuPluginMissing,
    OpenVinoCpuPluginMissing,
    OpenVinoIrFrontendMissing,
    OpenVinoGenAiMissing,
    OpenVinoTokenizersMissing,
    OpenVinoTbbMissing,
    RuntimeConflict,
}

impl BundleValidationFailureClass {
    pub fn code(self) -> &'static str {
        match self {
            Self::MissingRoot => "missing_root",
            Self::NonAbsoluteRoot => "non_absolute_root",
            Self::CanonicalizationFailed => "canonicalization_failed",
            Self::OrtCoreMissing => "ort_core_missing",
            Self::OrtProvidersSharedMissing => "ort_providers_shared_missing",
            Self::OrtGenAiMissing => "ort_genai_missing",
            Self::DirectMlMissing => "directml_missing",
            Self::OpenVinoRuntimeMissing => "openvino_runtime_missing",
            Self::OpenVinoCapiMissing => "openvino_capi_missing",
            Self::OpenVinoNpuPluginMissing => "openvino_npu_plugin_missing",
            Self::OpenVinoCpuPluginMissing => "openvino_cpu_plugin_missing",
            Self::OpenVinoIrFrontendMissing => "openvino_ir_frontend_missing",
            Self::OpenVinoGenAiMissing => "openvino_genai_missing",
            Self::OpenVinoTokenizersMissing => "openvino_tokenizers_missing",
            Self::OpenVinoTbbMissing => "openvino_tbb_missing",
            Self::RuntimeConflict => "runtime_conflict",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeBundleFingerprint {
    pub family: RuntimeFamily,
    pub canonical_root: Option<PathBuf>,
    pub inventory_hash: String,
    pub value: String,
}

impl RuntimeBundleFingerprint {
    pub fn new(
        family: RuntimeFamily,
        canonical_root: Option<PathBuf>,
        bundle_root: &Path,
        required_files: &[RequiredRuntimeFile],
        version_metadata: &[RuntimeVersionMetadata],
    ) -> Self {
        let inventory_hash = compute_bundle_inventory_hash(
            family,
            canonical_root.as_ref().map_or(bundle_root, |value| value),
            required_files,
            version_metadata,
        );
        let value = format!(
            "{}:{}:{}",
            family.as_str(),
            canonical_root
                .as_ref()
                .map_or(bundle_root, |value| value)
                .display()
                .to_string()
                .to_ascii_lowercase(),
            inventory_hash
        );
        Self {
            family,
            canonical_root,
            inventory_hash,
            value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OrtRuntimeBundle {
    pub bundle_root: PathBuf,
    pub canonical_root: Option<PathBuf>,
    pub onnxruntime_dll: PathBuf,
    pub onnxruntime_providers_shared_dll: PathBuf,
    pub onnxruntime_genai_dll: PathBuf,
    pub directml_dll: PathBuf,
    pub required_files: Vec<RequiredRuntimeFile>,
    pub version_metadata: Vec<RuntimeVersionMetadata>,
    pub ort_validation_failure: Option<BundleValidationFailureClass>,
    pub genai_validation_failure: Option<BundleValidationFailureClass>,
    pub directml_validation_failure: Option<BundleValidationFailureClass>,
    pub fingerprint: RuntimeBundleFingerprint,
}

impl OrtRuntimeBundle {
    pub fn display_root(&self) -> &Path {
        self.canonical_root
            .as_deref()
            .unwrap_or(self.bundle_root.as_path())
    }

    pub fn ort_validated(&self) -> bool {
        self.ort_validation_failure.is_none()
    }

    pub fn cpu_validated(&self) -> bool {
        self.ort_validated()
    }

    pub fn genai_validated(&self) -> bool {
        self.genai_validation_failure.is_none()
    }

    pub fn directml_validated(&self) -> bool {
        self.directml_validation_failure.is_none()
    }

    pub fn ort_failure_code(&self) -> Option<&'static str> {
        self.ort_validation_failure
            .map(BundleValidationFailureClass::code)
    }

    pub fn directml_failure_code(&self) -> Option<&'static str> {
        self.directml_validation_failure
            .map(BundleValidationFailureClass::code)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OpenVinoRuntimeBundle {
    pub bundle_root: PathBuf,
    pub canonical_root: Option<PathBuf>,
    pub openvino_dll: PathBuf,
    pub openvino_c_dll: PathBuf,
    pub openvino_intel_npu_plugin_dll: PathBuf,
    pub openvino_intel_cpu_plugin_dll: PathBuf,
    pub openvino_ir_frontend_dll: PathBuf,
    pub openvino_genai_dll: PathBuf,
    pub openvino_tokenizers_dll: PathBuf,
    pub tbb_dll: PathBuf,
    pub required_files: Vec<RequiredRuntimeFile>,
    pub version_metadata: Vec<RuntimeVersionMetadata>,
    pub npu_validation_failure: Option<BundleValidationFailureClass>,
    pub fingerprint: RuntimeBundleFingerprint,
}

impl OpenVinoRuntimeBundle {
    pub fn display_root(&self) -> &Path {
        self.canonical_root
            .as_deref()
            .unwrap_or(self.bundle_root.as_path())
    }

    pub fn npu_validated(&self) -> bool {
        self.npu_validation_failure.is_none()
    }

    pub fn failure_code(&self) -> Option<&'static str> {
        self.npu_validation_failure
            .map(BundleValidationFailureClass::code)
    }
}

#[derive(Clone)]
pub struct RetainedLibrary {
    pub path: PathBuf,
    _lib: Arc<Library>,
}

impl std::fmt::Debug for RetainedLibrary {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetainedLibrary")
            .field("path", &self.path)
            .finish()
    }
}

impl RetainedLibrary {
    pub fn load(path: &Path) -> Result<Self, String> {
        let lib = load_runtime_library(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            _lib: Arc::new(lib),
        })
    }

    pub unsafe fn get<T: Copy>(&self, name: &[u8]) -> Result<T, String> {
        let symbol: Symbol<'_, T> = self
            ._lib
            .get(name)
            .map_err(|e| format!("Missing symbol {}: {e}", String::from_utf8_lossy(name)))?;
        Ok(*symbol)
    }
}

#[derive(Debug, Clone)]
pub struct OrtRuntimeHandle {
    pub fingerprint: RuntimeBundleFingerprint,
    pub bundle_root: PathBuf,
    _support_libs: Vec<RetainedLibrary>,
}

#[derive(Debug, Clone)]
pub struct OpenVinoRuntimeHandle {
    pub fingerprint: RuntimeBundleFingerprint,
    pub bundle_root: PathBuf,
    _support_libs: Vec<RetainedLibrary>,
}

#[derive(Clone)]
enum CachedOrtInit {
    Success(OrtRuntimeHandle),
    Failure(String),
}

#[derive(Default)]
struct OrtRuntimeLoaderState {
    active_fingerprint: Option<String>,
    results: HashMap<String, CachedOrtInit>,
}

static ORT_RUNTIME_STATE: OnceLock<Mutex<OrtRuntimeLoaderState>> = OnceLock::new();

pub struct OrtRuntimeLoader;

impl OrtRuntimeLoader {
    pub fn ensure_initialized(bundle: &OrtRuntimeBundle) -> Result<OrtRuntimeHandle, String> {
        let state = ORT_RUNTIME_STATE.get_or_init(|| Mutex::new(OrtRuntimeLoaderState::default()));
        let fingerprint = bundle.fingerprint.value.clone();

        {
            let guard = lock_mutex(state);
            if let Some(cached) = guard.results.get(&fingerprint) {
                return match cached {
                    CachedOrtInit::Success(handle) => Ok(handle.clone()),
                    CachedOrtInit::Failure(error) => Err(error.clone()),
                };
            }

            if let Some(active) = guard.active_fingerprint.as_ref() {
                if active != &fingerprint {
                    let error = format!(
                        "ONNX Runtime already initialized from bundle fingerprint '{active}'; restart the process to use '{}'",
                        fingerprint
                    );
                    drop(guard);
                    let mut guard = lock_mutex(state);
                    guard
                        .results
                        .insert(fingerprint, CachedOrtInit::Failure(error.clone()));
                    return Err(error);
                }
            }
        }

        if let Some(failure) = bundle.ort_validation_failure {
            let error = format!(
                "ONNX Runtime bundle is not validated ({}) at {}",
                failure.code(),
                bundle.display_root().display()
            );
            let mut guard = lock_mutex(state);
            guard
                .results
                .insert(fingerprint, CachedOrtInit::Failure(error.clone()));
            return Err(error);
        }

        let runtime_core = RetainedLibrary::load(&bundle.onnxruntime_dll)?;
        let providers_shared = RetainedLibrary::load(&bundle.onnxruntime_providers_shared_dll)?;
        let init_result = ort::init_from(bundle.onnxruntime_dll.to_string_lossy().to_string())
            .map_err(|e| format!("Failed to initialize ONNX Runtime: {e}"))
            .and_then(|builder| {
                if builder.commit() {
                    log::info!(
                        "Initialized ONNX Runtime from {}",
                        bundle.onnxruntime_dll.display()
                    );
                } else {
                    log::info!(
                        "Reused existing ONNX Runtime initialization for {}",
                        bundle.onnxruntime_dll.display()
                    );
                }
                Ok(())
            });

        let mut guard = lock_mutex(state);
        match init_result {
            Ok(()) => {
                guard.active_fingerprint = Some(fingerprint.clone());
                let handle = OrtRuntimeHandle {
                    fingerprint: bundle.fingerprint.clone(),
                    bundle_root: bundle.display_root().to_path_buf(),
                    _support_libs: vec![runtime_core, providers_shared],
                };
                guard
                    .results
                    .insert(fingerprint, CachedOrtInit::Success(handle.clone()));
                Ok(handle)
            }
            Err(error) => {
                guard
                    .results
                    .insert(fingerprint, CachedOrtInit::Failure(error.clone()));
                Err(error)
            }
        }
    }
}

#[derive(Clone)]
enum CachedOpenVinoInit {
    Success(OpenVinoRuntimeHandle),
    Failure(String),
}

#[derive(Default)]
struct OpenVinoRuntimeLoaderState {
    active_fingerprint: Option<String>,
    results: HashMap<String, CachedOpenVinoInit>,
}

static OPENVINO_RUNTIME_STATE: OnceLock<Mutex<OpenVinoRuntimeLoaderState>> = OnceLock::new();

pub struct OpenVinoRuntimeLoader;

impl OpenVinoRuntimeLoader {
    pub fn ensure_initialized(
        bundle: &OpenVinoRuntimeBundle,
    ) -> Result<OpenVinoRuntimeHandle, String> {
        let state = OPENVINO_RUNTIME_STATE
            .get_or_init(|| Mutex::new(OpenVinoRuntimeLoaderState::default()));
        let fingerprint = bundle.fingerprint.value.clone();

        {
            let guard = lock_mutex(state);
            if let Some(cached) = guard.results.get(&fingerprint) {
                return match cached {
                    CachedOpenVinoInit::Success(handle) => Ok(handle.clone()),
                    CachedOpenVinoInit::Failure(error) => Err(error.clone()),
                };
            }

            if let Some(active) = guard.active_fingerprint.as_ref() {
                if active != &fingerprint {
                    let error = format!(
                        "OpenVINO already initialized from bundle fingerprint '{active}'; restart the process to use '{}'",
                        fingerprint
                    );
                    drop(guard);
                    let mut guard = lock_mutex(state);
                    guard
                        .results
                        .insert(fingerprint, CachedOpenVinoInit::Failure(error.clone()));
                    return Err(error);
                }
            }
        }

        if let Some(failure) = bundle.npu_validation_failure {
            let error = format!(
                "OpenVINO bundle is not validated ({}) at {}",
                failure.code(),
                bundle.display_root().display()
            );
            let mut guard = lock_mutex(state);
            guard
                .results
                .insert(fingerprint, CachedOpenVinoInit::Failure(error.clone()));
            return Err(error);
        }

        let libs = [
            &bundle.tbb_dll,
            &bundle.openvino_dll,
            &bundle.openvino_c_dll,
            &bundle.openvino_ir_frontend_dll,
            &bundle.openvino_intel_cpu_plugin_dll,
            &bundle.openvino_intel_npu_plugin_dll,
            &bundle.openvino_tokenizers_dll,
            &bundle.openvino_genai_dll,
        ]
        .into_iter()
        .map(|path| RetainedLibrary::load(path))
        .collect::<Result<Vec<_>, _>>()?;

        let mut guard = lock_mutex(state);
        guard.active_fingerprint = Some(fingerprint.clone());
        let handle = OpenVinoRuntimeHandle {
            fingerprint: bundle.fingerprint.clone(),
            bundle_root: bundle.display_root().to_path_buf(),
            _support_libs: libs,
        };
        guard
            .results
            .insert(fingerprint, CachedOpenVinoInit::Success(handle.clone()));
        Ok(handle)
    }
}

fn compute_bundle_inventory_hash(
    family: RuntimeFamily,
    bundle_root: &Path,
    required_files: &[RequiredRuntimeFile],
    version_metadata: &[RuntimeVersionMetadata],
) -> String {
    let mut hasher = DefaultHasher::new();
    family.hash(&mut hasher);
    bundle_root
        .display()
        .to_string()
        .to_ascii_lowercase()
        .hash(&mut hasher);

    for file in required_files {
        file.logical_name.hash(&mut hasher);
        file.path
            .display()
            .to_string()
            .to_ascii_lowercase()
            .hash(&mut hasher);
        file.exists.hash(&mut hasher);
        if let Ok(metadata) = file.path.metadata() {
            metadata.len().hash(&mut hasher);
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(UNIX_EPOCH) {
                    duration.as_secs().hash(&mut hasher);
                    duration.subsec_nanos().hash(&mut hasher);
                }
            }
        }
    }

    for version in version_metadata {
        version.component.hash(&mut hasher);
        version.version.hash(&mut hasher);
    }

    format!("{:016x}", hasher.finish())
}

fn validate_runtime_library_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute() {
        return Err(format!(
            "Runtime library path must be absolute: {}",
            path.display()
        ));
    }
    if !path.exists() {
        return Err(format!(
            "Runtime library does not exist: {}",
            path.display()
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn load_runtime_library(path: &Path) -> Result<Library, String> {
    use libloading::os::windows::{
        Library as WindowsLibrary, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
    };

    validate_runtime_library_path(path)?;

    // Restrict dependency resolution to the DLL's own directory plus System32.
    let flags = LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32;
    let lib = unsafe { WindowsLibrary::load_with_flags(path.as_os_str(), flags) }
        .map(Library::from)
        .map_err(|e| format!("Failed to load {}: {e}", path.display()))?;
    Ok(lib)
}

#[cfg(not(target_os = "windows"))]
fn load_runtime_library(path: &Path) -> Result<Library, String> {
    validate_runtime_library_path(path)?;
    let lib = unsafe { Library::new(path) }
        .map_err(|e| format!("Failed to load {}: {e}", path.display()))?;
    Ok(lib)
}

fn lock_mutex<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RequiredRuntimeFile, RuntimeBundleFingerprint, RuntimeFamily, RuntimeVersionMetadata,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    #[test]
    fn fingerprint_changes_when_bundle_root_changes() {
        let files = vec![RequiredRuntimeFile::new(
            "onnxruntime.dll",
            PathBuf::from("C:/runtime/onnxruntime.dll"),
        )];
        let versions = vec![RuntimeVersionMetadata::new("ort-crate", "2.0.0-rc.11")];
        let a = RuntimeBundleFingerprint::new(
            RuntimeFamily::Ort,
            Some(PathBuf::from("C:/runtime")),
            &PathBuf::from("C:/runtime"),
            &files,
            &versions,
        );
        let b = RuntimeBundleFingerprint::new(
            RuntimeFamily::Ort,
            Some(PathBuf::from("C:/runtime-alt")),
            &PathBuf::from("C:/runtime-alt"),
            &files,
            &versions,
        );
        assert_ne!(a.value, b.value);
    }

    #[test]
    fn fingerprint_changes_when_versions_change() {
        let files = vec![RequiredRuntimeFile::new(
            "onnxruntime.dll",
            PathBuf::from("C:/runtime/onnxruntime.dll"),
        )];
        let a = RuntimeBundleFingerprint::new(
            RuntimeFamily::Ort,
            Some(PathBuf::from("C:/runtime")),
            &PathBuf::from("C:/runtime"),
            &files,
            &[RuntimeVersionMetadata::new("ort-crate", "2.0.0-rc.11")],
        );
        let b = RuntimeBundleFingerprint::new(
            RuntimeFamily::Ort,
            Some(PathBuf::from("C:/runtime")),
            &PathBuf::from("C:/runtime"),
            &files,
            &[RuntimeVersionMetadata::new("ort-crate", "2.0.0-rc.12")],
        );
        assert_ne!(a.value, b.value);
    }

    #[test]
    fn runtime_library_loader_requires_absolute_paths() {
        let temp = tempdir().expect("temp dir");
        let relative = temp.path().join("runtime.dll");
        fs::write(&relative, []).expect("write file");
        let relative_name = relative
            .file_name()
            .expect("file name")
            .to_string_lossy()
            .to_string();

        let err = super::RetainedLibrary::load(Path::new(&relative_name))
            .expect_err("relative runtime library path should fail");
        assert!(err.contains("must be absolute"));
    }

    #[test]
    fn runtime_loading_is_centralized() {
        let _guard = source_invariant_lock().lock().expect("source lock");
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(|dir| dir.parent())
            .expect("workspace root")
            .to_path_buf();
        let forbidden_patterns = [
            ["Library::", "new("].concat(),
            ["load_with_", "flags("].concat(),
        ];

        let mut offenders = Vec::new();
        visit_rs_files(&workspace.join("crates"), &mut |path| {
            let path_string = path.to_string_lossy().replace('\\', "/");
            let allowed =
                path_string.ends_with("crates/smolpc-engine-core/src/inference/runtime_loading.rs");
            let content = fs::read_to_string(path).expect("read source file");
            if allowed {
                return;
            }

            for pattern in forbidden_patterns.iter() {
                if content.contains(pattern) {
                    offenders.push(format!("{} contains {}", path.display(), pattern));
                }
            }
        });

        assert!(
            offenders.is_empty(),
            "runtime DLL loading must stay centralized in runtime_loading.rs: {}",
            offenders.join("; ")
        );
    }

    fn visit_rs_files(root: &Path, visit: &mut impl FnMut(&Path)) {
        if !root.exists() {
            return;
        }
        let entries = fs::read_dir(root).expect("read dir");
        for entry in entries {
            let entry = entry.expect("dir entry");
            let path = entry.path();
            if path.is_dir() {
                visit_rs_files(&path, visit);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                visit(&path);
            }
        }
    }

    fn source_invariant_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }
}
