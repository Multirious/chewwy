use crate::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use serde::Deserialize;

pub const FILE_NAME: &str = "cfg.toml";

#[derive(Debug, Error)]
pub enum LoadCfgError {
    #[error("invalid cfg {0}")]
    Invalid(toml::de::Error),
    #[error("io error {0}")]
    Io(io::Error),
}

pub fn load_cfg<P: AsRef<Path>>(cfg_file_path: P) -> Result<Cfg, LoadCfgError> {
    let content =
        fs::read_to_string(cfg_file_path).map_err(LoadCfgError::Io)?;
    let cfg = toml::from_str(&content).map_err(LoadCfgError::Invalid)?;
    Ok(cfg)
}

pub fn root_cfg_path<P: AsRef<Path>>(root: P) -> PathBuf {
    root.as_ref().join(crate::DOT_DIR).join(FILE_NAME)
}

#[derive(
    Debug,
    Default,
    Deserialize,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
)]
#[serde(transparent)]
pub struct Configure<T>(pub Option<T>);

impl<T> Configure<T> {
    pub fn c(&self) -> &T {
        self.0.as_ref().unwrap()
    }
}

impl<T: Clone> Configure<T> {
    fn merge_value(&mut self, other: &Configure<T>) {
        if let (None, Some(o)) = (&self.0, &other.0) {
            self.0 = Some(o.clone())
        }
    }
}

impl<T: Clone + StructMerge> Configure<T> {
    fn merge_struct(&mut self, other: &Configure<T>) {
        match (&mut self.0, &other.0) {
            (None, Some(o)) => self.0 = Some(o.clone()),
            (Some(s), Some(o)) => s.struct_merge(o),
            _ => {}
        }
    }
}

impl<T: Clone + StructMerge> Configure<HashMap<String, T>> {
    fn merge_struct_with_identical_key(
        &mut self,
        other: &Configure<HashMap<String, T>>,
    ) {
        match (&mut self.0, &other.0) {
            (None, Some(o)) => self.0 = Some(o.clone()),
            (Some(s), Some(o)) => {
                for (k, v) in s.iter_mut() {
                    let Some(ov) = o.get(k) else {
                        continue;
                    };
                    v.struct_merge(ov);
                }
            }
            _ => {}
        }
    }
}

pub trait StructMerge {
    fn struct_merge(&mut self, other: &Self);
}

impl<T> From<Option<T>> for Configure<T> {
    fn from(value: Option<T>) -> Self {
        Configure(value)
    }
}

impl<T> From<Configure<T>> for Option<T> {
    fn from(value: Configure<T>) -> Self {
        value.0
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Cfg {
    pub formats: Configure<HashMap<String, Format>>,
    pub commands: Configure<CommandsCfg>,
}

impl StructMerge for Cfg {
    fn struct_merge(&mut self, other: &Cfg) {
        self.formats.merge_struct_with_identical_key(&other.formats);
        self.commands.merge_struct(&other.commands);
    }
}

impl Default for Cfg {
    fn default() -> Self {
        toml::from_str::<Cfg>(include_str!("../cfg.toml")).unwrap()
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct Format {
    /// Search for the following extensions
    pub extensions: Configure<HashSet<String>>,
    /// Will use the first command that exists
    pub decompress: Configure<Vec<Command>>,
    // /// Will use the first command that exists
    // pub compress: Vec<Command>,
}

impl StructMerge for Format {
    fn struct_merge(&mut self, other: &Format) {
        self.extensions.merge_value(&other.extensions);
        self.decompress.merge_value(&other.decompress);
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct Command {
    pub path: String,
    /// `{FILE}` for origin file path
    /// `{DIR} for output directory path
    pub args: Vec<String>,
}

impl Command {
    pub fn decompress_command_format(
        &self,
        file: &str,
        dir: &str,
    ) -> process::Command {
        let mut command = process::Command::new(&self.path);
        command.args(
            self.args
                .iter()
                .map(|arg| arg.replace("{FILE}", file).replace("{DIR}", dir)),
        );
        command
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct CommandsCfg {
    pub manage: Configure<ManageCommandCfg>,
}

impl StructMerge for CommandsCfg {
    fn struct_merge(&mut self, other: &CommandsCfg) {
        self.manage.merge_struct(&other.manage);
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "kebab-case")]
pub struct ManageCommandCfg {
    /// Smart decompress output to a no nest directory
    pub smart_decompress_directory: Configure<bool>,
    pub search_file: Configure<bool>,
    /// What to do with the output file after finishing
    pub output_file_action: Configure<OutputFileAction>,
    /// What to do with the compressed file after finishing
    pub compressed_file_action: Configure<CompressedFileAction>,
    pub directories: Configure<Directories>,
}

impl StructMerge for ManageCommandCfg {
    fn struct_merge(&mut self, other: &ManageCommandCfg) {
        self.smart_decompress_directory
            .merge_value(&other.smart_decompress_directory);
        self.search_file.merge_value(&other.search_file);
        self.output_file_action
            .merge_value(&other.output_file_action);
        self.compressed_file_action
            .merge_value(&other.compressed_file_action);
        self.directories.merge_struct(&other.directories);
    }
}

#[derive(Debug, Default, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OutputFileAction {
    #[default]
    DecompressToOutputDir,
}

#[derive(Debug, Default, Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CompressedFileAction {
    #[default]
    MoveToArchiveDir,
    DoNothing,
}

/// Will resolve path variable and stuff
#[derive(Debug, Default, Deserialize, Clone)]
pub struct Directories {
    pub search: Configure<Option<PathBuf>>,
    pub output: Configure<Option<PathBuf>>,
    pub archive: Configure<Option<PathBuf>>,
}

impl Directories {
    pub fn to_absolute<P: AsRef<Path>>(&self, relative_to: P) -> Directories {
        let search = self.search.c().as_ref().map(|search| {
            if search.is_relative() {
                relative_to.as_ref().join(search)
            } else {
                search.to_owned()
            }
        });
        let output = self.output.c().as_ref().map(|output| {
            if output.is_relative() {
                relative_to.as_ref().join(output)
            } else {
                output.to_owned()
            }
        });
        let archive = self.archive.c().as_ref().map(|archive| {
            if archive.is_relative() {
                relative_to.as_ref().join(archive)
            } else {
                archive.to_owned()
            }
        });
        Directories {
            search: Configure(Some(search)),
            output: Configure(Some(output)),
            archive: Configure(Some(archive)),
        }
    }
}

impl StructMerge for Directories {
    fn struct_merge(&mut self, other: &Directories) {
        self.search.merge_value(&other.search);
        self.output.merge_value(&other.output);
        self.archive.merge_value(&other.archive);
    }
}
