use crate::{
    config::{Config, Paths},
    ingest::{self, QueuedFile},
};
use anyhow::{Context, Result, bail};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::Sender,
};

// Every argument goes directly to Git; no shell interpolation. Credential helpers
// remain available, but interactive prompts cannot corrupt the raw-mode terminal.
fn command(directory: Option<&Path>) -> Command {
    let mut cmd = Command::new("git");
    cmd.args(["-c", "core.hooksPath=", "-c", "core.fsmonitor=false"])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .env(
            "GIT_SSH_COMMAND",
            "ssh -o BatchMode=yes -o ConnectTimeout=15",
        )
        .stdin(Stdio::null());
    // Do not inherit a parent repository's routing environment.
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
    ] {
        cmd.env_remove(key);
    }
    if let Some(dir) = directory {
        cmd.current_dir(dir);
    }
    cmd
}

fn git(directory: Option<&Path>, args: &[&str]) -> Result<String> {
    let output = command(directory)
        .args(args)
        .output()
        .context("Git could not start. Install Git, reopen Aero Archive and press F2.")?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        bail!(
            "Git {} failed: {}\nFor authentication, run `gh auth login` then `gh auth setup-git` in another terminal, or set up your SSH key. Check repository write access. Retry with F6.",
            args.first().unwrap_or(&"command"),
            error.trim()
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().into())
}

pub fn available() -> bool {
    git(None, &["--version"]).is_ok()
}

pub fn global_identity(key: &str) -> String {
    git(None, &["config", "--global", "--get", key]).unwrap_or_default()
}

pub fn doctor(config: &Config) -> Vec<String> {
    let mut lines = vec![if available() {
        "OK   Git is installed.".into()
    } else {
        "WARN Git is missing. Follow the installation steps below.".into()
    }];
    for (label, value, key) in [
        ("Author", &config.author, "user.name"),
        ("Email", &config.email, "user.email"),
    ] {
        lines.push(if !value.is_empty() || !global_identity(key).is_empty() {
            format!("OK   {label} is configured.")
        } else {
            format!("WARN {label} is missing. Enter it in Settings (F3).")
        });
    }
    lines.push(match config.validate() {
        Ok(()) => "OK   Repository settings are valid.".into(),
        Err(e) => format!("WARN {e}"),
    });
    lines.push("INFO Authentication and write access are checked on upload; GitHub credentials are managed by Git.".into());
    lines
}

pub fn setup_help() -> String {
    #[cfg(target_os = "macos")]
    let install = "1. Install Git in Terminal: xcode-select --install";
    #[cfg(target_os = "windows")]
    let install = "1. Install Git in PowerShell: winget install --id Git.Git -e";
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let install = "1. Install Git with your package manager:\n   Debian/Ubuntu: sudo apt install git\n   Fedora: sudo dnf install git | Arch: sudo pacman -S git";
    format!(
        "{install}\n2. Reopen Aero Archive; press F2 to recheck.\n3. Create a GitHub repository with write access.\n4. Install GitHub CLI from https://cli.github.com\n   Run: gh auth login\n   Then: gh auth setup-git\n   Or set up SSH and use git@github.com:OWNER/REPO.git.\n5. F3: enter repository URL, project, author and email.\n   Author details apply only to this app's checkout.\n6. Add files, tags and notes. F5 reviews your upload.\n\nNever put passwords or tokens in the URL or settings."
    )
}

#[derive(Clone)]
pub struct Upload {
    pub paths: Paths,
    pub config: Config,
    pub files: Vec<QueuedFile>,
    pub tags: String,
    pub notes: String,
}

#[derive(Debug)]
pub enum Progress {
    Status(String),
    Done(Result<String, String>),
}

pub fn start(request: Upload, retry: bool, tx: Sender<Progress>) {
    std::thread::spawn(move || {
        let report = |text: &str| {
            let _ = tx.send(Progress::Status(text.into()));
        };
        let result = execute(&request, retry, &report).map_err(|e| format!("{e:#}"));
        let _ = tx.send(Progress::Done(result));
    });
}

struct Lock(PathBuf);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn execute(request: &Upload, retry: bool, report: &dyn Fn(&str)) -> Result<String> {
    request.config.validate()?;
    fs::create_dir_all(&request.paths.data)?;
    let repo = request.paths.repository(&request.config.remote);
    fs::create_dir_all(repo.parent().context("Invalid repository directory")?)?;
    let lock = repo.with_extension("lock");
    OpenOptions::new().write(true).create_new(true).open(&lock)
        .with_context(|| format!("Another upload holds {}. If the app previously crashed, close all instances and remove that lock file.", lock.display()))?;
    let _lock = Lock(lock);
    if !repo.exists() {
        if retry {
            bail!("No managed checkout exists yet. Use F5 to upload a batch.");
        }
        report("Cloning GitHub repository…");
        // Clone into a temporary directory so a failed clone cannot poison the checkout.
        let clone = repo.with_extension("clone");
        if clone.exists() {
            bail!(
                "A previous clone was interrupted. Inspect and remove {} before retrying.",
                clone.display()
            );
        }
        let result = command(None)
            .args([
                "clone",
                "--no-recurse-submodules",
                "--",
                &request.config.remote,
            ])
            .arg(&clone)
            .output()?;
        if !result.status.success() {
            let _ = fs::remove_dir_all(&clone);
            bail!(
                "Clone failed: {}\nRun gh auth login and gh auth setup-git outside the app; check repository access.",
                String::from_utf8_lossy(&result.stderr)
            );
        }
        fs::rename(clone, &repo)?;
    }
    let origin = git(Some(&repo), &["remote", "get-url", "origin"])?;
    if origin != request.config.remote {
        bail!("Managed checkout origin changed. Restore its origin before uploading.");
    }
    pipeline(request, &repo, retry, report)
}

#[derive(Serialize, Deserialize)]
struct Pending {
    relative: String,
    project: String,
    author: String,
    email: String,
}

fn pipeline(request: &Upload, repo: &Path, retry: bool, report: &dyn Fn(&str)) -> Result<String> {
    let pending_path = repo.with_extension("pending.json");
    if pending_path.exists() {
        if !retry {
            bail!(
                "An earlier batch is waiting. Press F6 to finish it before adding a new version."
            );
        }
        let pending: Pending = serde_json::from_slice(&fs::read(&pending_path)?)?;
        if !pending.relative.starts_with("sim-data/")
            || Path::new(&pending.relative)
                .components()
                .any(|p| !matches!(p, std::path::Component::Normal(_)))
        {
            bail!(
                "Invalid pending upload journal. Inspect {}.",
                pending_path.display()
            );
        }
        return finish(request, repo, &pending, &pending_path, report);
    }
    if retry {
        bail!("There is no pending batch. Use F5 to review and upload your files.");
    }
    if !git(Some(repo), &["status", "--porcelain"])?.is_empty() {
        bail!(
            "Managed checkout contains unfinished changes: {}. Inspect it before uploading.",
            repo.display()
        );
    }
    report("Fetching latest version…");
    git(Some(repo), &["fetch", "origin"])?;
    let branch = git(Some(repo), &["symbolic-ref", "--short", "HEAD"])?;
    let remote_ref = format!("refs/remotes/origin/{branch}");
    if git(Some(repo), &["rev-parse", "--verify", &remote_ref]).is_ok() {
        // Never fold an unpushed local commit into a later batch.
        let ahead = git(
            Some(repo),
            &["rev-list", "--count", &format!("{remote_ref}..HEAD")],
        )?;
        if ahead != "0" {
            bail!(
                "Managed checkout has unpushed commits without an upload journal. Inspect {} before continuing.",
                repo.display()
            );
        }
        git(Some(repo), &["merge", "--ff-only", &remote_ref])?;
    }
    report("Copying files and calculating SHA-256 checksums…");
    let staging = repo.with_extension("staging");
    if staging.exists() {
        bail!(
            "An interrupted staging folder exists: {}. Inspect and remove it before retrying.",
            staging.display()
        );
    }
    let built = ingest::build_batch(
        &staging,
        &request.config.project,
        &request.config.author,
        &request.notes,
        &request.tags,
        &request.files,
    );
    let manifest = match built {
        Ok(m) => m,
        Err(e) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(e);
        }
    };
    let relative = format!(
        "sim-data/{}/versions/{}",
        request.config.project, manifest.version
    );
    let target = repo.join(&relative);
    // Refuse symlinked path components supplied by a repository.
    let mut parent = repo.to_path_buf();
    for component in ["sim-data", request.config.project.as_str(), "versions"] {
        parent.push(component);
        if let Ok(meta) = fs::symlink_metadata(&parent) {
            if !meta.is_dir() || meta.file_type().is_symlink() {
                bail!(
                    "Destination is not a regular directory: {}",
                    parent.display()
                );
            }
        } else {
            fs::create_dir(&parent)?;
        }
    }
    let pending = Pending {
        relative,
        project: request.config.project.clone(),
        author: request.config.author.clone(),
        email: request.config.email.clone(),
    };
    // Persist intent before moving or committing; F6 can recover after any later failure.
    fs::write(&pending_path, serde_json::to_vec_pretty(&pending)?)?;
    fs::rename(&staging, &target)?;
    finish(request, repo, &pending, &pending_path, report)
}

fn finish(
    request: &Upload,
    repo: &Path,
    pending: &Pending,
    pending_path: &Path,
    report: &dyn Fn(&str),
) -> Result<String> {
    let target = repo.join(&pending.relative);
    if !target.exists() {
        let staging = repo.with_extension("staging");
        if staging.exists() {
            fs::rename(staging, &target)?;
        } else {
            bail!(
                "Pending batch files are missing. Inspect {}.",
                repo.display()
            );
        }
    }
    // Only app-owned batch files may be part of the pending commit.
    let output = command(Some(repo))
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .output()?;
    if !output.status.success() {
        bail!("Cannot inspect managed checkout.");
    }
    let prefix = format!("{}/", pending.relative);
    let entries: Vec<_> = output
        .stdout
        .split(|b| *b == 0)
        .filter(|b| !b.is_empty())
        .collect();
    for entry in &entries {
        let name = String::from_utf8_lossy(entry.get(3..).context("Invalid Git status record")?);
        if !name.starts_with(&prefix) {
            bail!(
                "Unrelated changes exist in {}. Inspect them before retrying.",
                repo.display()
            );
        }
    }
    if !entries.is_empty() {
        report("Committing version and metadata…");
        git(Some(repo), &["add", "-f", "--", &pending.relative])?;
        let message = format!(
            "sim: archive {} [{}]",
            pending.project,
            target.file_name().unwrap_or_default().to_string_lossy()
        );
        git(
            Some(repo),
            &[
                "-c",
                &format!("user.name={}", pending.author),
                "-c",
                &format!("user.email={}", pending.email),
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                &message,
            ],
        )?;
    }
    report("Checking for concurrent uploads…");
    git(Some(repo), &["fetch", "origin"])?;
    let branch = git(Some(repo), &["symbolic-ref", "--short", "HEAD"])?;
    let remote_ref = format!("refs/remotes/origin/{branch}");
    if git(Some(repo), &["rev-parse", "--verify", &remote_ref]).is_ok() {
        // Rebase only unpublished app commits; never force-push shared history.
        git(Some(repo), &[
            "-c", &format!("user.name={}", pending.author),
            "-c", &format!("user.email={}", pending.email),
            "-c", "commit.gpgsign=false", "rebase", &remote_ref,
        ]).with_context(|| format!("Could not reconcile concurrent changes. Inspect the managed checkout at {} before retrying.", repo.display()))?;
    }
    report("Pushing to GitHub… (F6 retries safely if the connection fails)");
    git(Some(repo), &["push", "--set-upstream", "origin", "HEAD"])?;
    let commit = git(Some(repo), &["rev-parse", "--short", "HEAD"])?;
    fs::remove_file(pending_path)?;
    let result = format!(
        "{} · {} · {}",
        Utc::now().format("%Y-%m-%d %H:%M UTC"),
        commit,
        pending.relative
    );
    // Local receipts are informational; a receipt write must not turn a successful push into an error.
    let _ = fs::create_dir_all(&request.paths.data);
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(request.paths.data.join("history.log"))
    {
        use std::io::Write;
        let _ = writeln!(file, "{result}");
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publishes_to_bare_remote_and_recovers_rejected_push() {
        let tmp = tempfile::tempdir().unwrap();
        let remote = tmp.path().join("remote.git");
        let repo = tmp.path().join("checkout");
        assert!(
            Command::new("git")
                .args(["init", "--bare"])
                .arg(&remote)
                .output()
                .unwrap()
                .status
                .success()
        );
        assert!(
            Command::new("git")
                .arg("clone")
                .arg(&remote)
                .arg(&repo)
                .output()
                .unwrap()
                .status
                .success()
        );
        let input = tmp.path().join("flight.ork");
        fs::write(&input, b"simulation-data\r\n").unwrap();
        git(Some(&repo), &["config", "core.autocrlf", "true"]).unwrap();
        let request = Upload {
            paths: Paths {
                config: tmp.path().join("settings.json"),
                data: tmp.path().join("data"),
            },
            config: Config {
                remote: remote.to_string_lossy().into(),
                project: "flight".into(),
                author: "Test Pilot".into(),
                email: "test@example.com".into(),
            },
            files: vec![QueuedFile::inspect(input).unwrap()],
            tags: "baseline".into(),
            notes: "Review me".into(),
        };
        let first = pipeline(&request, &repo, false, &|_| {}).unwrap();
        assert!(first.contains("sim-data/flight/versions/"));
        let versions = repo.join("sim-data/flight/versions");
        let version = fs::read_dir(&versions)
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .file_name();
        let blob = format!(
            "HEAD:sim-data/flight/versions/{}/openrocket/001-flight.ork",
            version.to_string_lossy()
        );
        let archived = command(Some(&repo)).args(["show", &blob]).output().unwrap();
        assert!(archived.status.success());
        assert_eq!(archived.stdout, b"simulation-data\r\n");
        assert_eq!(
            git(Some(&repo), &["rev-list", "--count", "HEAD"]).unwrap(),
            "1"
        );
        // A local receive policy rejects the next push after its commit succeeds.
        git(
            Some(&remote),
            &["config", "receive.denyNonFastForwards", "true"],
        )
        .unwrap();
        git(
            Some(&remote),
            &["config", "receive.denyCurrentBranch", "refuse"],
        )
        .unwrap();
        let branch = git(Some(&repo), &["symbolic-ref", "--short", "HEAD"]).unwrap();
        git(Some(&remote), &["config", "core.bare", "false"]).unwrap();
        git(
            Some(&remote),
            &["symbolic-ref", "HEAD", &format!("refs/heads/{branch}")],
        )
        .unwrap();
        assert!(pipeline(&request, &repo, false, &|_| {}).is_err());
        assert!(repo.with_extension("pending.json").exists());
        assert_eq!(
            git(Some(&repo), &["rev-list", "--count", "HEAD"]).unwrap(),
            "2"
        );
        assert!(pipeline(&request, &repo, false, &|_| {}).is_err());
        git(Some(&remote), &["config", "core.bare", "true"]).unwrap();
        // Another reviewer uploads while this batch is awaiting retry.
        let other = tmp.path().join("reviewer");
        assert!(
            Command::new("git")
                .arg("clone")
                .arg(&remote)
                .arg(&other)
                .output()
                .unwrap()
                .status
                .success()
        );
        fs::write(other.join("review.txt"), "Independent review").unwrap();
        git(Some(&other), &["add", "review.txt"]).unwrap();
        git(
            Some(&other),
            &[
                "-c",
                "user.name=Reviewer",
                "-c",
                "user.email=review@example.com",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-m",
                "review",
            ],
        )
        .unwrap();
        git(Some(&other), &["push", "origin", "HEAD"]).unwrap();
        pipeline(&request, &repo, true, &|_| {}).unwrap();
        assert!(!repo.with_extension("pending.json").exists());
        assert_eq!(
            git(Some(&repo), &["rev-list", "--count", "HEAD"]).unwrap(),
            "3"
        );
        assert_eq!(
            fs::read_to_string(repo.join("review.txt")).unwrap(),
            "Independent review"
        );
        assert_eq!(
            fs::read_dir(repo.join("sim-data/flight/versions"))
                .unwrap()
                .count(),
            2
        );
        assert_eq!(
            git(Some(&repo), &["rev-parse", "HEAD"]).unwrap(),
            git(Some(&remote), &["rev-parse", "HEAD"]).unwrap()
        );
    }
}
