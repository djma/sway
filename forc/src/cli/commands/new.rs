use crate::{cli::init::Command as InitCommand, ops::forc_init::init};
use anyhow::{anyhow, bail, Result};
use clap::Parser;
use forc_util::validate_name;
use std::path::{Path, PathBuf};

/// Create a new Forc project at `<path>`.
#[derive(Debug, Parser)]
pub struct Command {
    /// The default program type. Excluding all flags or adding this flag creates a basic contract
    /// program.
    #[clap(long)]
    pub contract: bool,
    /// Adding this flag creates an empty script program.
    #[clap(long)]
    pub script: bool,
    /// Adding this flag creates an empty predicate program.
    #[clap(long)]
    pub predicate: bool,
    /// Adding this flag creates an empty library program.
    #[clap(long)]
    pub library: bool,
    /// Adding this flag creates an empty workspace.
    #[clap(long)]
    pub workspace: bool,
    /// Set the package name. Defaults to the directory name.
    #[clap(long)]
    pub name: Option<String>,
    /// The path at which the standard library is located.
    /// If unspecified, the std will be implicitly included as a git dependency
    /// via the git tag that matches `forc`'s version.
    /// If the path is relative, forc makes it relative w.r.t. the project directory.
    /// If the path is absolute, forc uses it as is.
    #[clap(long)]
    pub stdlib: Option<String>,
    /// The path at which the project directory will be created.
    pub path: String,
}

pub(crate) fn exec(command: Command) -> Result<()> {
    // `forc new` is roughly short-hand for `forc init`, but we create the directory first if it
    // doesn't yet exist. Here we create the directory if it doesn't exist then re-use the existing
    // `forc init` logic.
    let Command {
        contract,
        script,
        predicate,
        library,
        workspace,
        name,
        stdlib,
        path,
    } = command;

    match &name {
        Some(name) => validate_name(name, "project name")?,
        None => {
            // If there is no name specified for the project, the last component of the `path` (directory name)
            // will be used by default so we should also check that.
            let project_path = PathBuf::from(&path);
            let directory_name = project_path
                .file_name()
                .ok_or_else(|| anyhow!("missing path for new command"))?
                .to_string_lossy();
            validate_name(&directory_name, "project_name")?;
        }
    }

    let dir_path = Path::new(&path);
    if dir_path.exists() {
        bail!(
            "Directory \"{}\" already exists.\nIf you wish to initialise a forc project inside \
            this directory, consider using `forc init --path {}`",
            dir_path.canonicalize()?.display(),
            dir_path.display(),
        );
    } else {
        std::fs::create_dir_all(dir_path)?;
    }

    let init_cmd = InitCommand {
        path: Some(path),
        contract,
        script,
        predicate,
        library,
        workspace,
        stdlib,
        name,
    };

    init(init_cmd)
}
