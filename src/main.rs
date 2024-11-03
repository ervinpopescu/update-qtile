use std::{path::Path, process::exit};

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
        let source = if let Some(p) = &self.args.path {
            format!("file://{p}")
        } else if let Some(f) = &self.args.fork {
            format!("https://github.com/{f}/qtile")
        } else {
            "https://github.com/qtile/qtile".to_owned()
        };
        if let Some(c) = &self.args.commit {
            log::info!("selected repo `{}` - commit `{}`", source, c);
            format!("{}#commit={}", source, c)
        } else if let Some(t) = &self.args.tag {
            log::info!("selected repo `{}` - tag `{}`", source, t);
            format!("{}#tag={}", source, t)
        } else if let Some(b) = &self.args.branch {
            log::info!("selected repo `{}` - branch `{}`", source, b);
            format!("{}#branch={}", source, b)
        } else {
            log::info!("selected repo `{}` - branch `master`", source);
            source
        }
    }
    fn remove_dir(&self) -> anyhow::Result<()> {
        if self.repo_path.exists() {
            log::info!("removing cached AUR repo");
            match std::fs::remove_dir_all(&self.repo_path) {
                Ok(()) => log::info!("removed {:?}", self.repo_path),
                Err(err) => {
                    log::error!("couldn't remove AUR cached repo");
                    log::error!("\tError: {err}");
                    println!("Would you like to try with root permissions? [Y/n]");
                    let ans: String = read!("{}\n");
                    log::info!("{ans}");
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
    fn clone_dir(&self) -> anyhow::Result<()> {
        log::info!("cloning AUR repo");
        let aur_url = "https://aur.archlinux.org/qtile-git";
        match git2::Repository::clone(aur_url, &self.repo_path) {
            Ok(_) => self.modify_pkgbuild()?,
            Err(err) => error_and_exit(
                ("AUR URL ".to_owned() + aur_url + " is unreachable, error: " + &err.to_string())
                    .as_str(),
            ),
        }
        Ok(())
    }

    fn modify_pkgbuild(&self) -> anyhow::Result<()> {
        log::info!("modifying PKGBUILD");
        let lines = std::fs::read_to_string(self.repo_path.join("PKGBUILD"));
        match lines {
            Ok(lines) => {
                let mut lines = lines
                    .split_inclusive('\n')
                    .map(|s| s.to_owned())
                    .collect::<Vec<String>>();
                let license = Regex::new(r"license=\(.*\)").unwrap();
                let source = Regex::new(r"source=\(.*\)").unwrap();
                let cd = Regex::new(r".*cd qtile").unwrap();
                let describe = Regex::new(r".*git describe").unwrap();
                for (index, line) in lines.clone().into_iter().enumerate() {
                    if license.is_match(&line) {
                        lines.insert(index + 1, "groups=('modified')\n".to_owned());
                    }
                    if source.is_match(&line) {
                        let source = self.get_source();
                        let inserted = format!("source=('git+{source}')\n");
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
                    if cd.is_match(&line) && describe.is_match(&lines[index + 2]) {
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
    fn install(self) -> anyhow::Result<()> {
        log::info!("installing with `makepkg`");
        match std::fs::File::create(self.repo_path.join("install.log")) {
            Ok(f) => {
                let f = f;
                let exit_status = (Exec::cmd("yes")
                    | Exec::cmd("makepkg")
                        .args(&["-r", "-i", "-s", "--nocheck"])
                        .cwd(&self.repo_path)
                        .stderr(Redirection::Merge))
                .stdout(f)
                .join()?
                .success();
                match exit_status {
                    true => {
                        log::info!("installed successfully");
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
                                        log::error!("restart failed, please restart manually");
                                        exit(1);
                                    }
                                },
                                Err(err) => error_and_exit(
                                    (err.to_string() + "\nQtile is probably not running").as_str(),
                                ),
                            }
                        }
                    }
                    false => log::error!(
                        "Qtile install failed, check in {}/install.log",
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
    match up.remove_dir() {
        Ok(()) => match up.clone_dir() {
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
