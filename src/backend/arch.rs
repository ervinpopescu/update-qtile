// src/backend/arch.rs
use std::io::Write;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};

use glob::glob;
use regex::Regex;
use subprocess::{Exec, Redirection};

use super::{error_and_exit, remove_dir_with_sudo_fallback, InstallBackend, RefType, Source};

pub struct ArchBackend {
    repo_path: PathBuf,
}

impl ArchBackend {
    pub fn new() -> Self {
        let xdg_cache_home =
            std::env::var("XDG_CACHE_HOME").unwrap_or_else(|_| {
                let home = std::env::var("HOME").expect("HOME not set");
                format!("{home}/.cache")
            });
        let repo_path = Path::new(&xdg_cache_home)
            .join("yay")
            .join("qtile-git");
        Self { repo_path }
    }

    fn format_pkgbuild_source(source: &Source) -> String {
        match &source.ref_type {
            RefType::Default => source.url.clone(),
            RefType::Branch(b) => format!("{}#branch={}", source.url, b),
            RefType::Commit(c) => format!("{}#commit={}", source.url, c),
            RefType::Tag(t) => format!("{}#tag={}", source.url, t),
            RefType::Pull(_) => source.url.clone(),
        }
    }

    fn modify_pkgbuild(&self, source: &Source) -> anyhow::Result<()> {
        log::info!("modifying PKGBUILD");
        let contents = std::fs::read_to_string(self.repo_path.join("PKGBUILD"))
            .map_err(|e| {
                anyhow::anyhow!("could not read PKGBUILD: {e}")
            })?;

        let mut lines: Vec<String> = contents
            .split_inclusive('\n')
            .map(|s| s.to_owned())
            .collect();

        let license_regex = Regex::new(r"license=\(.*\)").unwrap();
        let source_regex = Regex::new(r"source=\(.*\)").unwrap();
        let describe_regex = Regex::new(r".*git describe").unwrap();

        let remote = Self::format_pkgbuild_source(source);

        for (index, line) in lines.clone().into_iter().enumerate() {
            if license_regex.is_match(&line) {
                lines.insert(index + 1, "groups=('modified')\n".to_owned());
            }
            if source_regex.is_match(&line) {
                lines[index + 1] = format!("source=('git+{}')\n", remote);
            }
            if describe_regex.is_match(&line) {
                lines.insert(
                    index + 1,
                    "  git remote add upstream https://github.com/qtile/qtile.git\n"
                        .to_owned(),
                );
                lines.insert(
                    index + 2,
                    "  git fetch upstream --tags --force\n".to_owned(),
                );
                if let RefType::Pull(pr) = &source.ref_type {
                    lines.insert(
                        index + 3,
                        format!("  git fetch upstream pull/{pr}/head:pr{pr}\n"),
                    );
                    lines.insert(
                        index + 4,
                        format!("  git checkout pr{pr}\n"),
                    );
                }
            }
        }

        std::fs::write(self.repo_path.join("PKGBUILD"), lines.concat())
            .map_err(|e| anyhow::anyhow!("could not write to PKGBUILD: {e}"))?;

        Ok(())
    }

    fn remove_file_or_dir_if_exists(path: &str) -> anyhow::Result<()> {
        if let Ok(true) = std::fs::exists(path) {
            let filetype = std::fs::metadata(path).unwrap().file_type();
            if filetype.is_dir() {
                std::fs::remove_dir_all(path)?;
            }
            if filetype.is_file() {
                std::fs::remove_file(path)?;
            }
        }
        Ok(())
    }
}

impl InstallBackend for ArchBackend {
    fn prepare(&mut self) -> anyhow::Result<()> {
        remove_dir_with_sudo_fallback(&self.repo_path)
    }

    fn build(&mut self, source: &Source) -> anyhow::Result<()> {
        log::info!("cloning AUR repo");
        let aur_url = "https://aur.archlinux.org/qtile-git";
        match git2::Repository::clone(aur_url, &self.repo_path) {
            Ok(_) => self.modify_pkgbuild(source)?,
            Err(err) => error_and_exit(
                &format!("AUR URL {aur_url} is unreachable, error: {err}"),
            ),
        }
        Ok(())
    }

    fn install(&mut self) -> anyhow::Result<()> {
        log::info!("building with `makepkg`");
        let log_path = self.repo_path.join("install.log");
        let mut f = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .map_err(|e| {
                anyhow::anyhow!("could not create install.log: {e}")
            })?;

        writeln!(f, "\n------------------------------- building new package -------------------------------\n")?;

        let exit_status = (Exec::cmd("yes")
            | Exec::cmd("makepkg")
                .args(&["-rsc", "--nocheck"])
                .cwd(&self.repo_path)
                .stderr(Redirection::Merge))
        .stdout(f.try_clone().expect("could not clone file handle"))
        .join()?
        .success();

        if !exit_status {
            log::error!(
                "Qtile build failed, check in {}/install.log",
                self.repo_path.display()
            );
            return Ok(());
        }

        log::info!("removing old package");
        writeln!(f, "\n------------------------------- removing old package -------------------------------\n")?;

        if Exec::cmd("sudo")
            .args(&["pacman", "-Qq", "qtile-git"])
            .cwd(&self.repo_path)
            .stderr(Redirection::Merge)
            .stdout(f.try_clone().expect("could not clone file handle"))
            .join()?
            .success()
        {
            // package is tracked by pacman, will be overwritten
        } else {
            let to_be_deleted = [
                "/usr/bin/qtile",
                "/usr/lib/python3.*/site-packages/libqtile",
                "/usr/share/doc/qtile-git",
                "/usr/share/licenses/qtile-git/LICENSE",
                "/usr/share/wayland-sessions/qtile-wayland.desktop",
                "/usr/share/xsessions/qtile.desktop",
            ];
            for pattern in to_be_deleted {
                for entry in glob(pattern).unwrap().flatten() {
                    Self::remove_file_or_dir_if_exists(
                        entry.to_str().unwrap(),
                    )?;
                }
            }
        }

        log::info!("installing new package");
        writeln!(f, "\n------------------------------- installing new package -------------------------------\n")?;

        let pkg_path = glob(
            &format!("{}/*.tar.zst", self.repo_path.display()),
        )
        .unwrap()
        .next()
        .unwrap()
        .unwrap();

        let exit_status = (Exec::cmd("yes")
            | Exec::cmd("sudo")
                .args(&[
                    "pacman",
                    "-U",
                    pkg_path.to_str().expect("package built successfully"),
                    "--overwrite",
                    "'*'",
                ])
                .cwd(&self.repo_path)
                .stderr(Redirection::Merge))
        .stdout(f.try_clone().expect("could not clone file handle"))
        .join()?
        .success();

        if !exit_status {
            log::error!(
                "Qtile install failed, check in {}/install.log",
                self.repo_path.display()
            );
        }

        writeln!(f, "\n------------------------------- package installed successfully -------------------------------")?;

        Ok(())
    }
}
