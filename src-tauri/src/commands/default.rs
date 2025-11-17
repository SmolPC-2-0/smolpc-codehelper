use super::errors::Error;
use rfd::FileDialog;
use std::fs;

#[tauri::command]
pub fn read(path: String) -> Result<String, Error> {
    let data = fs::read(path)?;
    let string = String::from_utf8(data)?;
    Ok(string)
}

#[tauri::command]
pub fn write(path: String, contents: String) -> Result<(), Error> {
    fs::write(path, contents)?;
    Ok(())
}

/// Save code to a file with native file dialog
#[tauri::command]
pub fn save_code(code: String) -> Result<(), Error> {
    let file_path = FileDialog::new()
        .add_filter("All Files", &["*"])
        .add_filter("Python", &["py"])
        .add_filter("JavaScript", &["js"])
        .add_filter("TypeScript", &["ts"])
        .add_filter("Rust", &["rs"])
        .add_filter("HTML", &["html"])
        .add_filter("CSS", &["css"])
        .add_filter("Text", &["txt"])
        .set_file_name("code.txt")
        .save_file();

    if let Some(path) = file_path {
        fs::write(path, code)?;
        Ok(())
    } else {
        Err(Error::Other("File save cancelled".to_string()))
    }
}
