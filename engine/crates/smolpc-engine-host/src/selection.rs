use smolpc_engine_core::inference::backend::{
    DecisionReason, FailureCounters, InferenceBackend,
};
use smolpc_engine_core::inference::backend_store::BackendDecisionRecord;

pub(crate) fn choose_preferred_backend(
    force_override: Option<InferenceBackend>,
    failure_counters: &FailureCounters,
    stored: Option<&BackendDecisionRecord>,
    has_dml_candidate: bool,
    has_openvino_candidate: bool,
) -> (InferenceBackend, DecisionReason) {
    if let Some(override_backend) = force_override {
        return (override_backend, DecisionReason::ForcedOverride);
    }
    if let Some(record) = stored {
        if let Some(decision) = record.persisted_decision.as_ref() {
            match decision.backend {
                InferenceBackend::OpenVinoNpu => {
                    if has_openvino_candidate {
                        return (
                            InferenceBackend::OpenVinoNpu,
                            DecisionReason::PersistedDecision,
                        );
                    }
                }
                InferenceBackend::DirectML => {
                    if has_dml_candidate && !failure_counters.should_demote_directml() {
                        return (
                            InferenceBackend::DirectML,
                            DecisionReason::PersistedDecision,
                        );
                    }
                }
                InferenceBackend::Cpu => {
                    return (InferenceBackend::Cpu, DecisionReason::PersistedDecision);
                }
            }
        }
    }
    if failure_counters.should_demote_directml() {
        if has_openvino_candidate {
            return (
                InferenceBackend::OpenVinoNpu,
                DecisionReason::DefaultOpenVinoCandidate,
            );
        }
        return (InferenceBackend::Cpu, DecisionReason::DemotedAfterFailures);
    }
    if has_dml_candidate {
        return (
            InferenceBackend::DirectML,
            DecisionReason::DefaultDirectMLCandidate,
        );
    }
    if has_openvino_candidate {
        return (
            InferenceBackend::OpenVinoNpu,
            DecisionReason::DefaultOpenVinoCandidate,
        );
    }
    (InferenceBackend::Cpu, DecisionReason::NoDirectMLCandidate)
}

pub(crate) fn should_release_current_adapter_for_load(
    current_backend: Option<InferenceBackend>,
    next_backend: InferenceBackend,
    has_loaded_model: bool,
) -> bool {
    has_loaded_model
        && (current_backend == Some(InferenceBackend::DirectML)
            || next_backend == InferenceBackend::DirectML)
}
