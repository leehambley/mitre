use super::filename;
use std::fs;
use std::io;
use std::path::Path;

pub struct Migration<'a> {
  parsed: filename::Parsed<'a>,
}

// TODO: anything to document here about max depth, and or/whether we
// traverse filesystems, or whether there is a timeout (e.g slow network share)
fn migrations(dir: &Path) -> Result<Vec<Migration>, io::Error> {
  let migrations: Vec<Migration> = Vec::new();

  for entry in fs::read_dir(dir)? {
    let entry = entry?;
    let path = entry.path();

    let metadata = fs::metadata(&path)?;

    println!(
      "seconds, is read only: {:?}, size: {:?} bytes, filename: {:?}",
      metadata.permissions().readonly(),
      metadata.len(),
      path.file_name().ok_or("No filename").unwrap()
    );
  }
  return Ok(migrations);
}

#[cfg(test)]
mod tests {

  use super::*;

  // unsupportted runner
  // use of reserved word out of place
  // dot separated parts not at end of filename
}
