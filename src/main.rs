mod backend;

use clap::Parser;
use qtile_client_lib::utils::client::InteractiveCommandClient;

use backend::arch::ArchBackend;
use backend::debian::DebianBackend;
use backend::uv::UvBackend;
use backend::{error_and_exit, get_source, InstallBackend, InstallMethod};

/// Build and install qtile from source with custom fork/branch/commit/tag/PR
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
    #[arg(short = 'P', long, num_args = 1, default_value = None, group = "remote")]
    path: Option<String>,
    #[arg(short, long, num_args = 1, default_value = None, group = "identifier", conflicts_with_all = ["branch", "tag", "pull"])]
    commit: Option<String>,
    #[arg(short, long, num_args = 1, default_value = None, group = "identifier")]
    branch: Option<String>,
    #[arg(short, long, num_args = 1, default_value = None, group = "identifier")]
    tag: Option<String>,
    #[arg(short = 'p', long, default_value = None, group = "identifier")]
    pull: Option<String>,
    #[arg(short, long, default_value_t = false)]
    restart: bool,
    /// Installation method: arch, uv, debian. Auto-detected if omitted.
    #[arg(short, long, value_enum)]
    method: Option<InstallMethod>,
}

fn detect_install_method() -> InstallMethod {
    if std::path::Path::new("/etc/arch-release").exists() {
        log::info!("detected Arch Linux");
        InstallMethod::Arch
    } else if std::path::Path::new("/etc/debian_version").exists() {
        log::info!("detected Debian/Ubuntu");
        InstallMethod::Debian
    } else {
        error_and_exit(
            "could not auto-detect distro. Please specify --method (arch, uv, or debian)",
        );
    }
}

fn restart_qtile() {
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
            _ => error_and_exit("restart failed, please restart manually"),
        },
        Err(err) => error_and_exit(&format!("{err}\nQtile is probably not running")),
    }
}

fn main() {
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Info)
        .env()
        .init()
        .unwrap();

    let args = Args::parse();
    let method = args.method.clone().unwrap_or_else(detect_install_method);
    let source = get_source(
        &args.fork,
        &args.path,
        &args.branch,
        &args.commit,
        &args.tag,
        &args.pull,
    );

    let mut backend: Box<dyn InstallBackend> = match method {
        InstallMethod::Arch => Box::new(ArchBackend::new()),
        InstallMethod::Uv => Box::new(UvBackend::new()),
        InstallMethod::Debian => Box::new(DebianBackend::new()),
    };

    if let Err(err) = backend.prepare() {
        error_and_exit(&err.to_string());
    }
    if let Err(err) = backend.build(&source) {
        error_and_exit(&err.to_string());
    }
    if let Err(err) = backend.install() {
        error_and_exit(&err.to_string());
    }

    if args.restart {
        restart_qtile();
    } else {
        log::info!("please restart qtile");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn cli_defaults() {
        let args = Args::parse_from(["qup"]);
        assert_eq!(args.fork, Some("qtile".into()));
        assert!(args.method.is_none());
        assert!(!args.restart);
    }

    #[test]
    fn cli_method_arch() {
        let args = Args::parse_from(["qup", "--method", "arch"]);
        assert!(matches!(args.method, Some(InstallMethod::Arch)));
    }

    #[test]
    fn cli_method_uv() {
        let args = Args::parse_from(["qup", "--method", "uv"]);
        assert!(matches!(args.method, Some(InstallMethod::Uv)));
    }

    #[test]
    fn cli_method_debian() {
        let args = Args::parse_from(["qup", "--method", "debian"]);
        assert!(matches!(args.method, Some(InstallMethod::Debian)));
    }

    #[test]
    fn cli_fork_and_branch() {
        let args = Args::parse_from(["qup", "--fork", "user", "--branch", "dev"]);
        assert_eq!(args.fork, Some("user".into()));
        assert_eq!(args.branch, Some("dev".into()));
    }

    #[test]
    fn cli_path_flag() {
        let args = Args::parse_from(["qup", "--path", "/tmp/qtile", "--branch", "main"]);
        assert_eq!(args.path, Some("/tmp/qtile".into()));
        assert_eq!(args.branch, Some("main".into()));
    }

    #[test]
    fn cli_restart_flag() {
        let args = Args::parse_from(["qup", "--restart"]);
        assert!(args.restart);
    }
}
