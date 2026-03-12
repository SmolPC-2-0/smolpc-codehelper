use smolpc_engine_core::inference::backend::ORT_CRATE_VERSION;
use smolpc_engine_core::inference::runtime_loading::{
    BundleValidationFailureClass, OpenVinoRuntimeBundle, OrtRuntimeBundle, RequiredRuntimeFile,
    RuntimeBundleFingerprint, RuntimeFamily, RuntimeVersionMetadata,
};
use std::path::{Path, PathBuf};

const OPENVINO_RUNTIME_VERSION: &str = "2026.0.0";
const OPENVINO_GENAI_VERSION: &str = "2026.0.0";
const OPENVINO_TOKENIZERS_VERSION: &str = "2026.0.0";
const ORT_GENAI_VERSION: &str = "bundled";

#[derive(Debug, Clone, Copy)]
pub enum RuntimeLoadMode {
    Production,
    Development,
}

impl RuntimeLoadMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::Development => "development",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedRuntimeBundles {
    pub mode: RuntimeLoadMode,
    pub ort: OrtRuntimeBundle,
    pub openvino: OpenVinoRuntimeBundle,
}

pub fn resolve_runtime_bundles(resource_dir: Option<&Path>) -> ResolvedRuntimeBundles {
    let mode = if cfg!(debug_assertions) {
        RuntimeLoadMode::Development
    } else {
        RuntimeLoadMode::Production
    };

    resolve_runtime_bundles_for_mode(resource_dir, mode)
}

pub(crate) fn resolve_runtime_bundles_for_mode(
    resource_dir: Option<&Path>,
    mode: RuntimeLoadMode,
) -> ResolvedRuntimeBundles {
    let ort_candidates = ort_bundle_candidates(resource_dir, mode);
    let openvino_candidates = openvino_bundle_candidates(resource_dir, mode);

    let ort = select_best_ort_bundle(ort_candidates);
    let openvino = select_best_openvino_bundle(openvino_candidates);

    ResolvedRuntimeBundles {
        mode,
        ort,
        openvino,
    }
}

fn ort_bundle_candidates(resource_dir: Option<&Path>, mode: RuntimeLoadMode) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if matches!(mode, RuntimeLoadMode::Development) {
        if let Some(path) = absolute_env_override("SMOLPC_ORT_BUNDLE_ROOT") {
            candidates.push(path);
        }
    }

    if let Some(path) = production_lib_root(resource_dir) {
        candidates.push(path);
    }

    if matches!(mode, RuntimeLoadMode::Development) {
        candidates.extend(dev_workspace_lib_roots());
    }

    dedupe_paths(candidates)
}

fn openvino_bundle_candidates(resource_dir: Option<&Path>, mode: RuntimeLoadMode) -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if matches!(mode, RuntimeLoadMode::Development) {
        if let Some(path) = absolute_env_override("SMOLPC_OPENVINO_BUNDLE_ROOT") {
            candidates.push(path);
        }
    }

    if let Some(path) = production_lib_root(resource_dir) {
        candidates.push(path.join("openvino"));
    }

    if matches!(mode, RuntimeLoadMode::Development) {
        candidates.extend(
            dev_workspace_lib_roots()
                .into_iter()
                .map(|root| root.join("openvino")),
        );
    }

    dedupe_paths(candidates)
}

fn absolute_env_override(key: &str) -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os(key)?);
    path.is_absolute().then_some(path)
}

fn production_lib_root(resource_dir: Option<&Path>) -> Option<PathBuf> {
    if let Some(resource_dir) = resource_dir {
        return Some(resource_dir.join("libs"));
    }

    std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("libs")))
}

fn dev_workspace_lib_roots() -> Vec<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let Some(workspace_root) = manifest_dir.parent().and_then(|parent| parent.parent()) else {
        return Vec::new();
    };

    vec![
        workspace_root
            .join("apps")
            .join("codehelper")
            .join("src-tauri")
            .join("libs"),
        workspace_root.join("src-tauri").join("libs"),
    ]
}

fn dedupe_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut unique = Vec::new();
    for path in paths {
        let absolute = absolutize_path(&path);
        if !unique.iter().any(|existing| existing == &absolute) {
            unique.push(absolute);
        }
    }
    unique
}

fn absolutize_path(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return path.to_path_buf();
    }

    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

fn canonical_root(root: &Path) -> Option<PathBuf> {
    if !root.exists() {
        return None;
    }
    root.canonicalize().ok()
}

fn select_best_ort_bundle(candidates: Vec<PathBuf>) -> OrtRuntimeBundle {
    let mut iter = candidates
        .into_iter()
        .map(build_ort_bundle)
        .collect::<Vec<_>>()
        .into_iter();
    let mut best = iter
        .next()
        .unwrap_or_else(|| build_ort_bundle(missing_bundle_root("libs")));
    let mut best_rank = ort_rank(&best);

    for candidate in iter {
        let rank = ort_rank(&candidate);
        if rank > best_rank {
            best = candidate;
            best_rank = rank;
        }
    }

    best
}

fn select_best_openvino_bundle(candidates: Vec<PathBuf>) -> OpenVinoRuntimeBundle {
    let mut iter = candidates
        .into_iter()
        .map(build_openvino_bundle)
        .collect::<Vec<_>>()
        .into_iter();
    let mut best = iter
        .next()
        .unwrap_or_else(|| build_openvino_bundle(missing_bundle_root("openvino")));
    let mut best_rank = openvino_rank(&best);

    for candidate in iter {
        let rank = openvino_rank(&candidate);
        if rank > best_rank {
            best = candidate;
            best_rank = rank;
        }
    }

    best
}

fn ort_rank(bundle: &OrtRuntimeBundle) -> (u8, bool) {
    let readiness = if bundle.directml_validated() {
        3
    } else if bundle.genai_validated() {
        2
    } else if bundle.ort_validated() {
        1
    } else {
        0
    };
    (readiness, bundle.canonical_root.is_some())
}

fn openvino_rank(bundle: &OpenVinoRuntimeBundle) -> (u8, bool) {
    let readiness = if bundle.npu_validated() { 1 } else { 0 };
    (readiness, bundle.canonical_root.is_some())
}

fn missing_bundle_root(name: &str) -> PathBuf {
    std::env::temp_dir()
        .join("smolpc-runtime-missing")
        .join(name)
}

fn build_ort_bundle(bundle_root: PathBuf) -> OrtRuntimeBundle {
    let canonical_root = canonical_root(&bundle_root);
    let onnxruntime_dll = bundle_root.join("onnxruntime.dll");
    let providers_shared = bundle_root.join("onnxruntime_providers_shared.dll");
    let genai_dll = bundle_root.join("onnxruntime-genai.dll");
    let directml_dll = bundle_root.join("DirectML.dll");
    let required_files = vec![
        RequiredRuntimeFile::new("onnxruntime.dll", onnxruntime_dll.clone()),
        RequiredRuntimeFile::new("onnxruntime_providers_shared.dll", providers_shared.clone()),
        RequiredRuntimeFile::new("onnxruntime-genai.dll", genai_dll.clone()),
        RequiredRuntimeFile::new("DirectML.dll", directml_dll.clone()),
    ];
    let version_metadata = vec![
        RuntimeVersionMetadata::new("ort-crate", ORT_CRATE_VERSION),
        RuntimeVersionMetadata::new("ort-genai", ORT_GENAI_VERSION),
    ];
    let ort_validation_failure = validate_root(&bundle_root, canonical_root.as_ref())
        .or_else(|| {
            missing_file(
                &onnxruntime_dll,
                BundleValidationFailureClass::OrtCoreMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &providers_shared,
                BundleValidationFailureClass::OrtProvidersSharedMissing,
            )
        });
    let genai_validation_failure = ort_validation_failure
        .or_else(|| missing_file(&genai_dll, BundleValidationFailureClass::OrtGenAiMissing));
    let directml_validation_failure = genai_validation_failure
        .or_else(|| missing_file(&directml_dll, BundleValidationFailureClass::DirectMlMissing));
    let fingerprint = RuntimeBundleFingerprint::new(
        RuntimeFamily::Ort,
        canonical_root.clone(),
        &bundle_root,
        &required_files,
        &version_metadata,
    );

    OrtRuntimeBundle {
        bundle_root,
        canonical_root,
        onnxruntime_dll,
        onnxruntime_providers_shared_dll: providers_shared,
        onnxruntime_genai_dll: genai_dll,
        directml_dll,
        required_files,
        version_metadata,
        ort_validation_failure,
        genai_validation_failure,
        directml_validation_failure,
        fingerprint,
    }
}

fn build_openvino_bundle(bundle_root: PathBuf) -> OpenVinoRuntimeBundle {
    let canonical_root = canonical_root(&bundle_root);
    let openvino_dll = bundle_root.join("openvino.dll");
    let openvino_c_dll = bundle_root.join("openvino_c.dll");
    let npu_plugin = bundle_root.join("openvino_intel_npu_plugin.dll");
    let npu_compiler = bundle_root.join("openvino_intel_npu_compiler.dll");
    let cpu_plugin = bundle_root.join("openvino_intel_cpu_plugin.dll");
    let ir_frontend = bundle_root.join("openvino_ir_frontend.dll");
    let genai_dll = bundle_root.join("openvino_genai.dll");
    let genai_c_dll = bundle_root.join("openvino_genai_c.dll");
    let tokenizers_dll = bundle_root.join("openvino_tokenizers.dll");
    let tbb_dll = bundle_root.join("tbb12.dll");
    let tbbbind_dll = bundle_root.join("tbbbind_2_5.dll");
    let tbbmalloc_dll = bundle_root.join("tbbmalloc.dll");
    let tbbmalloc_proxy_dll = bundle_root.join("tbbmalloc_proxy.dll");
    let icudt_dll = bundle_root.join("icudt70.dll");
    let icuuc_dll = bundle_root.join("icuuc70.dll");
    let required_files = vec![
        RequiredRuntimeFile::new("openvino.dll", openvino_dll.clone()),
        RequiredRuntimeFile::new("openvino_c.dll", openvino_c_dll.clone()),
        RequiredRuntimeFile::new("openvino_intel_npu_plugin.dll", npu_plugin.clone()),
        RequiredRuntimeFile::new("openvino_intel_npu_compiler.dll", npu_compiler.clone()),
        RequiredRuntimeFile::new("openvino_intel_cpu_plugin.dll", cpu_plugin.clone()),
        RequiredRuntimeFile::new("openvino_ir_frontend.dll", ir_frontend.clone()),
        RequiredRuntimeFile::new("openvino_genai.dll", genai_dll.clone()),
        RequiredRuntimeFile::new("openvino_genai_c.dll", genai_c_dll.clone()),
        RequiredRuntimeFile::new("openvino_tokenizers.dll", tokenizers_dll.clone()),
        RequiredRuntimeFile::new("tbb12.dll", tbb_dll.clone()),
        RequiredRuntimeFile::new("tbbbind_2_5.dll", tbbbind_dll.clone()),
        RequiredRuntimeFile::new("tbbmalloc.dll", tbbmalloc_dll.clone()),
        RequiredRuntimeFile::new("tbbmalloc_proxy.dll", tbbmalloc_proxy_dll.clone()),
        RequiredRuntimeFile::new("icudt70.dll", icudt_dll.clone()),
        RequiredRuntimeFile::new("icuuc70.dll", icuuc_dll.clone()),
    ];
    let version_metadata = vec![
        RuntimeVersionMetadata::new("openvino-runtime", OPENVINO_RUNTIME_VERSION),
        RuntimeVersionMetadata::new("openvino-genai", OPENVINO_GENAI_VERSION),
        RuntimeVersionMetadata::new("openvino-tokenizers", OPENVINO_TOKENIZERS_VERSION),
    ];
    let npu_validation_failure = validate_root(&bundle_root, canonical_root.as_ref())
        .or_else(|| {
            missing_file(
                &openvino_dll,
                BundleValidationFailureClass::OpenVinoRuntimeMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &openvino_c_dll,
                BundleValidationFailureClass::OpenVinoCapiMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &npu_plugin,
                BundleValidationFailureClass::OpenVinoNpuPluginMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &npu_compiler,
                BundleValidationFailureClass::OpenVinoNpuPluginMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &cpu_plugin,
                BundleValidationFailureClass::OpenVinoCpuPluginMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &ir_frontend,
                BundleValidationFailureClass::OpenVinoIrFrontendMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &genai_dll,
                BundleValidationFailureClass::OpenVinoGenAiMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &genai_c_dll,
                BundleValidationFailureClass::OpenVinoGenAiCApiMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &tokenizers_dll,
                BundleValidationFailureClass::OpenVinoTokenizersMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &icudt_dll,
                BundleValidationFailureClass::OpenVinoTokenizersMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &icuuc_dll,
                BundleValidationFailureClass::OpenVinoTokenizersMissing,
            )
        })
        .or_else(|| missing_file(&tbb_dll, BundleValidationFailureClass::OpenVinoTbbMissing))
        .or_else(|| {
            missing_file(
                &tbbbind_dll,
                BundleValidationFailureClass::OpenVinoTbbMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &tbbmalloc_dll,
                BundleValidationFailureClass::OpenVinoTbbMissing,
            )
        })
        .or_else(|| {
            missing_file(
                &tbbmalloc_proxy_dll,
                BundleValidationFailureClass::OpenVinoTbbMissing,
            )
        });
    let fingerprint = RuntimeBundleFingerprint::new(
        RuntimeFamily::OpenVino,
        canonical_root.clone(),
        &bundle_root,
        &required_files,
        &version_metadata,
    );

    OpenVinoRuntimeBundle {
        bundle_root,
        canonical_root,
        openvino_dll,
        openvino_c_dll,
        openvino_intel_npu_plugin_dll: npu_plugin,
        openvino_intel_npu_compiler_dll: npu_compiler,
        openvino_intel_cpu_plugin_dll: cpu_plugin,
        openvino_ir_frontend_dll: ir_frontend,
        openvino_genai_dll: genai_dll,
        openvino_genai_c_dll: genai_c_dll,
        openvino_tokenizers_dll: tokenizers_dll,
        tbb_dll,
        tbbbind_dll,
        tbbmalloc_dll,
        tbbmalloc_proxy_dll,
        icudt_dll,
        icuuc_dll,
        required_files,
        version_metadata,
        npu_validation_failure,
        fingerprint,
    }
}

fn validate_root(
    bundle_root: &Path,
    canonical_root: Option<&PathBuf>,
) -> Option<BundleValidationFailureClass> {
    if !bundle_root.is_absolute() {
        return Some(BundleValidationFailureClass::NonAbsoluteRoot);
    }
    if !bundle_root.exists() {
        return Some(BundleValidationFailureClass::MissingRoot);
    }
    if canonical_root.is_none() {
        return Some(BundleValidationFailureClass::CanonicalizationFailed);
    }
    None
}

fn missing_file(
    path: &Path,
    failure: BundleValidationFailureClass,
) -> Option<BundleValidationFailureClass> {
    (!path.exists()).then_some(failure)
}

#[cfg(test)]
mod tests {
    use super::{
        build_openvino_bundle, build_ort_bundle, resolve_runtime_bundles_for_mode, RuntimeLoadMode,
    };
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};
    use tempfile::tempdir;

    #[test]
    fn ort_bundle_marks_missing_root() {
        let bundle = build_ort_bundle(PathBuf::from("C:/missing/bundle"));
        assert_eq!(
            bundle.ort_validation_failure.map(|failure| failure.code()),
            Some("missing_root")
        );
    }

    #[test]
    fn resolve_runtime_bundles_is_deterministic() {
        let bundles = resolve_runtime_bundles_for_mode(None, RuntimeLoadMode::Development);
        assert!(matches!(
            bundles.mode,
            RuntimeLoadMode::Development | RuntimeLoadMode::Production
        ));
        assert!(!bundles.ort.fingerprint.value.is_empty());
        assert!(!bundles.openvino.fingerprint.value.is_empty());
    }

    #[test]
    fn missing_genai_dll_keeps_ort_cpu_validated() {
        let temp = tempdir().expect("temp dir");
        let libs = temp.path().join("libs");
        create_ort_files(
            &libs,
            &["onnxruntime.dll", "onnxruntime_providers_shared.dll"],
        );

        let bundle = build_ort_bundle(libs);
        assert!(bundle.ort_validated());
        assert!(!bundle.genai_validated());
        assert_eq!(bundle.directml_failure_code(), Some("ort_genai_missing"));
    }

    #[test]
    fn missing_directml_dll_only_disables_directml_lane() {
        let temp = tempdir().expect("temp dir");
        let libs = temp.path().join("libs");
        create_ort_files(
            &libs,
            &[
                "onnxruntime.dll",
                "onnxruntime_providers_shared.dll",
                "onnxruntime-genai.dll",
            ],
        );

        let bundle = build_ort_bundle(libs);
        assert!(bundle.ort_validated());
        assert!(bundle.genai_validated());
        assert!(!bundle.directml_validated());
        assert_eq!(bundle.directml_failure_code(), Some("directml_missing"));
    }

    #[test]
    fn development_mode_prefers_absolute_override_roots() {
        let _guard = lock_env();
        let temp = tempdir().expect("temp dir");
        let resource_dir = temp.path().join("resource");
        let override_root = temp.path().join("override-libs");
        let override_openvino = temp.path().join("override-openvino");

        create_ort_files(
            &resource_dir.join("libs"),
            &[
                "onnxruntime.dll",
                "onnxruntime_providers_shared.dll",
                "onnxruntime-genai.dll",
                "DirectML.dll",
            ],
        );
        create_openvino_files(&resource_dir.join("libs").join("openvino"));
        create_ort_files(
            &override_root,
            &[
                "onnxruntime.dll",
                "onnxruntime_providers_shared.dll",
                "onnxruntime-genai.dll",
                "DirectML.dll",
            ],
        );
        create_openvino_files(&override_openvino);

        let ort_guard = EnvVarGuard::set("SMOLPC_ORT_BUNDLE_ROOT", override_root.as_os_str());
        let openvino_guard =
            EnvVarGuard::set("SMOLPC_OPENVINO_BUNDLE_ROOT", override_openvino.as_os_str());
        let bundles =
            resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Development);
        drop(openvino_guard);
        drop(ort_guard);

        assert_eq!(
            bundles.ort.display_root(),
            override_root
                .canonicalize()
                .expect("canonical ort override")
        );
        assert_eq!(
            bundles.openvino.display_root(),
            override_openvino
                .canonicalize()
                .expect("canonical openvino override")
        );
    }

    #[test]
    fn production_mode_ignores_dev_override_roots() {
        let _guard = lock_env();
        let temp = tempdir().expect("temp dir");
        let resource_dir = temp.path().join("resource");
        let override_root = temp.path().join("override-libs");
        let override_openvino = temp.path().join("override-openvino");

        create_ort_files(
            &resource_dir.join("libs"),
            &[
                "onnxruntime.dll",
                "onnxruntime_providers_shared.dll",
                "onnxruntime-genai.dll",
                "DirectML.dll",
            ],
        );
        create_openvino_files(&resource_dir.join("libs").join("openvino"));
        create_ort_files(
            &override_root,
            &[
                "onnxruntime.dll",
                "onnxruntime_providers_shared.dll",
                "onnxruntime-genai.dll",
                "DirectML.dll",
            ],
        );
        create_openvino_files(&override_openvino);

        let ort_guard = EnvVarGuard::set("SMOLPC_ORT_BUNDLE_ROOT", override_root.as_os_str());
        let openvino_guard =
            EnvVarGuard::set("SMOLPC_OPENVINO_BUNDLE_ROOT", override_openvino.as_os_str());
        let bundles =
            resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Production);
        drop(openvino_guard);
        drop(ort_guard);

        assert_eq!(
            bundles.ort.display_root(),
            resource_dir
                .join("libs")
                .canonicalize()
                .expect("canonical libs")
        );
        assert_eq!(
            bundles.openvino.display_root(),
            resource_dir
                .join("libs")
                .join("openvino")
                .canonicalize()
                .expect("canonical openvino")
        );
    }

    #[test]
    fn relative_dev_override_is_ignored() {
        let _guard = lock_env();
        let temp = tempdir().expect("temp dir");
        let resource_dir = temp.path().join("resource");

        create_ort_files(
            &resource_dir.join("libs"),
            &[
                "onnxruntime.dll",
                "onnxruntime_providers_shared.dll",
                "onnxruntime-genai.dll",
                "DirectML.dll",
            ],
        );
        create_openvino_files(&resource_dir.join("libs").join("openvino"));

        let ort_guard = EnvVarGuard::set("SMOLPC_ORT_BUNDLE_ROOT", "relative-libs");
        let openvino_guard = EnvVarGuard::set("SMOLPC_OPENVINO_BUNDLE_ROOT", "relative-openvino");
        let bundles =
            resolve_runtime_bundles_for_mode(Some(&resource_dir), RuntimeLoadMode::Development);
        drop(openvino_guard);
        drop(ort_guard);

        assert_eq!(
            bundles.ort.display_root(),
            resource_dir
                .join("libs")
                .canonicalize()
                .expect("canonical libs")
        );
        assert_eq!(
            bundles.openvino.display_root(),
            resource_dir
                .join("libs")
                .join("openvino")
                .canonicalize()
                .expect("canonical openvino")
        );
    }

    #[test]
    fn openvino_bundle_missing_plugin_is_reported() {
        let temp = tempdir().expect("temp dir");
        let root = temp.path().join("openvino");
        create_openvino_files(&root);
        fs::remove_file(root.join("openvino_intel_npu_plugin.dll")).expect("remove npu plugin");

        let bundle = build_openvino_bundle(root);
        assert!(!bundle.npu_validated());
        assert_eq!(bundle.failure_code(), Some("openvino_npu_plugin_missing"));
    }

    #[test]
    fn openvino_bundle_missing_compiler_is_reported_as_plugin_failure() {
        let temp = tempdir().expect("temp dir");
        let root = temp.path().join("openvino");
        create_openvino_files(&root);
        fs::remove_file(root.join("openvino_intel_npu_compiler.dll")).expect("remove npu compiler");

        let bundle = build_openvino_bundle(root);
        assert!(!bundle.npu_validated());
        assert_eq!(bundle.failure_code(), Some("openvino_npu_plugin_missing"));
    }

    #[test]
    fn openvino_bundle_missing_icu_is_reported_as_tokenizers_failure() {
        let temp = tempdir().expect("temp dir");
        let root = temp.path().join("openvino");
        create_openvino_files(&root);
        fs::remove_file(root.join("icuuc70.dll")).expect("remove icuuc");

        let bundle = build_openvino_bundle(root);
        assert!(!bundle.npu_validated());
        assert_eq!(bundle.failure_code(), Some("openvino_tokenizers_missing"));
    }

    #[test]
    fn openvino_bundle_missing_tbb_support_is_reported_as_tbb_failure() {
        let temp = tempdir().expect("temp dir");
        let root = temp.path().join("openvino");
        create_openvino_files(&root);
        fs::remove_file(root.join("tbbmalloc_proxy.dll")).expect("remove tbbmalloc proxy");

        let bundle = build_openvino_bundle(root);
        assert!(!bundle.npu_validated());
        assert_eq!(bundle.failure_code(), Some("openvino_tbb_missing"));
    }

    fn create_ort_files(root: &Path, files: &[&str]) {
        fs::create_dir_all(root).expect("create ort root");
        for file in files {
            fs::write(root.join(file), []).expect("write ort runtime file");
        }
    }

    fn create_openvino_files(root: &Path) {
        fs::create_dir_all(root).expect("create openvino root");
        for file in [
            "openvino.dll",
            "openvino_c.dll",
            "openvino_intel_npu_plugin.dll",
            "openvino_intel_npu_compiler.dll",
            "openvino_intel_cpu_plugin.dll",
            "openvino_ir_frontend.dll",
            "openvino_genai.dll",
            "openvino_genai_c.dll",
            "openvino_tokenizers.dll",
            "tbb12.dll",
            "tbbbind_2_5.dll",
            "tbbmalloc.dll",
            "tbbmalloc_proxy.dll",
            "icudt70.dll",
            "icuuc70.dll",
        ] {
            fs::write(root.join(file), []).expect("write openvino runtime file");
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        match env_lock().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: impl AsRef<OsStr>) -> Self {
            let previous = std::env::var_os(key);
            std::env::set_var(key, value);
            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.take() {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }
}
