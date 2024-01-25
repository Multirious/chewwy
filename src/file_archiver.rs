use crate::prelude::*;
use std::collections::HashMap;

use crate::cfg;
use cfg::Format;

#[derive(Debug, Error)]
pub enum DecompressError {
    #[error("no format available for \"{file}\"")]
    NoFormatAvailable { file: String },
    #[error("found format \"{found_format_name}\" for file \"{file}\" but no command available")]
    NoCommandAvailable {
        file: String,
        found_format_name: String,
    },
    #[error("error {io} trying to run the commannd {command_str} from command config {command:?} in format {format}")]
    RunCommandError {
        command_str: String,
        command: cfg::Command,
        io: io::Error,
        format: String,
    },
    #[error("error code {code} from commannd {command_str} from command config {command:?} in format {format}")]
    ChildReturnErrorCode {
        command_str: String,
        command: cfg::Command,
        code: i32,
        format: String,
    },
    #[error("error return from commannd {command_str} from command config {command:?} in format {format}")]
    ChildError {
        command_str: String,
        command: cfg::Command,
        format: String,
    },
    #[error("error {io} return from commannd {command_str} from command config {command:?} in format {format}")]
    ChildWaitReturnError {
        command_str: String,
        command: cfg::Command,
        format: String,
        io: io::Error,
    },
}

pub struct FileArchiver<'cfg> {
    formats: &'cfg HashMap<String, Format>,
}

impl<'cfg> FileArchiver<'cfg> {
    pub fn new(formats: &'cfg HashMap<String, Format>) -> Self {
        FileArchiver { formats }
    }

    pub fn decompress_to_dir<F, D>(
        &self,
        file: F,
        dir: D,
    ) -> Result<(), DecompressError>
    where
        F: AsRef<Path>,
        D: AsRef<Path>,
    {
        let file_str = file.as_ref().to_string_lossy();
        let dir_str = dir.as_ref().to_string_lossy();
        let Some((format_name, format)) = self.find_format(&file) else {
            return Err(DecompressError::NoFormatAvailable {
                file: file_str.to_string(),
            });
        };

        let decompress_commands = format.decompress.c();
        let mut child = None;
        for decompress_command in decompress_commands {
            let mut command = decompress_command
                .decompress_command_format(&file_str, &dir_str);
            match command.spawn() {
                Ok(c) => {
                    child = Some((c, command, decompress_command));
                    break;
                }
                Err(e) => {
                    if e.kind() == io::ErrorKind::NotFound {
                        continue;
                    }
                    return Err(DecompressError::RunCommandError {
                        command_str: format!("{command:?}"),
                        command: decompress_command.clone(),
                        io: e,
                        format: format_name.to_string(),
                    });
                }
            }
        }

        let Some((mut child, command, command_cfg)) = child else {
            return Err(DecompressError::NoCommandAvailable {
                file: file_str.to_string(),
                found_format_name: format_name.clone(),
            });
        };
        match child.wait() {
            Ok(o) => 'ok: {
                if o.success() {
                    break 'ok;
                }
                if let Some(code) = o.code() {
                    return Err(DecompressError::ChildReturnErrorCode {
                        command_str: format!("{command:?}"),
                        command: command_cfg.clone(),
                        code,
                        format: format_name.clone(),
                    });
                } else {
                    return Err(DecompressError::ChildError {
                        command_str: format!("{command:?}"),
                        command: command_cfg.clone(),
                        format: format_name.clone(),
                    });
                }
            }
            Err(e) => {
                return Err(DecompressError::ChildWaitReturnError {
                    command_str: format!("{command:?}"),
                    command: command_cfg.clone(),
                    format: format_name.clone(),
                    io: e,
                })
            }
        }

        Ok(())
    }

    fn find_format<P: AsRef<Path>>(
        &self,
        file: P,
    ) -> Option<(&String, &Format)> {
        fn file_extensions<P: AsRef<Path>>(file: &P) -> Option<Vec<&str>> {
            let file = file.as_ref();
            let file = file.components().last()?;
            let file = file.as_os_str().to_str()?;
            let extensions = file.split('.').skip(1).collect::<Vec<_>>();
            Some(extensions)
        }

        let file = file.as_ref();
        let extensions = file_extensions(&file)?;
        self.formats.iter().find(|(_, f)| {
            'exts: for cfg_ext in f.extensions.c() {
                for (i, cfg_ext_part) in cfg_ext.split('.').enumerate() {
                    let Some(ext_part) = extensions.get(i) else {
                        continue 'exts;
                    };
                    if *ext_part != cfg_ext_part {
                        continue 'exts;
                    }
                }
                return true;
            }
            false
        })
    }
}
