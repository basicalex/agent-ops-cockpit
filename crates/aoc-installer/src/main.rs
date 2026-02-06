use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
struct Config {
    repo: Option<String>,
    reference: Option<String>,
    yes: bool,
    skip_doctor: bool,
}

#[derive(Clone, Copy, Debug)]
enum Downloader {
    Curl,
    Wget,
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Result<Self, String> {
        let pid = std::process::id();
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|err| format!("system clock error: {err}"))?
            .as_millis();

        let path = env::temp_dir().join(format!("{prefix}-{pid}-{millis}"));
        fs::create_dir_all(&path).map_err(|err| format!("failed to create temp dir: {err}"))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn main() {
    if let Err(err) = run() {
        eprintln!("aoc-installer: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let config = parse_args(env::args().skip(1))?;
    let downloader = detect_downloader()?;
    let repo = resolve_repo(config.repo)?;

    let reference = match config.reference {
        Some(reference) => reference,
        None => resolve_reference(&repo, downloader).unwrap_or_else(|_| "main".to_string()),
    };

    println!("AOC installer");
    println!("- Repo: {}", repo);
    println!("- Ref:  {}", reference);
    println!("- Scope: user-local (~/.local/bin, ~/.config)");

    if !config.yes && !confirm_install()? {
        println!("Install cancelled.");
        return Ok(());
    }

    if !command_exists("tar") {
        return Err("'tar' is required but was not found in PATH".to_string());
    }

    let temp_dir = TempDir::new("aoc-installer")?;
    let archive = temp_dir.path().join("aoc-src.tar.gz");

    let tag_url = format!(
        "https://github.com/{}/archive/refs/tags/{}.tar.gz",
        repo, reference
    );
    let head_url = format!(
        "https://github.com/{}/archive/refs/heads/{}.tar.gz",
        repo, reference
    );

    println!("Downloading source archive...");
    if downloader.download_to_file(&tag_url, &archive).is_err() {
        downloader
            .download_to_file(&head_url, &archive)
            .map_err(|err| format!("failed to download source archive: {err}"))?;
    }

    run_command(
        "tar",
        [
            OsStr::new("-xzf"),
            archive.as_os_str(),
            OsStr::new("-C"),
            temp_dir.path().as_os_str(),
        ],
    )?;

    let source_root = resolve_source_root(&archive, temp_dir.path())?;
    let install_script = source_root.join("install.sh");
    if !install_script.is_file() {
        return Err("install.sh not found in downloaded archive".to_string());
    }

    println!("Running install.sh...");
    run_command("bash", [install_script.as_os_str()])?;

    if !config.skip_doctor {
        maybe_run_doctor();
    }

    println!("AOC install completed.");
    println!("Next: run 'aoc-init' in your project, then 'aoc'.");
    Ok(())
}

fn parse_args<I>(mut args: I) -> Result<Config, String>
where
    I: Iterator<Item = String>,
{
    let mut config = Config {
        repo: None,
        reference: None,
        yes: false,
        skip_doctor: false,
    };

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo" => {
                let value = args.next().ok_or("--repo requires a value")?;
                config.repo = Some(value);
            }
            "--ref" => {
                let value = args.next().ok_or("--ref requires a value")?;
                config.reference = Some(value);
            }
            "--yes" => config.yes = true,
            "--skip-doctor" => config.skip_doctor = true,
            "-h" | "--help" => {
                print_usage();
                std::process::exit(0);
            }
            _ => return Err(format!("unknown argument: {arg}")),
        }
    }

    Ok(config)
}

fn print_usage() {
    println!("Usage: aoc-installer [options]");
    println!();
    println!("Options:");
    println!("  --repo <owner/name>   GitHub repository (required unless auto-detected)");
    println!("  --ref <tag-or-branch> Release tag or branch to install");
    println!("  --yes                 Non-interactive install");
    println!("  --skip-doctor         Skip post-install aoc-doctor check");
    println!("  -h, --help            Show help");
}

fn confirm_install() -> Result<bool, String> {
    print!("Install AOC to user-local paths (~/.local/bin, ~/.config)? [Y/n]: ");
    io::stdout()
        .flush()
        .map_err(|err| format!("failed to flush stdout: {err}"))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|err| format!("failed to read input: {err}"))?;

    let answer = input.trim().to_lowercase();
    Ok(!(answer == "n" || answer == "no"))
}

fn detect_downloader() -> Result<Downloader, String> {
    if command_exists("curl") {
        return Ok(Downloader::Curl);
    }
    if command_exists("wget") {
        return Ok(Downloader::Wget);
    }
    Err("curl or wget is required but neither command was found".to_string())
}

fn resolve_repo(repo_arg: Option<String>) -> Result<String, String> {
    if let Some(repo) = repo_arg {
        return Ok(repo);
    }

    if let Ok(repo) = env::var("GITHUB_REPOSITORY") {
        if is_valid_repo_slug(&repo) {
            return Ok(repo);
        }
    }

    if command_exists("git") {
        if let Ok(url) = capture_command("git", ["config", "--get", "remote.origin.url"]) {
            if let Some(repo) = parse_repo_from_remote_url(url.trim()) {
                return Ok(repo);
            }
        }
    }

    Err("could not determine GitHub repo; pass --repo <owner/name>".to_string())
}

fn parse_repo_from_remote_url(url: &str) -> Option<String> {
    let stripped = if let Some(rest) = url.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = url.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = url.strip_prefix("http://github.com/") {
        rest
    } else {
        return None;
    };

    let slug = stripped.strip_suffix(".git").unwrap_or(stripped);
    if is_valid_repo_slug(slug) {
        Some(slug.to_string())
    } else {
        None
    }
}

fn is_valid_repo_slug(value: &str) -> bool {
    let mut parts = value.split('/');
    match (parts.next(), parts.next(), parts.next()) {
        (Some(owner), Some(repo), None) => !owner.is_empty() && !repo.is_empty(),
        _ => false,
    }
}

fn resolve_reference(repo: &str, downloader: Downloader) -> Result<String, String> {
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let body = downloader.fetch_string(&url)?;
    parse_tag_name(&body).ok_or("failed to parse latest release tag".to_string())
}

fn parse_tag_name(body: &str) -> Option<String> {
    let key = "\"tag_name\"";
    let idx = body.find(key)?;
    let after_key = &body[idx + key.len()..];
    let colon_idx = after_key.find(':')?;
    let value = after_key[colon_idx + 1..].trim_start();
    if !value.starts_with('"') {
        return None;
    }
    let rest = &value[1..];
    let end_quote = rest.find('"')?;
    Some(rest[..end_quote].to_string())
}

fn resolve_source_root(archive: &Path, temp_dir: &Path) -> Result<PathBuf, String> {
    let listing = capture_command("tar", [OsStr::new("-tzf"), archive.as_os_str()])?;
    let first_line = listing
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or("archive appears to be empty")?;

    let root_name = first_line
        .split('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .ok_or("failed to detect source root")?;

    let root = temp_dir.join(root_name);
    if !root.is_dir() {
        return Err("extracted source directory was not found".to_string());
    }
    Ok(root)
}

fn maybe_run_doctor() {
    if !command_exists("aoc-doctor") {
        println!("Skipping aoc-doctor (not found in PATH yet).");
        return;
    }

    println!("Running aoc-doctor...");
    let status = Command::new("aoc-doctor")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status();

    match status {
        Ok(exit) if exit.success() => {}
        Ok(_) => eprintln!("aoc-doctor reported issues."),
        Err(err) => eprintln!("failed to run aoc-doctor: {err}"),
    }
}

impl Downloader {
    fn fetch_string(self, url: &str) -> Result<String, String> {
        match self {
            Downloader::Curl => capture_command("curl", ["-fsSL", url]),
            Downloader::Wget => capture_command("wget", ["-qO-", url]),
        }
    }

    fn download_to_file(self, url: &str, path: &Path) -> Result<(), String> {
        let status = match self {
            Downloader::Curl => Command::new("curl")
                .args(["-fsSL", "-o"])
                .arg(path)
                .arg(url)
                .status(),
            Downloader::Wget => Command::new("wget").arg("-qO").arg(path).arg(url).status(),
        }
        .map_err(|err| format!("failed to launch downloader: {err}"))?;

        if !status.success() {
            return Err(format!("downloader exited with status {status}"));
        }

        Ok(())
    }
}

fn run_command<I, S>(program: &str, args: I) -> Result<(), String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let status = Command::new(program)
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| format!("failed to launch {program}: {err}"))?;

    if !status.success() {
        return Err(format!("{program} exited with status {status}"));
    }

    Ok(())
}

fn capture_command<I, S>(program: &str, args: I) -> Result<String, String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("failed to launch {program}: {err}"))?;

    if !output.status.success() {
        return Err(format!("{program} exited with status {}", output.status));
    }

    String::from_utf8(output.stdout).map_err(|err| format!("invalid UTF-8 from {program}: {err}"))
}

fn command_exists(name: &str) -> bool {
    env::var_os("PATH")
        .map(|paths| {
            env::split_paths(&paths).any(|path| {
                let full = path.join(name);
                full.is_file()
            })
        })
        .unwrap_or(false)
}
