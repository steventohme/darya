use std::path::PathBuf;

/// A top-level grouping in the sidebar (e.g. "MyProject", "OtherRepo").
#[derive(Debug, Clone)]
pub struct Section {
    pub name: String,
    pub collapsed: bool,
    pub items: Vec<SidebarItem>,
    /// Root directory for worktree discovery. None for the auto-generated section.
    pub root_path: Option<PathBuf>,
}

/// A directory or worktree within a section.
#[derive(Debug, Clone)]
pub struct SidebarItem {
    pub path: PathBuf,
    pub display_name: String,
    /// Some for git worktrees (branch name), None for plain directories.
    pub branch: Option<String>,
    pub is_main: bool,
    pub collapsed: bool,
    pub sessions: Vec<SessionSlot>,
}

/// A named session slot attached to a sidebar item.
#[derive(Debug, Clone)]
pub struct SessionSlot {
    pub kind: SessionKind,
    pub label: String,
    /// None until the session is actually spawned.
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    Claude,
    Shell,
}
