use std::path::PathBuf;
use std::process::Command;

use super::types::Worktree;
use crate::error::{DaryaError, Result};

pub struct WorktreeManager {
    pub repo_root: PathBuf,
}

impl WorktreeManager {
    pub fn new(repo_root: PathBuf) -> Self {
        Self { repo_root }
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

    pub fn add(&self, name: &str, branch: &str) -> Result<()> {
        let worktree_path = self.repo_root.parent().unwrap_or(&self.repo_root).join(name);
        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                "-b",
                branch,
                worktree_path.to_str().unwrap_or(name),
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

    pub fn remove(&self, path: &PathBuf) -> Result<()> {
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
