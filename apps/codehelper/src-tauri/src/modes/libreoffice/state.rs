use std::path::PathBuf;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LibreOfficeProviderState {
    pub scaffold_dir: Option<PathBuf>,
    pub last_error: Option<String>,
}
