use crate::prelude::*;

#[derive(Debug, Error)]
pub enum UnnestDirError {
    #[error("directory is empty")]
    Empty,
    #[error("directory is not nested")]
    NotNested,
    #[error("{0}")]
    Io(io::Error),
}

pub fn unnest_dir<P: AsRef<Path>>(dir: P) -> Result<(), UnnestDirError> {
    let dir = dir.as_ref();
    let outer_dir_name = dir.file_name().expect("file name");
    let outer_dir_path = dir.canonicalize().map_err(UnnestDirError::Io)?;

    let mut first_entry = None;
    for entry in fs::read_dir(&outer_dir_path).map_err(UnnestDirError::Io)? {
        if first_entry.is_some() {
            return Err(UnnestDirError::NotNested);
        }
        let entry = entry.map_err(UnnestDirError::Io)?;
        first_entry = Some(entry);
    }
    let Some(entry) = first_entry else {
        return Err(UnnestDirError::Empty);
    };

    if !entry.file_type().map_err(UnnestDirError::Io)?.is_dir() {
        return Err(UnnestDirError::NotNested);
    }

    // HACK: random name so it have the least chance for name conflict
    // and it being the easiest solution
    let inner_dir_path = entry.path();
    let random_inner_dir_to_outside_path = inner_dir_path
        .parent()
        .expect("parent")
        .with_file_name("an7v54k42xp2zu4cijb2xg3ipn7bsa");
    fs::rename(&inner_dir_path, &random_inner_dir_to_outside_path)
        .map_err(UnnestDirError::Io)?;
    fs::remove_dir(&outer_dir_path).map_err(UnnestDirError::Io)?;
    fs::rename(
        &random_inner_dir_to_outside_path,
        random_inner_dir_to_outside_path.with_file_name(outer_dir_name),
    )
    .map_err(UnnestDirError::Io)?;
    Ok(())
}
