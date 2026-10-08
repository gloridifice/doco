//! Filesystem-derived library scope. Discovery never writes a registry or follows links.
use crate::{Project, safety};
use anyhow::{Context, Result, bail};
use std::{fs, io::ErrorKind, path::Path};

fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("inspect {}", path.display())),
    }
}

fn git_boundary(path: &Path) -> Result<bool> {
    // Both repositories (.git directory) and worktrees/submodules (.git file).
    exists(&path.join(".git"))
}

fn library_at(path: &Path) -> Result<Option<Project>> {
    let doco = path.join("doco");
    safety::inspect_absolute(&doco)?;
    // A directory merely named `doco` is not a library. Recognize partial
    // libraries too, so damaged local state cannot silently select a parent.
    if !exists(&doco.join("architecture.md"))? && !exists(&doco.join("changes"))? {
        return Ok(None);
    }
    let project = Project::open(path)?;
    project
        .initialized()
        .with_context(|| format!("invalid doco library: {}", path.display()))?;
    Ok(Some(project))
}

impl Project {
    /// Resolve ordinary CLI commands to the nearest library, within this Git worktree.
    /// `open` deliberately remains exact for explicit roots and library callers.
    pub fn discover(start: &Path) -> Result<Self> {
        let start = Self::open(start)?;
        for path in start.root().ancestors() {
            if let Some(project) = library_at(path)? {
                return Ok(project);
            }
            if git_boundary(path)? {
                break;
            }
        }
        bail!("no doco library found; run doco init in the intended project directory")
    }

    /// Nearest ancestor library, excluding self and never crossing a Git root.
    pub fn parent(&self) -> Result<Option<Self>> {
        if git_boundary(self.root())? {
            return Ok(None);
        }
        for path in self.root().ancestors().skip(1) {
            if let Some(project) = library_at(path)? {
                return Ok(Some(project));
            }
            if git_boundary(path)? {
                break;
            }
        }
        Ok(None)
    }
}

fn excluded(name: &std::ffi::OsStr) -> bool {
    matches!(
        name.to_str(),
        Some(
            "doco"
                | ".git"
                | ".hg"
                | ".svn"
                | ".agents"
                | ".claude"
                | ".pi"
                | ".codex"
                | ".tgrep"
                | "node_modules"
                | "target"
                | "dist"
                | "build"
                | "vendor"
                | ".venv"
                | "venv"
                | "__pycache__"
        )
    )
}

/// Return the top library and all libraries in display order: current, then
/// depth-first parent-before-child traversal, with current removed from that walk.
pub(crate) fn family(current: &Project) -> Result<(Project, Vec<Project>)> {
    current.initialized()?;
    let mut top = Project::open(current.root())?;
    while let Some(parent) = top.parent()? {
        top = parent;
    }
    let mut projects = vec![Project::open(current.root())?];
    let mut pending = vec![top.root().to_path_buf()];
    while let Some(path) = pending.pop() {
        safety::inspect(top.root(), &path)?;
        if path != top.root() && git_boundary(&path)? {
            continue;
        }
        if path != current.root()
            && let Some(project) = library_at(&path)?
        {
            projects.push(project);
        }
        let mut directories = Vec::new();
        for entry in
            fs::read_dir(&path).with_context(|| format!("scan libraries in {}", path.display()))?
        {
            let entry = entry?;
            if excluded(&entry.file_name()) {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path())?;
            if metadata.is_dir() && !safety::is_link(&metadata) {
                directories.push(entry.path());
            }
        }
        directories.sort();
        pending.extend(directories.into_iter().rev());
    }
    Ok((top, projects))
}
