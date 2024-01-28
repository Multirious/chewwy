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
        find_format(self.formats, file)
    }
}

fn find_format<P: AsRef<Path>>(
    formats: &HashMap<String, Format>,
    file: P,
) -> Option<(&String, &Format)> {
    fn file_extension_vec<P: AsRef<Path>>(file: &P) -> Option<Vec<&str>> {
        let file = file.as_ref();
        let file = file.components().last()?;
        let file = file.as_os_str().to_str()?;
        let extensions = file.split('.').skip(1).collect::<Vec<_>>();
        Some(extensions)
    }

    let file = file.as_ref();
    let extension_vec = file_extension_vec(&file)?;
    let extension_format_cache = ExtensionFormatCache::new(formats);
    let mut found_format = None;
    for i in 0..extension_vec.len() {
        let extension_vec = &extension_vec[i..];
        let extension = {
            let mut s = String::new();
            s.push_str(extension_vec[0]);
            for ext_part in &extension_vec[1..] {
                s.push('.');
                s.push_str(ext_part);
            }
            s
        };
        if let Some(format) =
            extension_format_cache.extension_format.get(&extension)
        {
            found_format = Some(format);
            break;
        }
    }
    found_format.map(|format_name| (*format_name, &formats[*format_name]))
}

struct ExtensionFormatCache<'a> {
    extension_format: HashMap<&'a String, &'a String>,
}

impl<'a> ExtensionFormatCache<'a> {
    fn new(formats: &'a HashMap<String, Format>) -> Self {
        let extension_format = formats
            .iter()
            .flat_map(|(format_name, format)| {
                let extensions = format.extensions.c();
                extensions
                    .iter()
                    .map(move |extension| (extension, format_name))
            })
            .collect();
        ExtensionFormatCache { extension_format }
    }
}

#[cfg(test)]
mod test {
    use std::collections::{HashMap, HashSet};

    fn s(s: &str) -> String {
        s.to_string()
    }

    fn c<T>(t: T) -> crate::cfg::Configure<T> {
        crate::cfg::Configure(Some(t))
    }

    fn hashset<T, I>(i: I) -> HashSet<T>
    where
        T: std::cmp::Eq + std::hash::Hash,
        I: IntoIterator<Item = T>,
    {
        HashSet::from_iter(i)
    }

    #[test]
    fn find_format() {
        use crate::cfg::Format as F;
        let formats: HashMap<String, F> = HashMap::from_iter([
            (
                s("first"),
                F {
                    extensions: c(hashset([s("abc")])),
                    decompress: c(vec![]),
                },
            ),
            (
                s("second"),
                F {
                    extensions: c(hashset([s("abc.def")])),
                    decompress: c(vec![]),
                },
            ),
            (
                s("third"),
                F {
                    extensions: c(hashset([s("def")])),
                    decompress: c(vec![]),
                },
            ),
        ]);
        assert_eq!(
            Some(&s("first")),
            super::find_format(&formats, "a_file.abc").map(|a| a.0)
        );
        assert_eq!(
            Some(&s("first")),
            super::find_format(&formats, "a_file.hiya.abc").map(|a| a.0)
        );
        assert_eq!(
            Some(&s("third")),
            super::find_format(&formats, "a_file.def").map(|a| a.0)
        );
        assert_eq!(
            Some(&s("second")),
            super::find_format(&formats, "a_file.abc.def").map(|a| a.0)
        );
    }
}
