use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

fn format_date_ymd() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    // Simplified date calculation from unix timestamp
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days: [i64; 12] = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md {
            m = i;
            break;
        }
        remaining -= md;
    }
    format!("{:04}-{:02}-{:02}", y, m + 1, remaining + 1)
}

fn format_timestamp_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = secs / 86400;
    let day_secs = secs % 86400;
    let h = day_secs / 3600;
    let min = (day_secs % 3600) / 60;
    let s = day_secs % 60;
    let mut y = 1970i64;
    let mut remaining = days as i64;
    loop {
        let days_in_year = if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        y += 1;
    }
    let leap = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days: [i64; 12] = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut m = 0usize;
    for (i, &md) in month_days.iter().enumerate() {
        if remaining < md {
            m = i;
            break;
        }
        remaining -= md;
    }
    format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z", y, m + 1, remaining + 1, h, min, s)
}

/// Creates the logs directory and returns the path to today's log file
pub fn setup_log_file(app_data_dir: &PathBuf) -> Result<PathBuf, String> {
    let logs_dir = app_data_dir.join("logs");

    // Create logs directory if it doesn't exist
    fs::create_dir_all(&logs_dir)
        .map_err(|e| format!("Failed to create logs directory: {}", e))?;

    // Generate log filename with today's date
    let log_filename = format!("server_{}.log", format_date_ymd());
    let log_file = logs_dir.join(log_filename);

    let _ = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .map_err(|e| format!("Failed to create log file {}: {}", log_file.display(), e))?;

    Ok(log_file)
}

pub fn append_log_line(log_file: &PathBuf, message: &str) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_file)
        .map_err(|e| format!("Failed to open log file {}: {}", log_file.display(), e))?;

    writeln!(
        file,
        "{} {}",
        format_timestamp_iso(),
        message
    )
    .map_err(|e| format!("Failed writing to log file {}: {}", log_file.display(), e))
}

/// Opens the logs directory in the system file explorer
#[cfg(windows)]
pub fn open_logs_directory(app_data_dir: &PathBuf) -> Result<(), String> {
    let logs_dir = app_data_dir.join("logs");

    Command::new("explorer")
        .arg(logs_dir)
        .spawn()
        .map_err(|e| format!("Failed to open logs directory: {}", e))?;

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn open_logs_directory(app_data_dir: &PathBuf) -> Result<(), String> {
    let logs_dir = app_data_dir.join("logs");

    Command::new("open")
        .arg(logs_dir)
        .spawn()
        .map_err(|e| format!("Failed to open logs directory: {}", e))?;

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn open_logs_directory(app_data_dir: &PathBuf) -> Result<(), String> {
    let logs_dir = app_data_dir.join("logs");

    Command::new("xdg-open")
        .arg(logs_dir)
        .spawn()
        .map_err(|e| format!("Failed to open logs directory: {}", e))?;

    Ok(())
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub fn open_logs_directory(_app_data_dir: &PathBuf) -> Result<(), String> {
    Err("Opening logs directory is not supported on this platform".to_string())
}
