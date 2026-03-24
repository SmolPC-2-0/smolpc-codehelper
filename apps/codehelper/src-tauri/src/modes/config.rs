use super::libreoffice::libreoffice_profile;
use smolpc_assistant_types::{AppMode, ModeCapabilitiesDto, ModeConfigDto, ProviderKind};

pub fn list_mode_configs() -> Vec<ModeConfigDto> {
    [
        AppMode::Code,
        AppMode::Gimp,
        AppMode::Blender,
        AppMode::Writer,
        AppMode::Impress,
    ]
    .into_iter()
    .map(mode_config)
    .collect()
}

pub fn mode_config(mode: AppMode) -> ModeConfigDto {
    match mode {
        AppMode::Code => ModeConfigDto {
            id: AppMode::Code,
            label: "Code".to_string(),
            subtitle: "Codehelper workspace for fixes, explanations, and new code".to_string(),
            icon: "code".to_string(),
            provider_kind: ProviderKind::Local,
            system_prompt_key: "mode.code.default".to_string(),
            suggestions: vec![
                "Fix this bug and explain the root cause".to_string(),
                "Write a function from this prompt".to_string(),
                "Review this snippet for mistakes".to_string(),
            ],
            capabilities: ModeCapabilitiesDto {
                supports_tools: false,
                supports_undo: false,
                show_model_info: true,
                show_hardware_panel: true,
                show_export: true,
                show_context_controls: true,
            },
        },
        AppMode::Gimp => ModeConfigDto {
            id: AppMode::Gimp,
            label: "GIMP".to_string(),
            subtitle: "Live image editing help for GIMP through the unified assistant shell"
                .to_string(),
            icon: "image".to_string(),
            provider_kind: ProviderKind::Mcp,
            system_prompt_key: "mode.gimp.default".to_string(),
            suggestions: vec![
                "Blur the top half of the image".to_string(),
                "Crop this image to a square".to_string(),
                "Rotate the image 90 degrees clockwise".to_string(),
            ],
            capabilities: shared_tool_mode_capabilities(true),
        },
        AppMode::Blender => ModeConfigDto {
            id: AppMode::Blender,
            label: "Blender".to_string(),
            subtitle: "Live Blender tutoring with scene-aware guidance and Blender-doc grounding"
                .to_string(),
            icon: "box".to_string(),
            provider_kind: ProviderKind::Hybrid,
            system_prompt_key: "mode.blender.default".to_string(),
            suggestions: vec![
                "What is in my scene right now?".to_string(),
                "How do I add a bevel to the selected object?".to_string(),
                "Explain what this modifier stack is doing".to_string(),
            ],
            capabilities: shared_tool_mode_capabilities(false),
        },
        AppMode::Writer => {
            let profile = libreoffice_profile(AppMode::Writer).expect("writer profile");
            ModeConfigDto {
                id: AppMode::Writer,
                label: profile.label.to_string(),
                subtitle: profile.subtitle.to_string(),
                icon: "file-text".to_string(),
                provider_kind: ProviderKind::Mcp,
                system_prompt_key: "mode.writer.default".to_string(),
                suggestions: profile
                    .suggestions
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                capabilities: shared_tool_mode_capabilities(false),
            }
        }
        AppMode::Impress => {
            let profile = libreoffice_profile(AppMode::Impress).expect("impress profile");
            ModeConfigDto {
                id: AppMode::Impress,
                label: profile.label.to_string(),
                subtitle: profile.subtitle.to_string(),
                icon: "presentation".to_string(),
                provider_kind: ProviderKind::Mcp,
                system_prompt_key: "mode.impress.default".to_string(),
                suggestions: profile
                    .suggestions
                    .iter()
                    .map(|value| (*value).to_string())
                    .collect(),
                capabilities: shared_tool_mode_capabilities(false),
            }
        }
    }
}

fn shared_tool_mode_capabilities(supports_undo: bool) -> ModeCapabilitiesDto {
    ModeCapabilitiesDto {
        supports_tools: true,
        supports_undo,
        show_model_info: true,
        show_hardware_panel: true,
        show_export: false,
        show_context_controls: false,
    }
}

#[cfg(test)]
mod tests {
    use super::list_mode_configs;
    use smolpc_assistant_types::AppMode;

    #[test]
    fn mode_config_list_contains_expected_modes_in_order() {
        let modes = list_mode_configs();
        let ids = modes.iter().map(|mode| mode.id).collect::<Vec<_>>();

        assert_eq!(
            ids,
            vec![
                AppMode::Code,
                AppMode::Gimp,
                AppMode::Blender,
                AppMode::Writer,
                AppMode::Impress,
            ]
        );
    }

    #[test]
    fn impress_mode_uses_slides_label() {
        let modes = list_mode_configs();
        let slides = modes
            .into_iter()
            .find(|mode| mode.id == AppMode::Impress)
            .expect("slides mode");

        assert_eq!(slides.label, "Slides");
    }
}
