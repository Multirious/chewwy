use prelude::*;

pub const DOT_DIR: &str = ".chewwy";

pub mod prelude {
    pub use error_stack::{Result as StackResult, ResultExt};
    pub use std::{
        env,
        ffi::{OsStr, OsString},
        fmt, fs, io, path,
        path::{Path, PathBuf},
        process,
    };
    pub use thiserror::Error;
}
pub mod cfg;
pub mod file_archiver;
pub mod utils;

pub fn search_chewwy_root<P: AsRef<Path>>(
    start_at_dir: P,
) -> io::Result<Option<PathBuf>> {
    for entry in fs::read_dir(&start_at_dir)? {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name() else {
            continue;
        };
        if name == DOT_DIR {
            return Ok(Some(path.parent().unwrap().to_path_buf()));
        }
    }
    // search_chewwy_root()
    let Some(parent) = start_at_dir.as_ref().parent() else {
        return Ok(None);
    };
    search_chewwy_root(parent)
}
