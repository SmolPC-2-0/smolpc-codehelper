use smolpc_assistant_types::AppMode;

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
    "open_in_libreoffice",
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
    "open_in_libreoffice",
];

const NO_ALLOWED_TOOLS: &[&str] = &[];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LibreOfficeModeProfile {
    pub label: &'static str,
    pub subtitle: &'static str,
    pub suggestions: &'static [&'static str],
    pub allowed_tools: &'static [&'static str],
}

pub fn libreoffice_profile(mode: AppMode) -> Option<LibreOfficeModeProfile> {
    match mode {
        AppMode::Writer => Some(LibreOfficeModeProfile {
            label: "Writer",
            subtitle:
                "Live LibreOffice Writer help for creating and editing documents through the unified assistant shell",
            suggestions: &[
                "Create a blank document called lesson-plan.odt",
                "Add a level 1 heading called Local AI in Schools",
                "Insert a two-column table for topic and notes",
            ],
            allowed_tools: WRITER_ALLOWED_TOOLS,
        }),
        AppMode::Impress => Some(LibreOfficeModeProfile {
            label: "Slides",
            subtitle:
                "Live LibreOffice Slides help for creating and editing presentations through the unified assistant shell",
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

pub fn allowed_tool_names(mode: AppMode) -> &'static [&'static str] {
    libreoffice_profile(mode)
        .map(|profile| profile.allowed_tools)
        .unwrap_or(NO_ALLOWED_TOOLS)
}
