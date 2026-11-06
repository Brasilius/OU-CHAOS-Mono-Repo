use anyhow::{Context, Result, bail};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf};

#[derive(Clone)]
pub struct Paths {
    pub config: PathBuf,
    pub data: PathBuf,
}

impl Paths {
    pub fn new() -> Result<Self> {
        let dirs = ProjectDirs::from("org", "OUChaos", "AeroArchive")
            .context("Cannot find your user application directories")?;
        Ok(Self {
            config: dirs.config_dir().join("settings.json"),
            data: dirs.data_local_dir().to_path_buf(),
        })
    }

    pub fn repository(&self, remote: &str) -> PathBuf {
        let hash = format!("{:x}", Sha256::digest(remote.as_bytes()));
        self.data.join("repositories").join(&hash[..16])
    }
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub remote: String,
    pub project: String,
    pub author: String,
    pub email: String,
}

impl Config {
    pub fn load(paths: &Paths) -> Result<Self> {
        match fs::read(&paths.config) {
            Ok(data) => serde_json::from_slice(&data)
                .context("Settings are invalid JSON; repair settings.json"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).context("Cannot read settings"),
        }
    }

    pub fn validate(&self) -> Result<()> {
        validate_remote(&self.remote)?;
        if self.project.is_empty()
            || self.project.len() > 64
            || !self
                .project
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            bail!("Project must be 1–64 letters, digits, hyphens or underscores.");
        }
        for (label, value) in [("Author", &self.author), ("Email", &self.email)] {
            if value.trim().is_empty() || value.chars().any(char::is_control) {
                bail!("{label} is required and must not contain control characters.");
            }
        }
        if !self.email.contains('@') {
            bail!("Enter an email address for the Git commit author.");
        }
        Ok(())
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        self.validate()?;
        fs::create_dir_all(paths.config.parent().context("Invalid settings path")?)?;
        fs::write(&paths.config, serde_json::to_vec_pretty(self)?)?;
        Ok(())
    }
}

pub fn validate_remote(remote: &str) -> Result<()> {
    let path = remote.strip_prefix("https://github.com/").or_else(|| remote.strip_prefix("git@github.com:"))
        .context("Use https://github.com/OWNER/REPO.git or git@github.com:OWNER/REPO.git (no embedded token).")?;
    let parts: Vec<_> = path.trim_end_matches(".git").split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|s| {
            s.is_empty()
                || *s == "."
                || *s == ".."
                || !s
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        })
    {
        bail!("Enter a GitHub repository URL with exactly OWNER/REPO.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_github_repository_urls_are_accepted() {
        for url in [
            "https://github.com/OU-CHAOS/sim.git",
            "git@github.com:OU-CHAOS/sim.git",
        ] {
            validate_remote(url).unwrap();
        }
        for url in [
            "--upload-pack=evil",
            "https://token@github.com/a/b",
            "file:///tmp/repo",
            "https://github.com/a/../b",
            "https://github.com/a/b?token=x",
        ] {
            assert!(validate_remote(url).is_err(), "{url}");
        }
    }
}
