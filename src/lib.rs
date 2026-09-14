pub mod check;
pub mod fix;
pub(crate) mod index;
pub mod init;
pub mod lifecycle;
pub mod markdown;
pub mod package;
pub mod safety;
pub mod templates;
pub mod ui;
pub mod update;

pub use package::{PROPOSAL_ONLY_MARKER, PackageMode, package_mode};

use anyhow::{Context, Result, bail};
use std::{
    fmt, fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum State {
    Active,
    Completed,
    Archived,
}
impl State {
    pub const ALL: [Self; 3] = [Self::Active, Self::Completed, Self::Archived];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Completed => "completed",
            Self::Archived => "archived",
        }
    }
}
impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
#[derive(Clone, Debug)]
pub struct Change {
    pub id: String,
    pub state: State,
    pub path: PathBuf,
}

pub struct Project {
    root: PathBuf,
}
impl Project {
    pub fn open(root: &Path) -> Result<Self> {
        let absolute = std::path::absolute(root)?;
        // Check before canonicalizing, so a symlink supplied as the root is not hidden.
        safety::inspect_absolute(&absolute)?;
        let root = fs::canonicalize(root).context("project root must already exist")?;
        if !root.is_dir() {
            bail!("project root is not a directory");
        }
        Ok(Self { root })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn path(&self, relative: impl AsRef<Path>) -> PathBuf {
        self.root.join(relative)
    }
    pub fn initialized(&self) -> Result<()> {
        for rel in [
            "doco/architecture.md",
            "doco/changes/active",
            "doco/changes/completed",
            "doco/changes/archived",
        ] {
            let path = self.path(rel);
            safety::inspect(&self.root, &path)?;
            if !path.exists() {
                bail!("missing {rel}; run doco init --agent <id>");
            }
        }
        Ok(())
    }
    pub fn changes(&self) -> Result<Vec<Change>> {
        index::entries(self)
    }
    pub fn resolve(&self, id: &str) -> Result<Change> {
        index::resolve(self, id)
    }
}

pub fn validate_id(id: &str) -> Result<()> {
    if id.is_empty()
        || id.len() > 80
        || id.starts_with('-')
        || id.ends_with('-')
        || id.contains("--")
        || !id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        bail!(
            "invalid change ID {id:?}; use 1-80 lowercase ASCII letters, digits, and single interior hyphens"
        );
    }
    let upper = id.to_ascii_uppercase();
    if ["CON", "PRN", "AUX", "NUL", "CLOCK$"].contains(&upper.as_str())
        || ((upper.starts_with("COM") || upper.starts_with("LPT"))
            && upper.len() == 4
            && upper.as_bytes()[3].is_ascii_digit())
    {
        bail!("reserved filesystem name: {id}");
    }
    Ok(())
}
