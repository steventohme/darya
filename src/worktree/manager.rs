use std::path::{Path, PathBuf};
use std::process::Command;

use super::types::Worktree;
use crate::error::{DaryaError, Result};

pub struct WorktreeManager {
    pub repo_root: PathBuf,
    pub dir_format: String,
}

impl WorktreeManager {
    pub fn new(repo_root: PathBuf, dir_format: String) -> Self {
        Self {
            repo_root,
            dir_format,
        }
    }

    pub fn list(&self) -> Result<Vec<Worktree>> {
        let output = Command::new("git")
            .args(["worktree", "list", "--porcelain"])
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| DaryaError::Git(format!("failed to run git worktree list: {}", e)))?;

        if !output.status.success() {
            return Err(DaryaError::Git(format!(
                "git worktree list failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(parse_porcelain(&stdout, &self.repo_root))
    }

    pub fn add(&self, branch: &str) -> Result<()> {
        let repo_name = self
            .repo_root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "repo".to_string());
        let dir_name = self
            .dir_format
            .replace("{repo}", &repo_name)
            .replace("{branch}", branch);
        let worktree_path = self
            .repo_root
            .parent()
            .unwrap_or(&self.repo_root)
            .join(dir_name);
        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                branch,
                worktree_path.to_str().unwrap_or(branch),
            ])
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| DaryaError::Git(format!("failed to run git worktree add: {}", e)))?;

        if !output.status.success() {
            return Err(DaryaError::Git(format!(
                "git worktree add failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }

    pub fn remove(&self, path: &Path) -> Result<()> {
        let output = Command::new("git")
            .args(["worktree", "remove", path.to_str().unwrap_or("")])
            .current_dir(&self.repo_root)
            .output()
            .map_err(|e| DaryaError::Git(format!("failed to run git worktree remove: {}", e)))?;

        if !output.status.success() {
            return Err(DaryaError::Git(format!(
                "git worktree remove failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(())
    }
}

/// Discover worktrees for an arbitrary root directory.
/// Runs `git worktree list --porcelain` in the given directory.
pub fn list_worktrees_for_root(root: &Path) -> Result<Vec<Worktree>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(root)
        .output()
        .map_err(|e| DaryaError::Git(format!("failed to run git worktree list: {}", e)))?;

    if !output.status.success() {
        return Err(DaryaError::Git(format!(
            "git worktree list failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_porcelain(&stdout, &root.to_path_buf()))
}

/// List local branch names for a given worktree/repo directory.
pub fn list_branches(worktree_path: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .args(["branch", "--list", "--format=%(refname:short)"])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| DaryaError::Git(format!("failed to run git branch: {}", e)))?;
    if !output.status.success() {
        return Err(DaryaError::Git(format!(
            "git branch failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect())
}

/// Get the current branch name for a worktree directory.
pub fn current_branch(worktree_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| DaryaError::Git(format!("failed to get current branch: {}", e)))?;
    if !output.status.success() {
        return Err(DaryaError::Git("not on a branch".to_string()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Switch to a branch in a worktree directory.
pub fn switch_branch(worktree_path: &Path, branch: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["switch", branch])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| DaryaError::Git(format!("failed to run git switch: {}", e)))?;
    if !output.status.success() {
        return Err(DaryaError::Git(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }
    Ok(())
}

fn parse_porcelain(output: &str, repo_root: &PathBuf) -> Vec<Worktree> {
    let mut worktrees = Vec::new();
    let mut current_path: Option<PathBuf> = None;
    let mut current_branch: Option<String> = None;
    let mut is_bare = false;

    for line in output.lines() {
        if let Some(path_str) = line.strip_prefix("worktree ") {
            // Save previous worktree if exists
            if let Some(path) = current_path.take() {
                if !is_bare {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "unknown".to_string());
                    let is_main = path == *repo_root;
                    worktrees.push(Worktree {
                        name,
                        path,
                        branch: current_branch.take(),
                        is_main,
                    });
                }
                is_bare = false;
            }
            current_path = Some(PathBuf::from(path_str));
            current_branch = None;
        } else if let Some(branch_ref) = line.strip_prefix("branch ") {
            // branch refs/heads/main → "main"
            current_branch = Some(
                branch_ref
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch_ref)
                    .to_string(),
            );
        } else if line == "bare" {
            is_bare = true;
        }
    }

    // Don't forget last entry
    if let Some(path) = current_path {
        if !is_bare {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let is_main = path == *repo_root;
            worktrees.push(Worktree {
                name,
                path,
                branch: current_branch,
                is_main,
            });
        }
    }

    worktrees
}
