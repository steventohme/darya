use std::path::PathBuf;

use crate::worktree::types::Worktree;

use super::types::{Section, SessionKind, SessionSlot, SidebarItem};

/// Address of a node in the sidebar tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeNode {
    /// Section header at sections[section_idx].
    Section(usize),
    /// Item at sections[section_idx].items[item_idx].
    Item(usize, usize),
    /// Session slot at sections[s].items[i].sessions[slot_idx].
    Session(usize, usize, usize),
}

pub struct SidebarTree {
    pub sections: Vec<Section>,
    /// Flattened list of visible tree nodes (rebuilt on expand/collapse).
    pub visible: Vec<TreeNode>,
    /// Index into `visible` — the cursor position.
    pub cursor: usize,
}

impl SidebarTree {
    /// Create a tree from worktrees, auto-generating a single section.
    pub fn from_worktrees(worktrees: &[Worktree]) -> Self {
        let repo_name = worktrees
            .iter()
            .find(|wt| wt.is_main)
            .map(|wt| wt.name.clone())
            .unwrap_or_else(|| "repo".to_string());

        let items: Vec<SidebarItem> = worktrees
            .iter()
            .map(|wt| SidebarItem {
                path: wt.path.clone(),
                display_name: wt.name.clone(),
                branch: wt.branch.clone(),
                is_main: wt.is_main,
                collapsed: true,
                sessions: vec![SessionSlot {
                    kind: SessionKind::Claude,
                    label: "claude".to_string(),
                    session_id: None,
                }],
            })
            .collect();

        let sections = vec![Section {
            name: repo_name,
            collapsed: false,
            items,
        }];

        let mut tree = Self {
            sections,
            visible: Vec::new(),
            cursor: 0,
        };
        tree.rebuild_visible();
        tree
    }

    /// Rebuild the flattened visible node list from current collapse state.
    pub fn rebuild_visible(&mut self) {
        self.visible.clear();
        for (si, section) in self.sections.iter().enumerate() {
            self.visible.push(TreeNode::Section(si));
            if !section.collapsed {
                for (ii, item) in section.items.iter().enumerate() {
                    self.visible.push(TreeNode::Item(si, ii));
                    if !item.collapsed {
                        for (slot_idx, _) in item.sessions.iter().enumerate() {
                            self.visible.push(TreeNode::Session(si, ii, slot_idx));
                        }
                    }
                }
            }
        }
        // Clamp cursor
        if !self.visible.is_empty() && self.cursor >= self.visible.len() {
            self.cursor = self.visible.len() - 1;
        }
    }

    pub fn move_down(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        self.cursor = (self.cursor + 1) % self.visible.len();
    }

    pub fn move_up(&mut self) {
        if self.visible.is_empty() {
            return;
        }
        self.cursor = if self.cursor == 0 {
            self.visible.len() - 1
        } else {
            self.cursor - 1
        };
    }

    /// Toggle collapse on the current node (section or item).
    /// Returns true if something was toggled.
    pub fn toggle_collapse(&mut self) -> bool {
        let Some(&node) = self.visible.get(self.cursor) else {
            return false;
        };
        match node {
            TreeNode::Section(si) => {
                self.sections[si].collapsed = !self.sections[si].collapsed;
                self.rebuild_visible();
                true
            }
            TreeNode::Item(si, ii) => {
                self.sections[si].items[ii].collapsed =
                    !self.sections[si].items[ii].collapsed;
                self.rebuild_visible();
                true
            }
            TreeNode::Session(..) => false,
        }
    }

    /// Expand current node. Returns true if expanded.
    pub fn expand(&mut self) -> bool {
        let Some(&node) = self.visible.get(self.cursor) else {
            return false;
        };
        match node {
            TreeNode::Section(si) if self.sections[si].collapsed => {
                self.sections[si].collapsed = false;
                self.rebuild_visible();
                true
            }
            TreeNode::Item(si, ii) if self.sections[si].items[ii].collapsed => {
                self.sections[si].items[ii].collapsed = false;
                self.rebuild_visible();
                true
            }
            _ => false,
        }
    }

    /// Collapse current node, or jump to parent if already collapsed/leaf.
    pub fn collapse_or_parent(&mut self) {
        let Some(&node) = self.visible.get(self.cursor) else {
            return;
        };
        match node {
            TreeNode::Section(si) => {
                if !self.sections[si].collapsed {
                    self.sections[si].collapsed = true;
                    self.rebuild_visible();
                }
            }
            TreeNode::Item(si, ii) => {
                if !self.sections[si].items[ii].collapsed {
                    self.sections[si].items[ii].collapsed = true;
                    self.rebuild_visible();
                } else {
                    // Jump to parent section
                    if let Some(pos) = self
                        .visible
                        .iter()
                        .position(|n| matches!(n, TreeNode::Section(s) if *s == si))
                    {
                        self.cursor = pos;
                    }
                }
            }
            TreeNode::Session(si, ii, _) => {
                // Jump to parent item
                if let Some(pos) = self
                    .visible
                    .iter()
                    .position(|n| matches!(n, TreeNode::Item(s, i) if *s == si && *i == ii))
                {
                    self.cursor = pos;
                }
            }
        }
    }

    /// Get the currently selected tree node.
    pub fn selected_node(&self) -> Option<&TreeNode> {
        self.visible.get(self.cursor)
    }

    /// Get the SidebarItem the cursor is on (or the parent item for a session node).
    pub fn selected_item(&self) -> Option<&SidebarItem> {
        match self.visible.get(self.cursor)? {
            TreeNode::Section(_) => None,
            TreeNode::Item(si, ii) => self.sections.get(*si)?.items.get(*ii),
            TreeNode::Session(si, ii, _) => self.sections.get(*si)?.items.get(*ii),
        }
    }

    /// Get the mutable SidebarItem the cursor is on (or parent for session).
    pub fn selected_item_mut(&mut self) -> Option<&mut SidebarItem> {
        match self.visible.get(self.cursor)?.clone() {
            TreeNode::Section(_) => None,
            TreeNode::Item(si, ii) => self.sections.get_mut(si)?.items.get_mut(ii),
            TreeNode::Session(si, ii, _) => self.sections.get_mut(si)?.items.get_mut(ii),
        }
    }

    /// Get the selected session slot (only if cursor is on a Session node).
    pub fn selected_session(&self) -> Option<&SessionSlot> {
        match self.visible.get(self.cursor)? {
            TreeNode::Session(si, ii, slot) => {
                self.sections.get(*si)?.items.get(*ii)?.sessions.get(*slot)
            }
            _ => None,
        }
    }

    /// Get mutable selected session slot.
    pub fn selected_session_mut(&mut self) -> Option<&mut SessionSlot> {
        match self.visible.get(self.cursor)?.clone() {
            TreeNode::Session(si, ii, slot) => {
                self.sections.get_mut(si)?.items.get_mut(ii)?.sessions.get_mut(slot)
            }
            _ => None,
        }
    }

    /// Jump to the nth visible item (skipping section headers and session slots).
    /// `n` is 0-based. Returns true if jumped.
    pub fn jump_to_nth_item(&mut self, n: usize) -> bool {
        let mut count = 0;
        for (pos, node) in self.visible.iter().enumerate() {
            if matches!(node, TreeNode::Item(..)) {
                if count == n {
                    self.cursor = pos;
                    return true;
                }
                count += 1;
            }
        }
        false
    }

    /// Add a new section with the given name.
    pub fn add_section(&mut self, name: String) {
        self.sections.push(Section {
            name,
            collapsed: false,
            items: Vec::new(),
        });
        self.rebuild_visible();
    }

    /// Add a sidebar item to a section.
    pub fn add_item(&mut self, section_idx: usize, item: SidebarItem) {
        if let Some(section) = self.sections.get_mut(section_idx) {
            section.items.push(item);
            self.rebuild_visible();
        }
    }

    /// Add a session slot to the currently selected item.
    pub fn add_session_slot(&mut self, kind: SessionKind, label: String) -> bool {
        let node = self.visible.get(self.cursor).copied();
        let (si, ii) = match node {
            Some(TreeNode::Item(si, ii)) | Some(TreeNode::Session(si, ii, _)) => (si, ii),
            _ => return false,
        };
        if let Some(item) = self.sections.get_mut(si).and_then(|s| s.items.get_mut(ii)) {
            item.sessions.push(SessionSlot {
                kind,
                label,
                session_id: None,
            });
            item.collapsed = false;
            self.rebuild_visible();
            return true;
        }
        false
    }

    /// Set the session ID on a specific session slot.
    pub fn set_session_id(
        &mut self,
        section_idx: usize,
        item_idx: usize,
        slot_idx: usize,
        id: String,
    ) {
        if let Some(slot) = self
            .sections
            .get_mut(section_idx)
            .and_then(|s| s.items.get_mut(item_idx))
            .and_then(|item| item.sessions.get_mut(slot_idx))
        {
            slot.session_id = Some(id);
        }
    }

    /// Clear a session ID (e.g. on close/remove).
    pub fn clear_session_id(&mut self, session_id: &str) {
        for section in &mut self.sections {
            for item in &mut section.items {
                for slot in &mut item.sessions {
                    if slot.session_id.as_deref() == Some(session_id) {
                        slot.session_id = None;
                    }
                }
            }
        }
    }

    /// Collect all active session IDs across the tree.
    pub fn all_session_ids(&self) -> Vec<&str> {
        let mut ids = Vec::new();
        for section in &self.sections {
            for item in &section.items {
                for slot in &item.sessions {
                    if let Some(ref id) = slot.session_id {
                        ids.push(id.as_str());
                    }
                }
            }
        }
        ids
    }

    /// Find the item path for a given session ID.
    pub fn path_for_session(&self, session_id: &str) -> Option<&PathBuf> {
        for section in &self.sections {
            for item in &section.items {
                for slot in &item.sessions {
                    if slot.session_id.as_deref() == Some(session_id) {
                        return Some(&item.path);
                    }
                }
            }
        }
        None
    }

    /// Find the item name for a given session ID.
    pub fn name_for_session(&self, session_id: &str) -> Option<&str> {
        for section in &self.sections {
            for item in &section.items {
                for slot in &item.sessions {
                    if slot.session_id.as_deref() == Some(session_id) {
                        return Some(&item.display_name);
                    }
                }
            }
        }
        None
    }

    /// Find a session slot's tree coordinates by session ID.
    pub fn find_session_slot(&self, session_id: &str) -> Option<(usize, usize, usize)> {
        for (si, section) in self.sections.iter().enumerate() {
            for (ii, item) in section.items.iter().enumerate() {
                for (slot_idx, slot) in item.sessions.iter().enumerate() {
                    if slot.session_id.as_deref() == Some(session_id) {
                        return Some((si, ii, slot_idx));
                    }
                }
            }
        }
        None
    }

    /// Refresh worktrees: merge updated worktree list into existing sections.
    /// Preserves session IDs and user-added shell slots.
    pub fn refresh_worktrees(&mut self, worktrees: &[Worktree]) {
        if self.sections.is_empty() {
            *self = Self::from_worktrees(worktrees);
            return;
        }

        // For the first section (auto-generated), sync items with worktrees.
        let section = &mut self.sections[0];

        // Remove items whose paths no longer appear in worktrees
        section
            .items
            .retain(|item| worktrees.iter().any(|wt| wt.path == item.path));

        // Add new worktrees that don't exist yet
        for wt in worktrees {
            if !section.items.iter().any(|item| item.path == wt.path) {
                section.items.push(SidebarItem {
                    path: wt.path.clone(),
                    display_name: wt.name.clone(),
                    branch: wt.branch.clone(),
                    is_main: wt.is_main,
                    collapsed: true,
                    sessions: vec![SessionSlot {
                        kind: SessionKind::Claude,
                        label: "claude".to_string(),
                        session_id: None,
                    }],
                });
            }
        }

        // Update branch info for existing items
        for item in &mut section.items {
            if let Some(wt) = worktrees.iter().find(|wt| wt.path == item.path) {
                item.branch = wt.branch.clone();
                item.is_main = wt.is_main;
                item.display_name = wt.name.clone();
            }
        }

        self.rebuild_visible();
        // Clamp cursor
        if !self.visible.is_empty() && self.cursor >= self.visible.len() {
            self.cursor = self.visible.len() - 1;
        }
    }

    /// Get the index of the currently selected item among all items (for backward compat).
    /// Returns None if cursor is on a section header.
    pub fn selected_item_index(&self) -> Option<usize> {
        let node = self.visible.get(self.cursor)?;
        match node {
            TreeNode::Section(_) => None,
            TreeNode::Item(_, ii) | TreeNode::Session(_, ii, _) => Some(*ii),
        }
    }

    /// Get a flat list of all items across all sections (for backward compat).
    pub fn all_items(&self) -> Vec<&SidebarItem> {
        self.sections
            .iter()
            .flat_map(|s| s.items.iter())
            .collect()
    }

    /// Get the session slot coordinates for the cursor position.
    /// If on an Item node, returns the first session (Claude) slot coords.
    /// If on a Session node, returns those coords directly.
    pub fn cursor_session_coords(&self) -> Option<(usize, usize, usize)> {
        match self.visible.get(self.cursor)? {
            TreeNode::Section(_) => None,
            TreeNode::Item(si, ii) => Some((*si, *ii, 0)),
            TreeNode::Session(si, ii, slot) => Some((*si, *ii, *slot)),
        }
    }

    /// Find the first Claude session slot for an item by path.
    pub fn claude_session_for_path(&self, path: &PathBuf) -> Option<(usize, usize, usize)> {
        for (si, section) in self.sections.iter().enumerate() {
            for (ii, item) in section.items.iter().enumerate() {
                if &item.path == path {
                    for (slot_idx, slot) in item.sessions.iter().enumerate() {
                        if slot.kind == SessionKind::Claude {
                            return Some((si, ii, slot_idx));
                        }
                    }
                }
            }
        }
        None
    }

    /// Find the first Shell session slot for an item by path.
    pub fn shell_session_for_path(&self, path: &PathBuf) -> Option<(usize, usize, usize)> {
        for (si, section) in self.sections.iter().enumerate() {
            for (ii, item) in section.items.iter().enumerate() {
                if &item.path == path {
                    for (slot_idx, slot) in item.sessions.iter().enumerate() {
                        if slot.kind == SessionKind::Shell {
                            return Some((si, ii, slot_idx));
                        }
                    }
                }
            }
        }
        None
    }

    /// Convert to config format for persistence.
    pub fn to_sections_config(&self) -> crate::config::SectionsConfig {
        use crate::config::{SectionItemToml, SectionShellToml, SectionToml, SectionsConfig};
        let sections = self.sections.iter().map(|section| {
            let items = section.items.iter().map(|item| {
                let shells: Vec<SectionShellToml> = item.sessions.iter()
                    .filter(|s| s.kind == SessionKind::Shell)
                    .map(|s| SectionShellToml {
                        label: s.label.clone(),
                        command: None,
                    })
                    .collect();
                SectionItemToml {
                    path: item.path.to_string_lossy().to_string(),
                    shells,
                }
            }).collect();
            SectionToml {
                name: section.name.clone(),
                items,
            }
        }).collect();
        SectionsConfig { sections }
    }

    /// Load sections from config, merging with discovered worktrees.
    pub fn from_config(config: &crate::config::SectionsConfig, worktrees: &[Worktree]) -> Self {
        if config.sections.is_empty() {
            return Self::from_worktrees(worktrees);
        }

        let sections: Vec<Section> = config.sections.iter().map(|sect_toml| {
            let items: Vec<SidebarItem> = sect_toml.items.iter().map(|item_toml| {
                let path = PathBuf::from(&item_toml.path);
                // Match with worktree info if available
                let wt = worktrees.iter().find(|w| w.path == path);
                let mut sessions = vec![SessionSlot {
                    kind: SessionKind::Claude,
                    label: "claude".to_string(),
                    session_id: None,
                }];
                // Add configured shell slots
                for shell in &item_toml.shells {
                    sessions.push(SessionSlot {
                        kind: SessionKind::Shell,
                        label: shell.label.clone(),
                        session_id: None,
                    });
                }
                SidebarItem {
                    path: path.clone(),
                    display_name: wt.map(|w| w.name.clone())
                        .unwrap_or_else(|| path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| item_toml.path.clone())),
                    branch: wt.and_then(|w| w.branch.clone()),
                    is_main: wt.map(|w| w.is_main).unwrap_or(false),
                    collapsed: true,
                    sessions,
                }
            }).collect();
            Section {
                name: sect_toml.name.clone(),
                collapsed: false,
                items,
            }
        }).collect();

        let mut tree = Self {
            sections,
            visible: Vec::new(),
            cursor: 0,
        };
        tree.rebuild_visible();
        tree
    }
}
