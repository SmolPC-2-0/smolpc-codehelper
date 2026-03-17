use smolpc_assistant_types::AppMode;

pub const LIBREOFFICE_DISABLED_REASON: &str =
    "LibreOffice integration is scaffolded in the unified app, but live document actions are not wired yet.";
pub const WRITER_COMPOSER_PLACEHOLDER: &str =
    "Ask LibreOffice Writer to create or edit a document (Shift+Enter for new line)...";
pub const CALC_COMPOSER_PLACEHOLDER: &str =
    "LibreOffice Calc is scaffolded here, but live spreadsheet chat is not active yet.";
pub const SLIDES_COMPOSER_PLACEHOLDER: &str =
    "Ask LibreOffice Slides to create or edit a presentation (Shift+Enter for new line)...";

pub const WRITER_ALLOWED_TOOLS: &[&str] = &[
    "create_blank_document",
    "read_text_document",
    "get_document_properties",
    "list_documents",
    "copy_document",
    "add_text",
    "add_heading",
    "add_paragraph",
    "add_table",
    "insert_image",
    "insert_page_break",
    "format_text",
    "search_replace_text",
    "delete_text",
    "format_table",
    "delete_paragraph",
    "apply_document_style",
];

pub const IMPRESS_ALLOWED_TOOLS: &[&str] = &[
    "create_blank_presentation",
    "read_presentation",
    "get_document_properties",
    "list_documents",
    "copy_document",
    "add_slide",
    "edit_slide_content",
    "edit_slide_title",
    "delete_slide",
    "apply_presentation_template",
    "format_slide_content",
    "format_slide_title",
    "insert_slide_image",
];

pub const CALC_ALLOWED_TOOLS: &[&str] = &[];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LibreOfficeModeProfile {
    pub label: &'static str,
    pub subtitle: &'static str,
    pub disabled_reason: &'static str,
    pub composer_placeholder: &'static str,
    pub source_coverage: &'static str,
    pub future_runtime_family: &'static str,
    pub live_in_phase_6b: bool,
    pub suggestions: &'static [&'static str],
    pub allowed_tools: &'static [&'static str],
}

pub fn libreoffice_profile(mode: AppMode) -> Option<LibreOfficeModeProfile> {
    match mode {
        AppMode::Writer => Some(LibreOfficeModeProfile {
            label: "Writer",
            subtitle:
                "Live LibreOffice Writer help for creating and editing documents through the unified assistant shell",
            disabled_reason: "",
            composer_placeholder: WRITER_COMPOSER_PLACEHOLDER,
            source_coverage:
                "Writer already has meaningful source coverage on codex/libreoffice-port-track-a.",
            future_runtime_family: "Shared LibreOffice MCP runtime over stdio",
            live_in_phase_6b: true,
            suggestions: &[
                "Create a blank document called lesson-plan.odt",
                "Add a level 1 heading called Local AI in Schools",
                "Insert a two-column table for topic and notes",
            ],
            allowed_tools: WRITER_ALLOWED_TOOLS,
        }),
        AppMode::Calc => Some(LibreOfficeModeProfile {
            label: "Calc",
            subtitle:
                "LibreOffice Calc scaffold in the unified shell; spreadsheet actions remain deferred for now",
            disabled_reason: LIBREOFFICE_DISABLED_REASON,
            composer_placeholder: CALC_COMPOSER_PLACEHOLDER,
            source_coverage:
                "Calc is not yet at parity on codex/libreoffice-port-track-a and remains scaffold-target only.",
            future_runtime_family: "Shared LibreOffice MCP runtime over stdio",
            live_in_phase_6b: false,
            suggestions: &[
                "LibreOffice Calc activation is planned next",
                "Spreadsheet tools are not wired yet",
                "Check back after the activation follow-up",
            ],
            allowed_tools: CALC_ALLOWED_TOOLS,
        }),
        AppMode::Impress => Some(LibreOfficeModeProfile {
            label: "Slides",
            subtitle:
                "Live LibreOffice Slides help for creating and editing presentations through the unified assistant shell",
            disabled_reason: "",
            composer_placeholder: SLIDES_COMPOSER_PLACEHOLDER,
            source_coverage:
                "Slides already has meaningful source coverage on codex/libreoffice-port-track-a.",
            future_runtime_family: "Shared LibreOffice MCP runtime over stdio",
            live_in_phase_6b: true,
            suggestions: &[
                "Create a blank presentation called demo-pitch.odp",
                "Add a title slide for Local AI in Classrooms",
                "Insert an image on slide 2 and scale it to fit",
            ],
            allowed_tools: IMPRESS_ALLOWED_TOOLS,
        }),
        _ => None,
    }
}

pub fn is_live_libreoffice_mode(mode: AppMode) -> bool {
    libreoffice_profile(mode)
        .map(|profile| profile.live_in_phase_6b)
        .unwrap_or(false)
}

pub fn allowed_tool_names(mode: AppMode) -> &'static [&'static str] {
    libreoffice_profile(mode)
        .map(|profile| profile.allowed_tools)
        .unwrap_or(CALC_ALLOWED_TOOLS)
}
