use subprocess::Exec;

use super::uv::UvBackend;
use super::{error_and_exit, InstallBackend, Source};

const DEBIAN_NATIVE_DEPS: &[&str] = &[
    "libpangocairo-1.0-0",
    "libcairo2-dev",
    "libpango1.0-dev",
    "python3-dev",
    "libffi-dev",
    "pkg-config",
];

pub struct DebianBackend {
    inner: UvBackend,
}

impl DebianBackend {
    pub fn new() -> Self {
        Self {
            inner: UvBackend::new(),
        }
    }

    fn install_native_deps() -> anyhow::Result<()> {
        log::info!("installing native dependencies via apt");
        let mut args = vec!["apt", "install", "-y"];
        args.extend(DEBIAN_NATIVE_DEPS);
        let exit_status = Exec::cmd("sudo").args(&args).join()?.success();
        if !exit_status {
            error_and_exit("failed to install native dependencies via apt");
        }
        Ok(())
    }
}

impl InstallBackend for DebianBackend {
    fn prepare(&mut self) -> anyhow::Result<()> {
        Self::install_native_deps()?;
        self.inner.prepare()
    }

    fn build(&mut self, source: &Source) -> anyhow::Result<()> {
        self.inner.build(source)
    }

    fn install(&mut self) -> anyhow::Result<()> {
        self.inner.install()
    }
}
