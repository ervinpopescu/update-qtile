use std::path::{Path, PathBuf};

use subprocess::Exec;

use super::{error_and_exit, remove_dir_with_sudo_fallback, InstallBackend, RefType, Source};

pub struct UvBackend {
    clone_dir: PathBuf,
}

impl UvBackend {
    pub fn new() -> Self {
        let xdg_cache_home = std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").expect("HOME not set");
            format!("{home}/.cache")
        });
        let clone_dir = Path::new(&xdg_cache_home)
            .join("update-qtile")
            .join("qtile-src");
        Self { clone_dir }
    }

    fn check_uv_available() {
        if which::which("uv").is_err() {
            error_and_exit(
                "uv not found on $PATH. Install it: curl -LsSf https://astral.sh/uv/install.sh | sh",
            );
        }
    }

    fn checkout_ref(repo: &git2::Repository, source: &Source) -> anyhow::Result<()> {
        match &source.ref_type {
            RefType::Default => {
                // default branch, nothing to do
            }
            RefType::Branch(branch) => {
                let remote_ref = format!("refs/remotes/origin/{branch}");
                let obj = repo.revparse_single(&remote_ref)?;
                let commit = obj.peel_to_commit()?;
                repo.branch(branch, &commit, false)?;
                repo.checkout_tree(&obj, None)?;
                repo.set_head(&format!("refs/heads/{branch}"))?;
            }
            RefType::Commit(commit) => {
                let oid = git2::Oid::from_str(commit)?;
                let obj = repo.find_object(oid, None)?;
                repo.checkout_tree(&obj, None)?;
                repo.set_head_detached(oid)?;
            }
            RefType::Tag(tag) => {
                let refspec = format!("refs/tags/{tag}");
                let obj = repo.revparse_single(&refspec)?;
                repo.checkout_tree(&obj, None)?;
                repo.set_head(&refspec)?;
            }
            RefType::Pull(pr_num) => {
                // Add upstream remote and fetch the PR ref
                let upstream_url = "https://github.com/qtile/qtile.git";
                let mut upstream = repo.remote("upstream", upstream_url)?;
                let refspec = format!("refs/pull/{pr_num}/head:refs/heads/pr{pr_num}");
                upstream.fetch(&[&refspec], None, None)?;
                let pr_ref = format!("refs/heads/pr{pr_num}");
                let obj = repo.revparse_single(&pr_ref)?;
                repo.checkout_tree(&obj, None)?;
                repo.set_head(&pr_ref)?;
            }
        }
        Ok(())
    }
}

impl InstallBackend for UvBackend {
    fn prepare(&mut self) -> anyhow::Result<()> {
        Self::check_uv_available();
        remove_dir_with_sudo_fallback(&self.clone_dir)
    }

    fn build(&mut self, source: &Source) -> anyhow::Result<()> {
        log::info!("cloning qtile source from {}", source.url);
        let repo = match git2::Repository::clone(&source.url, &self.clone_dir) {
            Ok(r) => r,
            Err(err) => {
                error_and_exit(&format!("could not clone {}: {err}", source.url));
            }
        };
        Self::checkout_ref(&repo, source)?;
        Ok(())
    }

    fn install(&mut self) -> anyhow::Result<()> {
        log::info!("installing with `uv tool install`");
        let exit_status = Exec::cmd("uv")
            .args(&["tool", "install", ".", "--force"])
            .cwd(&self.clone_dir)
            .join()?
            .success();
        if !exit_status {
            error_and_exit("uv tool install failed");
        }
        Ok(())
    }
}
