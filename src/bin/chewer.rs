use std::io::Write;

use chewwy::{
    cfg::{self, Cfg, StructMerge},
    prelude::*,
};
use clap::{Parser, Subcommand};

pub const DOT_DIR: &str = ".chewwy";

#[derive(Parser)]
struct Args {
    #[arg(short, long, value_name = "PATH")]
    config_file: Option<String>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Manage a file
    Manage {
        #[arg(value_name = "PATH")]
        file: Option<PathBuf>,
    },
}

#[derive(Debug, Error)]
enum CfgError {
    #[error("invalid cfg {0}")]
    Invalid(toml::de::Error),
    #[error("io error {0}")]
    Io(io::Error),
}

#[derive(Debug, Error)]
#[error("app error")]
struct AppError;

fn search_chewwy_root<P: AsRef<Path>>(dir: P) -> io::Result<Option<PathBuf>> {
    for entry in fs::read_dir(&dir)? {
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
    let Some(parent) = dir.as_ref().parent() else {
        return Ok(None);
    };
    search_chewwy_root(parent)
}

fn chewwy_root_cfg<P: AsRef<Path>>(root: P) -> Result<Cfg, CfgError> {
    load_cfg(root.as_ref().join(DOT_DIR).join(cfg::FILE_NAME))
}

fn load_cfg<P: AsRef<Path>>(cfg_file_path: P) -> Result<Cfg, CfgError> {
    let content = fs::read_to_string(cfg_file_path).map_err(CfgError::Io)?;
    let cfg = toml::from_str(&content).map_err(CfgError::Invalid)?;
    Ok(cfg)
}

fn main() -> StackResult<(), AppError> {
    let args = Args::parse();

    let current_dir = env::current_dir().change_context(AppError)?;
    let chewwy_root =
        search_chewwy_root(current_dir).change_context(AppError)?;

    let arg_cfg = match args.config_file {
        Some(c) => Some(load_cfg(c).change_context(AppError)?),
        None => None,
    };
    let chewwy_root_cfg = match &chewwy_root {
        Some(chewwy_root) => match chewwy_root_cfg(chewwy_root) {
            Ok(c) => Some(c),
            Err(CfgError::Io(e)) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e).change_context(AppError),
        },
        None => None,
    };
    let default_cfg = Cfg::default();
    let cfg = match (chewwy_root_cfg, arg_cfg) {
        (None, None) => default_cfg,
        (None, Some(mut a)) => {
            a.struct_merge(&default_cfg);
            a
        }
        (Some(mut r), None) => {
            r.struct_merge(&default_cfg);
            r
        }
        (Some(r), Some(mut a)) => {
            a.struct_merge(&r);
            a.struct_merge(&default_cfg);
            a
        }
    };

    match args.command {
        Some(command) => match command {
            Command::Manage { file } => {
                command_manage(&cfg, &chewwy_root, file)
                    .change_context(AppError)?;
            }
        },
        None => {
            todo!()
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
#[error("command manage error")]
struct CommandManageError;

fn command_manage<R: AsRef<Path>, F: AsRef<Path>>(
    cfg: &Cfg,
    chewwy_root: &Option<R>,
    file: Option<F>,
) -> StackResult<(), CommandManageError> {
    let Some(chewwy_root) = chewwy_root else {
        return Err(CommandManageError)
            .attach_printable("Chewwy root not found for this command");
    };
    let chewwy_root = chewwy_root.as_ref();
    let manage_cfg = cfg.commands.c().manage.c();
    let directories_cfg = manage_cfg.directories.c().to_absolute(chewwy_root);
    let formats_cfg = cfg.formats.c();
    let mut file = file.map(|f| f.as_ref().to_path_buf());

    if file.is_none() {
        if !manage_cfg.search_file.c() {
            return Err(CommandManageError).attach_printable(
                "File is not provided. Or try to use search-file feature",
            );
        }
        let Some(search_dir) = directories_cfg.search.c() else {
            return Err(CommandManageError).attach_printable(
                "File is not provided and search directory is not configured.",
            );
        };
        let search_dir_canon = match search_dir.canonicalize() {
            Ok(o) => o,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(CommandManageError).attach_printable_lazy(|| {
                    format!(
                        "Configured search dir `{}` is not found",
                        search_dir.display()
                    )
                })
            }
            Err(e) => {
                return Err(e)
                    .change_context(CommandManageError)
                    .attach_printable("search dir")
            }
        };

        let mut items = vec![];
        for entry in fs::read_dir(search_dir_canon)
            .change_context(CommandManageError)
            .attach_printable("cannot read search dir")?
        {
            let entry = entry
                .change_context(CommandManageError)
                .attach_printable("cannot read entry")?;
            let path = entry.path();
            items.push(path);
        }
        if items.is_empty() {
            return Err(CommandManageError)
                .attach_printable("no item found in search directory");
        }
        println!("Choose an item");
        for (i, item) in items.iter().enumerate() {
            println!(
                "[{i}] {}",
                item.file_name()
                    .unwrap_or_else(|| OsStr::new("???"))
                    .to_string_lossy()
            );
        }
        print!("> ");
        std::io::stdout()
            .flush()
            .change_context(CommandManageError)
            .attach_printable("error flushing")?;
        let mut input = String::new();
        std::io::stdin()
            .read_line(&mut input)
            .change_context(CommandManageError)?;
        let num = input
            .trim()
            .parse::<usize>()
            .change_context(CommandManageError)
            .attach_printable("what")?;
        let Some(choosen_file) = items.get(num) else {
            return Err(CommandManageError).attach_printable("no item exists");
        };
        file = Some(choosen_file.clone());
    }

    let compressed_file = file.unwrap();
    let compressed_file: &Path = compressed_file.as_ref();
    let canon_compressed_file_path = compressed_file
        .canonicalize()
        .change_context(CommandManageError)
        .attach_printable("cannot canonicalize")?;
    if !canon_compressed_file_path.is_file() {
        return Err(CommandManageError).attach_printable_lazy(|| {
            format!("{} is not a file", compressed_file.display())
        });
    }
    let output_file_dir_name =
        Path::new(canon_compressed_file_path.file_name().expect("file name"))
            .with_extension("");
    let output_file_dir_path;

    match manage_cfg.output_file_action.c() {
        cfg::OutputFileAction::DecompressToOutputDir => {
            let Some(output_dir) = directories_cfg.output.c() else {
                return Err(CommandManageError)
                    .attach_printable("`output` directory is not configured");
            };
            let file_archiver =
                chewwy::file_archiver::FileArchiver::new(formats_cfg);
            output_file_dir_path =
                Some(Path::new(output_dir).join(output_file_dir_name));
            file_archiver
                .decompress_to_dir(
                    &canon_compressed_file_path,
                    output_file_dir_path.as_ref().unwrap(),
                )
                .change_context(CommandManageError)
                .attach_printable("cannont decompress")?;
        }
    }

    if let Some(output_file_dir_path) = output_file_dir_path {
        if *manage_cfg.smart_decompress_directory.c() {
            println!("Unnesting dir");
            match chewwy::unnest_dir(output_file_dir_path) {
                Ok(())
                | Err(chewwy::UnnestDirError::Empty)
                | Err(chewwy::UnnestDirError::NotNested) => {}
                Err(chewwy::UnnestDirError::Io(e)) => {
                    return Err(e)
                        .change_context(CommandManageError)
                        .attach_printable("error unnesting dir");
                }
            }
        }
    }

    match manage_cfg.compressed_file_action.c() {
        cfg::CompressedFileAction::MoveToArchiveDir => {
            let Some(archive_dir) = directories_cfg.archive.c() else {
                return Err(CommandManageError)
                    .attach_printable("`achive` directory is not configured");
            };

            let file_name =
                canon_compressed_file_path.file_name().expect("file name");
            let new_path = archive_dir.join(file_name);
            fs::rename(canon_compressed_file_path, new_path)
                .change_context(CommandManageError)
                .attach_printable("can't move achive to achive dir")?;
        }
        cfg::CompressedFileAction::DoNothing => {}
    }

    Ok(())
}
