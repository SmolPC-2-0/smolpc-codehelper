use smolpc_assistant_types::AppMode;

pub const LIBREOFFICE_DISABLED_REASON: &str =
    "LibreOffice integration is scaffolded in the unified app, but live document actions are not wired yet.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LibreOfficeModeProfile {
    pub label: &'static str,
    pub subtitle: &'static str,
    pub disabled_reason: &'static str,
    pub source_coverage: &'static str,
    pub future_runtime_family: &'static str,
}

pub fn libreoffice_profile(mode: AppMode) -> Option<LibreOfficeModeProfile> {
    match mode {
        // Writer already has meaningful standalone tool coverage on
        // codex/libreoffice-port-track-a, but that behavior is not ported here yet.
        AppMode::Writer => Some(LibreOfficeModeProfile {
            label: "Writer",
            subtitle:
                "LibreOffice Writer scaffold in the unified shell; live document actions land in the activation phase",
            disabled_reason: LIBREOFFICE_DISABLED_REASON,
            source_coverage:
                "Writer already has meaningful source coverage on codex/libreoffice-port-track-a.",
            future_runtime_family: "Shared LibreOffice MCP runtime over stdio",
        }),
        // Calc is still scaffold-target only until the standalone branch matures further.
        AppMode::Calc => Some(LibreOfficeModeProfile {
            label: "Calc",
            subtitle:
                "LibreOffice Calc scaffold in the unified shell; spreadsheet actions remain deferred for now",
            disabled_reason: LIBREOFFICE_DISABLED_REASON,
            source_coverage:
                "Calc is not yet at parity on codex/libreoffice-port-track-a and remains scaffold-target only.",
            future_runtime_family: "Shared LibreOffice MCP runtime over stdio",
        }),
        // Impress / Slides already has meaningful standalone tool coverage, but remains inactive here.
        AppMode::Impress => Some(LibreOfficeModeProfile {
            label: "Slides",
            subtitle:
                "LibreOffice Slides scaffold in the unified shell; live presentation actions land in the activation phase",
            disabled_reason: LIBREOFFICE_DISABLED_REASON,
            source_coverage:
                "Slides already has meaningful source coverage on codex/libreoffice-port-track-a.",
            future_runtime_family: "Shared LibreOffice MCP runtime over stdio",
        }),
        _ => None,
    }
}
