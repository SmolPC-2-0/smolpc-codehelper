use ndarray::{Array2, Array4};
use ort::memory::AllocationDevice;
use ort::session::builder::GraphOptimizationLevel;
use ort::session::run_options::{OutputSelector, RunOptions};
use ort::{ep, session::Session, value::Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if cfg!(target_os = "windows")
        && std::env::var("ORT_PROBE_SKIP_PRELOAD")
            .ok()
            .as_deref()
            != Some("1")
    {
        let dml_path = std::path::PathBuf::from("libs").join("DirectML.dll");
        if dml_path.exists() {
            let lib = unsafe { libloading::Library::new(&dml_path)? };
            std::mem::forget(lib);
            println!("preloaded {}", dml_path.display());
        } else {
            println!("DirectML.dll not found at {}", dml_path.display());
        }
    }

    let dylib_path = std::env::var_os("ORT_PROBE_DYLIB")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("libs").join("onnxruntime.dll"));
    println!("init from {}", dylib_path.display());
    let builder = ort::init_from(dylib_path.to_string_lossy().to_string())?;
    let _ = builder.commit();
    println!("ort info: {}", ort::info());

    let mut sb = Session::builder()?;
    sb = sb.with_optimization_level(GraphOptimizationLevel::Level3)?;
    let backend = std::env::var("ORT_PROBE_BACKEND").unwrap_or_else(|_| "dml".to_string());
    let mut session = if backend.eq_ignore_ascii_case("cpu") {
        sb.with_execution_providers([ep::CPU::default().build().error_on_failure()])?
            .commit_from_file("models/qwen2.5-coder-1.5b/dml/model.onnx")?
    } else {
        let dml = ep::DirectML::default();
        sb.with_config_entry("ep.dml.disable_graph_fusion", "1")?
            .with_execution_providers([dml.build().error_on_failure()])?
            .commit_from_file("models/qwen2.5-coder-1.5b/dml/model.onnx")?
    };

    let mut ids: Vec<i64> = vec![
        8822, 912, 2877, 25, 600, 18, 17, 11, 293, 25, 600, 18, 17, 8, 1464, 600, 18, 17, 314,
    ];
    let mode = std::env::var("ORT_PROBE_MODE").unwrap_or_else(|_| "prefill".to_string());
    if mode == "prefill" {
        if let Ok(seq_raw) = std::env::var("ORT_PROBE_SEQ") {
            if let Ok(seq) = seq_raw.parse::<usize>() {
                ids.truncate(seq.min(ids.len()));
            }
        }
        let seq_len = ids.len();

        let input_ids = Array2::from_shape_vec((1, seq_len), ids)?;
        let attention_mask = Array2::from_shape_vec((1, seq_len), vec![1_i64; seq_len])?;
        let position_ids = Array2::from_shape_vec((1, seq_len), (0..seq_len as i64).collect())?;

        let mut inputs: Vec<(String, ort::session::SessionInputValue<'static>)> = Vec::new();
        inputs.push((
            "input_ids".to_string(),
            ort::session::SessionInputValue::Owned(Value::from_array(input_ids)?.into()),
        ));
        inputs.push((
            "attention_mask".to_string(),
            ort::session::SessionInputValue::Owned(Value::from_array(attention_mask)?.into()),
        ));
        inputs.push((
            "position_ids".to_string(),
            ort::session::SessionInputValue::Owned(Value::from_array(position_ids)?.into()),
        ));

        for layer in 0..28 {
            let empty: Array4<half::f16> = Array4::from_shape_vec((1, 2, 0, 128), vec![])?;
            inputs.push((
                format!("past_key_values.{layer}.key"),
                ort::session::SessionInputValue::Owned(Value::from_array(empty.clone())?.into()),
            ));
            inputs.push((
                format!("past_key_values.{layer}.value"),
                ort::session::SessionInputValue::Owned(Value::from_array(empty)?.into()),
            ));
        }

        let model_inputs = session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect::<Vec<_>>();
        let mut ordered = Vec::with_capacity(model_inputs.len());
        for name in model_inputs {
            let idx = inputs.iter().position(|(k, _)| *k == name).unwrap();
            let (_, v) = inputs.swap_remove(idx);
            ordered.push(v);
        }

        let options = RunOptions::new()?.with_outputs(OutputSelector::no_default().with("logits"));
        let outputs = session.run_with_options(ordered.as_slice(), &options)?;
        let logits = outputs.get("logits").ok_or("missing logits")?;
        let logits = logits.downcast_ref::<ort::value::DynTensorValueType>()?;
        println!(
            "logits allocation device before copy: {:?} (cpu_accessible={})",
            logits.memory_info().allocation_device(),
            logits.memory_info().is_cpu_accessible()
        );
        let logits_cpu = logits.to(AllocationDevice::CPU, 0)?;
        println!(
            "logits allocation device after copy: {:?} (cpu_accessible={})",
            logits_cpu.memory_info().allocation_device(),
            logits_cpu.memory_info().is_cpu_accessible()
        );
        let (shape, data) = logits_cpu.try_extract_tensor::<half::f16>()?;
        println!("logits shape: {:?}, len={}", shape, data.len());
        let mut nonfinite = 0usize;
        let mut max = f32::NEG_INFINITY;
        let mut min = f32::INFINITY;
        let vocab = 151_936usize;
        let seq = shape[1] as usize;
        for pos in 0..seq {
            let start = pos * vocab;
            let end = start + vocab;
            let nf = data[start..end]
                .iter()
                .filter(|v| !v.to_f32().is_finite())
                .count();
            println!("pos {pos} nonfinite={nf}");
        }
        for v in data.iter() {
            let f = v.to_f32();
            if !f.is_finite() {
                nonfinite += 1;
            }
            if f > max {
                max = f;
            }
            if f < min {
                min = f;
            }
        }
        println!("nonfinite={} min={} max={}", nonfinite, min, max);
    } else if mode == "decode_zero3" {
        let input_ids = Array2::from_shape_vec((1, 1), vec![25_i64])?;
        let attention_mask = Array2::from_shape_vec((1, 4), vec![1_i64; 4])?;
        let position_ids = Array2::from_shape_vec((1, 1), vec![3_i64])?;

        let mut inputs: Vec<(String, ort::session::SessionInputValue<'static>)> = Vec::new();
        inputs.push((
            "input_ids".to_string(),
            ort::session::SessionInputValue::Owned(Value::from_array(input_ids)?.into()),
        ));
        inputs.push((
            "attention_mask".to_string(),
            ort::session::SessionInputValue::Owned(Value::from_array(attention_mask)?.into()),
        ));
        inputs.push((
            "position_ids".to_string(),
            ort::session::SessionInputValue::Owned(Value::from_array(position_ids)?.into()),
        ));

        for layer in 0..28 {
            let zero: Array4<half::f16> = Array4::from_shape_vec((1, 2, 3, 128), vec![half::f16::from_f32(0.0); 1 * 2 * 3 * 128])?;
            inputs.push((
                format!("past_key_values.{layer}.key"),
                ort::session::SessionInputValue::Owned(Value::from_array(zero.clone())?.into()),
            ));
            inputs.push((
                format!("past_key_values.{layer}.value"),
                ort::session::SessionInputValue::Owned(Value::from_array(zero)?.into()),
            ));
        }

        let model_inputs = session
            .inputs()
            .iter()
            .map(|i| i.name().to_string())
            .collect::<Vec<_>>();
        let mut ordered = Vec::with_capacity(model_inputs.len());
        for name in model_inputs {
            let idx = inputs.iter().position(|(k, _)| *k == name).unwrap();
            let (_, v) = inputs.swap_remove(idx);
            ordered.push(v);
        }

        let options = RunOptions::new()?.with_outputs(OutputSelector::no_default().with("logits"));
        let outputs = session.run_with_options(ordered.as_slice(), &options)?;
        let logits = outputs.get("logits").ok_or("missing logits")?;
        let logits = logits.downcast_ref::<ort::value::DynTensorValueType>()?;
        let logits_cpu = logits.to(AllocationDevice::CPU, 0)?;
        let (shape, data) = logits_cpu.try_extract_tensor::<half::f16>()?;
        let nonfinite = data.iter().filter(|v| !v.to_f32().is_finite()).count();
        let mut row_min = f32::INFINITY;
        let mut row_max = f32::NEG_INFINITY;
        for v in data.iter() {
            let f = v.to_f32();
            if f < row_min {
                row_min = f;
            }
            if f > row_max {
                row_max = f;
            }
        }
        println!(
            "decode_zero3 logits shape {:?}, nonfinite={}, min={}, max={}",
            shape, nonfinite, row_min, row_max
        );
    } else {
        println!("unknown ORT_PROBE_MODE='{mode}'");
    }

    Ok(())
}
