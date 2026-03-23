// src/backend/mod.rs
pub mod arch;

use clap::ValueEnum;
use std::path::Path;
use std::process::exit;
use subprocess::Exec;
use text_io::read;

#[derive(Debug, Clone)]
pub struct Source {
    pub url: String,
    pub ref_type: RefType,
}

#[derive(Debug, Clone)]
pub enum RefType {
    Default,
    Branch(String),
    Commit(String),
    Tag(String),
    Pull(String),
}

pub trait InstallBackend {
    fn prepare(&mut self) -> anyhow::Result<()>;
    fn build(&mut self, source: &Source) -> anyhow::Result<()>;
    fn install(&mut self) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, ValueEnum)]
pub enum InstallMethod {
    Arch,
    Uv,
    Debian,
}

pub fn error_and_exit(err: &str) -> ! {
    log::error!("{err}");
    exit(1);
}

/// Resolve CLI args into a backend-agnostic Source.
pub fn get_source(
    fork: &Option<String>,
    path: &Option<String>,
    branch: &Option<String>,
    commit: &Option<String>,
    tag: &Option<String>,
    pull: &Option<String>,
) -> Source {
    let url = if let Some(path) = path {
        format!("file://{path}")
    } else if let Some(fork) = fork {
        format!("https://github.com/{fork}/qtile")
    } else {
        "https://github.com/qtile/qtile".to_owned()
    };

    let ref_type = if let Some(c) = commit {
        RefType::Commit(c.clone())
    } else if let Some(b) = branch {
        RefType::Branch(b.clone())
    } else if let Some(t) = tag {
        RefType::Tag(t.clone())
    } else if let Some(p) = pull {
        RefType::Pull(p.clone())
    } else {
        RefType::Default
    };

    log::info!(
        "selected repo `{}` - {}",
        url,
        match &ref_type {
            RefType::Default => "branch `master`".to_owned(),
            RefType::Branch(b) => format!("branch `{b}`"),
            RefType::Commit(c) => format!("commit `{c}`"),
            RefType::Tag(t) => format!("tag `{t}`"),
            RefType::Pull(p) => format!("PR `{p}`"),
        }
    );

    Source { url, ref_type }
}

/// Remove a directory, prompting for sudo if normal removal fails.
pub fn remove_dir_with_sudo_fallback(path: &Path) -> anyhow::Result<()> {
    if path.exists() {
        log::info!("removing cached repo {:?}", path);
        match std::fs::remove_dir_all(path) {
            Ok(()) => {}
            Err(err) => {
                log::error!("couldn't remove cached repo");
                log::error!("\tError: {err}");
                log::info!("Would you like to try with root permissions? [Y/n]");
                let ans: String = read!("{}\n");
                if ["Y", "y", ""].contains(&ans.as_str()) {
                    let path_str = path.as_os_str();
                    let exit_status = Exec::shell(format!("sudo rm -rf {path_str:?}"))
                        .join()?
                        .success();
                    if !exit_status {
                        error_and_exit("could not run sudo");
                    }
                }
            }
        }
    }
    Ok(())
}
