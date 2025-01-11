use std::io::Write;
use std::{fs::OpenOptions, path::Path, process::exit};

use clap::Parser;
use qtile_client_lib::utils::client::InteractiveCommandClient;
use regex::Regex;
use subprocess::{Exec, Redirection};
use text_io::read;

/// Qtile command client
#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[arg(
        short,
        long,
        num_args = 1,
        default_value = "qtile",
        group = "remote",
        conflicts_with = "path"
    )]
    fork: Option<String>,
    #[arg(short, long, num_args = 1, default_value = None, group = "remote")]
    path: Option<String>,
    #[arg(short, long, num_args = 1, default_value = None, group = "identifier",conflicts_with_all = ["branch", "tag"])]
    commit: Option<String>,
    #[arg(short, long, num_args = 1, default_value = None, group = "identifier")]
    branch: Option<String>,
    #[arg(short, long, num_args = 1, default_value = None, group = "identifier")]
    tag: Option<String>,
    #[arg(short, long, default_value_t = false)]
    restart: bool,
}

fn error_and_exit(err: &str) {
    log::error!("{err}");
    exit(1);
}

struct UpdateQtile {
    repo_path: Box<Path>,
    args: Args,
}
impl UpdateQtile {
    pub fn new(args: Args) -> Self {
        let xdg_cache_home = std::env::var("XDG_CACHE_HOME").unwrap_or("~/.cache".to_string());
        let repo_path = Path::new(&xdg_cache_home)
            .join("yay")
            .join("qtile-git")
            .as_path()
            .into();
        Self { repo_path, args }
    }
    fn get_source(&self) -> String {
        let source = if let Some(path) = &self.args.path {
            format!("file://{path}")
        } else if let Some(fork) = &self.args.fork {
            format!("https://github.com/{fork}/qtile")
        } else {
            "https://github.com/qtile/qtile".to_owned()
        };
        if let Some(commit) = &self.args.commit {
            log::info!("selected repo `{}` - commit `{}`", source, commit);
            format!("{}#commit={}", source, commit)
        } else if let Some(tag) = &self.args.tag {
            log::info!("selected repo `{}` - tag `{}`", source, tag);
            format!("{}#tag={}", source, tag)
        } else if let Some(branch) = &self.args.branch {
            log::info!("selected repo `{}` - branch `{}`", source, branch);
            format!("{}#branch={}", source, branch)
        } else {
            log::info!("selected repo `{}` - branch `master`", source);
            source
        }
    }
    fn remove_repo(&self) -> anyhow::Result<()> {
        if self.repo_path.exists() {
            log::info!("removing cached AUR repo {:?}", self.repo_path);
            match std::fs::remove_dir_all(&self.repo_path) {
                Ok(()) => {}
                Err(err) => {
                    log::error!("couldn't remove AUR cached repo");
                    log::error!("\tError: {err}");
                    log::info!("Would you like to try with root permissions? [Y/n]");
                    let ans: String = read!("{}\n");
                    if ["Y", "y", ""].contains(&ans.as_str()) {
                        let repo_path = self.repo_path.as_os_str();
                        let exit_status = Exec::shell(format!("sudo rm -rf {repo_path:?}"))
                            .join()?
                            .success();
                        match exit_status {
                            true => {}
                            false => error_and_exit("could not run sudo"),
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn clone_repo(&self, source: String) -> anyhow::Result<()> {
        log::info!("cloning AUR repo");
        let aur_url = "https://aur.archlinux.org/qtile-git";
        match git2::Repository::clone(aur_url, &self.repo_path) {
            Ok(_) => self.modify_pkgbuild(source)?,
            Err(err) => error_and_exit(
                ("AUR URL ".to_owned() + aur_url + " is unreachable, error: " + &err.to_string())
                    .as_str(),
            ),
        }
        Ok(())
    }

    fn modify_pkgbuild(&self, source: String) -> anyhow::Result<()> {
        log::info!("modifying PKGBUILD");
        let lines = std::fs::read_to_string(self.repo_path.join("PKGBUILD"));
        match lines {
            Ok(lines) => {
                let mut lines = lines
                    .split_inclusive('\n')
                    .map(|s| s.to_owned())
                    .collect::<Vec<String>>();
                let license_regex = Regex::new(r"license=\(.*\)").unwrap();
                let source_regex = Regex::new(r"source=\(.*\)").unwrap();
                let cd_regex = Regex::new(r".*cd qtile").unwrap();
                let describe_regex = Regex::new(r".*git describe").unwrap();
                for (index, line) in lines.clone().into_iter().enumerate() {
                    if license_regex.is_match(&line) {
                        lines.insert(index + 1, "groups=('modified')\n".to_owned());
                    }
                    if source_regex.is_match(&line) {
                        let inserted = format!("source=('git+{}')\n", source);
                        lines[index + 1] = inserted;
                    }
                    //if Regex::new(r".*build\(\).*").unwrap().is_match(&line) {
                    //    lines.insert(
                    //        index + 3,
                    //        "  export CFLAGS=\"$CFLAGS -I/usr/include/wlroots0.17\"\n".to_owned(),
                    //    );
                    //    lines.insert(
                    //        index + 4,
                    //        "  export LDFLAGS=\"$LDFLAGS -L/usr/lib/wlroots0.17\"\n".to_owned(),
                    //    );
                    //}
                    if cd_regex.is_match(&line) && describe_regex.is_match(&lines[index + 2]) {
                        lines.insert(
                            index + 2,
                            "  git remote add upstream https://github.com/qtile/qtile.git\n"
                                .to_owned(),
                        );
                        lines.insert(
                            index + 3,
                            "  git fetch upstream --tags --force\n".to_owned(),
                        );
                    }
                }
                let lines = lines.concat();
                match std::fs::write(self.repo_path.join("PKGBUILD"), lines) {
                    Ok(()) => {}
                    Err(err) => {
                        error_and_exit(&format!("{}\n{}", &"could not write to PKGBUILD", err))
                    }
                }
            }
            Err(err) => error_and_exit(&err.to_string()),
        }
        Ok(())
    }

    fn remove_file_or_dir_if_exists(&self, path: &str) -> anyhow::Result<()> {
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

    fn install(self) -> anyhow::Result<()> {
        log::info!("building with `makepkg`");
        match std::fs::File::create(self.repo_path.join("install.log")) {
            Ok(_) => {
                let mut f = OpenOptions::new()
                    .append(true)
                    .open(self.repo_path.join("install.log"))
                    .unwrap();
                writeln!(
                    f,
                    "\n------------------------------- building new package -------------------------------\n"
                )?;
                let exit_status = (Exec::cmd("yes")
                    | Exec::cmd("makepkg")
                        .args(&["-rsc", "--nocheck"])
                        .cwd(&self.repo_path)
                        .stderr(Redirection::Merge))
                .stdout(
                    f.try_clone()
                        .expect("no one is writing to the install log now"),
                )
                .join()?
                .success();
                match exit_status {
                    true => {
                        log::info!("removing old package");
                        writeln!(f, "\n------------------------------- removing old package -------------------------------\n")?;

                        if Exec::cmd("sudo")
                            .args(&["pacman", "-Qq", "qtile-git"])
                            .cwd(&self.repo_path)
                            .stderr(Redirection::Merge)
                            .stdout(
                                f.try_clone()
                                    .expect("no one is writing to the install log now"),
                            )
                            .join()?
                            .success()
                        {
                            // let f =
                            //     std::fs::File::create(self.repo_path.join("install.log")).unwrap();
                            // let exit_status = (Exec::cmd("yes")
                            //     | Exec::cmd("sudo")
                            //         .args(&["pacman", "-Rns", "qtile-git"])
                            //         .cwd(&self.repo_path)
                            //         .stderr(Redirection::Merge))
                            // .stdout(f)
                            // .join()?
                            // .success();
                            // match exit_status {
                            //     true => {}
                            //     false => error_and_exit(
                            //         format!(
                            //             "Qtile uninstall failed, check in {}/install.log",
                            //             &self.repo_path.to_str().unwrap()
                            //         )
                            //         .as_str(),
                            //     ),
                            // }
                        } else {
                            let to_be_deleted = [
                                "/usr/bin/qtile",
                                "/usr/lib/python3.12/site-packages/libqtile",
                                "/usr/share/doc/qtile-git",
                                "/usr/share/licenses/qtile-git/LICENSE",
                                "/usr/share/wayland-sessions/qtile-wayland.desktop",
                                "/usr/share/xsessions/qtile.desktop",
                            ];
                            for s in to_be_deleted {
                                self.remove_file_or_dir_if_exists(s)?;
                            }
                        }
                        log::info!("installing new package");
                        writeln!(f, "\n------------------------------- installing new package -------------------------------\n")?;
                        let exit_status = (Exec::cmd("yes")
                            | Exec::cmd("sudo")
                                .args(&[
                                    "pacman",
                                    "-U",
                                    glob::glob(
                                        format!(
                                            "{}/{}",
                                            self.repo_path.to_str().unwrap(),
                                            "*.tar.zst"
                                        )
                                        .as_str(),
                                    )
                                    .unwrap()
                                    .next()
                                    .unwrap()
                                    .unwrap()
                                    .to_str()
                                    .expect("package built successfully"),
                                    "--overwrite",
                                    "'*'",
                                ])
                                .cwd(&self.repo_path)
                                .stderr(Redirection::Merge))
                        .stdout(
                            f.try_clone()
                                .expect("no one is writing to the install log now"),
                        )
                        .join()?
                        .success();
                        match exit_status {
                            true => {}
                            false => log::error!(
                                "Qtile install failed, check in {}/install.log",
                                &self.repo_path.to_str().unwrap()
                            ),
                        }
                        writeln!(f, "\n------------------------------- package installed successfully -------------------------------")?;
                        if self.args.restart {
                            log::info!("restarting");
                            let response = InteractiveCommandClient::call(
                                Some(vec![]),
                                Some("restart".to_owned()),
                                Some(vec![]),
                                false,
                            );
                            match response {
                                Ok(r) => match r {
                                    serde_json::Value::Null => {}
                                    serde_json::Value::Bool(_)
                                    | serde_json::Value::Number(_)
                                    | serde_json::Value::String(_)
                                    | serde_json::Value::Array(_)
                                    | serde_json::Value::Object(_) => {
                                        error_and_exit("restart failed, please restart manually");
                                    }
                                },
                                Err(err) => error_and_exit(
                                    (err.to_string() + "\nQtile is probably not running").as_str(),
                                ),
                            }
                        } else {
                            log::info!("please restart qtile");
                        }
                    }
                    false => log::error!(
                        "Qtile build failed, check in {}/install.log",
                        &self.repo_path.to_str().unwrap()
                    ),
                }
            }
            Err(_) => todo!(),
        }
        Ok(())
    }
}
fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .env()
        .init()
        .unwrap();
    let args = Args::parse();
    let up = UpdateQtile::new(args);
    let source = up.get_source();
    match up.remove_repo() {
        Ok(()) => match up.clone_repo(source) {
            Ok(()) => match up.install() {
                Ok(()) => {}
                Err(err) => {
                    error_and_exit(&err.to_string());
                }
            },
            Err(err) => {
                error_and_exit(&err.to_string());
            }
        },
        Err(err) => {
            error_and_exit(&err.to_string());
        }
    }
}
