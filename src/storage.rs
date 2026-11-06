use std::{fs::File, path::Path};

/// Replace the destination only after a complete, flushed write in the same directory.
pub fn replace_file(
    path: &Path,
    write: impl FnOnce(&mut File) -> Result<(), String>,
) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    write(temporary.as_file_mut())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn failed_write_preserves_existing_file_and_cleans_staging_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("project.json");
        std::fs::write(&path, b"original").unwrap();
        assert!(
            replace_file(&path, |file| {
                file.write_all(b"partial").unwrap();
                Err("simulated failure".into())
            })
            .is_err()
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"original");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        replace_file(&path, |file| {
            file.write_all(b"complete").map_err(|e| e.to_string())
        })
        .unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"complete");
    }
}
