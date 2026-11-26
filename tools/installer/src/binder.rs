//! Read-only Binder preflight and the scoped, opt-in startup drop-in.

use super::{output, root_install, root_path, run, Result};
use std::fs;
use std::io;
use std::path::Path;
use std::process::Command;

pub(super) const DROPIN: &str = "/etc/systemd/system/droidloomd.service.d/binder.conf";
const CONTENTS: &[u8] = include_bytes!("../../../examples/binder.conf");
const MODPROBE: &str = "/usr/sbin/modprobe";
const MODINFO: &str = "/usr/sbin/modinfo";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Action {
    BuiltIn,
    Ready,
    Install,
}

impl Action {
    pub(super) fn needs_install(self) -> bool {
        self == Self::Install
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Support {
    BuiltIn,
    Module,
}

fn kernel_support(config: &str) -> Result<Support> {
    let value = |name: &str| {
        config
            .lines()
            .find_map(|line| line.strip_prefix(name).and_then(|s| s.strip_prefix('=')))
    };
    match (
        value("CONFIG_ANDROID_BINDER_IPC"),
        value("CONFIG_ANDROID_BINDERFS"),
    ) {
        (Some("y"), Some("y")) => Ok(Support::BuiltIn),
        (Some("m"), Some("m" | "y")) => Ok(Support::Module),
        (Some("y"), Some("m")) => Err(
            "This kernel has a separate Binderfs module layout; review its distribution-specific loader configuration. The installer only manages binder_linux."
                .into(),
        ),
        _ => Err(
            "The running kernel configuration does not provide the supported Android Binder IPC and Binderfs combination. Use a supported distribution kernel; no kernel rebuild or security-policy change is attempted."
                .into(),
        ),
    }
}

fn registered(filesystems: &str) -> bool {
    // The mount type is "binder", although its Android mountpoint is /dev/binderfs.
    filesystems
        .lines()
        .any(|line| line.split_whitespace().last() == Some("binder"))
}

fn loading_disabled(value: &str) -> Result<bool> {
    match value.trim() {
        "0" => Ok(false),
        "1" => Ok(true),
        _ => Err("Cannot determine the host's kernel-module loading policy.".into()),
    }
}

fn check_dropin(existing: Option<&[u8]>) -> Result<()> {
    if existing.is_some_and(|bytes| bytes != CONTENTS) {
        return Err(format!(
            "Existing {DROPIN} differs from the project example; it will not be overwritten."
        ));
    }
    Ok(())
}

fn action(
    support: Support,
    fs_registered: bool,
    module_available: bool,
    modules_disabled: bool,
    existing: Option<&[u8]>,
) -> Result<Action> {
    check_dropin(existing)?;
    match support {
        Support::BuiltIn if fs_registered => Ok(Action::BuiltIn),
        Support::BuiltIn => Err(
            "The kernel configuration says Binderfs is built in, but /proc/filesystems does not register binder. Review the running kernel before continuing."
                .into(),
        ),
        Support::Module if !module_available => Err(
            "The stock binder_linux module is unavailable for the running kernel. Review/install its matching distribution module package; no replacement driver is downloaded or built."
                .into(),
        ),
        Support::Module if !fs_registered && modules_disabled => Err(
            "Binder is not registered and kernel module loading is disabled by host policy. Arrange for the distribution to load Binder before that policy is locked; the installer will not disable it."
                .into(),
        ),
        Support::Module if existing.is_some() => Ok(Action::Ready),
        Support::Module => Ok(Action::Install),
    }
}

fn check_modprobe_policy(config: &str) -> Result<()> {
    for line in config.lines() {
        let mut words = line.split('#').next().unwrap_or("").split_whitespace();
        let directive = words.next();
        if matches!(directive, Some("blacklist" | "install"))
            && words
                .next()
                .is_some_and(|name| name.replace('-', "_") == "binder_linux")
        {
            return Err(
                "An existing modprobe blacklist/install rule manages binder_linux. Review that host policy manually; the installer will not bypass or replace it."
                    .into(),
            );
        }
    }
    Ok(())
}

fn matching_module(release: &str, filename: &str, vermagic: &str) -> bool {
    let filename = filename.trim();
    let relative = filename
        .strip_prefix(&format!("/lib/modules/{release}/kernel/drivers/android/"))
        .or_else(|| {
            filename.strip_prefix(&format!(
                "/usr/lib/modules/{release}/kernel/drivers/android/"
            ))
        });
    matches!(
        relative,
        Some(
            "binder_linux.ko" | "binder_linux.ko.xz" | "binder_linux.ko.zst" | "binder_linux.ko.gz"
        )
    ) && vermagic.split_whitespace().next() == Some(release)
}

fn effective_precheck(value: &str) -> bool {
    value.split('}').any(|entry| {
        entry.contains("argv[]=/usr/sbin/modprobe binder_linux ;")
            && entry.contains("ignore_errors=no")
    })
}

fn verify_effective_precheck() -> Result<()> {
    let effective = output(Command::new("/usr/bin/systemctl").args([
        "show",
        "droidloomd.service",
        "-p",
        "ExecStartPre",
        "--value",
    ]))?;
    if !effective_precheck(&effective) {
        return Err(
            "The Binder precheck is not effective in droidloomd.service. Reload systemd definitions and inspect other drop-ins that reset ExecStartPre; no conflicting override will be replaced."
                .into(),
        );
    }
    Ok(())
}

/// Inspect only: no sudo, driver loading, filesystem writes, or service changes.
pub(super) fn preflight(root: &Path, runtime_installed: bool) -> Result<Action> {
    if fs::read(root.join("examples/binder.conf")).map_err(|e| e.to_string())? != CONTENTS {
        return Err("Binder example differs from the compiled startup precheck.".into());
    }
    root_path(Path::new(DROPIN))?;
    let existing = match fs::read(DROPIN) {
        Ok(bytes) => Some(bytes),
        Err(e) if e.kind() == io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{DROPIN}: {e}")),
    };
    check_dropin(existing.as_deref())?;

    let release = output(Command::new("/usr/bin/uname").arg("-r"))?;
    let release = release.trim();
    let config = fs::read_to_string(format!("/boot/config-{release}")).map_err(|e| {
        format!("The running kernel configuration is required for Binder checks: {e}")
    })?;
    let support = kernel_support(&config)?;
    let filesystems = fs::read_to_string("/proc/filesystems").map_err(|e| e.to_string())?;
    let fs_registered = registered(&filesystems);
    let mut module_available = false;
    let mut modules_disabled = false;
    if support == Support::Module || existing.is_some() {
        // Also validate an existing precheck after a switch to a built-in kernel:
        // leaving an unusable modprobe command active would break service startup.
        let policy = output(Command::new(MODPROBE).arg("--show-config"))
            .map_err(|e| format!("Cannot inspect stock module-loader policy: {e}"))?;
        check_modprobe_policy(&policy)?;
        let dependencies = output(
            Command::new(MODPROBE)
                .args(["--show-depends", "binder_linux"])
                .env("LC_ALL", "C"),
        )
        .map_err(|e| format!("Cannot resolve the stock Binder module for kernel {release}: {e}"))?;
        if dependencies
            .lines()
            .any(|line| line.trim_start().starts_with("install "))
        {
            return Err(
                "Binder resolves to a custom modprobe install command; review it manually.".into(),
            );
        }
    }
    if support == Support::Module {
        let filename = Command::new(MODINFO)
            .args(["-F", "filename", "binder_linux"])
            .output()
            .map_err(|e| format!("Cannot inspect the distribution's binder_linux module: {e}"))?;
        if filename.status.success() {
            let filename = String::from_utf8(filename.stdout).map_err(|e| e.to_string())?;
            let vermagic = output(Command::new(MODINFO).args(["-F", "vermagic", "binder_linux"]))?;
            if let Ok(actual) = fs::canonicalize(filename.trim()) {
                root_path(&actual).map_err(|e| {
                    format!("Unsafe stock Binder module ownership/permissions: {e}")
                })?;
                module_available = matching_module(release, &filename, &vermagic)
                    && matching_module(release, &actual.to_string_lossy(), &vermagic)
                    && actual.is_file();
            }
        }
        modules_disabled = loading_disabled(
            &fs::read_to_string("/proc/sys/kernel/modules_disabled").map_err(|e| e.to_string())?,
        )?;
    }
    let planned = action(
        support,
        fs_registered,
        module_available,
        modules_disabled,
        existing.as_deref(),
    )?;
    if planned == Action::Ready && runtime_installed {
        verify_effective_precheck()?;
    }
    // A newly installed runtime needs a definitions reload even if the exact
    // precheck survived an earlier package removal.
    if planned == Action::Ready && !runtime_installed {
        return Ok(Action::Install);
    }
    Ok(planned)
}

/// Called only after the main installer obtains approval and stops Droidloom.
pub(super) fn install(root: &Path) -> Result<()> {
    // Recheck immediately before installation; edited files are never replaced.
    if !preflight(root, true)?.needs_install() {
        return Ok(());
    }
    let state = output(Command::new("/usr/bin/systemctl").args([
        "show",
        "droidloomd.service",
        "-p",
        "ActiveState",
        "--value",
    ]))?;
    if !matches!(state.trim(), "inactive" | "failed") {
        return Err("Stop only Droidloom before installing its Binder precheck.".into());
    }
    if !Path::new(DROPIN).exists() {
        root_install(&root.join("examples/binder.conf"), Path::new(DROPIN), "644")?;
    }
    run(Command::new("/usr/bin/sudo").args(["--", "/usr/bin/systemctl", "daemon-reload"]))?;
    verify_effective_precheck()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODULAR: &str = "CONFIG_ANDROID_BINDER_IPC=m\nCONFIG_ANDROID_BINDERFS=m\n";
    const BUILTIN: &str = "CONFIG_ANDROID_BINDER_IPC=y\nCONFIG_ANDROID_BINDERFS=y\n";

    #[test]
    fn recognizes_builtin_and_modular_kernel_support() {
        assert_eq!(kernel_support(BUILTIN).unwrap(), Support::BuiltIn);
        assert_eq!(kernel_support(MODULAR).unwrap(), Support::Module);
        assert_eq!(
            kernel_support("CONFIG_ANDROID_BINDER_IPC=m\nCONFIG_ANDROID_BINDERFS=y\n").unwrap(),
            Support::Module,
        );
    }

    #[test]
    fn missing_or_unsupported_kernel_features_stop_before_changes() {
        for config in [
            "",
            "CONFIG_ANDROID_BINDER_IPC=m\n# CONFIG_ANDROID_BINDERFS is not set\n",
            "CONFIG_ANDROID_BINDER_IPC=y\nCONFIG_ANDROID_BINDERFS=m\n",
        ] {
            assert!(kernel_support(config).is_err());
        }
    }

    #[test]
    fn builtin_registered_driver_needs_no_module_or_dropin() {
        assert_eq!(
            action(Support::BuiltIn, true, false, true, None).unwrap(),
            Action::BuiltIn,
        );
        assert!(!Action::BuiltIn.needs_install());
    }

    #[test]
    fn builtin_config_without_registered_filesystem_is_not_accepted() {
        assert!(action(Support::BuiltIn, false, false, false, None).is_err());
    }

    #[test]
    fn loaded_module_still_needs_persistence_if_dropin_is_missing() {
        assert_eq!(
            action(Support::Module, true, true, false, None).unwrap(),
            Action::Install,
        );
        assert!(Action::Install.needs_install());
    }

    #[test]
    fn unloaded_module_can_be_prepared_without_loading_it() {
        assert_eq!(
            action(Support::Module, false, true, false, None).unwrap(),
            Action::Install,
        );
    }

    #[test]
    fn existing_exact_precheck_is_a_noop_even_before_driver_load() {
        for fs_registered in [false, true] {
            assert_eq!(
                action(Support::Module, fs_registered, true, false, Some(CONTENTS)).unwrap(),
                Action::Ready,
            );
        }
        assert!(!Action::Ready.needs_install());
    }

    #[test]
    fn missing_module_files_are_not_excused_by_a_currently_loaded_driver() {
        for fs_registered in [false, true] {
            assert!(action(Support::Module, fs_registered, false, false, None).is_err());
        }
    }

    #[test]
    fn disabled_loading_is_not_bypassed_to_recover_a_missing_driver() {
        assert!(action(Support::Module, false, true, true, None).is_err());
        assert_eq!(
            action(Support::Module, true, true, true, Some(CONTENTS)).unwrap(),
            Action::Ready,
        );
        assert!(loading_disabled("invalid").is_err());
        assert!(loading_disabled("1\n").unwrap());
        assert!(!loading_disabled("0\n").unwrap());
    }

    #[test]
    fn edited_dropins_are_not_overwritten_even_on_builtin_kernels() {
        for support in [Support::BuiltIn, Support::Module] {
            assert!(action(
                support,
                true,
                true,
                false,
                Some(b"[Service]\nExecStart=/other\n")
            )
            .is_err());
        }
    }

    #[test]
    fn filesystem_detection_matches_the_exact_mount_type() {
        assert!(registered("nodev\tbinder\n"));
        assert!(!registered("nodev\tbinder-control\nnodev\tbinderfs\n"));
    }

    #[test]
    fn respects_blacklists_and_custom_install_rules() {
        for config in [
            "blacklist binder_linux\n",
            "blacklist binder-linux\n",
            " install binder_linux /bin/false # administrator rule\n",
        ] {
            assert!(check_modprobe_policy(config).is_err());
        }
        assert!(
            check_modprobe_policy("# blacklist binder_linux\nblacklist binder_linux_extra\n")
                .is_ok()
        );
    }

    #[test]
    fn rejects_module_metadata_for_other_kernels_or_external_paths() {
        assert!(matching_module(
            "7.0-test",
            "/lib/modules/7.0-test/kernel/drivers/android/binder_linux.ko.zst\n",
            "7.0-test SMP mod_unload\n",
        ));
        assert!(!matching_module(
            "7.0-test",
            "/tmp/binder_linux.ko",
            "7.0-test SMP"
        ));
        assert!(!matching_module(
            "7.0-test",
            "/lib/modules/7.0-test/updates/dkms/binder_linux.ko",
            "7.0-test SMP"
        ));
        assert!(!matching_module(
            "7.0-test",
            "/lib/modules/7.0-test/kernel/drivers/android/binder_linux.ko",
            "6.8-old SMP",
        ));
    }

    #[test]
    fn only_accepts_an_effective_nonignored_binder_precheck() {
        let entry = "{ path=/usr/sbin/modprobe ; argv[]=/usr/sbin/modprobe binder_linux ; ignore_errors=no ; }";
        assert!(effective_precheck(entry));
        assert!(!effective_precheck(
            &entry.replace("ignore_errors=no", "ignore_errors=yes")
        ));
        assert!(!effective_precheck(
            &entry.replace("binder_linux ;", "other_module ;")
        ));
    }
}
