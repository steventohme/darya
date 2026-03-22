use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use edtui::{
    EditorEventHandler, EditorMode, EditorState as EdtuiState, Index2, Lines as EdtuiLines,
};

use ratatui::layout::Rect;
use ratatui::style::Color;

use crate::config;
use crate::planet::sprites::PlanetAnimation;
use crate::planet::types::PlanetKind;
use crate::sidebar::tree::SidebarTree;
use crate::sidebar::types::SessionKind;

const IGNORED_NAMES: &[&str] = &["target", "node_modules", "__pycache__"];

/// Sidebar width as a percentage (minimum).
pub const SIDEBAR_MIN_WIDTH: u16 = 15;
/// Sidebar width as a percentage (maximum).
pub const SIDEBAR_MAX_WIDTH: u16 = 50;
/// Sidebar width step size for resize.
pub const SIDEBAR_STEP: u16 = 2;

/// Wrapping move-up for list navigation. Returns new index.
pub fn wrapping_prev(selected: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    if selected == 0 {
        len - 1
    } else {
        selected - 1
    }
}

/// Wrapping move-down for list navigation. Returns new index.
pub fn wrapping_next(selected: usize, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (selected + 1) % len
}

/// Which sidebar node a color is being assigned to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorTarget {
    Section(usize),
    Item(usize, usize),
    Session(usize, usize, usize),
}

/// Preset colors for the color picker (None = clear). 7 columns × 2 rows.
pub const PRESET_COLORS: &[Option<Color>] = &[
    None,                               // clear
    Some(Color::Rgb(0xE0, 0x7A, 0x2A)), // amber (border accent)
    Some(Color::Rgb(0xD4, 0x9A, 0x6A)), // warm sand
    Some(Color::Rgb(0xCC, 0x8A, 0x4E)), // copper
    Some(Color::Rgb(0xC4, 0x6B, 0x5E)), // terracotta
    Some(Color::Rgb(0xB0, 0x5A, 0x78)), // dusty rose
    Some(Color::Rgb(0x9A, 0x6E, 0xB0)), // muted lavender
    Some(Color::Rgb(0x72, 0xA5, 0xC5)), // slate blue
    Some(Color::Rgb(0x6B, 0xC2, 0xA5)), // sage teal
    Some(Color::Rgb(0x7A, 0xB0, 0x7A)), // muted green
    Some(Color::Rgb(0xA0, 0xB8, 0x70)), // olive
    Some(Color::Rgb(0xD4, 0xC4, 0x7A)), // soft gold
    Some(Color::Rgb(0xB0, 0xB0, 0xB0)), // silver
    Some(Color::Rgb(0x78, 0x88, 0x98)), // cool gray
];
const MAX_FILE_SIZE: u64 = 1_048_576; // 1MB
/// Maximum depth of nested splits (allows up to 16 leaf panes).
const MAX_SPLIT_DEPTH: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub enum PaneContent {
    Terminal(String), // Claude session ID
    Shell(String),    // Shell session ID
    Editor,           // File editor (uses app.editor state)
}

impl PaneContent {
    /// Extract the session ID if this pane contains a terminal or shell.
    pub fn session_id(&self) -> Option<&str> {
        match self {
            PaneContent::Terminal(id) | PaneContent::Shell(id) => Some(id),
            PaneContent::Editor => None,
        }
    }

    /// Derive the ViewKind for this pane content.
    pub fn to_view_kind(&self) -> ViewKind {
        match self {
            PaneContent::Terminal(_) => ViewKind::Terminal,
            PaneContent::Shell(_) => ViewKind::Shell,
            PaneContent::Editor => ViewKind::Editor,
        }
    }

    /// Human-readable label for display in pickers.
    pub fn display_label(&self) -> String {
        match self {
            PaneContent::Terminal(id) => format!("Terminal: {}", id),
            PaneContent::Shell(id) => format!("Shell: {}", id),
            PaneContent::Editor => "Editor".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

/// A binary tree node representing either a single pane or a split containing two children.
#[derive(Debug, Clone)]
pub enum SplitNode {
    Leaf(PaneContent),
    Split {
        direction: SplitDirection,
        first: Box<SplitNode>,
        second: Box<SplitNode>,
    },
}

impl SplitNode {
    /// Count the number of leaf panes in this tree.
    pub fn leaf_count(&self) -> usize {
        match self {
            SplitNode::Leaf(_) => 1,
            SplitNode::Split { first, second, .. } => first.leaf_count() + second.leaf_count(),
        }
    }

    /// In-order traversal of all leaf pane contents.
    pub fn leaves(&self) -> Vec<&PaneContent> {
        match self {
            SplitNode::Leaf(content) => vec![content],
            SplitNode::Split { first, second, .. } => {
                let mut result = first.leaves();
                result.extend(second.leaves());
                result
            }
        }
    }

    /// Maximum depth of the tree (Leaf = 0, Split with two leaves = 1).
    pub fn depth(&self) -> usize {
        match self {
            SplitNode::Leaf(_) => 0,
            SplitNode::Split { first, second, .. } => 1 + first.depth().max(second.depth()),
        }
    }

    /// Get leaf content by flat index (in-order traversal) without allocating.
    pub fn leaf_at(&self, index: usize) -> Option<&PaneContent> {
        self.leaf_at_inner(index, &mut 0)
    }

    fn leaf_at_inner(&self, target: usize, counter: &mut usize) -> Option<&PaneContent> {
        match self {
            SplitNode::Leaf(content) => {
                if *counter == target {
                    Some(content)
                } else {
                    *counter += 1;
                    None
                }
            }
            SplitNode::Split { first, second, .. } => {
                if let Some(result) = first.leaf_at_inner(target, counter) {
                    return Some(result);
                }
                second.leaf_at_inner(target, counter)
            }
        }
    }

    /// Remove a leaf by flat index, collapsing its parent Split to the sibling.
    /// Returns true if the removal was performed.
    pub fn remove_leaf(&mut self, index: usize) -> bool {
        self.remove_leaf_inner(index, &mut 0)
    }

    fn remove_leaf_inner(&mut self, target: usize, counter: &mut usize) -> bool {
        match self {
            SplitNode::Leaf(_) => {
                // Can't remove root leaf from itself
                false
            }
            SplitNode::Split { first, second, .. } => {
                let first_count = first.leaf_count();
                if target < *counter + first_count {
                    // Target is in the first subtree
                    if let SplitNode::Leaf(_) = first.as_ref() {
                        // First child is the target leaf — replace self with second
                        *self = *second.clone();
                        return true;
                    }
                    return first.remove_leaf_inner(target, counter);
                }
                *counter += first_count;
                if let SplitNode::Leaf(_) = second.as_ref() {
                    if *counter == target {
                        // Second child is the target leaf — replace self with first
                        *self = *first.clone();
                        return true;
                    }
                    return false;
                }
                second.remove_leaf_inner(target, counter)
            }
        }
    }

    /// Split the leaf at `index` into a Split node with the original leaf and new content.
    /// The new content is placed as the second child.
    /// Returns false if depth would exceed MAX_SPLIT_DEPTH or index is invalid.
    pub fn split_leaf(
        &mut self,
        index: usize,
        direction: SplitDirection,
        new_content: PaneContent,
    ) -> bool {
        if self.depth() >= MAX_SPLIT_DEPTH {
            return false;
        }
        self.split_leaf_inner(index, direction, new_content, &mut 0)
    }

    fn split_leaf_inner(
        &mut self,
        target: usize,
        direction: SplitDirection,
        new_content: PaneContent,
        counter: &mut usize,
    ) -> bool {
        match self {
            SplitNode::Leaf(_) => {
                if *counter == target {
                    let old = std::mem::replace(self, SplitNode::Leaf(PaneContent::Editor));
                    *self = SplitNode::Split {
                        direction,
                        first: Box::new(old),
                        second: Box::new(SplitNode::Leaf(new_content)),
                    };
                    true
                } else {
                    false
                }
            }
            SplitNode::Split { first, second, .. } => {
                let first_count = first.leaf_count();
                if target < *counter + first_count {
                    return first.split_leaf_inner(target, direction, new_content, counter);
                }
                *counter += first_count;
                second.split_leaf_inner(target, direction, new_content, counter)
            }
        }
    }

    /// Human-readable label for display in pickers.
    pub fn display_label(&self) -> String {
        match self {
            SplitNode::Leaf(content) => content.display_label(),
            SplitNode::Split { first, second, .. } => {
                format!(
                    "Split [{} | {}]",
                    first.display_short(),
                    second.display_short()
                )
            }
        }
    }

    /// Short label for nested display.
    fn display_short(&self) -> String {
        match self {
            SplitNode::Leaf(content) => content.display_label(),
            SplitNode::Split { .. } => {
                let count = self.leaf_count();
                format!("{} panes", count)
            }
        }
    }

    /// Check if any leaf references the given session ID.
    pub fn contains_session(&self, session_id: &str) -> bool {
        match self {
            SplitNode::Leaf(content) => content.session_id() == Some(session_id),
            SplitNode::Split { first, second, .. } => {
                first.contains_session(session_id) || second.contains_session(session_id)
            }
        }
    }

    /// Remove the first leaf containing the given session ID in a single pass.
    /// Returns true if a leaf was removed.
    pub fn remove_session(&mut self, session_id: &str) -> bool {
        match self {
            SplitNode::Leaf(_) => false,
            SplitNode::Split { first, second, .. } => {
                // Check if first child is the target leaf
                if let SplitNode::Leaf(ref content) = **first {
                    if content.session_id() == Some(session_id) {
                        *self = *second.clone();
                        return true;
                    }
                }
                // Check if second child is the target leaf
                if let SplitNode::Leaf(ref content) = **second {
                    if content.session_id() == Some(session_id) {
                        *self = *first.clone();
                        return true;
                    }
                }
                // Recurse into children
                if first.remove_session(session_id) {
                    return true;
                }
                second.remove_session(session_id)
            }
        }
    }

    /// Collect all session IDs from all leaves.
    pub fn all_session_ids(&self) -> Vec<&str> {
        self.leaves()
            .into_iter()
            .filter_map(|c| c.session_id())
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct PaneLayout {
    pub root: SplitNode,
    pub focused: usize, // index into in-order leaf traversal
}

use crate::config::{KeybindingsConfig, Theme};
use crate::event::AppEvent;
use crate::worktree::types::Worktree;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Navigation,
    Terminal,
    Editor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotePosition {
    Hidden,
    Sidebar, // Notes panel in sidebar bottom (read-only preview or inline editor)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    Left,
    Center, // Reserved for future use
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    Worktrees,
    Terminal,
    FileExplorer,
    Editor,
    Search,
    GitStatus,
    DiffView,
    GitBlame,
    GitLog,
    Shell,
    Notes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarView {
    Worktrees,
    FileExplorer,
    Search,
    GitStatus,
}

impl SidebarView {
    pub fn to_view_kind(self) -> ViewKind {
        match self {
            SidebarView::Worktrees => ViewKind::Worktrees,
            SidebarView::FileExplorer => ViewKind::FileExplorer,
            SidebarView::Search => ViewKind::Search,
            SidebarView::GitStatus => ViewKind::GitStatus,
        }
    }

    pub fn next(self) -> Self {
        match self {
            SidebarView::Worktrees => SidebarView::FileExplorer,
            SidebarView::FileExplorer => SidebarView::GitStatus,
            SidebarView::Search => SidebarView::GitStatus,
            SidebarView::GitStatus => SidebarView::Worktrees,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            SidebarView::Worktrees => SidebarView::GitStatus,
            SidebarView::FileExplorer => SidebarView::Worktrees,
            SidebarView::Search => SidebarView::FileExplorer,
            SidebarView::GitStatus => SidebarView::FileExplorer,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainView {
    Terminal,
    Editor,
    DiffView,
    GitBlame,
    GitLog,
    Shell,
}

impl MainView {
    /// Cycle forward through primary main views: Terminal → Editor → Shell → Terminal.
    /// Skips contextual views (DiffView, GitBlame, GitLog).
    pub fn next(self) -> Self {
        match self {
            MainView::Terminal => MainView::Editor,
            MainView::Editor => MainView::Shell,
            MainView::Shell => MainView::Terminal,
            // Contextual views cycle back to Terminal
            MainView::DiffView | MainView::GitBlame | MainView::GitLog => MainView::Terminal,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            MainView::Terminal => MainView::Shell,
            MainView::Editor => MainView::Terminal,
            MainView::Shell => MainView::Editor,
            MainView::DiffView | MainView::GitBlame | MainView::GitLog => MainView::Terminal,
        }
    }

    pub fn to_view_kind(self) -> ViewKind {
        match self {
            MainView::Terminal => ViewKind::Terminal,
            MainView::Editor => ViewKind::Editor,
            MainView::DiffView => ViewKind::DiffView,
            MainView::GitBlame => ViewKind::GitBlame,
            MainView::GitLog => ViewKind::GitLog,
            MainView::Shell => ViewKind::Shell,
        }
    }
}

/// Overlay prompts for user input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prompt {
    /// Creating a worktree: user types a branch name
    CreateWorktree { input: String },
    /// Confirming worktree deletion
    ConfirmDelete { worktree_name: String },
    /// Project search input
    SearchInput { input: String },
    /// Add a named shell session slot
    AddShellSlot { input: String },
    /// Confirming section deletion
    ConfirmDeleteSection {
        section_name: String,
        section_idx: usize,
    },
    /// Color picker overlay
    ColorPicker { target: ColorTarget, cursor: usize },
    /// First-launch setup guide for Cmd key configuration
    SetupGuide,
    /// Offer to restore previous sessions
    RestoreSession { count: usize },
    /// Planet/theme picker overlay
    ThemePicker {
        selected: usize,
        previous_theme: Theme,
    },
}

/// An entry in the directory browser (a subdirectory).
#[derive(Debug, Clone)]
pub struct DirBrowserEntry {
    pub path: PathBuf,
    pub name: String,
    pub depth: usize,
}

/// Modal directory browser for selecting a root directory when creating a section.
pub struct DirBrowser {
    pub entries: Vec<DirBrowserEntry>,
    pub expanded: HashSet<PathBuf>,
    pub selected: usize,
}

impl DirBrowser {
    pub fn new(root: PathBuf) -> Self {
        let mut browser = Self {
            entries: Vec::new(),
            expanded: HashSet::new(),
            selected: 0,
        };
        // Start with the home dir expanded
        browser.expanded.insert(root.clone());
        browser.rebuild(&root);
        browser
    }

    /// Rebuild the flat entry list from the expanded state.
    fn rebuild(&mut self, root: &Path) {
        self.entries.clear();
        self.build_entries(root, 0);
        if !self.entries.is_empty() && self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }
    }

    fn build_entries(&mut self, dir: &Path, depth: usize) {
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return;
        };

        let mut dirs: Vec<(String, PathBuf)> = Vec::new();
        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                dirs.push((name, path));
            }
        }
        dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        for (name, path) in dirs {
            let is_expanded = self.expanded.contains(&path);
            self.entries.push(DirBrowserEntry {
                path: path.clone(),
                name,
                depth,
            });
            if is_expanded {
                self.build_entries(&path, depth + 1);
            }
        }
    }

    pub fn move_up(&mut self) {
        self.selected = wrapping_prev(self.selected, self.entries.len());
    }

    pub fn move_down(&mut self) {
        self.selected = wrapping_next(self.selected, self.entries.len());
    }

    /// Expand the selected directory (show its children).
    pub fn expand(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            let path = entry.path.clone();
            if !self.expanded.contains(&path) {
                let root = self.entries.first().map(|e| {
                    if e.depth == 0 {
                        e.path.parent().unwrap_or(&e.path).to_path_buf()
                    } else {
                        e.path.clone()
                    }
                });
                self.expanded.insert(path);
                if let Some(root) = root {
                    self.rebuild(&root);
                }
            }
        }
    }

    /// Collapse the selected directory (hide its children).
    pub fn collapse(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            let path = entry.path.clone();
            if self.expanded.contains(&path) {
                self.expanded.remove(&path);
                let root = self.entries.first().map(|e| {
                    if e.depth == 0 {
                        e.path.parent().unwrap_or(&e.path).to_path_buf()
                    } else {
                        e.path.clone()
                    }
                });
                if let Some(root) = root {
                    self.rebuild(&root);
                }
            } else {
                // Not expanded — jump to parent
                let current_depth = entry.depth;
                if current_depth > 0 {
                    for i in (0..self.selected).rev() {
                        if self.entries[i].depth < current_depth {
                            self.selected = i;
                            return;
                        }
                    }
                }
            }
        }
    }

    /// Get the currently selected path.
    pub fn selected_path(&self) -> Option<&PathBuf> {
        self.entries.get(self.selected).map(|e| &e.path)
    }

    /// Check if a path is expanded.
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }
}

pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub is_dir: bool,
    pub depth: usize,
}

pub struct FileExplorerState {
    pub entries: Vec<FileEntry>,
    pub selected: usize,
    pub expanded: HashSet<PathBuf>,
    pub root: PathBuf,
    pub git_indicators: HashMap<String, GitFileStatus>,
    /// Pre-computed set of directory relative paths that contain dirty files.
    pub dirty_dirs: HashSet<String>,
    /// Whether git indicators need to be recomputed before next render.
    pub git_indicators_stale: bool,
}

impl FileExplorerState {
    pub fn new(root: PathBuf) -> Self {
        let mut state = Self {
            entries: Vec::new(),
            selected: 0,
            expanded: HashSet::new(),
            root,
            git_indicators: HashMap::new(),
            dirty_dirs: HashSet::new(),
            git_indicators_stale: true,
        };
        state.refresh();
        state
    }

    pub fn refresh(&mut self) {
        self.entries.clear();
        let root = self.root.clone();
        self.build_entries(&root, 0);
        if !self.entries.is_empty() && self.selected >= self.entries.len() {
            self.selected = self.entries.len() - 1;
        }
    }

    fn build_entries(&mut self, dir: &PathBuf, depth: usize) {
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return;
        };

        let mut dirs: Vec<(String, PathBuf)> = Vec::new();
        let mut files: Vec<(String, PathBuf)> = Vec::new();

        for entry in read_dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if IGNORED_NAMES.contains(&name.as_str()) {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                dirs.push((name, path));
            } else {
                files.push((name, path));
            }
        }

        dirs.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        files.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

        for (name, path) in dirs {
            let is_expanded = self.expanded.contains(&path);
            self.entries.push(FileEntry {
                path: path.clone(),
                name,
                is_dir: true,
                depth,
            });
            if is_expanded {
                self.build_entries(&path, depth + 1);
            }
        }

        for (name, path) in files {
            self.entries.push(FileEntry {
                path,
                name,
                is_dir: false,
                depth,
            });
        }
    }

    pub fn move_up(&mut self) {
        self.selected = wrapping_prev(self.selected, self.entries.len());
    }

    pub fn move_down(&mut self) {
        self.selected = wrapping_next(self.selected, self.entries.len());
    }

    /// Enter on selected entry: toggle dir expand/collapse, return Some(path) for files.
    pub fn enter(&mut self) -> Option<PathBuf> {
        let entry = self.entries.get(self.selected)?;
        if entry.is_dir {
            let path = entry.path.clone();
            if self.expanded.contains(&path) {
                self.expanded.remove(&path);
            } else {
                self.expanded.insert(path);
            }
            self.refresh();
            None
        } else {
            Some(entry.path.clone())
        }
    }

    /// Collapse current dir or jump to parent entry.
    pub fn collapse_or_parent(&mut self) {
        if let Some(entry) = self.entries.get(self.selected) {
            // If it's an expanded dir, collapse it
            if entry.is_dir && self.expanded.contains(&entry.path) {
                let path = entry.path.clone();
                self.expanded.remove(&path);
                self.refresh();
                return;
            }
            // Otherwise jump to parent dir entry
            let current_depth = entry.depth;
            if current_depth > 0 {
                for i in (0..self.selected).rev() {
                    if self.entries[i].is_dir && self.entries[i].depth < current_depth {
                        self.selected = i;
                        return;
                    }
                }
            }
        }
    }

    /// Navigate root to parent directory.
    pub fn go_up_root(&mut self) {
        if let Some(parent) = self.root.parent() {
            self.root = parent.to_path_buf();
            self.expanded.clear();
            self.selected = 0;
            self.refresh();
        }
    }

    /// Refresh the git indicator cache by running `git status` on the root.
    pub fn refresh_git_indicators(&mut self) {
        self.git_indicators_stale = false;
        self.git_indicators.clear();
        self.dirty_dirs.clear();
        let Ok(entries) = run_git_status(&self.root) else {
            return;
        };
        for entry in entries {
            // Pre-compute all ancestor directories that contain dirty files.
            let path = std::path::Path::new(&entry.path);
            let mut current = path.parent();
            while let Some(dir) = current {
                let dir_str = dir.to_string_lossy().to_string();
                if dir_str.is_empty() {
                    break;
                }
                if !self.dirty_dirs.insert(dir_str) {
                    break; // already recorded this ancestor and all its parents
                }
                current = dir.parent();
            }
            self.git_indicators
                .entry(entry.path.clone())
                .and_modify(|existing| {
                    if status_priority(&entry.status) > status_priority(existing) {
                        *existing = entry.status;
                    }
                })
                .or_insert(entry.status);
        }
    }

    /// Set root to a new path (e.g. when switching worktrees).
    pub fn set_root(&mut self, path: PathBuf) {
        if self.root != path {
            self.root = path;
            self.expanded.clear();
            self.selected = 0;
            self.refresh();
            self.git_indicators.clear();
            self.dirty_dirs.clear();
            self.git_indicators_stale = true;
        }
    }

    /// Ensure git indicators are up-to-date. Call before rendering.
    pub fn ensure_git_indicators(&mut self) {
        if self.git_indicators_stale {
            self.refresh_git_indicators(); // already sets stale = false
        }
    }
}

pub struct EditorViewState {
    pub file_path: PathBuf,
    pub editor_state: EdtuiState,
    pub event_handler: EditorEventHandler,
    pub modified: bool,
    pub read_only: bool,
    pub file_extension: String,
}

impl EditorViewState {
    pub fn open(path: PathBuf) -> Result<Self, String> {
        let metadata = std::fs::metadata(&path).map_err(|e| format!("Cannot read file: {}", e))?;
        if metadata.len() > MAX_FILE_SIZE {
            return Err(format!(
                "File too large ({}KB > 1MB)",
                metadata.len() / 1024
            ));
        }

        let content =
            std::fs::read_to_string(&path).map_err(|e| format!("Cannot read file: {}", e))?;
        let lines = EdtuiLines::from(content.as_str());
        let editor_state = EdtuiState::new(lines);
        let event_handler = EditorEventHandler::default();

        let file_extension = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string();

        Ok(Self {
            file_path: path,
            editor_state,
            event_handler,
            modified: false,
            read_only: true,
            file_extension,
        })
    }

    /// Reload the file from disk if content has changed.
    /// Returns Ok(true) if reloaded, Ok(false) if unchanged.
    pub fn reload(&mut self) -> Result<bool, String> {
        let content = std::fs::read_to_string(&self.file_path)
            .map_err(|e| format!("Cannot read file: {}", e))?;
        let current = self.editor_state.lines.to_string();
        if content == current {
            return Ok(false);
        }
        let lines = EdtuiLines::from(content.as_str());
        let new_state = EdtuiState::new(lines);
        // Clamp cursor to new content bounds
        let max_row = new_state.lines.len().saturating_sub(1);
        let old_cursor = self.editor_state.cursor;
        let row = old_cursor.row.min(max_row);
        let col = new_state
            .lines
            .len_col(row)
            .map(|len| old_cursor.col.min(len))
            .unwrap_or(0);
        self.editor_state = new_state;
        self.editor_state.cursor = Index2::new(row, col);
        self.modified = false;
        Ok(true)
    }

    pub fn save(&mut self) -> Result<(), String> {
        let content = self.editor_state.lines.to_string();
        std::fs::write(&self.file_path, content).map_err(|e| format!("Failed to save: {}", e))?;
        self.modified = false;
        Ok(())
    }

    pub fn file_name(&self) -> &str {
        self.file_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
    }
}

/// Per-worktree notepad state, backed by a markdown file in ~/.config/darya/notes/.
pub struct NoteViewState {
    pub editor_state: EdtuiState,
    pub event_handler: EditorEventHandler,
    pub modified: bool,
    pub read_only: bool,
    pub file_path: PathBuf,
    pub worktree_name: String,
}

impl NoteViewState {
    /// Compute the notes file path for a given worktree path.
    /// Stored in ~/.config/darya/notes/ with sanitized filename.
    pub fn note_path_for_worktree(worktree_path: &Path) -> PathBuf {
        let notes_dir = config::config_dir().join("notes");
        let sanitized = worktree_path
            .to_string_lossy()
            .replace('/', "_")
            .trim_start_matches('_')
            .to_string();
        notes_dir.join(format!("{}.md", sanitized))
    }

    /// Open an existing note or create an empty one for the given worktree.
    pub fn open_or_create(worktree_path: &Path) -> Self {
        let file_path = Self::note_path_for_worktree(worktree_path);
        let worktree_name = worktree_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("notes")
            .to_string();

        let content = if file_path.exists() {
            std::fs::read_to_string(&file_path).unwrap_or_default()
        } else {
            String::new()
        };

        let lines = EdtuiLines::from(content.as_str());
        let editor_state = EdtuiState::new(lines);
        let event_handler = EditorEventHandler::default();

        Self {
            editor_state,
            event_handler,
            modified: false,
            read_only: true,
            file_path,
            worktree_name,
        }
    }

    /// Save the note content to disk.
    pub fn save(&mut self) -> Result<(), String> {
        // Ensure notes directory exists
        if let Some(parent) = self.file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create notes dir: {}", e))?;
        }
        let content = self.editor_state.lines.to_string();
        std::fs::write(&self.file_path, content)
            .map_err(|e| format!("Failed to save note: {}", e))?;
        self.modified = false;
        Ok(())
    }

    /// Get the content as a string for preview.
    pub fn content_string(&self) -> String {
        self.editor_state.lines.to_string()
    }
}

pub struct FuzzyFinderState {
    pub input: String,
    pub all_files: Vec<String>,
    pub root: PathBuf,
    pub results: Vec<(String, PathBuf)>,
    pub selected: usize,
}

impl FuzzyFinderState {
    pub fn new(root: PathBuf) -> Self {
        let all_files = walk_project_files(&root);
        let mut state = Self {
            input: String::new(),
            results: Vec::new(),
            root,
            all_files,
            selected: 0,
        };
        state.update_matches();
        state
    }

    pub fn update_matches(&mut self) {
        use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
        use nucleo_matcher::{Config, Matcher};

        if self.input.is_empty() {
            self.results = self
                .all_files
                .iter()
                .take(100)
                .map(|f| (f.clone(), self.root.join(f)))
                .collect();
            self.selected = 0;
            return;
        }

        let mut matcher = Matcher::new(Config::DEFAULT.match_paths());
        let pattern = Pattern::parse(&self.input, CaseMatching::Smart, Normalization::Smart);

        let mut scored: Vec<(u32, &str)> = self
            .all_files
            .iter()
            .filter_map(|f| {
                let mut buf = Vec::new();
                let haystack = nucleo_matcher::Utf32Str::new(f, &mut buf);
                pattern
                    .score(haystack, &mut matcher)
                    .map(|s| (s, f.as_str()))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));

        self.results = scored
            .into_iter()
            .take(100)
            .map(|(_, f)| (f.to_string(), self.root.join(f)))
            .collect();
        self.selected = 0;
    }

    pub fn move_up(&mut self) {
        self.selected = wrapping_prev(self.selected, self.results.len());
    }

    pub fn move_down(&mut self) {
        self.selected = wrapping_next(self.selected, self.results.len());
    }

    pub fn selected_path(&self) -> Option<&PathBuf> {
        self.results.get(self.selected).map(|(_, p)| p)
    }
}

fn walk_project_files(root: &Path) -> Vec<String> {
    use ignore::WalkBuilder;

    let mut files = Vec::new();
    let walker = WalkBuilder::new(root)
        .hidden(true)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .build();

    for entry in walker.flatten() {
        if entry.file_type().is_some_and(|ft| ft.is_file()) {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                files.push(rel.to_string_lossy().to_string());
            }
        }
    }
    files.sort();
    files
}

// ── Branch Switcher ─────────────────────────────────────────

pub struct BranchSwitcherState {
    pub input: String,
    pub all_branches: Vec<String>,
    pub current_branch: String,
    pub worktree_path: PathBuf,
    pub results: Vec<String>,
    pub selected: usize,
}

impl BranchSwitcherState {
    pub fn new(worktree_path: PathBuf) -> crate::error::Result<Self> {
        let all_branches = crate::worktree::manager::list_branches(&worktree_path)?;
        let current_branch =
            crate::worktree::manager::current_branch(&worktree_path).unwrap_or_default();
        let results = all_branches.clone();
        Ok(Self {
            input: String::new(),
            all_branches,
            current_branch,
            worktree_path,
            results,
            selected: 0,
        })
    }

    pub fn update_matches(&mut self) {
        use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
        use nucleo_matcher::{Config, Matcher};

        if self.input.is_empty() {
            self.results = self.all_branches.clone();
            self.selected = 0;
            return;
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(&self.input, CaseMatching::Smart, Normalization::Smart);

        let mut scored: Vec<(u32, &str)> = self
            .all_branches
            .iter()
            .filter_map(|b| {
                let mut buf = Vec::new();
                let haystack = nucleo_matcher::Utf32Str::new(b, &mut buf);
                pattern
                    .score(haystack, &mut matcher)
                    .map(|s| (s, b.as_str()))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));

        self.results = scored.into_iter().map(|(_, b)| b.to_string()).collect();
        self.selected = 0;
    }

    pub fn move_up(&mut self) {
        self.selected = wrapping_prev(self.selected, self.results.len());
    }

    pub fn move_down(&mut self) {
        self.selected = wrapping_next(self.selected, self.results.len());
    }

    pub fn selected_branch(&self) -> Option<&str> {
        self.results.get(self.selected).map(|s| s.as_str())
    }
}

// ── Split Picker ─────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitPickerStep {
    PickFirst,
    PickSecond,
}

#[derive(Debug, Clone)]
pub enum SplitPickerItem {
    Terminal { session_id: String, label: String },
    Shell { session_id: String, label: String },
    Editor { label: String },
    ExistingLayout { label: String, node: SplitNode },
}

impl SplitPickerItem {
    pub fn label(&self) -> &str {
        match self {
            SplitPickerItem::Terminal { label, .. } => label,
            SplitPickerItem::Shell { label, .. } => label,
            SplitPickerItem::Editor { label, .. } => label,
            SplitPickerItem::ExistingLayout { label, .. } => label,
        }
    }

    /// Convert to a SplitNode (Leaf for single items, tree for existing layout).
    pub fn to_split_node(&self) -> SplitNode {
        match self {
            SplitPickerItem::Terminal { session_id, .. } => {
                SplitNode::Leaf(PaneContent::Terminal(session_id.clone()))
            }
            SplitPickerItem::Shell { session_id, .. } => {
                SplitNode::Leaf(PaneContent::Shell(session_id.clone()))
            }
            SplitPickerItem::Editor { .. } => SplitNode::Leaf(PaneContent::Editor),
            SplitPickerItem::ExistingLayout { node, .. } => node.clone(),
        }
    }
}

pub struct SplitPickerState {
    pub items: Vec<SplitPickerItem>,
    /// Indices into `items` that are visible (filtered after step 1).
    pub visible: Vec<usize>,
    pub selected: usize,
    pub step: SplitPickerStep,
    pub first_choice: Option<usize>,
    pub direction: SplitDirection,
}

impl SplitPickerState {
    pub fn move_up(&mut self) {
        if !self.visible.is_empty() {
            self.selected = wrapping_prev(self.selected, self.visible.len());
        }
    }

    pub fn move_down(&mut self) {
        if !self.visible.is_empty() {
            self.selected = wrapping_next(self.selected, self.visible.len());
        }
    }

    pub fn selected_item(&self) -> Option<&SplitPickerItem> {
        self.visible
            .get(self.selected)
            .and_then(|&idx| self.items.get(idx))
    }

    pub fn selected_item_index(&self) -> Option<usize> {
        self.visible.get(self.selected).copied()
    }

    pub fn toggle_direction(&mut self) {
        self.direction = match self.direction {
            SplitDirection::Horizontal => SplitDirection::Vertical,
            SplitDirection::Vertical => SplitDirection::Horizontal,
        };
    }

    pub fn direction_name(&self) -> &'static str {
        match self.direction {
            SplitDirection::Horizontal => "horizontal",
            SplitDirection::Vertical => "vertical",
        }
    }
}

// ── Command Palette ──────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandId {
    ViewWorktrees,
    ViewTerminal,
    ViewFiles,
    ViewEditor,
    ViewSearch,
    ViewGitStatus,
    ViewGitBlame,
    ViewGitLog,
    ViewShell,
    StartSession,
    RestartSession,
    CloseSession,
    FuzzyFinder,
    ProjectSearch,
    RefreshGitStatus,
    SplitPane,
    SplitTerminal,
    SplitShell,
    SplitEditor,
    ClosePane,
    ToggleHelp,
    Quit,
    AddSection,
    AddShellSlot,
    AssignColor,
    SidebarGrow,
    SidebarShrink,
    SplitPaneVertical,
    ToggleSplitDirection,
    ThemePicker,
    TogglePlanet,
    BranchSwitcher,
}

#[derive(Debug, Clone)]
pub struct PaletteCommand {
    pub id: CommandId,
    pub name: String,
    pub keybinding: Option<String>,
}

pub struct CommandPaletteState {
    pub input: String,
    pub all_commands: Vec<PaletteCommand>,
    pub results: Vec<PaletteCommand>,
    pub selected: usize,
}

impl CommandPaletteState {
    pub fn new(keybindings: &KeybindingsConfig) -> Self {
        let all_commands = vec![
            // ── Search ──
            PaletteCommand {
                id: CommandId::FuzzyFinder,
                name: "Find File".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.fuzzy_finder)),
            },
            PaletteCommand {
                id: CommandId::ProjectSearch,
                name: "Search Project".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.project_search)),
            },
            // ── Git ──
            PaletteCommand {
                id: CommandId::BranchSwitcher,
                name: "Git: Switch Branch".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.branch_switcher)),
            },
            PaletteCommand {
                id: CommandId::ViewGitStatus,
                name: "View: Git Status".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.git_status)),
            },
            PaletteCommand {
                id: CommandId::RefreshGitStatus,
                name: "Refresh Git Status".to_string(),
                keybinding: None,
            },
            PaletteCommand {
                id: CommandId::ViewGitBlame,
                name: "View: Git Blame".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.git_blame)),
            },
            PaletteCommand {
                id: CommandId::ViewGitLog,
                name: "View: Git Log".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.git_log)),
            },
            // ── Views ──
            PaletteCommand {
                id: CommandId::ViewWorktrees,
                name: "View: Worktrees".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.worktrees)),
            },
            PaletteCommand {
                id: CommandId::ViewTerminal,
                name: "View: Terminal".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.terminal)),
            },
            PaletteCommand {
                id: CommandId::ViewFiles,
                name: "View: Files".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.files)),
            },
            PaletteCommand {
                id: CommandId::ViewEditor,
                name: "View: Editor".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.editor)),
            },
            PaletteCommand {
                id: CommandId::ViewSearch,
                name: "View: Search".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.search)),
            },
            PaletteCommand {
                id: CommandId::ViewShell,
                name: "View: Shell".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.shell)),
            },
            // ── Sessions ──
            PaletteCommand {
                id: CommandId::StartSession,
                name: "Session: Start".to_string(),
                keybinding: None,
            },
            PaletteCommand {
                id: CommandId::RestartSession,
                name: "Session: Restart".to_string(),
                keybinding: None,
            },
            PaletteCommand {
                id: CommandId::CloseSession,
                name: "Session: Close".to_string(),
                keybinding: None,
            },
            // ── Split ──
            PaletteCommand {
                id: CommandId::SplitPane,
                name: "Split: Same Type".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.split_pane)),
            },
            PaletteCommand {
                id: CommandId::SplitPaneVertical,
                name: "Split: Vertical Same Type".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.split_pane_vertical)),
            },
            PaletteCommand {
                id: CommandId::SplitTerminal,
                name: "Split: Terminal".to_string(),
                keybinding: None,
            },
            PaletteCommand {
                id: CommandId::SplitShell,
                name: "Split: Shell".to_string(),
                keybinding: None,
            },
            PaletteCommand {
                id: CommandId::SplitEditor,
                name: "Split: Editor".to_string(),
                keybinding: None,
            },
            PaletteCommand {
                id: CommandId::ClosePane,
                name: "Close Pane".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.close_pane)),
            },
            PaletteCommand {
                id: CommandId::ToggleSplitDirection,
                name: "Toggle Split Direction".to_string(),
                keybinding: None,
            },
            // ── Sidebar ──
            PaletteCommand {
                id: CommandId::AddSection,
                name: "Sidebar: Add Section".to_string(),
                keybinding: Some("Shift+N".to_string()),
            },
            PaletteCommand {
                id: CommandId::AddShellSlot,
                name: "Sidebar: Add Shell Slot".to_string(),
                keybinding: Some("Shift+S".to_string()),
            },
            PaletteCommand {
                id: CommandId::AssignColor,
                name: "Sidebar: Assign Color".to_string(),
                keybinding: Some("c".to_string()),
            },
            PaletteCommand {
                id: CommandId::SidebarGrow,
                name: "Sidebar: Grow".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.sidebar_grow)),
            },
            PaletteCommand {
                id: CommandId::SidebarShrink,
                name: "Sidebar: Shrink".to_string(),
                keybinding: Some(KeybindingsConfig::format(&keybindings.sidebar_shrink)),
            },
            // ── Theme ──
            PaletteCommand {
                id: CommandId::ThemePicker,
                name: "Theme: Choose Planet".to_string(),
                keybinding: None,
            },
            PaletteCommand {
                id: CommandId::TogglePlanet,
                name: "Theme: Toggle Planet Display".to_string(),
                keybinding: None,
            },
            // ── App ──
            PaletteCommand {
                id: CommandId::ToggleHelp,
                name: "Toggle Help".to_string(),
                keybinding: Some("?".to_string()),
            },
            PaletteCommand {
                id: CommandId::Quit,
                name: "Quit".to_string(),
                keybinding: Some("Ctrl+Q".to_string()),
            },
        ];
        let results = all_commands.clone();
        Self {
            input: String::new(),
            all_commands,
            results,
            selected: 0,
        }
    }

    pub fn update_matches(&mut self) {
        use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
        use nucleo_matcher::{Config, Matcher};

        if self.input.is_empty() {
            self.results = self.all_commands.clone();
            self.selected = 0;
            return;
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(&self.input, CaseMatching::Smart, Normalization::Smart);

        let mut scored: Vec<(u32, usize)> = self
            .all_commands
            .iter()
            .enumerate()
            .filter_map(|(i, cmd)| {
                let mut buf = Vec::new();
                let haystack = nucleo_matcher::Utf32Str::new(&cmd.name, &mut buf);
                pattern.score(haystack, &mut matcher).map(|s| (s, i))
            })
            .collect();

        scored.sort_by(|a, b| b.0.cmp(&a.0));

        self.results = scored
            .into_iter()
            .map(|(_, i)| self.all_commands[i].clone())
            .collect();
        self.selected = 0;
    }

    pub fn move_up(&mut self) {
        self.selected = wrapping_prev(self.selected, self.results.len());
    }

    pub fn move_down(&mut self) {
        self.selected = wrapping_next(self.selected, self.results.len());
    }

    pub fn selected_command(&self) -> Option<CommandId> {
        self.results.get(self.selected).map(|c| c.id)
    }
}

pub struct SearchResult {
    pub file_path: PathBuf,
    pub file_relative: String,
    pub line_number: usize,
    pub line_text: String,
}

pub struct SearchViewState {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub selected: usize,
    pub error: Option<String>,
}

impl SearchViewState {
    pub fn new(query: &str, root: &Path) -> Self {
        match run_ripgrep(query, root) {
            Ok(results) => Self {
                query: query.to_string(),
                results,
                selected: 0,
                error: None,
            },
            Err(e) => Self {
                query: query.to_string(),
                results: Vec::new(),
                selected: 0,
                error: Some(e),
            },
        }
    }

    pub fn move_up(&mut self) {
        self.selected = wrapping_prev(self.selected, self.results.len());
    }

    pub fn move_down(&mut self) {
        self.selected = wrapping_next(self.selected, self.results.len());
    }

    pub fn selected_result(&self) -> Option<&SearchResult> {
        self.results.get(self.selected)
    }
}

fn run_ripgrep(query: &str, root: &Path) -> Result<Vec<SearchResult>, String> {
    let output = std::process::Command::new("rg")
        .args([
            "--line-number",
            "--no-heading",
            "--color=never",
            "--max-count=200",
            query,
        ])
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed to run rg: {}. Is ripgrep installed?", e))?;

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        if code == 1 {
            // Exit code 1 = no matches
            return Ok(Vec::new());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("No such file or directory") || stderr.contains("not found") {
            return Err("ripgrep (rg) not found. Install it to use project search.".to_string());
        }
        // Exit code 2 = error (bad regex, etc.)
        return Err(format!("rg error: {}", stderr.trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    for line in text.lines().take(500) {
        // Format: file:line:text
        let mut parts = line.splitn(3, ':');
        let file = match parts.next() {
            Some(f) => f,
            None => continue,
        };
        let line_num: usize = match parts.next().and_then(|n| n.parse().ok()) {
            Some(n) => n,
            None => continue,
        };
        let text = parts.next().unwrap_or("").to_string();
        results.push(SearchResult {
            file_path: root.join(file),
            file_relative: file.to_string(),
            line_number: line_num,
            line_text: text,
        });
    }
    Ok(results)
}

// ── Git Status Types ────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Untracked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitStatusCategory {
    Staged,
    Unstaged,
    Untracked,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatusEntry {
    pub category: GitStatusCategory,
    pub status: GitFileStatus,
    pub path: String,
    pub orig_path: Option<String>,
}

pub struct GitStatusState {
    pub entries: Vec<GitStatusEntry>,
    pub selected: usize,
    pub error: Option<String>,
    pub worktree_path: PathBuf,
    pub stale: bool,
}

impl GitStatusState {
    pub fn new(worktree_path: PathBuf) -> Self {
        let (entries, error) = match run_git_status(&worktree_path) {
            Ok(entries) => (entries, None),
            Err(e) => (Vec::new(), Some(e)),
        };
        Self {
            entries,
            selected: 0,
            error,
            worktree_path,
            stale: false,
        }
    }

    pub fn mark_stale(&mut self) {
        self.stale = true;
    }

    /// Refresh only if stale. Call before rendering.
    pub fn ensure_fresh(&mut self) {
        if self.stale {
            self.refresh();
        }
    }

    pub fn refresh(&mut self) {
        self.stale = false;
        match run_git_status(&self.worktree_path) {
            Ok(entries) => {
                self.entries = entries;
                self.error = None;
                if !self.entries.is_empty() && self.selected >= self.entries.len() {
                    self.selected = self.entries.len() - 1;
                }
            }
            Err(e) => {
                self.entries.clear();
                self.error = Some(e);
                self.selected = 0;
            }
        }
    }

    pub fn move_up(&mut self) {
        self.selected = wrapping_prev(self.selected, self.entries.len());
    }

    pub fn move_down(&mut self) {
        self.selected = wrapping_next(self.selected, self.entries.len());
    }

    pub fn selected_entry(&self) -> Option<&GitStatusEntry> {
        self.entries.get(self.selected)
    }
}

pub fn run_git_status(root: &Path) -> Result<Vec<GitStatusEntry>, String> {
    let output = Command::new("git")
        .args(["status", "--porcelain=v1"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed to run git: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git status error: {}", stderr.trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();

    for line in text.lines() {
        if line.len() < 3 {
            continue;
        }
        let index_char = line.as_bytes()[0] as char;
        let worktree_char = line.as_bytes()[1] as char;
        let file_part = &line[3..];

        // Parse rename: "old -> new"
        let (path, orig_path) = if file_part.contains(" -> ") {
            let mut parts = file_part.splitn(2, " -> ");
            let orig = parts.next().unwrap_or("").to_string();
            let new = parts.next().unwrap_or("").to_string();
            (new, Some(orig))
        } else {
            (file_part.to_string(), None)
        };

        // Untracked
        if index_char == '?' && worktree_char == '?' {
            untracked.push(GitStatusEntry {
                category: GitStatusCategory::Untracked,
                status: GitFileStatus::Untracked,
                path,
                orig_path,
            });
            continue;
        }

        // Staged changes (index column)
        if index_char != ' ' && index_char != '?' {
            let status = match index_char {
                'A' => GitFileStatus::Added,
                'M' => GitFileStatus::Modified,
                'D' => GitFileStatus::Deleted,
                'R' => GitFileStatus::Renamed,
                _ => GitFileStatus::Modified,
            };
            staged.push(GitStatusEntry {
                category: GitStatusCategory::Staged,
                status,
                path: path.clone(),
                orig_path: orig_path.clone(),
            });
        }

        // Unstaged changes (worktree column)
        if worktree_char != ' ' && worktree_char != '?' {
            let status = match worktree_char {
                'M' => GitFileStatus::Modified,
                'D' => GitFileStatus::Deleted,
                _ => GitFileStatus::Modified,
            };
            unstaged.push(GitStatusEntry {
                category: GitStatusCategory::Unstaged,
                status,
                path: path.clone(),
                orig_path: orig_path.clone(),
            });
        }
    }

    let mut entries = Vec::new();
    entries.extend(staged);
    entries.extend(unstaged);
    entries.extend(untracked);
    Ok(entries)
}

/// Priority for merging duplicate git status entries — higher wins.
pub fn status_priority(status: &GitFileStatus) -> u8 {
    match status {
        GitFileStatus::Untracked => 0,
        GitFileStatus::Renamed => 1,
        GitFileStatus::Added => 2,
        GitFileStatus::Modified => 3,
        GitFileStatus::Deleted => 4,
    }
}

// ── Diff View Types ─────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffLineKind {
    Header,
    Addition,
    Deletion,
    Context,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffLine {
    pub kind: DiffLineKind,
    pub content: String,
}

pub struct DiffViewState {
    pub file_path: String,
    pub lines: Vec<DiffLine>,
    pub scroll_offset: usize,
    pub visible_height: usize,
}

impl DiffViewState {
    pub fn new(file: &str, root: &Path, category: GitStatusCategory) -> Self {
        let lines = match run_git_diff(file, root, category) {
            Ok(lines) => lines,
            Err(e) => vec![DiffLine {
                kind: DiffLineKind::Header,
                content: format!("Error: {}", e),
            }],
        };
        Self {
            file_path: file.to_string(),
            lines,
            scroll_offset: 0,
            visible_height: 24,
        }
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    pub fn scroll_down(&mut self, n: usize) {
        let max_scroll = self.lines.len().saturating_sub(self.visible_height);
        self.scroll_offset = (self.scroll_offset + n).min(max_scroll);
    }
}

pub fn run_git_diff(
    file: &str,
    root: &Path,
    category: GitStatusCategory,
) -> Result<Vec<DiffLine>, String> {
    let output = match category {
        GitStatusCategory::Staged => Command::new("git")
            .args(["diff", "--cached", "--", file])
            .current_dir(root)
            .output()
            .map_err(|e| format!("Failed to run git diff: {}", e))?,
        GitStatusCategory::Unstaged => Command::new("git")
            .args(["diff", "--", file])
            .current_dir(root)
            .output()
            .map_err(|e| format!("Failed to run git diff: {}", e))?,
        GitStatusCategory::Untracked => Command::new("git")
            .args(["diff", "--no-index", "/dev/null", file])
            .current_dir(root)
            .output()
            .map_err(|e| format!("Failed to run git diff: {}", e))?,
    };

    // git diff --no-index exits 1 when files differ (expected for untracked)
    if !output.status.success() && category != GitStatusCategory::Untracked {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            return Err(format!("git diff error: {}", stderr.trim()));
        }
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_diff_lines(&text))
}

pub fn parse_diff_lines(text: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    for line in text.lines().take(5000) {
        let kind = if line.starts_with("+++")
            || line.starts_with("---")
            || line.starts_with("diff ")
            || line.starts_with("index ")
            || line.starts_with("@@")
        {
            DiffLineKind::Header
        } else if line.starts_with('+') {
            DiffLineKind::Addition
        } else if line.starts_with('-') {
            DiffLineKind::Deletion
        } else {
            DiffLineKind::Context
        };
        lines.push(DiffLine {
            kind,
            content: line.to_string(),
        });
    }
    lines
}

// ── Git Blame Types ─────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameLine {
    pub commit_short: String,
    pub author: String,
    pub relative_time: String,
    pub line_number: usize,
    pub content: String,
    pub is_recent: bool,
}

pub struct GitBlameState {
    pub file_path: String,
    pub lines: Vec<BlameLine>,
    pub scroll_offset: usize,
    pub visible_height: usize,
    pub worktree_path: PathBuf,
    pub stale: bool,
}

impl GitBlameState {
    pub fn scroll_up(&mut self, n: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(n);
    }

    pub fn scroll_down(&mut self, n: usize) {
        let max_scroll = self.lines.len().saturating_sub(self.visible_height);
        self.scroll_offset = (self.scroll_offset + n).min(max_scroll);
    }

    pub fn mark_stale(&mut self) {
        self.stale = true;
    }

    pub fn ensure_fresh(&mut self) {
        if self.stale {
            self.refresh();
        }
    }

    pub fn refresh(&mut self) {
        self.stale = false;
        if let Ok(lines) = run_git_blame(&self.file_path, &self.worktree_path) {
            self.lines = lines;
            let max_scroll = self.lines.len().saturating_sub(self.visible_height);
            self.scroll_offset = self.scroll_offset.min(max_scroll);
        }
    }
}

pub fn run_git_blame(file: &str, root: &Path) -> Result<Vec<BlameLine>, String> {
    let output = Command::new("git")
        .args(["blame", "--porcelain", file])
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed to run git blame: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git blame error: {}", stderr.trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    parse_blame_porcelain(&text)
}

fn parse_blame_porcelain(text: &str) -> Result<Vec<BlameLine>, String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut lines = Vec::new();
    let mut current_hash = String::new();
    let mut current_author = String::new();
    let mut current_time: u64 = 0;
    let mut current_line_number: usize = 0;

    for line in text.lines() {
        if let Some(content) = line.strip_prefix('\t') {
            // Content line — emit BlameLine
            let elapsed = now.saturating_sub(current_time);
            let is_recent = elapsed < 7 * 24 * 3600;
            lines.push(BlameLine {
                commit_short: if current_hash.len() >= 8 {
                    current_hash[..8].to_string()
                } else {
                    current_hash.clone()
                },
                author: current_author.clone(),
                relative_time: format_relative_time(current_time, now),
                line_number: current_line_number,
                content: content.to_string(),
                is_recent,
            });
        } else if let Some(author) = line.strip_prefix("author ") {
            current_author = author.to_string();
        } else if let Some(time_str) = line.strip_prefix("author-time ") {
            current_time = time_str.parse().unwrap_or(0);
        } else {
            // Check if it's a commit header line: 40-char hash + line numbers
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3
                && parts[0].len() == 40
                && parts[0].chars().all(|c| c.is_ascii_hexdigit())
            {
                current_hash = parts[0].to_string();
                current_line_number = parts[2].parse().unwrap_or(0);
            }
        }
    }
    Ok(lines)
}

pub fn format_relative_time(epoch_secs: u64, now: u64) -> String {
    let elapsed = now.saturating_sub(epoch_secs);
    let minutes = elapsed / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    let weeks = days / 7;
    let months = days / 30;
    let years = days / 365;

    if years > 0 {
        format!("{} year{} ago", years, if years == 1 { "" } else { "s" })
    } else if months > 0 {
        format!("{} month{} ago", months, if months == 1 { "" } else { "s" })
    } else if weeks > 0 {
        format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
    } else if days > 0 {
        format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
    } else if hours > 0 {
        format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
    } else if minutes > 0 {
        format!("{} min{} ago", minutes, if minutes == 1 { "" } else { "s" })
    } else {
        "just now".to_string()
    }
}

// ── Git Log Types ───────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitLogEntry {
    pub hash_short: String,
    pub subject: String,
    pub author: String,
    pub relative_date: String,
}

pub struct GitLogState {
    pub entries: Vec<GitLogEntry>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub visible_height: usize,
    pub worktree_path: PathBuf,
    pub file_filter: Option<String>,
    pub stale: bool,
}

impl GitLogState {
    pub fn move_up(&mut self) {
        self.selected = wrapping_prev(self.selected, self.entries.len());
    }

    pub fn move_down(&mut self) {
        self.selected = wrapping_next(self.selected, self.entries.len());
    }

    pub fn selected_entry(&self) -> Option<&GitLogEntry> {
        self.entries.get(self.selected)
    }

    pub fn mark_stale(&mut self) {
        self.stale = true;
    }

    pub fn ensure_fresh(&mut self) {
        if self.stale {
            self.refresh();
        }
    }

    pub fn refresh(&mut self) {
        self.stale = false;
        if let Ok(entries) = run_git_log(&self.worktree_path, self.file_filter.as_deref()) {
            self.entries = entries;
            if !self.entries.is_empty() && self.selected >= self.entries.len() {
                self.selected = self.entries.len() - 1;
            }
        }
    }
}

pub fn run_git_log(root: &Path, file_filter: Option<&str>) -> Result<Vec<GitLogEntry>, String> {
    let mut args = vec!["log", "--format=%h%x00%s%x00%an%x00%cr", "-200"];
    let dashdash;
    if let Some(file) = file_filter {
        dashdash = file.to_string();
        args.push("--");
        args.push(&dashdash);
    }

    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed to run git log: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git log error: {}", stderr.trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.splitn(4, '\0').collect();
        if parts.len() == 4 {
            entries.push(GitLogEntry {
                hash_short: parts[0].to_string(),
                subject: parts[1].to_string(),
                author: parts[2].to_string(),
                relative_date: parts[3].to_string(),
            });
        }
    }
    Ok(entries)
}

pub fn run_git_show(hash: &str, root: &Path) -> Result<Vec<DiffLine>, String> {
    let output = Command::new("git")
        .args([
            "show",
            "--format=commit %H%nAuthor: %an%nDate:   %ci%n%n    %s%n",
            hash,
        ])
        .current_dir(root)
        .output()
        .map_err(|e| format!("Failed to run git show: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git show error: {}", stderr.trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    Ok(parse_diff_lines(&text))
}

// ── Activity Animation ──────────────────────────────────────

/// Tracks bouncing-block animation for worktrees with active PTY output.
/// Uses input suppression so typing echoes don't trigger it — any output
/// that arrives without recent user input (i.e. Claude working) activates.
#[derive(Default)]
pub struct ActivityAnimation {
    /// Whether each session had output this tick
    had_output: HashSet<String>,
    /// Last time output was confirmed without recent user input
    last_active: HashMap<String, Instant>,
    /// Last time user input was sent to each session's PTY
    last_input: HashMap<String, Instant>,
    /// Current animation frame per session (0..17 = 18-frame scanner cycle)
    frame: HashMap<String, usize>,
    /// Alternates each tick so frames advance every other tick (100ms)
    tick_parity: bool,
}

/// How long after last confirmed activity before animation stops (ms)
const ACTIVITY_TIMEOUT_MS: u128 = 500;

/// How long after user input to suppress animation (ms) — filters out echoes
const INPUT_SUPPRESSION_MS: u128 = 300;

/// Scanner cycle: 18 frames total
/// Forward 5 (pos 0→4), hold end 3, backward 4 (pos 3→0), hold start 6
const SCANNER_CYCLE_LEN: usize = 18;

/// Head position for each frame in the 18-frame scanner cycle.
const SCANNER_HEAD: [usize; SCANNER_CYCLE_LEN] = [
    0, 1, 2, 3, 4, // forward
    4, 4, 4, // hold at end
    3, 2, 1, 0, // backward
    0, 0, 0, 0, 0, 0, // hold at start
];

/// Direction at each frame: true = forward (trail behind), false = backward (trail ahead).
const SCANNER_FWD: [bool; SCANNER_CYCLE_LEN] = [
    true, true, true, true, true, true, true, true, false, false, false, false, false, false,
    false, false, false, false,
];

impl ActivityAnimation {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a PtyOutput event for a session.
    pub fn mark_active(&mut self, session_id: &str) {
        self.had_output.insert(session_id.to_string());
    }

    /// Record that user input was just sent to a session's PTY.
    /// Suppresses animation briefly to filter out echoed keystrokes.
    pub fn mark_input(&mut self, session_id: &str) {
        self.last_input
            .insert(session_id.to_string(), Instant::now());
    }

    /// Advance animation frames. Called on each Tick (50ms).
    pub fn tick(&mut self) {
        let now = Instant::now();

        // Expire old activity
        self.last_active
            .retain(|_, t| now.duration_since(*t).as_millis() < ACTIVITY_TIMEOUT_MS);

        // Advance frames for existing active sessions, remove expired
        let active_ids: HashSet<&String> = self.last_active.keys().collect();
        self.frame.retain(|id, _| active_ids.contains(id));
        // Advance every other tick (100ms per frame instead of 50ms)
        self.tick_parity = !self.tick_parity;
        if self.tick_parity {
            for (_, frame) in self.frame.iter_mut() {
                *frame = (*frame + 1) % SCANNER_CYCLE_LEN;
            }
        }

        // Process sessions that had output this tick
        for id in self.had_output.drain() {
            // Check if user recently typed into this session
            let suppressed = self
                .last_input
                .get(&id)
                .map(|t| now.duration_since(*t).as_millis() < INPUT_SUPPRESSION_MS)
                .unwrap_or(false);

            if !suppressed {
                self.last_active.insert(id.clone(), now);
                self.frame.entry(id).or_insert(0);
            }
        }

        // Clean up stale input timestamps
        self.last_input
            .retain(|_, t| now.duration_since(*t).as_millis() < INPUT_SUPPRESSION_MS * 2);
    }

    /// Whether this session has an active animation.
    pub fn is_active(&self, session_id: &str) -> bool {
        self.last_active.contains_key(session_id)
    }

    /// Current bounce position (0..4) — kept for compatibility, prefer `trail()`.
    pub fn position(&self, session_id: &str) -> usize {
        self.frame
            .get(session_id)
            .map(|&f| SCANNER_HEAD[f])
            .unwrap_or(0)
    }

    /// Brightness levels (0-3) for each of the 5 scanner positions.
    /// 3 = head, 2 = trail-1, 1 = trail-2, 0 = inactive.
    pub fn trail(&self, session_id: &str) -> [u8; 5] {
        let f = self.frame.get(session_id).copied().unwrap_or(0);
        let head = SCANNER_HEAD[f];
        let fwd = SCANNER_FWD[f];

        // During hold frames, compute how many hold ticks have elapsed
        // to fade the trail out progressively.
        let is_hold_end = (5..=7).contains(&f);
        let is_hold_start = (12..=17).contains(&f);
        let hold_elapsed = if is_hold_end {
            f - 5 // 0, 1, 2
        } else if is_hold_start {
            f - 12 // 0, 1, 2, 3, 4, 5
        } else {
            0
        };

        let mut levels = [0u8; 5];
        levels[head] = 3;

        // Trail goes opposite to direction of motion
        let trail_dir: isize = if fwd { -1 } else { 1 };

        // Place trail segments, fading during holds
        for step in 1..=2u8 {
            let pos = head as isize + trail_dir * step as isize;
            if (0..5).contains(&pos) {
                let base_brightness = 3 - step; // 2 for step 1, 1 for step 2
                let brightness = base_brightness.saturating_sub(hold_elapsed as u8);
                if brightness > 0 {
                    levels[pos as usize] = brightness;
                }
            }
        }

        levels
    }

    /// Clean up state when a session is removed entirely.
    pub fn remove_session(&mut self, session_id: &str) {
        self.had_output.remove(session_id);
        self.last_active.remove(session_id);
        self.last_input.remove(session_id);
        self.frame.remove(session_id);
    }
}

/// Returns `false` for key codes that edtui's `From<crossterm::event::KeyCode>`
/// doesn't handle (it calls `unimplemented!()` for these, causing a panic).
/// Also rejects Tab+SHIFT, which Kitty keyboard enhancement sends instead of BackTab.
pub fn is_edtui_compatible(key: &KeyEvent) -> bool {
    // Kitty sends Tab+SHIFT instead of BackTab — reject it the same way.
    if key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT) {
        return false;
    }
    matches!(
        key.code,
        KeyCode::Char(_)
            | KeyCode::Enter
            | KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::Tab
            | KeyCode::Esc
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::F(1..=12)
    )
}

/// Mouse text selection state for click-drag copy.
#[derive(Debug, Clone)]
pub struct TextSelection {
    pub session_id: String,
    /// Inner rect of the pane (for rendering highlight).
    pub pane_inner: Rect,
    /// Start position in vt100 screen coords (row, col).
    pub start: (u16, u16),
    /// End position in vt100 screen coords (row, col).
    pub end: (u16, u16),
    /// Whether a drag is in progress.
    pub active: bool,
}

pub struct App {
    pub running: bool,
    pub input_mode: InputMode,
    pub panel_focus: PanelFocus,
    pub sidebar_view: SidebarView,
    pub main_view: MainView,
    pub keybindings: KeybindingsConfig,
    /// Hierarchical sidebar tree (sections → items → session slots)
    pub sidebar_tree: SidebarTree,
    /// Active prompt overlay (if any)
    pub prompt: Option<Prompt>,
    /// Sessions that have received a bell (needs attention)
    pub attention_sessions: HashSet<String>,
    /// Sessions whose process has exited
    pub exited_sessions: HashSet<String>,
    /// Window title status text from PTY sessions (set via OSC 0/2)
    pub session_statuses: HashMap<String, String>,
    /// Status message to show briefly
    pub status_message: Option<String>,
    pub show_help: bool,
    pub theme: Theme,
    pub terminal_start_bottom: bool,
    /// Per-session scrollback offset (lines scrolled back from live view)
    pub scroll_offsets: HashMap<String, usize>,
    /// Sessions where the user has intentionally scrolled back (suppress auto-scroll)
    pub user_scrolled: HashSet<String>,
    /// Height of the terminal panel area, used for page-scroll sizing
    pub terminal_height: u16,
    pub file_explorer: FileExplorerState,
    pub editor: Option<EditorViewState>,
    pub search: Option<SearchViewState>,
    pub fuzzy_finder: Option<FuzzyFinderState>,
    pub command_palette: Option<CommandPaletteState>,
    pub branch_switcher: Option<BranchSwitcherState>,
    pub git_status: Option<GitStatusState>,
    pub diff_view: Option<DiffViewState>,
    pub git_blame: Option<GitBlameState>,
    pub git_log: Option<GitLogState>,
    pub activity: ActivityAnimation,
    pub session_command: String,
    pub pane_layout: Option<PaneLayout>,
    /// Split picker overlay state.
    pub split_picker: Option<SplitPickerState>,
    /// Shell command to run (default: $SHELL)
    pub shell_command: String,
    /// Directory browser modal for section creation
    pub dir_browser: Option<DirBrowser>,
    /// Pending section refresh after creation: (section_idx, root_path)
    pub pending_section_refresh: Option<(usize, PathBuf)>,
    /// Session IDs that need cleanup in session_manager (after section deletion)
    pub pending_removed_sessions: Vec<String>,
    /// Pending layout to restore (loaded from layout.toml on startup)
    pub pending_layout: Option<config::LayoutConfig>,
    /// Set to true when user approves session restore via prompt
    pub restore_approved: bool,
    /// Sidebar width as a percentage (default 25).
    pub sidebar_width: u16,
    /// Set when sidebar was resized and PTY needs a resize.
    pub sidebar_resized: bool,
    /// Active mouse text selection (click-drag to copy).
    pub text_selection: Option<TextSelection>,
    /// User preference for split direction (used when creating new splits).
    pub split_direction: SplitDirection,
    /// Set when layout direction changed and PTY sessions need resizing.
    pub layout_changed: bool,
    /// Per-worktree notepad state.
    pub note: Option<NoteViewState>,
    /// Where the notes panel is displayed.
    pub note_position: NotePosition,
    /// Active planet kind (for theme + sidebar animation).
    pub planet_kind: Option<PlanetKind>,
    /// Whether to show the planet animation in the sidebar.
    pub show_planet: bool,
    /// Loaded planet animation frames.
    pub planet_animation: Option<PlanetAnimation>,
    /// Animation tick counter for the sidebar planet.
    pub planet_tick: usize,
}

impl App {
    pub fn new(
        worktrees: Vec<Worktree>,
        theme: Theme,
        terminal_start_bottom: bool,
        keybindings: KeybindingsConfig,
        session_command: String,
        shell_command: String,
    ) -> Self {
        let explorer_root = worktrees
            .first()
            .map(|wt| wt.path.clone())
            .unwrap_or_else(|| PathBuf::from("."));
        let sidebar_tree = SidebarTree::from_worktrees(&worktrees);
        Self {
            running: true,
            input_mode: InputMode::Navigation,
            panel_focus: PanelFocus::Left,
            sidebar_view: SidebarView::Worktrees,
            main_view: MainView::Terminal,
            keybindings,
            sidebar_tree,
            attention_sessions: HashSet::new(),
            exited_sessions: HashSet::new(),
            session_statuses: HashMap::new(),
            prompt: None,
            status_message: None,
            show_help: false,
            theme,
            terminal_start_bottom,
            scroll_offsets: HashMap::new(),
            user_scrolled: HashSet::new(),
            terminal_height: 24,
            file_explorer: FileExplorerState::new(explorer_root),
            editor: None,
            search: None,
            fuzzy_finder: None,
            command_palette: None,
            branch_switcher: None,
            git_status: None,
            diff_view: None,
            git_blame: None,
            git_log: None,
            activity: ActivityAnimation::new(),
            session_command,
            pane_layout: None,
            split_picker: None,
            shell_command,
            dir_browser: None,
            pending_section_refresh: None,
            pending_removed_sessions: Vec::new(),
            pending_layout: None,
            restore_approved: false,
            sidebar_width: 25,
            sidebar_resized: false,
            text_selection: None,
            split_direction: SplitDirection::Horizontal,
            layout_changed: false,
            note: None,
            note_position: NotePosition::Sidebar,
            planet_kind: None,
            show_planet: true,
            planet_animation: None,
            planet_tick: 0,
        }
    }

    // ── Backward-compat accessors ──

    /// Get the path of the currently selected item.
    pub fn selected_worktree_path(&self) -> Option<&PathBuf> {
        self.sidebar_tree.selected_item().map(|item| &item.path)
    }

    /// Get the active session ID of the given kind (derived from sidebar tree cursor).
    fn active_session_id_of_kind(&self, kind: SessionKind) -> Option<&str> {
        use crate::sidebar::tree::TreeNode;
        if let Some(TreeNode::Session(si, ii, slot_idx)) =
            self.sidebar_tree.visible.get(self.sidebar_tree.cursor)
        {
            let slot = self
                .sidebar_tree
                .sections
                .get(*si)?
                .items
                .get(*ii)?
                .sessions
                .get(*slot_idx)?;
            return if slot.kind == kind {
                slot.session_id.as_deref()
            } else {
                None
            };
        }
        let item = self.sidebar_tree.selected_item()?;
        item.sessions
            .iter()
            .find(|s| s.kind == kind)
            .and_then(|s| s.session_id.as_deref())
    }

    /// Get the currently active Claude session ID.
    pub fn active_session_id(&self) -> Option<&str> {
        self.active_session_id_of_kind(SessionKind::Claude)
    }

    /// Get the currently active shell session ID.
    pub fn active_shell_session_id(&self) -> Option<&str> {
        self.active_session_id_of_kind(SessionKind::Shell)
    }

    /// Notes column percentage when center column is visible, None otherwise.
    pub fn notes_pct(&self) -> Option<u16> {
        None
    }

    pub fn focused_view(&self) -> ViewKind {
        match self.panel_focus {
            PanelFocus::Left => self.sidebar_view.to_view_kind(),
            PanelFocus::Center => ViewKind::Notes,
            PanelFocus::Right => {
                // In split mode, derive from focused pane content
                if let Some(ref layout) = self.pane_layout {
                    if let Some(content) = layout.root.leaf_at(layout.focused) {
                        return content.to_view_kind();
                    }
                }
                self.main_view.to_view_kind()
            }
        }
    }

    pub fn set_sidebar_view(&mut self, view: SidebarView) {
        if matches!(view, SidebarView::GitStatus) && self.git_status.is_none() {
            let path = self
                .sidebar_tree
                .selected_path()
                .cloned()
                .unwrap_or_else(|| self.file_explorer.root.clone());
            self.git_status = Some(GitStatusState::new(path));
        }
        self.sidebar_view = view;
        self.panel_focus = PanelFocus::Left;
    }

    pub fn set_main_view(&mut self, view: MainView) {
        self.main_view = view;
        self.panel_focus = PanelFocus::Right;
    }

    /// Set main panel to Terminal and focus it.
    pub fn focus_terminal_panel(&mut self) {
        self.main_view = MainView::Terminal;
        self.panel_focus = PanelFocus::Right;
    }

    /// Set main panel to Editor (don't change focus — caller is in sidebar).
    pub fn open_editor_in_main_panel(&mut self) {
        self.main_view = MainView::Editor;
    }

    pub fn toggle_focus(&mut self) {
        self.panel_focus = match self.panel_focus {
            PanelFocus::Left | PanelFocus::Center => {
                // Reset pane focus to first pane when coming from sidebar
                if let Some(ref mut layout) = self.pane_layout {
                    layout.focused = 0;
                }
                PanelFocus::Right
            }
            PanelFocus::Right => PanelFocus::Left,
        };
    }

    /// Toggle notes: Hidden → Sidebar+edit, editing → save+read-only, read-only → edit.
    pub fn toggle_notes(&mut self) {
        match self.note_position {
            NotePosition::Hidden => {
                // Show notes in sidebar and enter edit mode
                self.note_position = NotePosition::Sidebar;
                if let Some(ref mut note) = self.note {
                    note.read_only = false;
                    note.editor_state.mode = EditorMode::Insert;
                }
                self.panel_focus = PanelFocus::Left;
                self.input_mode = InputMode::Editor;
            }
            NotePosition::Sidebar => {
                if let Some(ref mut note) = self.note {
                    if note.read_only {
                        // Read-only → enter edit mode
                        note.read_only = false;
                        note.editor_state.mode = EditorMode::Insert;
                        self.panel_focus = PanelFocus::Left;
                        self.input_mode = InputMode::Editor;
                    } else {
                        // Editing → save and go read-only
                        if note.modified {
                            let _ = note.save();
                        }
                        note.read_only = true;
                        note.editor_state.mode = EditorMode::Normal;
                        self.input_mode = InputMode::Navigation;
                    }
                }
            }
        }
        // Trigger PTY resize since layout changed
        self.sidebar_resized = true;
    }

    /// Load (or reload) the note for the currently selected worktree.
    pub fn load_note_for_current_worktree(&mut self) {
        if let Some(path) = self.sidebar_tree.selected_path().cloned() {
            // Auto-save previous note if modified
            if let Some(ref mut note) = self.note {
                if note.modified {
                    let _ = note.save();
                }
            }
            self.note = Some(NoteViewState::open_or_create(&path));
        }
    }

    /// If we just focused the main panel showing Terminal/Shell with an active non-exited session, enter terminal mode.
    /// For Editor panes in split mode, stays in Navigation.
    fn enter_terminal_if_focused(&mut self) {
        if self.panel_focus != PanelFocus::Right {
            return;
        }
        // In split mode, check focused pane content type
        if let Some(ref layout) = self.pane_layout {
            match layout.root.leaf_at(layout.focused) {
                Some(PaneContent::Terminal(_)) | Some(PaneContent::Shell(_)) => {
                    if let Some(id) = self.focused_session_id().cloned() {
                        self.attention_sessions.remove(&id);
                        if !self.exited_sessions.contains(&id) {
                            self.input_mode = InputMode::Terminal;
                            self.reset_scroll();
                        }
                    }
                }
                _ => {} // Editor pane or empty — stay in Navigation
            }
            return;
        }
        // No split: check main_view
        if self.main_view == MainView::Terminal || self.main_view == MainView::Shell {
            if let Some(id) = self.focused_session_id().cloned() {
                self.attention_sessions.remove(&id);
                if !self.exited_sessions.contains(&id) {
                    self.input_mode = InputMode::Terminal;
                    self.reset_scroll();
                }
            }
        }
    }

    pub fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(*key),
            AppEvent::Resize(_w, _h) => {}
            AppEvent::PtyOutput { session_id } => {
                self.activity.mark_active(session_id);
            }
            AppEvent::SessionBell { .. } | AppEvent::SessionDone { .. } => {
                // Attention is handled via debounce in the main event loop.
                // We don't set attention_sessions immediately here because
                // SessionBell/SessionDone fire on every tool call completion,
                // not just when the session is truly idle/waiting for input.
            }
            AppEvent::SessionExited { session_id } => {
                self.exited_sessions.insert(session_id.clone());
                self.activity.remove_session(session_id);
                // If user is in terminal mode on this session, kick to nav mode
                if self.focused_session_id().map(|s| s.as_str()) == Some(session_id)
                    && self.input_mode == InputMode::Terminal
                {
                    self.input_mode = InputMode::Navigation;
                }
            }
            AppEvent::FileChanged { paths } => {
                if let Some(ref mut editor) = self.editor {
                    if paths.iter().any(|p| p == &editor.file_path) {
                        if editor.modified {
                            self.status_message =
                                Some("File changed on disk (unsaved edits preserved)".to_string());
                        } else {
                            match editor.reload() {
                                Ok(true) => {
                                    self.status_message = Some("File reloaded".to_string());
                                }
                                Ok(false) => {} // identical content, no message
                                Err(e) => {
                                    self.status_message = Some(format!("Reload error: {}", e));
                                }
                            }
                        }
                    }
                }
                self.file_explorer.git_indicators_stale = true;
                if let Some(ref mut gs) = self.git_status {
                    gs.mark_stale();
                }
                if let Some(ref mut gl) = self.git_log {
                    gl.mark_stale();
                }
                if let Some(ref mut gb) = self.git_blame {
                    gb.mark_stale();
                }
            }
            AppEvent::FilesCreatedOrDeleted => {
                self.file_explorer.refresh();
                self.file_explorer.git_indicators_stale = true;
                if let Some(ref mut gs) = self.git_status {
                    gs.mark_stale();
                }
                if let Some(ref mut gl) = self.git_log {
                    gl.mark_stale();
                }
                if let Some(ref mut gb) = self.git_blame {
                    gb.mark_stale();
                }
            }
            AppEvent::MouseScroll { .. } => {
                // Handled in process_event() before reaching here
            }
            AppEvent::Tick => {
                self.activity.tick();
                if self.planet_animation.is_some() {
                    self.planet_tick = self.planet_tick.wrapping_add(1);
                }
            }
            AppEvent::Paste(_) => {
                // Handled in main.rs process_event() before reaching here
            }
            AppEvent::MouseDown { .. } | AppEvent::MouseDrag { .. } | AppEvent::MouseUp { .. } => {
                // Handled in main.rs process_event() before reaching here
            }
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+C is handled entirely in the main event loop
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return;
        }

        // Ctrl+Q quits from any mode
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q') {
            self.running = false;
            return;
        }

        // Fuzzy finder, project search, command palette, branch switcher keybindings are handled specially
        if KeybindingsConfig::matches(&self.keybindings.fuzzy_finder, key.modifiers, key.code)
            || KeybindingsConfig::matches(&self.keybindings.project_search, key.modifiers, key.code)
            || KeybindingsConfig::matches(
                &self.keybindings.command_palette,
                key.modifiers,
                key.code,
            )
            || KeybindingsConfig::matches(
                &self.keybindings.branch_switcher,
                key.modifiers,
                key.code,
            )
        {
            return;
        }

        // Fuzzy finder gets exclusive keyboard focus
        if self.fuzzy_finder.is_some() {
            self.handle_fuzzy_finder_key(key);
            return;
        }

        // Split picker gets exclusive keyboard focus
        if self.split_picker.is_some() {
            self.handle_split_picker_key(key);
            return;
        }

        // Branch switcher gets exclusive keyboard focus
        if self.branch_switcher.is_some() {
            self.handle_branch_switcher_key(key);
            return;
        }

        // Command palette gets exclusive keyboard focus
        if self.command_palette.is_some() {
            self.handle_command_palette_key(key);
            return;
        }

        // Directory browser gets exclusive keyboard focus
        if self.dir_browser.is_some() {
            self.handle_dir_browser_key(key);
            return;
        }

        // Handle prompt input
        if self.prompt.is_some() {
            self.handle_prompt_key(key);
            return;
        }

        // Notes toggle (works from any mode)
        if KeybindingsConfig::matches(&self.keybindings.notes_toggle, key.modifiers, key.code) {
            self.toggle_notes();
            return;
        }

        match self.input_mode {
            InputMode::Navigation => self.handle_nav_key(key),
            InputMode::Terminal => self.handle_terminal_key(key),
            InputMode::Editor => {
                // Handle notes editing (inline in sidebar or center)
                if (self.panel_focus == PanelFocus::Left || self.panel_focus == PanelFocus::Center)
                    && self.note.as_ref().map_or(false, |n| !n.read_only)
                {
                    self.handle_notes_editor_key(key);
                } else {
                    self.handle_editor_key(key);
                }
            }
        }
    }

    fn handle_prompt_key(&mut self, key: KeyEvent) {
        let prompt = self.prompt.as_mut().unwrap();
        match prompt {
            Prompt::CreateWorktree { input } => match key.code {
                KeyCode::Enter => {
                    // Signal handled in main loop via wants_create_worktree()
                }
                KeyCode::Esc => {
                    self.prompt = None;
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) => {
                    input.push(c);
                }
                _ => {}
            },
            Prompt::ConfirmDelete { .. } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    // Signal handled in main loop via wants_delete_worktree()
                }
                _ => {
                    self.prompt = None;
                }
            },
            Prompt::SearchInput { input } => match key.code {
                KeyCode::Enter => {
                    if !input.is_empty() {
                        let query = input.clone();
                        let root = self.file_explorer.root.clone();
                        let search_state = SearchViewState::new(&query, &root);
                        self.search = Some(search_state);
                        self.prompt = None;
                        self.sidebar_view = SidebarView::Search;
                        self.panel_focus = PanelFocus::Left;
                    }
                }
                KeyCode::Esc => {
                    self.prompt = None;
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) => {
                    input.push(c);
                }
                _ => {}
            },
            Prompt::AddShellSlot { input } => match key.code {
                KeyCode::Enter => {
                    if !input.is_empty() {
                        let label = input.clone();
                        self.sidebar_tree
                            .add_session_slot(SessionKind::Shell, label.clone());
                        self.prompt = None;
                        self.status_message = Some(format!("Added shell slot '{}'", label));
                    }
                }
                KeyCode::Esc => {
                    self.prompt = None;
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char(c) => {
                    input.push(c);
                }
                _ => {}
            },
            Prompt::ConfirmDeleteSection {
                section_idx,
                section_name,
            } => match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    let section_idx = *section_idx;
                    let section_name = section_name.clone();
                    let session_ids = self.sidebar_tree.remove_section(section_idx);
                    for sid in &session_ids {
                        self.attention_sessions.remove(sid);
                        self.exited_sessions.remove(sid);
                        self.session_statuses.remove(sid);
                        self.activity.remove_session(sid);
                        self.remove_session_from_panes(sid);
                    }
                    self.prompt = None;
                    self.status_message = Some(format!("Deleted section '{}'", section_name));
                    self.pending_removed_sessions = session_ids;
                }
                _ => {
                    self.prompt = None;
                }
            },
            Prompt::SetupGuide => match key.code {
                KeyCode::Enter | KeyCode::Esc => {
                    config::mark_setup_done();
                    self.prompt = None;
                }
                _ => {}
            },
            Prompt::RestoreSession { .. } => match key.code {
                KeyCode::Char('y') | KeyCode::Enter => {
                    self.restore_approved = true;
                    self.prompt = None;
                }
                KeyCode::Char('n') | KeyCode::Esc => {
                    self.pending_layout = None;
                    self.prompt = None;
                }
                _ => {}
            },
            Prompt::ThemePicker {
                selected,
                previous_theme,
            } => match key.code {
                KeyCode::Left | KeyCode::Right => {
                    let len = PlanetKind::all().len();
                    *selected = if key.code == KeyCode::Left {
                        wrapping_prev(*selected, len)
                    } else {
                        wrapping_next(*selected, len)
                    };
                    let planet = PlanetKind::all()[*selected];
                    let is_dark = self.theme.mode == config::ThemeMode::Dark;
                    self.theme = if is_dark {
                        planet.dark_theme()
                    } else {
                        planet.light_theme()
                    };
                    self.planet_animation = Some(PlanetAnimation::load(planet));
                }
                KeyCode::Char('d') => {
                    let planet = PlanetKind::all()[*selected];
                    self.theme = planet.dark_theme();
                }
                KeyCode::Char('l') => {
                    let planet = PlanetKind::all()[*selected];
                    self.theme = planet.light_theme();
                }
                KeyCode::Enter => {
                    let planet = PlanetKind::all()[*selected];
                    self.planet_kind = Some(planet);
                    // planet_animation is already loaded from browsing
                    config::save_planet_choice(planet, self.theme.mode);
                    if !config::setup_done() {
                        self.prompt = Some(Prompt::SetupGuide);
                    } else {
                        self.prompt = None;
                    }
                    self.status_message = Some(format!("Theme set to {}", planet.display_name()));
                }
                KeyCode::Esc => {
                    let prev = previous_theme.clone();
                    self.theme = prev;
                    match self.planet_kind {
                        None => self.planet_animation = None,
                        Some(kind) => self.planet_animation = Some(PlanetAnimation::load(kind)),
                    }
                    self.prompt = None;
                }
                _ => {}
            },
            Prompt::ColorPicker { target, cursor } => match key.code {
                KeyCode::Left | KeyCode::Char('h') => {
                    if *cursor > 0 {
                        *cursor -= 1;
                    }
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    if *cursor + 1 < PRESET_COLORS.len() {
                        *cursor += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    // Grid is 7 columns
                    if *cursor >= 7 {
                        *cursor -= 7;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if *cursor + 7 < PRESET_COLORS.len() {
                        *cursor += 7;
                    }
                }
                KeyCode::Enter => {
                    let color = PRESET_COLORS[*cursor];
                    let target = *target;
                    self.prompt = None;
                    match target {
                        ColorTarget::Section(si) => {
                            if let Some(section) = self.sidebar_tree.sections.get_mut(si) {
                                section.color = color;
                            }
                        }
                        ColorTarget::Item(si, ii) => {
                            if let Some(item) = self
                                .sidebar_tree
                                .sections
                                .get_mut(si)
                                .and_then(|s| s.items.get_mut(ii))
                            {
                                item.color = color;
                            }
                        }
                        ColorTarget::Session(si, ii, slot) => {
                            if let Some(s) = self
                                .sidebar_tree
                                .sections
                                .get_mut(si)
                                .and_then(|s| s.items.get_mut(ii))
                                .and_then(|item| item.sessions.get_mut(slot))
                            {
                                s.color = color;
                            }
                        }
                    }
                    self.status_message = Some(if color.is_some() {
                        "Color assigned".to_string()
                    } else {
                        "Color cleared".to_string()
                    });
                }
                KeyCode::Esc => {
                    self.prompt = None;
                }
                _ => {}
            },
        }
    }

    fn handle_dir_browser_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.dir_browser = None;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut browser) = self.dir_browser {
                    browser.move_down();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut browser) = self.dir_browser {
                    browser.move_up();
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if let Some(ref mut browser) = self.dir_browser {
                    browser.expand();
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                if let Some(ref mut browser) = self.dir_browser {
                    browser.collapse();
                }
            }
            KeyCode::Enter => {
                if let Some(ref browser) = self.dir_browser {
                    if let Some(path) = browser.selected_path().cloned() {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "section".to_string());
                        self.sidebar_tree
                            .add_section(name.clone(), Some(path.clone()));
                        let section_idx = self.sidebar_tree.sections.len() - 1;
                        self.pending_section_refresh = Some((section_idx, path));
                        self.dir_browser = None;
                        self.status_message = Some(format!("Created section '{}'", name));
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_fuzzy_finder_key(&mut self, key: KeyEvent) {
        // Ignore Ctrl+key combos (prevents triggering Ctrl+P from typing 'p')
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.fuzzy_finder = None;
            }
            KeyCode::Enter => {
                if let Some(ref finder) = self.fuzzy_finder {
                    if let Some(path) = finder.selected_path().cloned() {
                        self.fuzzy_finder = None;
                        match EditorViewState::open(path) {
                            Ok(state) => {
                                let name = state.file_name().to_string();
                                self.editor = Some(state);
                                self.main_view = MainView::Editor;
                                self.panel_focus = PanelFocus::Right;
                                self.status_message = Some(format!("Opened {}", name));
                            }
                            Err(e) => {
                                self.status_message = Some(e);
                            }
                        }
                    }
                }
            }
            KeyCode::Up => {
                if let Some(ref mut finder) = self.fuzzy_finder {
                    finder.move_up();
                }
            }
            KeyCode::Down => {
                if let Some(ref mut finder) = self.fuzzy_finder {
                    finder.move_down();
                }
            }
            KeyCode::Backspace => {
                if let Some(ref mut finder) = self.fuzzy_finder {
                    finder.input.pop();
                    finder.update_matches();
                }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut finder) = self.fuzzy_finder {
                    finder.input.push(c);
                    finder.update_matches();
                }
            }
            _ => {}
        }
    }

    fn handle_command_palette_key(&mut self, key: KeyEvent) {
        // Ignore Ctrl+key combos (same pattern as fuzzy finder)
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.command_palette = None;
            }
            KeyCode::Enter => {
                if let Some(ref palette) = self.command_palette {
                    if let Some(id) = palette.selected_command() {
                        self.command_palette = None;
                        self.execute_command(id);
                    }
                }
            }
            KeyCode::Up => {
                if let Some(ref mut palette) = self.command_palette {
                    palette.move_up();
                }
            }
            KeyCode::Down => {
                if let Some(ref mut palette) = self.command_palette {
                    palette.move_down();
                }
            }
            KeyCode::Backspace => {
                if let Some(ref mut palette) = self.command_palette {
                    palette.input.pop();
                    palette.update_matches();
                }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut palette) = self.command_palette {
                    palette.input.push(c);
                    palette.update_matches();
                }
            }
            _ => {}
        }
    }

    pub fn execute_command(&mut self, id: CommandId) {
        match id {
            CommandId::ViewWorktrees => self.set_sidebar_view(SidebarView::Worktrees),
            CommandId::ViewTerminal => self.set_main_view(MainView::Terminal),
            CommandId::ViewFiles => self.set_sidebar_view(SidebarView::FileExplorer),
            CommandId::ViewEditor => self.set_main_view(MainView::Editor),
            CommandId::ViewSearch => self.set_sidebar_view(SidebarView::Search),
            CommandId::ViewGitStatus => {
                if let Some(path) = self.sidebar_tree.selected_path().cloned() {
                    self.git_status = Some(GitStatusState::new(path));
                }
                self.set_sidebar_view(SidebarView::GitStatus);
            }
            CommandId::ViewGitBlame => self.open_git_blame(),
            CommandId::ViewGitLog => self.open_git_log(),
            CommandId::ViewShell => self.set_main_view(MainView::Shell),
            CommandId::FuzzyFinder => {
                let root = self.file_explorer.root.clone();
                self.fuzzy_finder = Some(FuzzyFinderState::new(root));
                self.input_mode = InputMode::Navigation;
            }
            CommandId::ProjectSearch => {
                self.prompt = Some(Prompt::SearchInput {
                    input: String::new(),
                });
                self.input_mode = InputMode::Navigation;
            }
            CommandId::RefreshGitStatus => {
                if let Some(path) = self.sidebar_tree.selected_path().cloned() {
                    self.git_status = Some(GitStatusState::new(path));
                    self.status_message = Some("Git status refreshed".to_string());
                }
            }
            CommandId::SplitPane => {
                self.open_split_picker(self.split_direction);
            }
            CommandId::SplitTerminal => {
                if let Some(id) = self.find_next_session_of_kind(SessionKind::Claude) {
                    self.split_add_pane_with(PaneContent::Terminal(id));
                } else if let Some(id) = self.active_session_id().map(|s| s.to_string()) {
                    self.split_add_pane_with(PaneContent::Terminal(id));
                } else {
                    self.status_message = Some("No terminal sessions available".to_string());
                }
            }
            CommandId::SplitShell => {
                if let Some(id) = self.find_next_session_of_kind(SessionKind::Shell) {
                    self.split_add_pane_with(PaneContent::Shell(id));
                } else if let Some(id) = self.active_shell_session_id().map(|s| s.to_string()) {
                    self.split_add_pane_with(PaneContent::Shell(id));
                } else {
                    self.status_message = Some("No shell sessions available".to_string());
                }
            }
            CommandId::SplitEditor => {
                self.split_add_pane_with(PaneContent::Editor);
            }
            CommandId::ClosePane => {
                self.close_focused_pane();
            }
            CommandId::ToggleHelp => {
                self.show_help = !self.show_help;
            }
            CommandId::Quit => {
                self.running = false;
            }
            CommandId::StartSession => {
                self.status_message =
                    Some("Use Enter in worktree list to start a session".to_string());
            }
            CommandId::RestartSession => {
                self.status_message = Some(
                    "Use 'r' to restart exited session, Shift+R to force-restart any session"
                        .to_string(),
                );
            }
            CommandId::CloseSession => {
                self.status_message = Some("Use Ctrl+C to close the active session".to_string());
            }
            CommandId::AddSection => {
                let home = std::env::var("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("/"));
                self.dir_browser = Some(DirBrowser::new(home));
            }
            CommandId::AddShellSlot => {
                self.prompt = Some(Prompt::AddShellSlot {
                    input: String::new(),
                });
            }
            CommandId::AssignColor => {
                use crate::sidebar::tree::TreeNode;
                if let Some(&node) = self.sidebar_tree.selected_node() {
                    let target = match node {
                        TreeNode::Section(si) => ColorTarget::Section(si),
                        TreeNode::Item(si, ii) => ColorTarget::Item(si, ii),
                        TreeNode::Session(si, ii, slot) => ColorTarget::Session(si, ii, slot),
                    };
                    self.prompt = Some(Prompt::ColorPicker { target, cursor: 0 });
                }
            }
            CommandId::SidebarGrow => {
                self.sidebar_width = (self.sidebar_width + SIDEBAR_STEP).min(SIDEBAR_MAX_WIDTH);
                self.sidebar_resized = true;
            }
            CommandId::SidebarShrink => {
                self.sidebar_width = self
                    .sidebar_width
                    .saturating_sub(SIDEBAR_STEP)
                    .max(SIDEBAR_MIN_WIDTH);
                self.sidebar_resized = true;
            }
            CommandId::SplitPaneVertical => {
                self.open_split_picker(SplitDirection::Vertical);
            }
            CommandId::ToggleSplitDirection => {
                self.toggle_split_direction();
            }
            CommandId::ThemePicker => {
                self.open_theme_picker();
            }
            CommandId::TogglePlanet => {
                self.show_planet = !self.show_planet;
                self.status_message = Some(if self.show_planet {
                    "Planet display enabled".to_string()
                } else {
                    "Planet display hidden".to_string()
                });
            }
            CommandId::BranchSwitcher => {
                if let Some(path) = self.sidebar_tree.selected_path().cloned() {
                    match BranchSwitcherState::new(path) {
                        Ok(state) => {
                            self.branch_switcher = Some(state);
                        }
                        Err(e) => {
                            self.status_message = Some(format!("Error: {}", e));
                        }
                    }
                }
            }
        }
    }

    /// Open the theme picker overlay.
    pub fn open_theme_picker(&mut self) {
        let selected = self
            .planet_kind
            .and_then(|k| PlanetKind::all().iter().position(|p| *p == k))
            .unwrap_or(0);
        // Load the initially selected planet's animation for preview
        let planet = PlanetKind::all()[selected];
        self.planet_animation = Some(PlanetAnimation::load(planet));
        self.planet_tick = 0;
        self.prompt = Some(Prompt::ThemePicker {
            selected,
            previous_theme: self.theme.clone(),
        });
    }

    /// Toggle the split direction between Horizontal and Vertical.
    /// Updates both the user preference and any existing layout.
    pub fn toggle_split_direction(&mut self) {
        self.split_direction = match self.split_direction {
            SplitDirection::Horizontal => SplitDirection::Vertical,
            SplitDirection::Vertical => SplitDirection::Horizontal,
        };
        // Toggle the root split direction if a layout exists
        if let Some(ref mut layout) = self.pane_layout {
            if let SplitNode::Split {
                ref mut direction, ..
            } = layout.root
            {
                *direction = self.split_direction;
            }
            self.layout_changed = true;
        }
        let dir_name = match self.split_direction {
            SplitDirection::Horizontal => "horizontal",
            SplitDirection::Vertical => "vertical",
        };
        self.status_message = Some(format!("Split direction: {}", dir_name));
    }

    fn handle_nav_key(&mut self, key: KeyEvent) {
        // Sidebar resize keybindings (global in nav mode)
        let kb = &self.keybindings;
        if KeybindingsConfig::matches(&kb.sidebar_grow, key.modifiers, key.code) {
            self.sidebar_width = (self.sidebar_width + SIDEBAR_STEP).min(SIDEBAR_MAX_WIDTH);
            self.sidebar_resized = true;
            return;
        }
        if KeybindingsConfig::matches(&kb.sidebar_shrink, key.modifiers, key.code) {
            self.sidebar_width = self
                .sidebar_width
                .saturating_sub(SIDEBAR_STEP)
                .max(SIDEBAR_MIN_WIDTH);
            self.sidebar_resized = true;
            return;
        }

        // Panel-aware view switching via configurable keybindings
        if KeybindingsConfig::matches(&kb.worktrees, key.modifiers, key.code) {
            self.set_sidebar_view(SidebarView::Worktrees);
            return;
        }
        if KeybindingsConfig::matches(&kb.terminal, key.modifiers, key.code) {
            self.set_main_view(MainView::Terminal);
            return;
        }
        if KeybindingsConfig::matches(&kb.files, key.modifiers, key.code) {
            self.set_sidebar_view(SidebarView::FileExplorer);
            return;
        }
        if KeybindingsConfig::matches(&kb.editor, key.modifiers, key.code) {
            self.set_main_view(MainView::Editor);
            return;
        }
        if KeybindingsConfig::matches(&kb.search, key.modifiers, key.code) {
            self.set_sidebar_view(SidebarView::Search);
            return;
        }
        if KeybindingsConfig::matches(&kb.git_status, key.modifiers, key.code) {
            // Refresh git status on activation
            if let Some(path) = self.sidebar_tree.selected_path().cloned() {
                self.git_status = Some(GitStatusState::new(path));
            }
            self.set_sidebar_view(SidebarView::GitStatus);
            return;
        }
        if KeybindingsConfig::matches(&kb.git_blame, key.modifiers, key.code) {
            self.open_git_blame();
            return;
        }
        if KeybindingsConfig::matches(&kb.git_log, key.modifiers, key.code) {
            self.open_git_log();
            return;
        }
        if KeybindingsConfig::matches(&kb.shell, key.modifiers, key.code) {
            self.set_main_view(MainView::Shell);
            return;
        }

        if key.code == KeyCode::Char('?') {
            self.show_help = !self.show_help;
            return;
        }
        if self.show_help {
            self.show_help = false;
            return;
        }
        // Shift+Tab: cycle sub-views within the current panel
        if key.code == KeyCode::BackTab
            || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT))
        {
            match self.panel_focus {
                PanelFocus::Left => {
                    let next = self.sidebar_view.next();
                    self.set_sidebar_view(next);
                }
                PanelFocus::Center => {
                    self.panel_focus = PanelFocus::Left;
                    self.input_mode = InputMode::Navigation;
                }
                PanelFocus::Right => {
                    self.main_view = self.main_view.next();
                }
            }
            return;
        }
        // h/l cycle sidebar views when left panel is focused (except in Worktrees view
        // where h/l do expand/collapse in the tree)
        if self.panel_focus == PanelFocus::Left && self.sidebar_view != SidebarView::Worktrees {
            if key.code == KeyCode::Char('l') {
                let next = self.sidebar_view.next();
                self.set_sidebar_view(next);
                return;
            }
            if key.code == KeyCode::Char('h') {
                let prev = self.sidebar_view.prev();
                self.set_sidebar_view(prev);
                return;
            }
        }
        match self.focused_view() {
            ViewKind::Worktrees => self.handle_worktrees_key(key),
            ViewKind::Terminal => self.handle_terminal_nav_key(key),
            ViewKind::FileExplorer => self.handle_file_explorer_key(key),
            ViewKind::Editor => self.handle_editor_key(key),
            ViewKind::Search => self.handle_search_key(key),
            ViewKind::GitStatus => self.handle_git_status_key(key),
            ViewKind::DiffView => self.handle_diff_view_key(key),
            ViewKind::GitBlame => self.handle_git_blame_key(key),
            ViewKind::GitLog => self.handle_git_log_key(key),
            ViewKind::Shell => self.handle_terminal_nav_key(key),
            ViewKind::Notes => self.handle_notes_nav_key(key),
        }
    }

    fn handle_worktrees_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.sidebar_tree.move_down();
                self.switch_to_selected_session();
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.sidebar_tree.move_up();
                self.switch_to_selected_session();
            }
            KeyCode::Enter => {
                // On section/item: toggle collapse. On session: signal spawn (handled by main loop).
                use crate::sidebar::tree::TreeNode;
                if let Some(&node) = self.sidebar_tree.selected_node() {
                    match node {
                        TreeNode::Section(_) => {
                            self.sidebar_tree.toggle_collapse();
                        }
                        TreeNode::Item(..) => {
                            // Signal session spawn — handled by main loop
                        }
                        TreeNode::Session(..) => {
                            // Signal session spawn — handled by main loop
                        }
                    }
                }
            }
            KeyCode::Char('l') | KeyCode::Right => {
                if !self.sidebar_tree.expand() {
                    // Already expanded — move to first child
                    self.sidebar_tree.move_down();
                }
            }
            KeyCode::Char('h') | KeyCode::Left => {
                self.sidebar_tree.collapse_or_parent();
            }
            // 1-9, 0 jump to item by index (0 = 10th)
            KeyCode::Char(c @ '1'..='9') => {
                self.jump_to_worktree((c as usize) - ('1' as usize));
            }
            KeyCode::Char('0') => {
                self.jump_to_worktree(9);
            }
            KeyCode::Char('a') => {
                self.prompt = Some(Prompt::CreateWorktree {
                    input: String::new(),
                });
            }
            KeyCode::Char('d') => {
                if let Some(item) = self.sidebar_tree.selected_item() {
                    if !item.is_main {
                        self.prompt = Some(Prompt::ConfirmDelete {
                            worktree_name: item.display_name.clone(),
                        });
                    } else {
                        self.status_message = Some("Cannot delete main worktree".to_string());
                    }
                }
            }
            KeyCode::Char('S') => {
                // Add named shell slot to current item
                self.prompt = Some(Prompt::AddShellSlot {
                    input: String::new(),
                });
            }
            KeyCode::Char('N') => {
                // Open directory browser for section creation
                let home = std::env::var("HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| PathBuf::from("/"));
                self.dir_browser = Some(DirBrowser::new(home));
            }
            KeyCode::Char('c') => {
                // Open color picker for current node
                use crate::sidebar::tree::TreeNode;
                if let Some(&node) = self.sidebar_tree.selected_node() {
                    let target = match node {
                        TreeNode::Section(si) => ColorTarget::Section(si),
                        TreeNode::Item(si, ii) => ColorTarget::Item(si, ii),
                        TreeNode::Session(si, ii, slot) => ColorTarget::Session(si, ii, slot),
                    };
                    self.prompt = Some(Prompt::ColorPicker { target, cursor: 0 });
                }
            }
            KeyCode::Backspace => {
                // Delete section when cursor is on a section header (not the first/auto section)
                use crate::sidebar::tree::TreeNode;
                if let Some(&TreeNode::Section(si)) = self.sidebar_tree.selected_node() {
                    if si == 0 {
                        self.status_message = Some("Cannot delete the default section".to_string());
                    } else if self.sidebar_tree.sections.len() <= 1 {
                        self.status_message = Some("Cannot delete the last section".to_string());
                    } else {
                        let name = self.sidebar_tree.sections[si].name.clone();
                        self.prompt = Some(Prompt::ConfirmDeleteSection {
                            section_name: name,
                            section_idx: si,
                        });
                    }
                }
                // For non-section nodes, Backspace is handled by needs_session_close in main loop
            }
            KeyCode::Tab => {
                self.toggle_focus();
                self.enter_terminal_if_focused();
            }
            _ => {}
        }
    }

    fn handle_terminal_nav_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Tab => {
                if let Some(ref layout) = self.pane_layout {
                    if layout.focused < layout.root.leaf_count() - 1 {
                        // Not last pane: cycle to next, enter appropriate mode
                        self.cycle_pane_focus_next();
                        self.enter_terminal_if_focused();
                    } else {
                        // Last pane: go to left panel
                        self.panel_focus = PanelFocus::Left;
                    }
                } else {
                    self.toggle_focus();
                }
            }
            KeyCode::Char('i') | KeyCode::Enter => {
                if let Some(id) = self.focused_session_id().cloned() {
                    self.attention_sessions.remove(&id);
                    if !self.exited_sessions.contains(&id) {
                        self.input_mode = InputMode::Terminal;
                        self.reset_scroll();
                    }
                }
            }
            KeyCode::PageUp => {
                self.scroll_up(self.terminal_height.saturating_sub(2) as usize);
            }
            KeyCode::PageDown => {
                self.scroll_down(self.terminal_height.saturating_sub(2) as usize);
            }
            KeyCode::Char(c @ '1'..='9') => {
                self.jump_to_worktree((c as usize) - ('1' as usize));
            }
            KeyCode::Char('0') => {
                self.jump_to_worktree(9);
            }
            _ => {}
        }
    }

    fn handle_terminal_key(&mut self, key: KeyEvent) {
        // Shift+Tab: exit terminal mode and cycle main views
        if key.code == KeyCode::BackTab
            || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT))
        {
            self.input_mode = InputMode::Navigation;
            self.main_view = self.main_view.next();
            return;
        }
        if key.code == KeyCode::Tab {
            if let Some(ref mut layout) = self.pane_layout {
                let leaf_count = layout.root.leaf_count();
                if layout.focused < leaf_count - 1 {
                    layout.focused += 1;
                    if matches!(
                        layout.root.leaf_at(layout.focused),
                        Some(PaneContent::Editor)
                    ) {
                        self.input_mode = InputMode::Navigation;
                    }
                } else {
                    self.input_mode = InputMode::Navigation;
                    self.panel_focus = PanelFocus::Left;
                }
            } else {
                self.input_mode = InputMode::Navigation;
                self.toggle_focus();
            }
        }
        // All other keys get forwarded to PTY (handled in main loop)
    }

    fn handle_file_explorer_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.file_explorer.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.file_explorer.move_up(),
            KeyCode::Enter | KeyCode::Char('l') => {
                if let Some(path) = self.file_explorer.enter() {
                    match EditorViewState::open(path) {
                        Ok(state) => {
                            let name = state.file_name().to_string();
                            self.editor = Some(state);
                            self.open_editor_in_main_panel();
                            self.status_message = Some(format!("Opened {}", name));
                        }
                        Err(e) => {
                            self.status_message = Some(e);
                        }
                    }
                }
            }
            KeyCode::Char('h') => self.file_explorer.collapse_or_parent(),
            KeyCode::Backspace => self.file_explorer.go_up_root(),
            KeyCode::Tab => {
                self.toggle_focus();
                self.enter_terminal_if_focused();
            }
            KeyCode::Char(c @ '1'..='9') => {
                self.jump_to_worktree((c as usize) - ('1' as usize));
            }
            KeyCode::Char('0') => {
                self.jump_to_worktree(9);
            }
            _ => {}
        }
    }

    fn handle_editor_key(&mut self, key: KeyEvent) {
        let Some(ref mut editor) = self.editor else {
            // No file open — only Tab and q work
            match key.code {
                KeyCode::Tab => self.toggle_focus(),
                    _ => {}
            }
            return;
        };

        if self.input_mode == InputMode::Editor {
            // Shift+Tab: exit editor mode and cycle main views
            if key.code == KeyCode::BackTab
                || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT))
            {
                editor.read_only = true;
                editor.editor_state.mode = EditorMode::Normal;
                self.input_mode = InputMode::Navigation;
                self.main_view = self.main_view.next();
                return;
            }
            // Edit mode: Ctrl+S saves, Esc exits to read-only, rest forwarded to edtui
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
                match editor.save() {
                    Ok(()) => self.status_message = Some("Saved".to_string()),
                    Err(e) => self.status_message = Some(e),
                }
                return;
            }
            if key.code == KeyCode::Esc {
                editor.read_only = true;
                editor.editor_state.mode = EditorMode::Normal;
                self.input_mode = InputMode::Navigation;
                return;
            }
            // Track modifications on any non-modifier keypress in insert mode
            if editor.editor_state.mode == EditorMode::Insert {
                editor.modified = true;
            }
            if is_edtui_compatible(&key) {
                editor
                    .event_handler
                    .on_key_event(key, &mut editor.editor_state);
            }
            return;
        }

        // Read-only mode (Navigation): intercept our keys, forward the rest to edtui Normal mode
        match key.code {
            KeyCode::Char('e') => {
                editor.read_only = false;
                editor.editor_state.mode = EditorMode::Insert;
                self.input_mode = InputMode::Editor;
            }
            KeyCode::Char('b') => {
                self.open_git_blame();
            }
            KeyCode::Tab => {
                if let Some(ref layout) = self.pane_layout {
                    if layout.focused < layout.root.leaf_count() - 1 {
                        self.cycle_pane_focus_next();
                        self.enter_terminal_if_focused();
                    } else {
                        self.panel_focus = PanelFocus::Left;
                    }
                } else {
                    self.toggle_focus();
                    self.enter_terminal_if_focused();
                }
            }
            KeyCode::Char(c @ '1'..='9') => {
                self.jump_to_worktree((c as usize) - ('1' as usize));
            }
            KeyCode::Char('0') => {
                self.jump_to_worktree(9);
            }
            _ => {
                // Forward navigation keys to edtui in Normal mode
                if is_edtui_compatible(&key) {
                    editor
                        .event_handler
                        .on_key_event(key, &mut editor.editor_state);
                    // Force back to Normal in case edtui changed mode
                    editor.editor_state.mode = EditorMode::Normal;
                }
            }
        }
    }

    /// Handle keys when notes center column is focused in Navigation mode.
    fn handle_notes_nav_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('e') => {
                // Enter edit mode
                if let Some(ref mut note) = self.note {
                    note.read_only = false;
                    note.editor_state.mode = EditorMode::Insert;
                    self.input_mode = InputMode::Editor;
                }
            }
            KeyCode::Tab => {
                self.toggle_focus();
                self.enter_terminal_if_focused();
            }
            KeyCode::Char(c @ '1'..='9') => {
                self.jump_to_worktree((c as usize) - ('1' as usize));
            }
            KeyCode::Char('0') => {
                self.jump_to_worktree(9);
            }
            _ => {
                // Forward navigation keys to edtui in Normal mode
                if let Some(ref mut note) = self.note {
                    if is_edtui_compatible(&key) {
                        note.event_handler.on_key_event(key, &mut note.editor_state);
                        note.editor_state.mode = EditorMode::Normal;
                    }
                }
            }
        }
    }

    /// Handle keys when notes center column is in Editor (edit) mode.
    fn handle_notes_editor_key(&mut self, key: KeyEvent) {
        let Some(ref mut note) = self.note else {
            return;
        };

        // Shift+Tab: exit edit mode
        if key.code == KeyCode::BackTab
            || (key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::SHIFT))
        {
            note.read_only = true;
            note.editor_state.mode = EditorMode::Normal;
            self.input_mode = InputMode::Navigation;
            self.panel_focus = PanelFocus::Left;
            return;
        }

        // Ctrl+S: save
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('s') {
            match note.save() {
                Ok(()) => self.status_message = Some("Note saved".to_string()),
                Err(e) => self.status_message = Some(e),
            }
            return;
        }

        // Esc: save and exit edit mode
        if key.code == KeyCode::Esc {
            if note.modified {
                let _ = note.save();
            }
            note.read_only = true;
            note.editor_state.mode = EditorMode::Normal;
            self.input_mode = InputMode::Navigation;
            return;
        }

        // Track modifications in insert mode
        if note.editor_state.mode == EditorMode::Insert {
            note.modified = true;
        }

        // Forward to edtui
        if is_edtui_compatible(&key) {
            note.event_handler.on_key_event(key, &mut note.editor_state);
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut search) = self.search {
                    search.move_down();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut search) = self.search {
                    search.move_up();
                }
            }
            KeyCode::Enter => {
                if let Some(ref search) = self.search {
                    if let Some(result) = search.selected_result() {
                        let path = result.file_path.clone();
                        let line = result.line_number.saturating_sub(1);
                        match EditorViewState::open(path) {
                            Ok(mut state) => {
                                state.editor_state.cursor = Index2::new(line, 0);
                                let name = state.file_name().to_string();
                                self.editor = Some(state);
                                self.open_editor_in_main_panel();
                                self.status_message = Some(format!("Opened {}", name));
                            }
                            Err(e) => {
                                self.status_message = Some(e);
                            }
                        }
                    }
                }
            }
            KeyCode::Tab => {
                self.toggle_focus();
                self.enter_terminal_if_focused();
            }
            KeyCode::Char(c @ '1'..='9') => {
                self.jump_to_worktree((c as usize) - ('1' as usize));
            }
            KeyCode::Char('0') => {
                self.jump_to_worktree(9);
            }
            _ => {}
        }
    }

    fn handle_git_status_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut gs) = self.git_status {
                    gs.move_down();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut gs) = self.git_status {
                    gs.move_up();
                }
            }
            KeyCode::Enter | KeyCode::Char('d') => {
                if let Some(ref gs) = self.git_status {
                    if let Some(entry) = gs.selected_entry() {
                        let file = entry.path.clone();
                        let category = entry.category;
                        let root = gs.worktree_path.clone();
                        self.diff_view = Some(DiffViewState::new(&file, &root, category));
                        self.main_view = MainView::DiffView;
                    }
                }
            }
            KeyCode::Tab => {
                self.toggle_focus();
                self.enter_terminal_if_focused();
            }
            KeyCode::Char(c @ '1'..='9') => {
                self.jump_to_worktree((c as usize) - ('1' as usize));
            }
            KeyCode::Char('0') => {
                self.jump_to_worktree(9);
            }
            _ => {}
        }
    }

    fn handle_diff_view_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut dv) = self.diff_view {
                    dv.scroll_down(1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut dv) = self.diff_view {
                    dv.scroll_up(1);
                }
            }
            KeyCode::PageDown => {
                if let Some(ref mut dv) = self.diff_view {
                    let h = dv.visible_height.saturating_sub(2);
                    dv.scroll_down(h);
                }
            }
            KeyCode::PageUp => {
                if let Some(ref mut dv) = self.diff_view {
                    let h = dv.visible_height.saturating_sub(2);
                    dv.scroll_up(h);
                }
            }
            KeyCode::Esc => {
                self.main_view = MainView::Terminal;
            }
            KeyCode::Tab => {
                self.toggle_focus();
            }
            KeyCode::Char(c @ '1'..='9') => {
                self.jump_to_worktree((c as usize) - ('1' as usize));
            }
            KeyCode::Char('0') => {
                self.jump_to_worktree(9);
            }
            _ => {}
        }
    }

    fn handle_git_blame_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut gb) = self.git_blame {
                    gb.scroll_down(1);
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut gb) = self.git_blame {
                    gb.scroll_up(1);
                }
            }
            KeyCode::PageDown => {
                if let Some(ref mut gb) = self.git_blame {
                    let h = gb.visible_height.saturating_sub(2);
                    gb.scroll_down(h);
                }
            }
            KeyCode::PageUp => {
                if let Some(ref mut gb) = self.git_blame {
                    let h = gb.visible_height.saturating_sub(2);
                    gb.scroll_up(h);
                }
            }
            KeyCode::Esc => {
                self.main_view = MainView::Editor;
            }
            KeyCode::Tab => {
                self.toggle_focus();
            }
            KeyCode::Char(c @ '1'..='9') => {
                self.jump_to_worktree((c as usize) - ('1' as usize));
            }
            KeyCode::Char('0') => {
                self.jump_to_worktree(9);
            }
            _ => {}
        }
    }

    fn handle_git_log_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                if let Some(ref mut gl) = self.git_log {
                    gl.move_down();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if let Some(ref mut gl) = self.git_log {
                    gl.move_up();
                }
            }
            KeyCode::Enter => {
                if let Some(ref gl) = self.git_log {
                    if let Some(entry) = gl.selected_entry() {
                        let hash = entry.hash_short.clone();
                        let root = gl.worktree_path.clone();
                        match run_git_show(&hash, &root) {
                            Ok(diff_lines) => {
                                self.diff_view = Some(DiffViewState {
                                    file_path: format!("commit {}", hash),
                                    lines: diff_lines,
                                    scroll_offset: 0,
                                    visible_height: 24,
                                });
                                self.main_view = MainView::DiffView;
                            }
                            Err(e) => {
                                self.status_message = Some(format!("git show error: {}", e));
                            }
                        }
                    }
                }
            }
            KeyCode::Esc => {
                self.main_view = MainView::Terminal;
            }
            KeyCode::Tab => {
                self.toggle_focus();
            }
            KeyCode::Char(c @ '1'..='9') => {
                self.jump_to_worktree((c as usize) - ('1' as usize));
            }
            KeyCode::Char('0') => {
                self.jump_to_worktree(9);
            }
            _ => {}
        }
    }

    fn open_git_blame(&mut self) {
        let Some(ref editor) = self.editor else {
            self.status_message = Some("No file open in editor".to_string());
            return;
        };
        let Some(item) = self.sidebar_tree.selected_item() else {
            return;
        };
        let root = item.path.clone();
        let file_path = editor.file_path.clone();
        let relative = file_path
            .strip_prefix(&root)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| file_path.to_string_lossy().to_string());

        match run_git_blame(&relative, &root) {
            Ok(lines) => {
                self.git_blame = Some(GitBlameState {
                    file_path: relative,
                    lines,
                    scroll_offset: 0,
                    visible_height: 24,
                    worktree_path: root,
                    stale: false,
                });
                self.set_main_view(MainView::GitBlame);
            }
            Err(e) => {
                self.status_message = Some(format!("Blame error: {}", e));
            }
        }
    }

    fn open_git_log(&mut self) {
        let Some(item) = self.sidebar_tree.selected_item() else {
            return;
        };
        let root = item.path.clone();
        let file_filter = self.editor.as_ref().and_then(|ed| {
            ed.file_path
                .strip_prefix(&root)
                .ok()
                .map(|p| p.to_string_lossy().to_string())
        });

        match run_git_log(&root, file_filter.as_deref()) {
            Ok(entries) => {
                self.git_log = Some(GitLogState {
                    entries,
                    selected: 0,
                    scroll_offset: 0,
                    visible_height: 24,
                    worktree_path: root,
                    file_filter,
                    stale: false,
                });
                self.set_main_view(MainView::GitLog);
            }
            Err(e) => {
                self.status_message = Some(format!("Log error: {}", e));
            }
        }
    }

    fn jump_to_worktree(&mut self, index: usize) {
        if self.sidebar_tree.jump_to_nth_item(index) {
            self.switch_to_selected_session();
        }
    }

    fn switch_to_selected_session(&mut self) {
        // When cursor lands on a specific session slot, update main_view to match its kind
        // so the right panel renders the correct session type.
        use crate::sidebar::tree::TreeNode;
        if let Some(TreeNode::Session(si, ii, slot_idx)) = self
            .sidebar_tree
            .visible
            .get(self.sidebar_tree.cursor)
            .copied()
        {
            if let Some(slot) = self
                .sidebar_tree
                .sections
                .get(si)
                .and_then(|s| s.items.get(ii))
                .and_then(|item| item.sessions.get(slot_idx))
            {
                match slot.kind {
                    SessionKind::Shell => self.main_view = MainView::Shell,
                    SessionKind::Claude => self.main_view = MainView::Terminal,
                }
            }
        }

        if let Some(item) = self.sidebar_tree.selected_item() {
            // Clear attention for any sessions in this item
            for slot in &item.sessions {
                if let Some(ref id) = slot.session_id {
                    self.attention_sessions.remove(id);
                }
            }
            self.file_explorer.set_root(item.path.clone());
            // Clear stale git state from previous worktree
            self.git_status = None;
            self.diff_view = None;
            self.git_blame = None;
            self.git_log = None;
        }
        // Load note for the newly selected worktree
        self.load_note_for_current_worktree();
    }

    /// Returns the session ID that should receive input: view-aware routing.
    /// In split mode, extracts session ID from focused pane content.
    /// Fallback: derives from sidebar tree cursor based on main_view.
    pub fn focused_session_id(&self) -> Option<&String> {
        if let Some(ref layout) = self.pane_layout {
            return match layout.root.leaf_at(layout.focused) {
                Some(PaneContent::Terminal(id)) | Some(PaneContent::Shell(id)) => Some(id),
                _ => None,
            };
        }
        // If cursor is on a specific session slot, use it directly (handles multiple slots of same kind)
        use crate::sidebar::tree::TreeNode;
        if let Some(TreeNode::Session(si, ii, slot_idx)) =
            self.sidebar_tree.visible.get(self.sidebar_tree.cursor)
        {
            return self
                .sidebar_tree
                .sections
                .get(*si)?
                .items
                .get(*ii)?
                .sessions
                .get(*slot_idx)?
                .session_id
                .as_ref();
        }
        // Cursor on Item or Section: fall back to kind-based lookup
        let item = self.sidebar_tree.selected_item()?;
        let target_kind = if self.main_view == MainView::Shell {
            SessionKind::Shell
        } else {
            SessionKind::Claude
        };
        item.sessions
            .iter()
            .find(|s| s.kind == target_kind)
            .and_then(|s| s.session_id.as_ref())
    }

    /// Returns the focused pane content, if in split mode.
    pub fn focused_pane_content(&self) -> Option<&PaneContent> {
        self.pane_layout
            .as_ref()
            .and_then(|layout| layout.root.leaf_at(layout.focused))
    }

    /// Whether a session is currently visible on screen.
    pub fn is_session_visible(&self, session_id: &str) -> bool {
        // Check pane layout for any Terminal/Shell pane with this session ID
        if let Some(ref layout) = self.pane_layout {
            return layout.root.contains_session(session_id);
        }
        // No split: check if focused session matches
        self.focused_session_id()
            .map(|id| id.as_str() == session_id)
            .unwrap_or(false)
            && (self.main_view == MainView::Terminal || self.main_view == MainView::Shell)
    }

    /// Hit-test which terminal/shell pane contains the given absolute (column, row) coords.
    /// Returns the session ID and the pane's inner rect (excluding border).
    pub fn pane_session_at_coords(
        &self,
        col: u16,
        row: u16,
        terminal_size: Rect,
    ) -> Option<(String, Rect)> {
        use ratatui::widgets::{Block, BorderType, Borders};
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Thick);

        if let Some(ref layout) = self.pane_layout {
            if layout.root.leaf_count() > 1 {
                let panel = crate::ui::right_panel_rect(
                    terminal_size,
                    self.sidebar_width,
                    self.notes_pct(),
                );
                let rects = crate::ui::compute_leaf_rects(&layout.root, panel);
                let leaves = layout.root.leaves();
                for (i, content) in leaves.iter().enumerate() {
                    if let Some(sid) = content.session_id() {
                        let inner = block.inner(rects[i]);
                        if col >= inner.x
                            && col < inner.x + inner.width
                            && row >= inner.y
                            && row < inner.y + inner.height
                        {
                            return Some((sid.to_string(), inner));
                        }
                    }
                }
                return None;
            }
        }
        // Single pane: use the right panel rect
        let panel =
            crate::ui::compute_pty_rect(terminal_size, self.sidebar_width, self.notes_pct());
        if col >= panel.x
            && col < panel.x + panel.width
            && row >= panel.y
            && row < panel.y + panel.height
        {
            // Determine which session is active based on main_view
            let session_id = match self.main_view {
                MainView::Terminal => self.active_session_id().map(|s| s.to_string()),
                MainView::Shell => self.active_shell_session_id().map(|s| s.to_string()),
                _ => None,
            };
            return session_id.map(|sid| (sid, panel));
        }
        None
    }

    /// Reverse lookup: find the worktree name for a given session ID.
    pub fn worktree_name_for_session(&self, session_id: &str) -> Option<&str> {
        self.sidebar_tree.name_for_session(session_id)
    }

    /// Add a pane with the given content. Creates PaneLayout if None.
    /// Splits the focused leaf in the tree to add the new content.
    pub fn split_add_pane_with(&mut self, content: PaneContent) -> bool {
        if let Some(ref mut layout) = self.pane_layout {
            // Check depth limit
            if layout.root.depth() >= MAX_SPLIT_DEPTH {
                self.status_message = Some("Maximum split depth reached".to_string());
                return false;
            }
            let direction = self.split_direction;
            if !layout.root.split_leaf(layout.focused, direction, content) {
                self.status_message = Some("Could not split pane".to_string());
                return false;
            }
            true
        } else {
            // Build initial pane from current state
            let current_pane = match self.main_view {
                MainView::Terminal => self
                    .active_session_id()
                    .map(|id| PaneContent::Terminal(id.to_string())),
                MainView::Shell => self
                    .active_shell_session_id()
                    .map(|id| PaneContent::Shell(id.to_string())),
                MainView::Editor => Some(PaneContent::Editor),
                _ => None,
            };
            let Some(first_pane) = current_pane else {
                self.status_message = Some("No active view to split".to_string());
                return false;
            };
            self.pane_layout = Some(PaneLayout {
                root: SplitNode::Split {
                    direction: self.split_direction,
                    first: Box::new(SplitNode::Leaf(first_pane)),
                    second: Box::new(SplitNode::Leaf(content)),
                },
                focused: 0,
            });
            true
        }
    }

    /// Add a pane of the same type as the current focused content, finding the next available session.
    /// Returns false if no sessions available.
    pub fn split_add_pane(&mut self) -> bool {
        // Determine the type to split based on focused content
        let split_type = if let Some(ref layout) = self.pane_layout {
            match layout.root.leaf_at(layout.focused) {
                Some(PaneContent::Terminal(_)) => "terminal",
                Some(PaneContent::Shell(_)) => "shell",
                Some(PaneContent::Editor) => "editor",
                None => return false,
            }
        } else {
            match self.main_view {
                MainView::Terminal => "terminal",
                MainView::Shell => "shell",
                MainView::Editor => "editor",
                _ => {
                    self.status_message = Some("Split not supported for this view".to_string());
                    return false;
                }
            }
        };

        match split_type {
            "terminal" => {
                let next = self.find_next_session_of_kind(SessionKind::Claude);
                match next {
                    Some(id) => self.split_add_pane_with(PaneContent::Terminal(id)),
                    None => {
                        self.status_message = Some("No other running sessions to show".to_string());
                        false
                    }
                }
            }
            "shell" => {
                let next = self.find_next_session_of_kind(SessionKind::Shell);
                match next {
                    Some(id) => self.split_add_pane_with(PaneContent::Shell(id)),
                    None => {
                        self.status_message =
                            Some("No other running shell sessions to show".to_string());
                        false
                    }
                }
            }
            "editor" => {
                // Editor pane is singular — just add it
                self.split_add_pane_with(PaneContent::Editor)
            }
            _ => false,
        }
    }

    /// Find next session of the given kind not already in pane layout.
    fn find_next_session_of_kind(&self, kind: SessionKind) -> Option<String> {
        let current_ids: Vec<&str> = if let Some(ref layout) = self.pane_layout {
            layout.root.all_session_ids()
        } else {
            let id = match kind {
                SessionKind::Claude => self.active_session_id(),
                SessionKind::Shell => self.active_shell_session_id(),
            };
            id.into_iter().collect()
        };

        for section in &self.sidebar_tree.sections {
            for item in &section.items {
                for slot in &item.sessions {
                    if slot.kind == kind {
                        if let Some(ref sid) = slot.session_id {
                            if !current_ids.contains(&sid.as_str())
                                && !self.exited_sessions.contains(sid.as_str())
                            {
                                return Some(sid.clone());
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Collapse pane layout to single-view if root is a single leaf.
    fn collapse_pane_layout_if_needed(&mut self) {
        let Some(ref layout) = self.pane_layout else {
            return;
        };
        if let SplitNode::Leaf(ref content) = layout.root {
            match content {
                PaneContent::Terminal(_) => self.main_view = MainView::Terminal,
                PaneContent::Shell(_) => self.main_view = MainView::Shell,
                PaneContent::Editor => self.main_view = MainView::Editor,
            }
            self.pane_layout = None;
        }
    }

    /// Remove the focused pane. Collapses to single mode if <=1 remains.
    pub fn close_focused_pane(&mut self) {
        let Some(ref mut layout) = self.pane_layout else {
            return;
        };
        let leaf_count = layout.root.leaf_count();
        if leaf_count <= 1 {
            self.pane_layout = None;
            return;
        }
        layout.root.remove_leaf(layout.focused);
        let new_count = layout.root.leaf_count();
        if layout.focused >= new_count {
            layout.focused = new_count.saturating_sub(1);
        }
        self.collapse_pane_layout_if_needed();
    }

    pub fn cycle_pane_focus_next(&mut self) {
        if let Some(ref mut layout) = self.pane_layout {
            layout.focused = (layout.focused + 1) % layout.root.leaf_count();
        }
    }

    pub fn cycle_pane_focus_prev(&mut self) {
        if let Some(ref mut layout) = self.pane_layout {
            let count = layout.root.leaf_count();
            layout.focused = if layout.focused == 0 {
                count - 1
            } else {
                layout.focused - 1
            };
        }
    }

    /// Remove a session from the pane layout if present. Collapses if needed.
    pub fn remove_session_from_panes(&mut self, session_id: &str) {
        let Some(ref mut layout) = self.pane_layout else {
            return;
        };
        layout.root.remove_session(session_id);
        let count = layout.root.leaf_count();
        if layout.focused >= count && count > 0 {
            layout.focused = count - 1;
        }
        self.collapse_pane_layout_if_needed();
    }

    /// Clean up all app-side state for a removed session.
    /// Caller is responsible for removing from SessionManager separately.
    pub fn cleanup_session(&mut self, session_id: &str) {
        self.sidebar_tree.clear_session_id(session_id);
        self.attention_sessions.remove(session_id);
        self.exited_sessions.remove(session_id);
        self.activity.remove_session(session_id);
        self.remove_session_from_panes(session_id);
    }

    pub fn scroll_up(&mut self, lines: usize) {
        if let Some(id) = self.focused_session_id().cloned() {
            let offset = self.scroll_offsets.entry(id.clone()).or_insert(0);
            *offset = offset.saturating_add(lines).min(1000);
            self.user_scrolled.insert(id);
        }
    }

    pub fn scroll_down(&mut self, lines: usize) {
        if let Some(id) = self.focused_session_id().cloned() {
            let offset = self.scroll_offsets.entry(id.clone()).or_insert(0);
            *offset = offset.saturating_sub(lines);
            if *offset == 0 {
                self.scroll_offsets.remove(&id);
                self.user_scrolled.remove(&id);
            }
        }
    }

    pub fn reset_scroll(&mut self) {
        if let Some(id) = self.focused_session_id().cloned() {
            self.scroll_offsets.remove(&id);
            self.user_scrolled.remove(&id);
        }
    }

    pub fn active_scroll_offset(&self) -> usize {
        self.focused_session_id()
            .and_then(|id| self.scroll_offsets.get(id))
            .copied()
            .unwrap_or(0)
    }

    /// Get scroll offset for a specific session (used in multi-pane rendering).
    pub fn scroll_offset_for(&self, session_id: &str) -> usize {
        self.scroll_offsets.get(session_id).copied().unwrap_or(0)
    }

    /// Count sessions that are running (have session_id but not exited).
    /// Build a LayoutConfig from current sidebar state (only slots with active sessions).
    pub fn to_layout_config(&self) -> config::LayoutConfig {
        let mut sessions = Vec::new();
        for section in &self.sidebar_tree.sections {
            for item in &section.items {
                for slot in &item.sessions {
                    if slot.session_id.is_some() {
                        let slot_kind = match slot.kind {
                            SessionKind::Claude => "claude",
                            SessionKind::Shell => "shell",
                        };
                        sessions.push(config::LayoutSessionToml {
                            path: item.path.to_string_lossy().to_string(),
                            slot_kind: slot_kind.to_string(),
                            slot_label: slot.label.clone(),
                        });
                    }
                }
            }
        }

        let sidebar_view = Some(
            match self.sidebar_view {
                SidebarView::Worktrees => "worktrees",
                SidebarView::FileExplorer => "files",
                SidebarView::Search => "search",
                SidebarView::GitStatus => "git_status",
            }
            .to_string(),
        );

        let main_view = Some(
            match self.main_view {
                MainView::Terminal => "terminal",
                MainView::Editor => "editor",
                MainView::DiffView => "diff",
                MainView::GitBlame => "blame",
                MainView::GitLog => "log",
                MainView::Shell => "shell",
            }
            .to_string(),
        );

        let panel_focus = Some(
            match self.panel_focus {
                PanelFocus::Left | PanelFocus::Center => "left", // Center reserved
                PanelFocus::Right => "right",
            }
            .to_string(),
        );

        config::LayoutConfig {
            sessions,
            sidebar_view,
            main_view,
            panel_focus,
        }
    }

    /// Count (running, exited) sessions in a single pass.
    pub fn session_counts(&self) -> (usize, usize) {
        let ids = self.sidebar_tree.all_session_ids();
        let exited = ids
            .iter()
            .filter(|id| self.exited_sessions.contains(**id))
            .count();
        (ids.len() - exited, exited)
    }

    /// Get branch name and dirty counts for the selected worktree.
    /// Returns (branch_name, untracked_count, modified_count).
    pub fn selected_branch_info(&self) -> Option<(String, usize, usize)> {
        let item = self.sidebar_tree.selected_item()?;
        let branch = item
            .branch
            .clone()
            .unwrap_or_else(|| "detached".to_string());
        let (untracked, modified) = match &self.git_status {
            Some(gs) if gs.worktree_path == item.path => {
                let u = gs
                    .entries
                    .iter()
                    .filter(|e| e.category == GitStatusCategory::Untracked)
                    .count();
                let m = gs
                    .entries
                    .iter()
                    .filter(|e| e.category != GitStatusCategory::Untracked)
                    .count();
                (u, m)
            }
            _ => (0, 0),
        };
        Some((branch, untracked, modified))
    }

    /// Check if cursor is on a session slot with an active session (for close on Backspace).
    pub fn needs_session_close(&self, key: &KeyEvent) -> bool {
        if key.code != KeyCode::Backspace
            || self.prompt.is_some()
            || self.branch_switcher.is_some()
            || self.fuzzy_finder.is_some()
            || self.command_palette.is_some()
            || self.dir_browser.is_some()
            || self.show_help
            || self.input_mode != InputMode::Navigation
        {
            return false;
        }
        // Close works from worktree sidebar or right panel terminal/shell view
        self.cursor_session_id().is_some()
    }

    /// Check if cursor is on an exited session (for restart on 'r').
    pub fn needs_session_restart(&self, key: &KeyEvent) -> bool {
        if key.code != KeyCode::Char('r')
            || self.prompt.is_some()
            || self.branch_switcher.is_some()
            || self.fuzzy_finder.is_some()
            || self.command_palette.is_some()
            || self.dir_browser.is_some()
            || self.show_help
            || self.input_mode != InputMode::Navigation
        {
            return false;
        }
        self.cursor_session_id()
            .map(|id| self.exited_sessions.contains(id))
            .unwrap_or(false)
    }

    /// Check if cursor is on any session for force-restart on Shift+R.
    pub fn needs_session_force_restart(&self, key: &KeyEvent) -> bool {
        if key.code != KeyCode::Char('R')
            || self.prompt.is_some()
            || self.branch_switcher.is_some()
            || self.fuzzy_finder.is_some()
            || self.command_palette.is_some()
            || self.dir_browser.is_some()
            || self.show_help
            || self.input_mode != InputMode::Navigation
        {
            return false;
        }
        self.cursor_session_id().is_some()
    }

    /// Check if Enter should spawn/switch a session.
    pub fn needs_session_spawn(&self, key: &KeyEvent) -> bool {
        self.prompt.is_none()
            && self.branch_switcher.is_none()
            && self.fuzzy_finder.is_none()
            && self.command_palette.is_none()
            && self.dir_browser.is_none()
            && !self.show_help
            && key.code == KeyCode::Enter
            && self.input_mode == InputMode::Navigation
            && ((self.sidebar_view == SidebarView::Worktrees
                && self.panel_focus == PanelFocus::Left)
                || (self.main_view == MainView::Terminal && self.panel_focus == PanelFocus::Right)
                || (self.main_view == MainView::Shell && self.panel_focus == PanelFocus::Right))
    }

    /// Get the session ID at the current cursor position.
    /// For Item nodes, returns the first Claude session's ID.
    /// For Session nodes, returns that slot's ID.
    pub fn cursor_session_id(&self) -> Option<&str> {
        use crate::sidebar::tree::TreeNode;
        match self.sidebar_tree.visible.get(self.sidebar_tree.cursor)? {
            TreeNode::Section(_) => None,
            TreeNode::Item(si, ii) => {
                // Return first session with an active ID
                self.sidebar_tree
                    .sections
                    .get(*si)?
                    .items
                    .get(*ii)?
                    .sessions
                    .iter()
                    .find_map(|s| s.session_id.as_deref())
            }
            TreeNode::Session(si, ii, slot) => self
                .sidebar_tree
                .sections
                .get(*si)?
                .items
                .get(*ii)?
                .sessions
                .get(*slot)?
                .session_id
                .as_deref(),
        }
    }

    /// Get the session kind + ID at cursor for spawn/switch logic.
    pub fn cursor_session_info(&self) -> Option<(SessionKind, Option<&str>, &PathBuf)> {
        use crate::sidebar::tree::TreeNode;
        let node = self.sidebar_tree.visible.get(self.sidebar_tree.cursor)?;
        match node {
            TreeNode::Section(_) => None,
            TreeNode::Item(si, ii) => {
                let item = self.sidebar_tree.sections.get(*si)?.items.get(*ii)?;
                let slot = item.sessions.first()?;
                Some((slot.kind, slot.session_id.as_deref(), &item.path))
            }
            TreeNode::Session(si, ii, slot_idx) => {
                let item = self.sidebar_tree.sections.get(*si)?.items.get(*ii)?;
                let slot = item.sessions.get(*slot_idx)?;
                Some((slot.kind, slot.session_id.as_deref(), &item.path))
            }
        }
    }

    /// Check if user confirmed worktree creation. Returns the branch name.
    pub fn wants_create_worktree(&self, key: &KeyEvent) -> Option<String> {
        if key.code != KeyCode::Enter {
            return None;
        }
        if let Some(Prompt::CreateWorktree { input }) = &self.prompt {
            if !input.is_empty() {
                return Some(input.clone());
            }
        }
        None
    }

    /// Check if user confirmed worktree deletion. Returns the selected worktree index.
    pub fn wants_delete_worktree(&self, key: &KeyEvent) -> bool {
        if let Some(Prompt::ConfirmDelete { .. }) = &self.prompt {
            return key.code == KeyCode::Char('y') || key.code == KeyCode::Char('Y');
        }
        false
    }

    /// Check if user confirmed a branch switch. Returns (worktree_path, branch_name).
    pub fn wants_switch_branch(&self, key: &KeyEvent) -> Option<(PathBuf, String)> {
        if key.code != KeyCode::Enter {
            return None;
        }
        if let Some(ref bs) = self.branch_switcher {
            if let Some(branch) = bs.selected_branch() {
                if branch != bs.current_branch {
                    return Some((bs.worktree_path.clone(), branch.to_string()));
                }
            }
        }
        None
    }

    /// Open the split picker overlay. Builds the list of available items.
    pub fn open_split_picker(&mut self, direction: SplitDirection) {
        let mut items: Vec<SplitPickerItem> = Vec::new();

        // Add existing layout as one item if it exists
        if let Some(ref layout) = self.pane_layout {
            items.push(SplitPickerItem::ExistingLayout {
                label: format!("Current Layout: {}", layout.root.display_label()),
                node: layout.root.clone(),
            });
        }

        // Collect session IDs already in a layout to mark them
        let in_layout: Vec<&str> = if let Some(ref layout) = self.pane_layout {
            layout.root.all_session_ids()
        } else {
            Vec::new()
        };

        // Add all running terminal and shell sessions
        for section in &self.sidebar_tree.sections {
            for item in &section.items {
                let worktree_name = &item.display_name;
                for slot in &item.sessions {
                    if let Some(ref sid) = slot.session_id {
                        if self.exited_sessions.contains(sid.as_str()) {
                            continue;
                        }
                        // Skip sessions already in the layout (they're represented by ExistingLayout)
                        if in_layout.contains(&sid.as_str()) {
                            continue;
                        }
                        match slot.kind {
                            SessionKind::Claude => {
                                items.push(SplitPickerItem::Terminal {
                                    session_id: sid.clone(),
                                    label: format!("Terminal: {}", worktree_name),
                                });
                            }
                            SessionKind::Shell => {
                                items.push(SplitPickerItem::Shell {
                                    session_id: sid.clone(),
                                    label: format!("Shell: {}", worktree_name),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Add editor if not already in layout
        if self.pane_layout.is_none()
            || !self
                .pane_layout
                .as_ref()
                .unwrap()
                .root
                .leaves()
                .iter()
                .any(|l| matches!(l, PaneContent::Editor))
        {
            items.push(SplitPickerItem::Editor {
                label: "Editor".to_string(),
            });
        }

        // If no layout exists, also add the current view as an item
        if self.pane_layout.is_none() {
            match self.main_view {
                MainView::Terminal => {
                    if let Some(id) = self.active_session_id().map(|s| s.to_string()) {
                        let name = self
                            .worktree_name_for_session(&id)
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| id.clone());
                        // Only add if not already in items
                        if !items.iter().any(|i| matches!(i, SplitPickerItem::Terminal { session_id, .. } if session_id == &id)) {
                            items.insert(0, SplitPickerItem::Terminal {
                                session_id: id,
                                label: format!("Terminal: {} (current)", name),
                            });
                        }
                    }
                }
                MainView::Shell => {
                    if let Some(id) = self.active_shell_session_id().map(|s| s.to_string()) {
                        let name = self
                            .worktree_name_for_session(&id)
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| id.clone());
                        if !items.iter().any(|i| matches!(i, SplitPickerItem::Shell { session_id, .. } if session_id == &id)) {
                            items.insert(0, SplitPickerItem::Shell {
                                session_id: id,
                                label: format!("Shell: {} (current)", name),
                            });
                        }
                    }
                }
                MainView::Editor => {
                    // Editor already added above
                }
                _ => {}
            }
        }

        if items.len() < 2 {
            self.status_message = Some("Not enough items to split".to_string());
            return;
        }

        let visible: Vec<usize> = (0..items.len()).collect();
        self.split_picker = Some(SplitPickerState {
            items,
            visible,
            selected: 0,
            step: SplitPickerStep::PickFirst,
            first_choice: None,
            direction,
        });
    }

    fn handle_split_picker_key(&mut self, key: KeyEvent) {
        // Ignore Ctrl combos except Ctrl+C (handled elsewhere)
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return;
        }
        match key.code {
            KeyCode::Esc => {
                self.split_picker = None;
            }
            KeyCode::Tab => {
                if let Some(ref mut picker) = self.split_picker {
                    picker.toggle_direction();
                }
            }
            KeyCode::Up => {
                if let Some(ref mut picker) = self.split_picker {
                    picker.move_up();
                }
            }
            KeyCode::Down => {
                if let Some(ref mut picker) = self.split_picker {
                    picker.move_down();
                }
            }
            KeyCode::Enter => {
                let (step, first_idx, second_idx, direction) = {
                    let Some(ref picker) = self.split_picker else {
                        return;
                    };
                    let idx = picker.selected_item_index();
                    (picker.step, picker.first_choice, idx, picker.direction)
                };

                match step {
                    SplitPickerStep::PickFirst => {
                        if let Some(idx) = second_idx {
                            if let Some(ref mut picker) = self.split_picker {
                                picker.first_choice = Some(idx);
                                picker.step = SplitPickerStep::PickSecond;
                                // Remove first choice from visible
                                picker.visible.retain(|&i| i != idx);
                                picker.selected = 0;
                            }
                        }
                    }
                    SplitPickerStep::PickSecond => {
                        if let (Some(first_idx), Some(second_idx)) = (first_idx, second_idx) {
                            // Clone items before mutating
                            let first_node = self.split_picker.as_ref().unwrap().items[first_idx]
                                .to_split_node();
                            let second_node = self.split_picker.as_ref().unwrap().items[second_idx]
                                .to_split_node();
                            self.split_picker = None;

                            // Clear existing layout if one of the items IS the existing layout
                            self.pane_layout = Some(PaneLayout {
                                root: SplitNode::Split {
                                    direction,
                                    first: Box::new(first_node),
                                    second: Box::new(second_node),
                                },
                                focused: 0,
                            });
                            self.layout_changed = true;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_branch_switcher_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.branch_switcher = None;
            }
            KeyCode::Enter => {
                // Signal handled in main loop via wants_switch_branch()
            }
            KeyCode::Up => {
                if let Some(ref mut bs) = self.branch_switcher {
                    bs.move_up();
                }
            }
            KeyCode::Down => {
                if let Some(ref mut bs) = self.branch_switcher {
                    bs.move_down();
                }
            }
            KeyCode::Backspace => {
                if let Some(ref mut bs) = self.branch_switcher {
                    bs.input.pop();
                    bs.update_matches();
                }
            }
            KeyCode::Char(c) => {
                if let Some(ref mut bs) = self.branch_switcher {
                    bs.input.push(c);
                    bs.update_matches();
                }
            }
            _ => {}
        }
    }

    /// Returns a notification message if the event warrants an OS alert, None otherwise.
    /// Returns (iterm2_msg, native_msg) — iTerm2 is suppressed when viewing the session.
    /// Native msg format: "subtitle\nmessage" where subtitle is the section name.
    pub fn notification_for_event(&self, event: &AppEvent) -> (Option<String>, Option<String>) {
        match event {
            AppEvent::SessionBell { session_id } => {
                // Bell = iTerm2 dock bounce only, no native notification
                let name = match self.worktree_name_for_session(session_id) {
                    Some(n) => n,
                    None => return (None, None),
                };
                let viewing = self.focused_session_id().map(|s| s.as_str()) == Some(session_id)
                    && self.input_mode == InputMode::Terminal;
                if viewing {
                    (None, None)
                } else {
                    (Some(format!("{} needs attention", name)), None)
                }
            }
            AppEvent::SessionDone { .. } => {
                // Don't send native notifications on SessionDone — these fire on
                // every tool call completion, not just when truly idle.
                // Attention is debounced in the main event loop instead.
                (None, None)
            }
            AppEvent::SessionExited { session_id } => {
                let (section, name) =
                    match self.sidebar_tree.section_and_name_for_session(session_id) {
                        Some(pair) => pair,
                        None => return (None, None),
                    };
                let iterm_msg = format!("{} session exited", name);
                let native_msg = format!("{}\n{} session exited", section, name);
                (Some(iterm_msg), Some(native_msg))
            }
            _ => (None, None),
        }
    }

    pub fn refresh_worktrees(&mut self, worktrees: Vec<Worktree>) {
        self.sidebar_tree.refresh_worktrees(&worktrees);
        // Sync file explorer root with current worktree
        if let Some(item) = self.sidebar_tree.selected_item() {
            self.file_explorer.set_root(item.path.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn activity_animation_timeout() {
        let mut anim = ActivityAnimation::new();
        anim.mark_active("s1");
        anim.tick();
        assert!(anim.is_active("s1"));

        // Wait longer than ACTIVITY_TIMEOUT_MS (500ms)
        std::thread::sleep(std::time::Duration::from_millis(600));
        anim.tick();
        assert!(!anim.is_active("s1"));
    }

    #[test]
    fn input_suppression_prevents_activation() {
        let mut anim = ActivityAnimation::new();
        anim.mark_input("s1");
        anim.mark_active("s1");
        anim.tick();
        assert!(!anim.is_active("s1"));
    }

    #[test]
    fn output_activates_after_suppression_expires() {
        let mut anim = ActivityAnimation::new();
        anim.mark_input("s1");
        // Wait for suppression to expire (300ms)
        std::thread::sleep(std::time::Duration::from_millis(350));
        anim.mark_active("s1");
        anim.tick();
        assert!(anim.is_active("s1"));
    }
}
