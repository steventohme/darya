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
                custom_name: None,
                branch: wt.branch.clone(),
                is_main: wt.is_main,
                collapsed: true,
                sessions: vec![SessionSlot {
                    kind: SessionKind::Claude,
                    label: "claude".to_string(),
                    session_id: None,
                    color: None,
                    conversation_id: None,
                }],
                color: None,
            })
            .collect();

        let sections = vec![Section {
            name: repo_name,
            collapsed: false,
            items,
            root_path: None,
            color: None,
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
        self.cursor = crate::app::wrapping_next(self.cursor, self.visible.len());
    }

    pub fn move_up(&mut self) {
        self.cursor = crate::app::wrapping_prev(self.cursor, self.visible.len());
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
                self.sections[si].items[ii].collapsed = !self.sections[si].items[ii].collapsed;
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

    /// Get the path for the current cursor position.
    /// For items/sessions, returns the item's path.
    /// For section headers, returns the first item's path in that section.
    pub fn selected_path(&self) -> Option<&std::path::PathBuf> {
        match self.visible.get(self.cursor)? {
            TreeNode::Section(si) => {
                let section = self.sections.get(*si)?;
                section.items.first().map(|item| &item.path)
            }
            TreeNode::Item(si, ii) => Some(&self.sections.get(*si)?.items.get(*ii)?.path),
            TreeNode::Session(si, ii, _) => Some(&self.sections.get(*si)?.items.get(*ii)?.path),
        }
    }

    /// Get the mutable SidebarItem the cursor is on (or parent for session).
    pub fn selected_item_mut(&mut self) -> Option<&mut SidebarItem> {
        match *self.visible.get(self.cursor)? {
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
        match *self.visible.get(self.cursor)? {
            TreeNode::Session(si, ii, slot) => self
                .sections
                .get_mut(si)?
                .items
                .get_mut(ii)?
                .sessions
                .get_mut(slot),
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

    /// Add a new section with the given name and optional root directory.
    pub fn add_section(&mut self, name: String, root_path: Option<PathBuf>) {
        self.sections.push(Section {
            name,
            collapsed: false,
            items: Vec::new(),
            root_path,
            color: None,
        });
        self.rebuild_visible();
    }

    /// Remove a section by index. Returns the removed section's active session IDs.
    /// Refuses to remove the last section (always keep at least one).
    pub fn remove_section(&mut self, section_idx: usize) -> Vec<String> {
        if self.sections.len() <= 1 || section_idx >= self.sections.len() {
            return Vec::new();
        }
        let section = self.sections.remove(section_idx);
        // Collect all active session IDs from the removed section
        let session_ids: Vec<String> = section
            .items
            .iter()
            .flat_map(|item| item.sessions.iter())
            .filter_map(|slot| slot.session_id.clone())
            .collect();
        self.rebuild_visible();
        session_ids
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
        let (si, ii, insert_after) = match node {
            Some(TreeNode::Item(si, ii)) => (si, ii, 0usize),
            Some(TreeNode::Session(si, ii, slot_idx)) => (si, ii, slot_idx + 1),
            _ => return false,
        };
        if let Some(item) = self.sections.get_mut(si).and_then(|s| s.items.get_mut(ii)) {
            let insert_idx = insert_after.min(item.sessions.len());
            item.sessions.insert(insert_idx, SessionSlot {
                kind,
                label,
                session_id: None,
                color: None,
                conversation_id: None,
            });
            item.collapsed = false;
            self.rebuild_visible();
            // Move cursor to the newly inserted session slot
            if let Some(pos) = self.visible.iter().position(|n| {
                matches!(n, TreeNode::Session(s, i, sl) if *s == si && *i == ii && *sl == insert_idx)
            }) {
                self.cursor = pos;
            }
            return true;
        }
        false
    }

    /// Remove a session slot at the given position. Returns the session_id if one was active.
    /// Refuses to remove the last remaining slot on an item.
    pub fn remove_session_slot(
        &mut self,
        section_idx: usize,
        item_idx: usize,
        slot_idx: usize,
    ) -> Option<String> {
        let item = self
            .sections
            .get_mut(section_idx)?
            .items
            .get_mut(item_idx)?;
        if item.sessions.len() <= 1 || slot_idx >= item.sessions.len() {
            return None;
        }
        let removed = item.sessions.remove(slot_idx);
        self.rebuild_visible();
        removed.session_id
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

    /// Set the Claude Code conversation ID on a specific session slot.
    pub fn set_conversation_id(
        &mut self,
        section_idx: usize,
        item_idx: usize,
        slot_idx: usize,
        conversation_id: String,
    ) {
        if let Some(slot) = self
            .sections
            .get_mut(section_idx)
            .and_then(|s| s.items.get_mut(item_idx))
            .and_then(|item| item.sessions.get_mut(slot_idx))
        {
            slot.conversation_id = Some(conversation_id);
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
                        return Some(item.visible_name());
                    }
                }
            }
        }
        None
    }

    /// Look up both the section name and item name for a session ID.
    pub fn section_and_name_for_session(&self, session_id: &str) -> Option<(&str, &str)> {
        for section in &self.sections {
            for item in &section.items {
                for slot in &item.sessions {
                    if slot.session_id.as_deref() == Some(session_id) {
                        return Some((&section.name, item.visible_name()));
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

    /// Refresh worktrees for a specific section by index.
    /// Preserves session IDs and user-added shell slots.
    pub fn refresh_section_worktrees(&mut self, section_idx: usize, worktrees: &[Worktree]) {
        let Some(section) = self.sections.get_mut(section_idx) else {
            return;
        };

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
                    custom_name: None,
                    branch: wt.branch.clone(),
                    is_main: wt.is_main,
                    collapsed: true,
                    sessions: vec![SessionSlot {
                        kind: SessionKind::Claude,
                        label: "claude".to_string(),
                        session_id: None,
                        color: None,
                        conversation_id: None,
                    }],
                    color: None,
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
    }

    /// Refresh worktrees: merge updated worktree list into the first (auto-generated) section.
    pub fn refresh_worktrees(&mut self, worktrees: &[Worktree]) {
        if self.sections.is_empty() {
            *self = Self::from_worktrees(worktrees);
            return;
        }

        self.refresh_section_worktrees(0, worktrees);
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
        self.sections.iter().flat_map(|s| s.items.iter()).collect()
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
        use crate::config::{
            color_to_hex, SectionItemToml, SectionShellToml, SectionToml, SectionsConfig,
        };
        let sections = self
            .sections
            .iter()
            .map(|section| {
                let items = section
                    .items
                    .iter()
                    .map(|item| {
                        let shells: Vec<SectionShellToml> = item
                            .sessions
                            .iter()
                            .filter(|s| s.kind == SessionKind::Shell)
                            .map(|s| SectionShellToml {
                                label: s.label.clone(),
                                command: None,
                                color: s.color.and_then(color_to_hex),
                            })
                            .collect();
                        SectionItemToml {
                            path: item.path.to_string_lossy().to_string(),
                            name: item.custom_name.clone(),
                            shells,
                            color: item.color.and_then(color_to_hex),
                        }
                    })
                    .collect();
                SectionToml {
                    name: section.name.clone(),
                    root: section
                        .root_path
                        .as_ref()
                        .map(|p| p.to_string_lossy().to_string()),
                    items,
                    color: section.color.and_then(color_to_hex),
                }
            })
            .collect();
        SectionsConfig { sections }
    }

    /// Load sections from config, merging with discovered worktrees.
    pub fn from_config(config: &crate::config::SectionsConfig, worktrees: &[Worktree]) -> Self {
        use crate::config::parse_hex_color;

        if config.sections.is_empty() {
            return Self::from_worktrees(worktrees);
        }

        let sections: Vec<Section> = config
            .sections
            .iter()
            .map(|sect_toml| {
                let items: Vec<SidebarItem> = sect_toml
                    .items
                    .iter()
                    .map(|item_toml| {
                        let path = PathBuf::from(&item_toml.path);
                        // Match with worktree info if available
                        let wt = worktrees.iter().find(|w| w.path == path);
                        let mut sessions = vec![SessionSlot {
                            kind: SessionKind::Claude,
                            label: "claude".to_string(),
                            session_id: None,
                            color: None,
                            conversation_id: None,
                        }];
                        // Add configured shell slots
                        for shell in &item_toml.shells {
                            sessions.push(SessionSlot {
                                kind: SessionKind::Shell,
                                label: shell.label.clone(),
                                session_id: None,
                                color: shell.color.as_deref().and_then(parse_hex_color),
                                conversation_id: None,
                            });
                        }
                        SidebarItem {
                            path: path.clone(),
                            display_name: wt.map(|w| w.name.clone()).unwrap_or_else(|| {
                                path.file_name()
                                    .map(|n| n.to_string_lossy().to_string())
                                    .unwrap_or_else(|| item_toml.path.clone())
                            }),
                            custom_name: item_toml.name.clone(),
                            branch: wt.and_then(|w| w.branch.clone()),
                            is_main: wt.map(|w| w.is_main).unwrap_or(false),
                            collapsed: true,
                            sessions,
                            color: item_toml.color.as_deref().and_then(parse_hex_color),
                        }
                    })
                    .collect();
                Section {
                    name: sect_toml.name.clone(),
                    collapsed: false,
                    items,
                    root_path: sect_toml.root.as_ref().map(PathBuf::from),
                    color: sect_toml.color.as_deref().and_then(parse_hex_color),
                }
            })
            .collect();

        let mut tree = Self {
            sections,
            visible: Vec::new(),
            cursor: 0,
        };
        tree.rebuild_visible();
        tree
    }
}
