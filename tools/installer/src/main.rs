//! Explicitly scoped installer; never run this driver as root.

use serde_json::{json, Value};
use std::env;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

type Result<T> = std::result::Result<T, String>;
const KERNEL_PREFIX: &str = "/usr/local/lib/droidloom-ubuntu-kernel-config";
const KERNEL_DROPIN: &str = "/etc/systemd/system/droidloomd.service.d/ubuntu-kernel-config.conf";
const PRIVATE_UNSHARE: &str = "/opt/droidloom/bin/unshare";
const UNSHARE_DROPIN: &str = "/etc/systemd/system/droidloomd.service.d/ubuntu-unshare.conf";
const PRIVATE_PATH: &str =
    "/opt/droidloom/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
const TOOLCHAIN: &str = "1.93.0";
const TITLEBAR_MARKER: &str = "droidloom-deb-titlebar-v1";

#[derive(Debug, Default, PartialEq)]
struct Options {
    check: bool,
    yes: bool,
    no_start: bool,
    runtime_deb: Option<PathBuf>,
    sysroot: Option<PathBuf>,
    archive: Option<PathBuf>,
}

impl Options {
    fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self> {
        let mut options = Self::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.to_str() {
                Some("--check") => options.check = true,
                Some("--yes") => options.yes = true,
                Some("--no-start") => options.no_start = true,
                Some("--runtime-deb" | "--build-sysroot" | "--util-linux-archive") => {
                    let value = args
                        .next()
                        .ok_or_else(|| format!("{} needs a path", arg.to_string_lossy()))?;
                    if value.is_empty() || value.to_string_lossy().starts_with("--") {
                        return Err(format!("{} needs a path", arg.to_string_lossy()));
                    }
                    let value = absolute(&PathBuf::from(value))?;
                    let slot = match arg.to_str().unwrap() {
                        "--runtime-deb" => &mut options.runtime_deb,
                        "--build-sysroot" => &mut options.sysroot,
                        _ => &mut options.archive,
                    };
                    if slot.replace(value).is_some() {
                        return Err(format!("Duplicate option: {}", arg.to_string_lossy()));
                    }
                }
                _ => {
                    return Err(format!(
                        "Unknown option: {}. See ./install.sh --help.",
                        arg.to_string_lossy()
                    ))
                }
            }
        }
        Ok(options)
    }
}

#[derive(Clone, Debug)]
struct Pinned {
    commit: String,
    repository: String,
    version: String,
    presenter: String,
    legacy_presenter: String,
    patch_sha: String,
    deb_sha: String,
    patch: PathBuf,
    archive_url: String,
    archive_sha: String,
}

impl Pinned {
    fn read(root: &Path) -> Result<Self> {
        let value = read_json(&root.join("upstream.json"))?;
        let pin = Self {
            commit: field(&value, "commit")?.to_owned(),
            repository: field(&value, "repository")?.to_owned(),
            version: field(&value["local_runtime_package"], "version")?.to_owned(),
            presenter: field(&value, "packaged_presenter_sha256")?.to_owned(),
            legacy_presenter: field(&value, "original_custom_presenter_sha256")?.to_owned(),
            patch_sha: field(&value, "titlebar_patch_sha256")?.to_owned(),
            deb_sha: field(&value["local_runtime_package"], "sha256")?.to_owned(),
            patch: root.join(
                value["patches"][0]
                    .as_str()
                    .ok_or("Missing title-bar patch in upstream.json")?,
            ),
            archive_url: field(&value["private_unshare"], "source_url")?.to_owned(),
            archive_sha: field(&value["private_unshare"], "sha256")?.to_owned(),
        };
        for hash in [
            &pin.presenter,
            &pin.legacy_presenter,
            &pin.patch_sha,
            &pin.deb_sha,
            &pin.archive_sha,
        ] {
            if !is_hash(hash, 64) {
                return Err("Invalid SHA256 in upstream.json".into());
            }
        }
        if !is_hash(&pin.commit, 40)
            || pin.repository != "https://github.com/denialwm/droidloom.git"
            || pin.archive_url
                != "https://mirrors.edge.kernel.org/pub/linux/utils/util-linux/v2.42/util-linux-2.42.tar.xz"
            || value["local_runtime_package"]["name"] != "droidloom"
            || value["local_runtime_package"]["architecture"] != "amd64"
        {
            return Err("Unreviewed upstream input; review installer pins before proceeding".into());
        }
        if sha256(&pin.patch)? != pin.patch_sha {
            return Err("Title-bar patch hash disagrees with upstream.json.".into());
        }
        Ok(pin)
    }
}

fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value[name]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("Missing string field: {name}"))
}

fn read_json(path: &Path) -> Result<Value> {
    serde_json::from_slice(&fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?)
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn absolute(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir().map_err(|e| e.to_string())?.join(path))
    }
}

fn is_hash(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

fn output(command: &mut Command) -> Result<String> {
    let result = command.output().map_err(|e| format!("{command:?}: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "{command:?} failed:\n{}{}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        ));
    }
    String::from_utf8(result.stdout).map_err(|e| e.to_string())
}

fn run(command: &mut Command) -> Result<()> {
    println!("> {command:?}");
    let status = command.status().map_err(|e| format!("{command:?}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{command:?} failed with {status}"))
    }
}

fn sha256(path: &Path) -> Result<String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = output(
        Command::new("/usr/bin/sha256sum")
            .stdin(Stdio::from(file))
            .arg("-"),
    )?;
    let hash = text.split_whitespace().next().unwrap_or("");
    if !is_hash(hash, 64) {
        return Err(format!("Could not hash {}", path.display()));
    }
    Ok(hash.to_owned())
}

fn required(program: &str) -> Result<()> {
    let found = env::var_os("PATH")
        .map(|path| env::split_paths(&path).any(|dir| dir.join(program).is_file()))
        .unwrap_or(false);
    if found {
        Ok(())
    } else {
        Err(format!(
            "Missing command: {program}. See docs/install.md prerequisites."
        ))
    }
}

fn root_path(path: &Path) -> Result<()> {
    for part in path.ancestors() {
        match fs::symlink_metadata(part) {
            Ok(meta)
                if meta.file_type().is_symlink() || meta.uid() != 0 || meta.mode() & 0o022 != 0 =>
            {
                return Err(format!("Unsafe privileged output path: {}", part.display()));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", part.display())),
        }
    }
    Ok(())
}

fn user_path(path: &Path, uid: u32) -> Result<()> {
    for part in path.ancestors() {
        match fs::symlink_metadata(part) {
            Ok(meta)
                if meta.file_type().is_symlink()
                    || !user_metadata_allowed(meta.uid(), meta.mode(), uid) =>
            {
                return Err(format!("Unsafe user installation path: {}", part.display()));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(format!("{}: {e}", part.display())),
        }
    }
    Ok(())
}

fn user_metadata_allowed(owner: u32, mode: u32, uid: u32) -> bool {
    // Per-user paths may deliberately be group-writable (e.g. ~/.local/lib).
    // We never chmod those existing directories or use them for root outputs.
    // Privileged paths use the separate, stricter root_path check above.
    (owner == 0 || owner == uid) && mode & 0o002 == 0
}

fn writable_parent(path: &Path) -> Result<()> {
    let parent = path
        .ancestors()
        .skip(1)
        .find(|parent| parent.exists())
        .ok_or("No existing user installation parent")?;
    if !parent.is_dir() {
        return Err(format!(
            "Installation parent is not a directory: {}",
            parent.display()
        ));
    }
    for flag in ["-w", "-x"] {
        if !Command::new("/usr/bin/test")
            .arg(flag)
            .arg(parent)
            .status()
            .map_err(|e| e.to_string())?
            .success()
        {
            return Err(format!(
                "User installation parent is not writable/searchable: {}",
                parent.display()
            ));
        }
    }
    Ok(())
}

fn version_at_least_242(text: &str) -> bool {
    let Some(version) = text.trim().strip_prefix("unshare from util-linux ") else {
        return false;
    };
    let mut numbers = version.split_whitespace().next().unwrap_or("").split('.');
    let major = numbers.next().and_then(|n| n.parse::<u32>().ok());
    let minor = numbers.next().and_then(|n| n.parse::<u32>().ok());
    matches!((major, minor), (Some(a), Some(b)) if (a, b) >= (2, 42))
}

fn active_unit_lines(text: &str, home: &Path) -> Vec<String> {
    text.replace(home.to_string_lossy().as_ref(), "%h")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

fn unit_matches(actual: &str, expected: &str, home: &Path) -> bool {
    active_unit_lines(actual, home) == active_unit_lines(expected, home)
}

fn environment_path(text: &str) -> Option<&str> {
    text.split_whitespace()
        .find_map(|word| word.trim_matches('"').strip_prefix("PATH="))
}

#[derive(Debug)]
struct Plan {
    install_runtime: bool,
    setup: bool,
    kernel: bool,
    unshare: bool,
    titlebar: bool,
}

impl Plan {
    fn changed(&self) -> bool {
        self.install_runtime || self.setup || self.kernel || self.unshare || self.titlebar
    }
}

struct Installer {
    root: PathBuf,
    home: PathBuf,
    uid: u32,
    pinned: Pinned,
    options: Options,
}

impl Installer {
    fn preflight(&self) -> Result<Plan> {
        if self.uid == 0 {
            return Err("Run as your ordinary desktop user, not root/sudo.".into());
        }
        if env::consts::OS != "linux" || env::consts::ARCH != "x86_64" {
            return Err("Only Linux x86_64 is supported.".into());
        }
        let release = fs::read_to_string("/etc/os-release").map_err(|e| e.to_string())?;
        if !release.lines().any(|s| s == "ID=ubuntu")
            || !release.lines().any(|s| s == "VERSION_ID=\"26.04\"")
        {
            return Err("The automated installer is currently limited to Ubuntu 26.04 x86_64; other distributions need separate validation.".into());
        }
        if env::var("XDG_SESSION_TYPE").as_deref() != Ok("wayland")
            || env::var_os("WAYLAND_DISPLAY").is_none()
        {
            return Err("Run inside your ordinary user's Wayland desktop session.".into());
        }
        for program in [
            "python3",
            "git",
            "cargo",
            "dpkg-query",
            "dpkg-deb",
            "systemctl",
        ] {
            required(program)?;
        }
        output(Command::new("/usr/bin/python3").args([
            "-I",
            "-c",
            "import sys; assert sys.version_info >= (3, 10), 'Python 3.10 or newer is required'",
        ]))?;
        output(Command::new("/usr/bin/systemctl").args(["--user", "show-environment"]))?;
        let guard = fs::read_to_string(self.root.join("examples/packaged-presenter.sha256"))
            .map_err(|e| e.to_string())?;
        if guard != format!("{}  /usr/bin/droidloom-wayland\n", self.pinned.presenter) {
            return Err("Example hash guard and upstream.json disagree.".into());
        }
        let installed = Command::new("/usr/bin/dpkg-query")
            .args([
                "-W",
                "-f=${db:Status-Status}\\n${Version}\\n${Architecture}\\n",
                "droidloom",
            ])
            .output()
            .map_err(|e| e.to_string())?;
        let installed_text = String::from_utf8_lossy(&installed.stdout);
        let install_runtime =
            !installed.status.success() || !installed_text.starts_with("installed\n");
        if install_runtime {
            let deb = self.options.runtime_deb.as_ref().ok_or(
                "Droidloom is not installed. Supply the trusted, supported local .deb with --runtime-deb FILE; this repo does not ship/download a base runtime."
            )?;
            if sha256(deb)? != self.pinned.deb_sha {
                return Err(
                    "Runtime .deb SHA256 does not match the reviewed local package; refusing it."
                        .into(),
                );
            }
            let metadata = output(
                Command::new("/usr/bin/dpkg-deb")
                    .args(["-f"])
                    .arg(deb)
                    .args(["Package", "Version", "Architecture"]),
            )?;
            for expected in [
                "Package: droidloom",
                &format!("Version: {}", self.pinned.version),
                "Architecture: amd64",
            ] {
                if !metadata.lines().any(|s| s == expected) {
                    return Err("Runtime .deb has unexpected package metadata.".into());
                }
            }
            required("apt-get")?;
        } else {
            let lines: Vec<_> = installed_text.lines().collect();
            if lines.get(1).copied() != Some(self.pinned.version.as_str())
                || lines.get(2).copied() != Some("amd64")
            {
                return Err(format!("Installed Droidloom is not the pinned {} amd64 package; no upgrade/downgrade will be attempted.", self.pinned.version));
            }
            if sha256(Path::new("/usr/bin/droidloom-wayland"))? != self.pinned.presenter {
                return Err(
                    "Packaged presenter hash mismatch; refusing an unmatched runtime.".into(),
                );
            }
        }
        let setup = !Path::new("/etc/droidloom/cell.json").exists();
        if setup {
            // Setup must never guess how to adopt or replace pre-existing storage.
            let data = PathBuf::from(format!("/var/lib/droidloom/users/{}/data", self.uid));
            if data.exists() {
                return Err(format!("cell.json is missing but {} exists. Review existing storage manually; no adoption/deletion is attempted.", data.display()));
            }
        }
        let kernel = if setup { true } else { self.kernel_needed()? };
        let unshare = self.unshare_needed(install_runtime)?;
        let titlebar = self.titlebar_needed()?;
        if kernel {
            required("dump.erofs")?;
            let kernel = output(Command::new("/usr/bin/uname").arg("-r"))?;
            let config = fs::read(format!("/boot/config-{}", kernel.trim()))
                .map_err(|e| format!("The real running kernel configuration is required: {e}"))?;
            if !config.split(|b| *b == b'\n').any(|line| {
                line == b"CONFIG_VMAP_STACK=y" || line == b"# CONFIG_VMAP_STACK is not set"
            }) {
                return Err(
                    "Running kernel configuration does not specify CONFIG_VMAP_STACK.".into(),
                );
            }
        }
        if titlebar {
            for command in ["cc", "pkg-config", "rustup"] {
                required(command)?;
            }
            let mut command = Command::new("pkg-config");
            command.args([
                "--exists",
                "libdrm",
                "gbm",
                "wayland-client",
                "xkbcommon",
                "libffi",
            ]);
            self.build_environment(&mut command);
            if !command.status().map_err(|e| e.to_string())?.success() {
                return Err("Missing presenter development libraries. Provide --build-sysroot DIR or prepare the build dependencies documented in docs/install.md. No apt changes were made.".into());
            }
        }
        if unshare {
            for command in ["cc", "make", "pkg-config", "tar"] {
                required(command)?;
            }
            if let Some(archive) = &self.options.archive {
                if sha256(archive)? != self.pinned.archive_sha {
                    return Err("util-linux source archive SHA256 mismatch.".into());
                }
            } else {
                required("curl")?;
            }
        }
        Ok(Plan {
            install_runtime,
            setup,
            kernel,
            unshare,
            titlebar,
        })
    }

    fn kernel_needed(&self) -> Result<bool> {
        let spec_path = Path::new("/etc/droidloom/cell.json");
        root_path(spec_path)?;
        let spec = read_json(spec_path)?;
        if spec["host_uid"].as_u64() != Some(self.uid as u64) {
            return Err("cell.json belongs to another desktop UID; refusing to change it.".into());
        }
        let checked = output(
            Command::new("/usr/bin/python3")
                .arg(self.root.join("ubuntu_kernel_config.py"))
                .arg("--check"),
        )?;
        let prefix = Path::new(KERNEL_PREFIX);
        if !prefix.exists() {
            if spec.to_string().contains(KERNEL_PREFIX) || Path::new(KERNEL_DROPIN).exists() {
                return Err(
                    "Partial kernel workaround without its manifest; review it manually.".into(),
                );
            }
            return Ok(true);
        }
        root_path(&prefix.join("manifest.json"))?;
        let state = read_json(&prefix.join("manifest.json"))?;
        if state["marker"] != "droidloom-ubuntu-kernel-config-v1" {
            return Err(
                "Unrecognized kernel workaround directory; it will not be overwritten.".into(),
            );
        }
        for (name, key) in [
            ("libandroid_runtime.so", "patched_library_sha256"),
            ("kernel/gz", "compressed_config_sha256"),
            ("helper.py", "helper_sha256"),
        ] {
            root_path(&prefix.join(name))?;
            if sha256(&prefix.join(name))? != field(&state, key)? {
                return Err(format!(
                    "Edited kernel workaround file: {name}. Nothing will overwrite it."
                ));
            }
        }
        let expected_dropin = format!(
            "# Droidloom-only Ubuntu kernel-config workaround.\n[Service]\nExecStartPre=/usr/bin/python3 {KERNEL_PREFIX}/helper.py --refresh\n"
        );
        if Path::new(KERNEL_DROPIN).exists() {
            root_path(Path::new(KERNEL_DROPIN))?;
            if fs::read_to_string(KERNEL_DROPIN).map_err(|e| e.to_string())? != expected_dropin {
                return Err(
                    "Kernel workaround drop-in was edited; refusing to overwrite it.".into(),
                );
            }
        }
        if state["enabled"] != true {
            return Ok(true);
        }
        for (key, target, source) in [
            (
                "android_file_overrides",
                "/system/lib64/libandroid_runtime.so",
                format!("{KERNEL_PREFIX}/libandroid_runtime.so"),
            ),
            (
                "android_runtime_directories",
                "/droidloom/k",
                format!("{KERNEL_PREFIX}/kernel"),
            ),
        ] {
            let expected = json!({"source": source, "target": target});
            if !spec[key]
                .as_array()
                .is_some_and(|entries| entries.contains(&expected))
            {
                return Err(
                    "Enabled kernel workaround has missing configuration entries; review manually."
                        .into(),
                );
            }
        }
        let current_hash = checked
            .lines()
            .find_map(|line| line.strip_prefix("Patched library SHA256:").map(str::trim))
            .ok_or("Kernel helper did not report its validated library hash")?;
        let kernel = output(Command::new("/usr/bin/uname").arg("-r"))?;
        let kernel_path = PathBuf::from(format!("/boot/config-{}", kernel.trim()));
        Ok(!Path::new(KERNEL_DROPIN).exists()
            || current_hash != field(&state, "patched_library_sha256")?
            || state["kernel"].as_str() != Some(kernel.trim())
            || sha256(&kernel_path)? != field(&state, "kernel_config_sha256")?
            || sha256(&self.root.join("ubuntu_kernel_config.py"))?
                != field(&state, "helper_sha256")?)
    }

    fn unshare_needed(&self, runtime_missing: bool) -> Result<bool> {
        let environment = if runtime_missing {
            String::new()
        } else {
            output(Command::new("/usr/bin/systemctl").args([
                "show",
                "droidloomd.service",
                "-p",
                "Environment",
                "--value",
            ]))?
        };
        if environment_path(&environment)
            .is_some_and(|path| path.split(':').next() == Some("/opt/droidloom/bin"))
        {
            root_path(Path::new(PRIVATE_UNSHARE))?;
            let version = output(
                Command::new(PRIVATE_UNSHARE)
                    .arg("--version")
                    .env("LC_ALL", "C"),
            )?;
            if !version_at_least_242(&version) {
                return Err(
                    "Existing Droidloom-private unshare is too old; refusing to overwrite it."
                        .into(),
                );
            }
            return Ok(false);
        }
        if environment_path(&environment).is_some() {
            return Err("Droidloom has a custom PATH without the private unshare. Review it manually; no existing override is replaced.".into());
        }
        let version = output(
            Command::new("/usr/bin/unshare")
                .arg("--version")
                .env("LC_ALL", "C"),
        )?;
        if version_at_least_242(&version) {
            return Ok(false);
        }
        root_path(Path::new(PRIVATE_UNSHARE))?;
        root_path(Path::new(UNSHARE_DROPIN))?;
        if Path::new(PRIVATE_UNSHARE).exists() || Path::new(UNSHARE_DROPIN).exists() {
            return Err(
                "An inactive private-unshare installation already exists; review it manually."
                    .into(),
            );
        }
        Ok(true)
    }

    fn titlebar_needed(&self) -> Result<bool> {
        let prefix = self.home.join(".local/lib/droidloom-titlebar");
        let dropin = self
            .home
            .join(".config/systemd/user/droidloom.service.d/titlebar.conf");
        user_path(&prefix, self.uid)?;
        user_path(&dropin, self.uid)?;
        if !prefix.exists() && !dropin.exists() {
            writable_parent(&prefix)?;
            writable_parent(&dropin)?;
            return Ok(true);
        }
        let unit = fs::read_to_string(&dropin)
            .map_err(|_| "Existing/incomplete title-bar installation; review it manually.")?;
        let expected = fs::read_to_string(self.root.join("examples/titlebar.conf"))
            .map_err(|e| e.to_string())?;
        if !unit_matches(&unit, &expected, &self.home) {
            return Err("Existing titlebar.conf is not ours; it will not be overwritten.".into());
        }
        let guard =
            fs::read(prefix.join("packaged-presenter.sha256")).map_err(|e| e.to_string())?;
        if guard
            != fs::read(self.root.join("examples/packaged-presenter.sha256"))
                .map_err(|e| e.to_string())?
        {
            return Err("Existing title-bar hash guard differs; refusing to overwrite it.".into());
        }
        let binary_sha = sha256(&prefix.join("droidloom-wayland"))?;
        let state_path = prefix.join("installer-state.json");
        if state_path.exists() {
            let state = read_json(&state_path)?;
            if state["marker"] != TITLEBAR_MARKER
                || state["upstream_commit"].as_str() != Some(self.pinned.commit.as_str())
                || state["patch_sha256"].as_str() != Some(self.pinned.patch_sha.as_str())
                || state["binary_sha256"].as_str() != Some(binary_sha.as_str())
            {
                return Err(
                    "Title-bar manifest or binary changed; refusing to overwrite it.".into(),
                );
            }
        } else if binary_sha != self.pinned.legacy_presenter {
            return Err("A custom title-bar binary without our manifest already exists. It will not be replaced automatically.".into());
        }
        Ok(false)
    }

    fn build_environment(&self, command: &mut Command) {
        if let Some(sysroot) = &self.options.sysroot {
            command.env("PKG_CONFIG_SYSROOT_DIR", sysroot);
            command.env(
                "PKG_CONFIG_PATH",
                sysroot.join("usr/lib/x86_64-linux-gnu/pkgconfig"),
            );
        }
    }

    fn print_plan(&self, plan: &Plan) {
        println!("\nDroidloom-deb installation plan:");
        for (description, needed) in [
            (
                "Install the explicitly supplied, SHA256-pinned local runtime .deb",
                plan.install_runtime,
            ),
            (
                "Run upstream first-time setup for this desktop UID",
                plan.setup,
            ),
            (
                "Build/install private unshare 2.42 and its Droidloom-only PATH override",
                plan.unshare,
            ),
            (
                "Apply/refresh the isolated kernel-config workaround",
                plan.kernel,
            ),
            (
                "Build/test/install the pinned per-user title-bar presenter",
                plan.titlebar,
            ),
        ] {
            println!(
                "  [{}] {description}",
                if needed { "apply" } else { "skip: verified" }
            );
        }
        if plan.changed() {
            println!("  Only Droidloom will be stopped during activation.");
            println!("  No host-library replacements, global sysctl/AppArmor changes, or Android-data resets.");
            if plan.install_runtime {
                println!("  apt will install the supplied package and its required dependencies, with its own confirmation.");
            }
            println!(
                "  Final state: {}",
                if self.options.no_start {
                    "Droidloom stopped"
                } else {
                    "start Droidloom and wait for Android readiness"
                }
            );
        } else {
            println!(
                "  Everything is already verified; no install, rebuild, or restart is needed."
            );
        }
    }

    fn install(&self, plan: &Plan) -> Result<()> {
        if self.options.check || !plan.changed() {
            return Ok(());
        }
        if !self.options.yes {
            print!("Proceed with this plan? [y/N] ");
            io::stdout().flush().map_err(|e| e.to_string())?;
            let mut line = String::new();
            io::stdin()
                .read_line(&mut line)
                .map_err(|e| e.to_string())?;
            if !matches!(line.trim(), "y" | "Y" | "yes" | "YES") {
                return Err("Cancelled; no installation or service changes were made.".into());
            }
        }
        let work = self.root.join(".work");
        user_path(&work, self.uid)?;
        fs::create_dir_all(&work).map_err(|e| e.to_string())?;
        let _lock = InstallLock::acquire(&work.join("install.lock"))?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let build = work.join(format!("install-build-{}-{stamp}", std::process::id()));
        fs::create_dir(&build).map_err(|e| e.to_string())?;
        // Expensive/fallible source checks, compilation, and tests precede sudo
        // and any stop, package installation, or runtime configuration change.
        let presenter = if plan.titlebar {
            Some(self.build_presenter(&build)?)
        } else {
            None
        };
        let unshare = if plan.unshare {
            Some(self.build_unshare(&build)?)
        } else {
            None
        };
        required("sudo")?;
        run(Command::new("/usr/bin/sudo").arg("-v"))?;
        // Recheck after the potentially long build; never act on stale state.
        let fresh = self.preflight()?;
        if (
            fresh.install_runtime,
            fresh.setup,
            fresh.kernel,
            fresh.unshare,
            fresh.titlebar,
        ) != (
            plan.install_runtime,
            plan.setup,
            plan.kernel,
            plan.unshare,
            plan.titlebar,
        ) {
            return Err("Installation state changed during the build. Re-run the installer; no services were stopped.".into());
        }
        if !plan.install_runtime {
            run(Command::new("/usr/bin/droidloomctl").arg("stop"))?;
        }
        if plan.install_runtime {
            // No -y, dist-upgrade, -f, or removal permission. apt owns dependency resolution.
            run(Command::new("/usr/bin/sudo")
                .args(["--", "/usr/bin/apt-get", "install", "--no-remove"])
                .arg(self.options.runtime_deb.as_ref().unwrap()))?;
            if sha256(Path::new("/usr/bin/droidloom-wayland"))? != self.pinned.presenter {
                return Err(
                    "Newly installed presenter does not match the pin; Droidloom was not started."
                        .into(),
                );
            }
        }
        if plan.setup {
            let data_home = env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| self.home.join(".local/share"));
            let state_home = env::var_os("XDG_STATE_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| self.home.join(".local/state"));
            run(Command::new("/usr/bin/sudo")
                .args([
                    "--",
                    "/usr/lib/droidloom/droidloom-package-helper",
                    "setup",
                    "--uid",
                ])
                .arg(self.uid.to_string())
                .arg("--data-home")
                .arg(data_home)
                .arg("--state-home")
                .arg(state_home))?;
            run(Command::new("/usr/bin/droidloomctl").arg("stop"))?;
            self.kernel_needed()?;
        }
        if let Some(unshare) = unshare {
            root_install(&unshare, Path::new(PRIVATE_UNSHARE), "755")?;
            root_install(
                &self.root.join("examples/ubuntu-unshare.conf"),
                Path::new(UNSHARE_DROPIN),
                "644",
            )?;
            run(Command::new("/usr/bin/sudo").args(["--", "/usr/bin/systemctl", "daemon-reload"]))?;
            let environment = output(Command::new("/usr/bin/systemctl").args([
                "show",
                "droidloomd.service",
                "-p",
                "Environment",
                "--value",
            ]))?;
            if environment_path(&environment) != Some(PRIVATE_PATH) {
                return Err("Another unit override takes precedence over the private PATH. Droidloom remains stopped; review the drop-ins.".into());
            }
        }
        if plan.kernel {
            run(Command::new("/usr/bin/sudo")
                .args(["--", "/usr/bin/python3"])
                .arg(self.root.join("ubuntu_kernel_config.py"))
                .arg("--apply"))?;
        }
        if let Some(presenter) = presenter {
            self.install_presenter(&presenter)?;
        }
        run(Command::new("/usr/bin/systemctl").args(["--user", "daemon-reload"]))?;
        if !self.options.no_start {
            if let Err(error) = run(Command::new("/usr/bin/droidloomctl").arg("start")) {
                let stopped = run(Command::new("/usr/bin/droidloomctl").arg("stop"));
                return Err(format!("{error}\nReadiness failed; stop attempt: {stopped:?}. Inspect the Droidloom logs before retrying."));
            }
        }
        println!("\nCompatibility installation completed. Original Android images and packaged presenter were not patched in place.");
        Ok(())
    }

    fn build_presenter(&self, build: &Path) -> Result<PathBuf> {
        let available = Command::new("rustup")
            .args(["run", TOOLCHAIN, "rustc", "--version"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|e| e.to_string())?
            .success();
        if !available {
            run(Command::new("rustup").args([
                "toolchain",
                "install",
                TOOLCHAIN,
                "--profile",
                "minimal",
            ]))?;
        }
        let source = build.join("droidloom");
        fs::create_dir(&source).map_err(|e| e.to_string())?;
        run(Command::new("git").arg("init").arg(&source))?;
        run(Command::new("git")
            .arg("-C")
            .arg(&source)
            .args(["remote", "add", "origin"])
            .arg(&self.pinned.repository))?;
        run(Command::new("git")
            .arg("-C")
            .arg(&source)
            .args(["fetch", "--depth", "1", "origin"])
            .arg(&self.pinned.commit))?;
        run(Command::new("git")
            .arg("-C")
            .arg(&source)
            .args(["switch", "--detach", "FETCH_HEAD"]))?;
        let actual = output(
            Command::new("git")
                .arg("-C")
                .arg(&source)
                .args(["rev-parse", "HEAD"]),
        )?;
        if actual.trim() != self.pinned.commit {
            return Err("Fetched commit does not match pin.".into());
        }
        run(Command::new("git")
            .arg("-C")
            .arg(&source)
            .args(["apply", "--check", "--whitespace=error"])
            .arg(&self.pinned.patch))?;
        run(Command::new("git")
            .arg("-C")
            .arg(&source)
            .args(["apply", "--whitespace=error"])
            .arg(&self.pinned.patch))?;
        for operation in ["test", "build"] {
            let mut command = Command::new("/usr/bin/nice");
            command.args([
                "-n",
                "10",
                "cargo",
                operation,
                "--locked",
                "-j",
                "1",
                "-p",
                "droidloom-wayland",
            ]);
            if operation == "build" {
                command.arg("--release");
            }
            command
                .current_dir(&source)
                .env("RUSTUP_TOOLCHAIN", TOOLCHAIN)
                .env("CARGO_TARGET_DIR", source.join("target"))
                .env("CARGO_BUILD_JOBS", "1");
            self.build_environment(&mut command);
            run(&mut command)?;
        }
        Ok(source.join("target/release/droidloom-wayland"))
    }

    fn build_unshare(&self, build: &Path) -> Result<PathBuf> {
        let archive = if let Some(archive) = &self.options.archive {
            archive.clone()
        } else {
            let archive = build.join("util-linux-2.42.tar.xz");
            run(Command::new("curl")
                .args([
                    "--fail",
                    "--location",
                    "--proto",
                    "=https",
                    "--tlsv1.2",
                    "--output",
                ])
                .arg(&archive)
                .arg(&self.pinned.archive_url))?;
            archive
        };
        if sha256(&archive)? != self.pinned.archive_sha {
            return Err(
                "util-linux archive failed SHA256 verification; it was not extracted.".into(),
            );
        }
        let source = build.join("util-linux");
        fs::create_dir(&source).map_err(|e| e.to_string())?;
        run(Command::new("tar")
            .args(["--extract", "--file"])
            .arg(&archive)
            .args(["--strip-components=1", "--no-same-owner", "--directory"])
            .arg(&source))?;
        run(Command::new("./configure").current_dir(&source).args([
            "--disable-all-programs",
            "--enable-unshare",
            "--disable-nls",
        ]))?;
        run(Command::new("/usr/bin/nice")
            .current_dir(&source)
            .args(["-n", "10", "make", "-j1", "unshare"]))?;
        let binary = source.join("unshare");
        let version = output(Command::new(&binary).arg("--version").env("LC_ALL", "C"))?;
        if !version_at_least_242(&version) {
            return Err("Built unshare has an unexpected version.".into());
        }
        Ok(binary)
    }

    fn install_presenter(&self, binary: &Path) -> Result<()> {
        let prefix = self.home.join(".local/lib/droidloom-titlebar");
        let dropin = self
            .home
            .join(".config/systemd/user/droidloom.service.d/titlebar.conf");
        user_path(&prefix, self.uid)?;
        user_path(&dropin, self.uid)?;
        // Check again immediately before exclusive writes. Never replace a
        // different presenter or a manually edited service override.
        if prefix.exists() || dropin.exists() {
            return Err(
                "A title-bar installation appeared during the build; refusing to overwrite it."
                    .into(),
            );
        }
        fs::create_dir_all(&prefix).map_err(|e| e.to_string())?;
        copy_new(binary, &prefix.join("droidloom-wayland"), 0o755)?;
        copy_new(
            &self.root.join("examples/packaged-presenter.sha256"),
            &prefix.join("packaged-presenter.sha256"),
            0o644,
        )?;
        let state = json!({
            "marker": TITLEBAR_MARKER,
            "upstream_commit": self.pinned.commit,
            "patch_sha256": self.pinned.patch_sha,
            "binary_sha256": sha256(binary)?,
        });
        write_new(
            &prefix.join("installer-state.json"),
            format!(
                "{}\n",
                serde_json::to_string_pretty(&state).map_err(|e| e.to_string())?
            )
            .as_bytes(),
            0o644,
        )?;
        fs::create_dir_all(dropin.parent().unwrap()).map_err(|e| e.to_string())?;
        copy_new(&self.root.join("examples/titlebar.conf"), &dropin, 0o644)?;
        Ok(())
    }
}

fn root_install(source: &Path, target: &Path, mode: &str) -> Result<()> {
    root_path(target)?;
    if fs::symlink_metadata(target).is_ok() {
        return Err(format!(
            "Refusing to overwrite existing privileged file: {}",
            target.display()
        ));
    }
    let parent = target.parent().ok_or("Missing installation parent")?;
    if !parent.exists() {
        run(Command::new("/usr/bin/sudo")
            .args(["--", "/usr/bin/install", "-d", "-m", "755"])
            .arg(parent))?;
    }
    run(Command::new("/usr/bin/sudo")
        .args(["--", "/usr/bin/install", "-m", mode])
        .arg(source)
        .arg(target))
}

fn write_new(path: &Path, contents: &[u8], mode: u32) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|e| e.to_string())?;
    file.write_all(contents).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}

fn copy_new(source: &Path, target: &Path, mode: u32) -> Result<()> {
    let mut data = Vec::new();
    File::open(source)
        .map_err(|e| e.to_string())?
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    write_new(target, &data, mode)
}

struct InstallLock(PathBuf);

impl InstallLock {
    fn acquire(path: &Path) -> Result<Self> {
        write_new(path, format!("{}\n", std::process::id()).as_bytes(), 0o600)
            .map_err(|e| format!("Another installer may be running, or a stale lock exists: {e}. Review it; do not remove a live installer's lock."))?;
        Ok(Self(path.to_owned()))
    }
}

impl Drop for InstallLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn main() {
    let result = (|| -> Result<()> {
        let options = Options::parse(env::args_os().skip(1))?;
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let home = PathBuf::from(env::var_os("HOME").ok_or("HOME is not set")?)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let uid = output(Command::new("/usr/bin/id").arg("-u"))?
            .trim()
            .parse::<u32>()
            .map_err(|e| e.to_string())?;
        let pinned = Pinned::read(&root)?;
        let installer = Installer {
            root,
            home,
            uid,
            pinned,
            options,
        };
        let plan = installer.preflight()?;
        installer.print_plan(&plan);
        if installer.options.check {
            println!("\nPreflight passed. No packages, configuration, or services were changed.");
        } else {
            installer.install(&plan)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("\nInstaller stopped: {error}\nNo broad cleanup or data reset is performed. See docs/install.md for failure recovery and rollback.");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = if Path::new("/tmp/opencode").is_dir() {
                "/tmp/opencode"
            } else {
                "/tmp"
            };
            let path = Path::new(root).join(format!(
                "droidloom-installer-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn default_options_are_interactive_and_start_after_install() {
        assert_eq!(Options::parse(args(&[])).unwrap(), Options::default());
    }
    #[test]
    fn parses_check_and_noninteractive_flags() {
        let options = Options::parse(args(&["--check", "--yes", "--no-start"])).unwrap();
        assert!(options.check && options.yes && options.no_start);
    }
    #[test]
    fn rejects_unknown_and_missing_or_duplicate_arguments() {
        for values in [
            vec!["--force"],
            vec!["--runtime-deb"],
            vec!["--build-sysroot", "--yes"],
            vec!["--runtime-deb", "a.deb", "--runtime-deb", "b.deb"],
        ] {
            assert!(Options::parse(args(&values)).is_err());
        }
    }
    #[test]
    fn preserves_spaces_in_paths_and_makes_relative_paths_absolute() {
        let options = Options::parse(args(&["--runtime-deb", "a package.deb"])).unwrap();
        assert_eq!(
            options.runtime_deb.unwrap(),
            env::current_dir().unwrap().join("a package.deb")
        );
    }
    #[test]
    fn only_accepts_valid_lowercase_hashes() {
        assert!(is_hash(&"a".repeat(64), 64));
        assert!(!is_hash(&"A".repeat(64), 64));
        assert!(!is_hash(&"a".repeat(63), 64));
        assert!(!is_hash(&"z".repeat(64), 64));
    }
    #[test]
    fn version_check_handles_patch_releases_and_rejects_old_or_invalid_versions() {
        assert!(version_at_least_242("unshare from util-linux 2.42\n"));
        assert!(version_at_least_242("unshare from util-linux 2.42.4\n"));
        assert!(version_at_least_242("unshare from util-linux 3.0\n"));
        assert!(!version_at_least_242("unshare from util-linux 2.41.3\n"));
        assert!(!version_at_least_242("error 242"));
        assert!(!version_at_least_242("error 3.0"));
        assert!(!version_at_least_242(
            "unshare from util-linux 2.41.3 warning 3.0"
        ));
    }
    #[test]
    fn legacy_absolute_home_override_matches_portable_template() {
        let home = Path::new("/home/example");
        assert!(unit_matches(
            "# legacy comment\n[Service]\nExecStart=\nExecStart=/home/example/.local/lib/example\n",
            "[Service]\nExecStart=\nExecStart=%h/.local/lib/example\n",
            home
        ));
    }
    #[test]
    fn extra_unit_directives_or_different_paths_are_never_adopted() {
        let expected = "[Service]\nExecStart=\nExecStart=%h/example\n";
        assert!(!unit_matches(
            &format!("{expected}Environment=OTHER=1\n"),
            expected,
            Path::new("/home/example")
        ));
        assert!(!unit_matches(
            "[Service]\nExecStart=/other\n",
            expected,
            Path::new("/home/example")
        ));
    }
    #[test]
    fn environment_parser_does_not_confuse_other_variables_with_path() {
        assert_eq!(environment_path("OTHER_PATH=/opt/droidloom/bin:/bin"), None);
        assert_eq!(
            environment_path("\"PATH=/opt/droidloom/bin:/bin\" OTHER=1"),
            Some("/opt/droidloom/bin:/bin")
        );
        assert_eq!(
            environment_path("OTHER=1 PATH=/usr/bin:/bin"),
            Some("/usr/bin:/bin")
        );
    }
    #[test]
    fn no_changes_means_no_install_work() {
        let mut plan = Plan {
            install_runtime: false,
            setup: false,
            kernel: false,
            unshare: false,
            titlebar: false,
        };
        assert!(!plan.changed());
        plan.kernel = true;
        assert!(plan.changed());
    }
    #[test]
    fn exclusive_writes_preserve_existing_files_and_symlink_targets() {
        let temp = Temp::new();
        let target = temp.0.join("target");
        write_new(&target, b"original", 0o600).unwrap();
        assert!(write_new(&target, b"replacement", 0o644).is_err());
        let link = temp.0.join("link");
        symlink(&target, &link).unwrap();
        assert!(write_new(&link, b"replacement", 0o644).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"original");
        assert_eq!(fs::metadata(&target).unwrap().mode() & 0o777, 0o600);
    }
    #[test]
    fn privileged_path_check_rejects_temporary_user_writable_parents() {
        let temp = Temp::new();
        fs::set_permissions(&temp.0, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(root_path(&temp.0.join("output")).is_err());
    }
    #[test]
    fn per_user_directories_allow_owner_group_access_but_not_world_write() {
        assert!(user_metadata_allowed(1000, 0o775, 1000));
        assert!(user_metadata_allowed(0, 0o755, 1000));
        assert!(!user_metadata_allowed(1000, 0o777, 1000));
        assert!(!user_metadata_allowed(1001, 0o755, 1000));
    }
    #[test]
    fn install_lock_is_exclusive_and_released_on_exit() {
        let temp = Temp::new();
        let path = temp.0.join("lock");
        let lock = InstallLock::acquire(&path).unwrap();
        assert!(InstallLock::acquire(&path).is_err());
        drop(lock);
        assert!(!path.exists());
        assert!(InstallLock::acquire(&path).is_ok());
    }
    #[test]
    fn check_mode_and_noop_never_enter_mutating_install_path() {
        let mut installer = Installer {
            root: PathBuf::from("/does-not-exist"),
            home: PathBuf::from("/does-not-exist"),
            uid: 1000,
            pinned: Pinned {
                commit: String::new(),
                repository: String::new(),
                version: String::new(),
                presenter: String::new(),
                legacy_presenter: String::new(),
                patch_sha: String::new(),
                deb_sha: String::new(),
                patch: PathBuf::new(),
                archive_url: String::new(),
                archive_sha: String::new(),
            },
            options: Options {
                check: true,
                ..Options::default()
            },
        };
        let plan = Plan {
            install_runtime: true,
            setup: true,
            kernel: true,
            unshare: true,
            titlebar: true,
        };
        assert!(installer.install(&plan).is_ok());
        installer.options.check = false;
        let noop = Plan {
            install_runtime: false,
            setup: false,
            kernel: false,
            unshare: false,
            titlebar: false,
        };
        assert!(installer.install(&noop).is_ok());
        assert!(!installer.root.exists());
    }
}
