#[cfg(target_os = "windows")]
use smolpc_engine_core::inference::genai::GenAiDirectMlGenerator;
use smolpc_engine_core::inference::{
    InferenceRuntimeAdapter, OpenVinoPipelineConfig, OpenVinoRuntimeBundle, OrtRuntimeBundle,
    OrtRuntimeLoader,
};
use std::path::Path;

use crate::openvino::{
    ensure_qwen3_nothink_template, openvino_generation_controls_for_model,
    openvino_model_tuning_for_model,
};
pub(crate) fn build_openvino_cpu_runtime_adapter(
    bundle: &OpenVinoRuntimeBundle,
    model_id: &str,
    model_dir: &Path,
) -> Result<InferenceRuntimeAdapter, String> {
    if model_id.starts_with("qwen3") {
        match ensure_qwen3_nothink_template(model_dir) {
            Ok(true) => log::info!("Patched Qwen3 chat template for CPU non-thinking default"),
            Ok(false) => {}
            Err(e) => {
                return Err(format!(
                    "Qwen3 CPU requires non-thinking template but patch failed: {e}"
                ));
            }
        }
    }

    let model_tuning = openvino_model_tuning_for_model(model_id);
    let pipeline_config = OpenVinoPipelineConfig::cpu()
        .with_generation_controls(openvino_generation_controls_for_model(model_id, model_dir))
        .with_disable_thinking(model_tuning.disable_thinking);
    let generator = smolpc_engine_core::inference::OpenVinoGenAiGenerator::new(
        bundle,
        model_dir,
        &pipeline_config,
    )?;
    generator.run_preflight("Warmup preflight")?;
    Ok(InferenceRuntimeAdapter::openvino_genai(generator))
}

#[cfg(target_os = "windows")]
pub(crate) fn build_directml_runtime_adapter(
    ort_bundle: &OrtRuntimeBundle,
    dml_model_path: &Path,
    directml_device_id: Option<i32>,
) -> Result<InferenceRuntimeAdapter, String> {
    let model_dir = dml_model_path
        .parent()
        .ok_or_else(|| format!("Invalid DirectML model path: {}", dml_model_path.display()))?;
    OrtRuntimeLoader::ensure_initialized(ort_bundle)?;
    let generator = GenAiDirectMlGenerator::new(ort_bundle, model_dir, directml_device_id)?;
    generator.run_preflight("Warmup preflight")?;
    Ok(InferenceRuntimeAdapter::genai_directml(generator))
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn build_directml_runtime_adapter(
    _ort_bundle: &OrtRuntimeBundle,
    _dml_model_path: &Path,
    _directml_device_id: Option<i32>,
) -> Result<InferenceRuntimeAdapter, String> {
    Err("DirectML is only supported on Windows".to_string())
}
