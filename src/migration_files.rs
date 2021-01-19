use std::fs;
use std::path::{Path, PathBuf};

pub const FORMAT_STR: &str = "%Y%m%d%H%M%S";

pub fn migrations_in(p: &Path) -> Result<bool, std::io::Error> {
    // Find any files, or directories with a name complying
    // with <timestamp>_anything_here...
    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in fs::read_dir(p)? {
        if let Ok(entry) = entry {
            if let Ok(file_type) = entry.file_type() {
                match extract_timestamp(entry.path()) {
                    Ok(_timestamp) => {
                        if file_type.is_dir() {
                            dirs.push(entry.path())
                        } else {
                            files.push(entry.path())
                        }
                    }
                    Err(_) => continue,
                }
            }
        }
    }

    return Ok(true);
}

fn extract_timestamp(p: PathBuf) -> Result<chrono::NaiveDateTime, &'static str> {
    match p
        .to_str()
        .ok_or_else(|| "could not call to_str")?
        .split(|x| x == std::path::MAIN_SEPARATOR || x == '_' || x == '.')
        .collect::<Vec<&str>>()
        .first()
    {
        Some(first_part) => match chrono::NaiveDateTime::parse_from_str(first_part, FORMAT_STR) {
            Ok(ndt) => Ok(ndt),
            Err(_) => Err("timestamp did not parse"),
        },
        None => Err("could not get first part"),
    }
}

#[cfg(test)]
mod tests {

    // use super::*;

    // unsupportted runner
    // use of reserved word out of place
    // dot separated parts not at end of filename
}
