// src/backend/mod.rs
pub mod arch;
pub mod debian;
pub mod uv;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_source_default_fork() {
        let s = get_source(&None, &None, &None, &None, &None, &None);
        assert_eq!(s.url, "https://github.com/qtile/qtile");
        assert!(matches!(s.ref_type, RefType::Default));
    }

    #[test]
    fn get_source_custom_fork() {
        let s = get_source(
            &Some("user".into()),
            &None,
            &None,
            &None,
            &None,
            &None,
        );
        assert_eq!(s.url, "https://github.com/user/qtile");
    }

    #[test]
    fn get_source_local_path() {
        let s = get_source(
            &None,
            &Some("/tmp/qtile".into()),
            &None,
            &None,
            &None,
            &None,
        );
        assert_eq!(s.url, "file:///tmp/qtile");
    }

    #[test]
    fn get_source_branch() {
        let s = get_source(&None, &None, &Some("dev".into()), &None, &None, &None);
        assert!(matches!(s.ref_type, RefType::Branch(ref b) if b == "dev"));
    }

    #[test]
    fn get_source_commit() {
        let s = get_source(&None, &None, &None, &Some("abc123".into()), &None, &None);
        assert!(matches!(s.ref_type, RefType::Commit(ref c) if c == "abc123"));
    }

    #[test]
    fn get_source_tag() {
        let s = get_source(&None, &None, &None, &None, &Some("v1.0".into()), &None);
        assert!(matches!(s.ref_type, RefType::Tag(ref t) if t == "v1.0"));
    }

    #[test]
    fn get_source_pull() {
        let s = get_source(&None, &None, &None, &None, &None, &Some("42".into()));
        assert!(matches!(s.ref_type, RefType::Pull(ref p) if p == "42"));
    }

    #[test]
    fn get_source_path_with_branch() {
        let s = get_source(
            &None,
            &Some("/tmp/qtile".into()),
            &Some("feature".into()),
            &None,
            &None,
            &None,
        );
        assert_eq!(s.url, "file:///tmp/qtile");
        assert!(matches!(s.ref_type, RefType::Branch(ref b) if b == "feature"));
    }
}
