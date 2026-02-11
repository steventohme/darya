use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use edtui::{EditorEventHandler, EditorMode, EditorState as EdtuiState, Lines as EdtuiLines};

const IGNORED_NAMES: &[&str] = &["target", "node_modules", "__pycache__"];
const MAX_FILE_SIZE: u64 = 1_048_576; // 1MB

use crate::config::Theme;
use crate::event::AppEvent;
use crate::worktree::types::Worktree;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Navigation,
    Terminal,
    Editor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelFocus {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKind {
    Worktrees,
    Terminal,
    FileExplorer,
    Editor,
}

pub struct PanelState {
    pub view: ViewKind,
}

/// Overlay prompts for user input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prompt {
    /// Creating a worktree: user types a branch name
    CreateWorktree { input: String },
    /// Confirming worktree deletion
    ConfirmDelete { worktree_name: String },
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
}

impl FileExplorerState {
    pub fn new(root: PathBuf) -> Self {
        let mut state = Self {
            entries: Vec::new(),
            selected: 0,
            expanded: HashSet::new(),
            root,
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
            if name.starts_with('.') || IGNORED_NAMES.contains(&name.as_str()) {
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
        if self.entries.is_empty() {
            return;
        }
        self.selected = if self.selected == 0 {
            self.entries.len() - 1
        } else {
            self.selected - 1
        };
    }

    pub fn move_down(&mut self) {
        if self.entries.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.entries.len();
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

    /// Set root to a new path (e.g. when switching worktrees).
    pub fn set_root(&mut self, path: PathBuf) {
        if self.root != path {
            self.root = path;
            self.expanded.clear();
            self.selected = 0;
            self.refresh();
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
        let metadata =
            std::fs::metadata(&path).map_err(|e| format!("Cannot read file: {}", e))?;
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

    pub fn save(&mut self) -> Result<(), String> {
        let content = self.editor_state.lines.to_string();
        std::fs::write(&self.file_path, content)
            .map_err(|e| format!("Failed to save: {}", e))?;
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

pub struct App {
    pub running: bool,
    pub input_mode: InputMode,
    pub panel_focus: PanelFocus,
    pub left_panel: PanelState,
    pub right_panel: PanelState,
    pub worktrees: Vec<Worktree>,
    pub selected_worktree: usize,
    /// Maps worktree path -> session ID for worktrees with active sessions
    pub session_ids: HashMap<PathBuf, String>,
    /// Currently active session ID (the one being displayed)
    pub active_session_id: Option<String>,
    /// Active prompt overlay (if any)
    pub prompt: Option<Prompt>,
    /// Sessions that have received a bell (needs attention)
    pub attention_sessions: HashSet<String>,
    /// Sessions whose process has exited
    pub exited_sessions: HashSet<String>,
    /// Status message to show briefly
    pub status_message: Option<String>,
    pub show_help: bool,
    pub theme: Theme,
    pub terminal_start_bottom: bool,
    /// Per-session scrollback offset (lines scrolled back from live view)
    pub scroll_offsets: HashMap<String, usize>,
    /// Height of the terminal panel area, used for page-scroll sizing
    pub terminal_height: u16,
    pub file_explorer: FileExplorerState,
    pub editor: Option<EditorViewState>,
}

impl App {
    pub fn new(worktrees: Vec<Worktree>, theme: Theme, terminal_start_bottom: bool) -> Self {
        let explorer_root = worktrees
            .first()
            .map(|wt| wt.path.clone())
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            running: true,
            input_mode: InputMode::Navigation,
            panel_focus: PanelFocus::Left,
            left_panel: PanelState { view: ViewKind::Worktrees },
            right_panel: PanelState { view: ViewKind::Terminal },
            worktrees,
            selected_worktree: 0,
            session_ids: HashMap::new(),
            active_session_id: None,
            attention_sessions: HashSet::new(),
            exited_sessions: HashSet::new(),
            prompt: None,
            status_message: None,
            show_help: false,
            theme,
            terminal_start_bottom,
            scroll_offsets: HashMap::new(),
            terminal_height: 24,
            file_explorer: FileExplorerState::new(explorer_root),
            editor: None,
        }
    }

    pub fn focused_view(&self) -> ViewKind {
        match self.panel_focus {
            PanelFocus::Left => self.left_panel.view,
            PanelFocus::Right => self.right_panel.view,
        }
    }

    pub fn focused_panel_mut(&mut self) -> &mut PanelState {
        match self.panel_focus {
            PanelFocus::Left => &mut self.left_panel,
            PanelFocus::Right => &mut self.right_panel,
        }
    }

    pub fn set_focused_view(&mut self, view: ViewKind) {
        self.focused_panel_mut().view = view;
    }

    /// Focus whichever panel currently shows the Terminal view.
    /// If neither panel has Terminal, switch the non-worktrees panel to Terminal and focus it.
    pub fn focus_terminal_panel(&mut self) {
        if self.left_panel.view == ViewKind::Terminal {
            self.panel_focus = PanelFocus::Left;
        } else if self.right_panel.view == ViewKind::Terminal {
            self.panel_focus = PanelFocus::Right;
        } else {
            // Neither panel shows Terminal — put it on the right panel
            self.right_panel.view = ViewKind::Terminal;
            self.panel_focus = PanelFocus::Right;
        }
    }

    pub fn open_editor_in_other_panel(&mut self) {
        match self.panel_focus {
            PanelFocus::Left => self.right_panel.view = ViewKind::Editor,
            PanelFocus::Right => self.left_panel.view = ViewKind::Editor,
        }
    }

    pub fn toggle_focus(&mut self) {
        self.panel_focus = match self.panel_focus {
            PanelFocus::Left => PanelFocus::Right,
            PanelFocus::Right => PanelFocus::Left,
        };
    }

    pub fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(*key),
            AppEvent::Resize(_w, _h) => {}
            AppEvent::PtyOutput { .. } => {}
            AppEvent::SessionBell { session_id } => {
                // Only mark as needing attention if it's not the currently viewed session in terminal mode
                if !(self.active_session_id.as_deref() == Some(session_id)
                    && self.input_mode == InputMode::Terminal)
                {
                    self.attention_sessions.insert(session_id.clone());
                }
            }
            AppEvent::SessionExited { session_id } => {
                self.exited_sessions.insert(session_id.clone());
                // If user is in terminal mode on this session, kick to nav mode
                if self.active_session_id.as_deref() == Some(session_id)
                    && self.input_mode == InputMode::Terminal
                {
                    self.input_mode = InputMode::Navigation;
                }
            }
            AppEvent::Tick => {}
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+C is handled entirely in the main event loop
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return;
        }

        // Handle prompt input first
        if self.prompt.is_some() {
            self.handle_prompt_key(key);
            return;
        }

        match self.input_mode {
            InputMode::Navigation => self.handle_nav_key(key),
            InputMode::Terminal => self.handle_terminal_key(key),
            InputMode::Editor => self.handle_editor_key(key),
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
        }
    }

    fn handle_nav_key(&mut self, key: KeyEvent) {
        // Ctrl+1/2/3: switch focused panel's view
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('1') => { self.set_focused_view(ViewKind::Worktrees); return; }
                KeyCode::Char('2') => { self.set_focused_view(ViewKind::Terminal); return; }
                KeyCode::Char('3') => { self.set_focused_view(ViewKind::FileExplorer); return; }
                KeyCode::Char('4') => { self.set_focused_view(ViewKind::Editor); return; }
                _ => {}
            }
        }

        if key.code == KeyCode::Char('?') {
            self.show_help = !self.show_help;
            return;
        }
        if self.show_help {
            self.show_help = false;
            return;
        }
        match self.focused_view() {
            ViewKind::Worktrees => self.handle_worktrees_key(key),
            ViewKind::Terminal => self.handle_terminal_nav_key(key),
            ViewKind::FileExplorer => self.handle_file_explorer_key(key),
            ViewKind::Editor => self.handle_editor_key(key),
        }
    }

    fn handle_worktrees_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char('j') | KeyCode::Down => {
                if !self.worktrees.is_empty() {
                    self.selected_worktree =
                        (self.selected_worktree + 1) % self.worktrees.len();
                    self.switch_to_selected_session();
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if !self.worktrees.is_empty() {
                    self.selected_worktree = if self.selected_worktree == 0 {
                        self.worktrees.len() - 1
                    } else {
                        self.selected_worktree - 1
                    };
                    self.switch_to_selected_session();
                }
            }
            KeyCode::Enter => {
                // Signal that we want to start a session for this worktree
                // Actual spawning is handled by main loop
            }
            // 1-9, 0 jump to worktree by index (0 = 10th)
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
                if let Some(wt) = self.worktrees.get(self.selected_worktree) {
                    if !wt.is_main {
                        self.prompt = Some(Prompt::ConfirmDelete {
                            worktree_name: wt.name.clone(),
                        });
                    } else {
                        self.status_message = Some("Cannot delete main worktree".to_string());
                    }
                }
            }
            KeyCode::Tab => {
                self.toggle_focus();
                // If we just focused a Terminal view with active non-exited session, enter terminal mode
                if self.focused_view() == ViewKind::Terminal {
                    if let Some(ref id) = self.active_session_id {
                        self.attention_sessions.remove(id);
                        if !self.exited_sessions.contains(id) {
                            self.input_mode = InputMode::Terminal;
                            self.reset_scroll();
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_terminal_nav_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Tab => {
                self.toggle_focus();
            }
            KeyCode::Char('i') | KeyCode::Enter => {
                if let Some(ref id) = self.active_session_id {
                    self.attention_sessions.remove(id);
                    if !self.exited_sessions.contains(id) {
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
        if key.code == KeyCode::Tab {
            self.input_mode = InputMode::Navigation;
            self.toggle_focus();
        }
        // All other keys get forwarded to PTY (handled in main loop)
    }

    fn handle_file_explorer_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char('j') | KeyCode::Down => self.file_explorer.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.file_explorer.move_up(),
            KeyCode::Enter | KeyCode::Char('l') => {
                if let Some(path) = self.file_explorer.enter() {
                    match EditorViewState::open(path) {
                        Ok(state) => {
                            let name = state.file_name().to_string();
                            self.editor = Some(state);
                            self.open_editor_in_other_panel();
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
                if self.focused_view() == ViewKind::Terminal {
                    if let Some(ref id) = self.active_session_id {
                        self.attention_sessions.remove(id);
                        if !self.exited_sessions.contains(id) {
                            self.input_mode = InputMode::Terminal;
                            self.reset_scroll();
                        }
                    }
                }
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
                KeyCode::Char('q') => self.running = false,
                _ => {}
            }
            return;
        };

        if self.input_mode == InputMode::Editor {
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
            editor.event_handler.on_key_event(key, &mut editor.editor_state);
            return;
        }

        // Read-only mode (Navigation): intercept our keys, forward the rest to edtui Normal mode
        match key.code {
            KeyCode::Char('e') => {
                editor.read_only = false;
                editor.editor_state.mode = EditorMode::Insert;
                self.input_mode = InputMode::Editor;
            }
            KeyCode::Char('q') => self.running = false,
            KeyCode::Tab => {
                self.toggle_focus();
                if self.focused_view() == ViewKind::Terminal {
                    if let Some(ref id) = self.active_session_id {
                        self.attention_sessions.remove(id);
                        if !self.exited_sessions.contains(id) {
                            self.input_mode = InputMode::Terminal;
                            self.reset_scroll();
                        }
                    }
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
                editor.event_handler.on_key_event(key, &mut editor.editor_state);
                // Force back to Normal in case edtui changed mode
                editor.editor_state.mode = EditorMode::Normal;
            }
        }
    }

    fn jump_to_worktree(&mut self, index: usize) {
        if index < self.worktrees.len() {
            self.selected_worktree = index;
            self.switch_to_selected_session();
        }
    }

    fn switch_to_selected_session(&mut self) {
        if let Some(wt) = self.worktrees.get(self.selected_worktree) {
            self.active_session_id = self.session_ids.get(&wt.path).cloned();
            // Clear attention when switching to a session
            if let Some(ref id) = self.active_session_id {
                self.attention_sessions.remove(id);
            }
            self.file_explorer.set_root(wt.path.clone());
        }
    }

    pub fn scroll_up(&mut self, lines: usize) {
        if let Some(ref id) = self.active_session_id {
            let offset = self.scroll_offsets.entry(id.clone()).or_insert(0);
            *offset = offset.saturating_add(lines).min(1000);
        }
    }

    pub fn scroll_down(&mut self, lines: usize) {
        if let Some(ref id) = self.active_session_id {
            let offset = self.scroll_offsets.entry(id.clone()).or_insert(0);
            *offset = offset.saturating_sub(lines);
            if *offset == 0 {
                self.scroll_offsets.remove(id);
            }
        }
    }

    pub fn reset_scroll(&mut self) {
        if let Some(ref id) = self.active_session_id {
            self.scroll_offsets.remove(id);
        }
    }

    pub fn active_scroll_offset(&self) -> usize {
        self.active_session_id
            .as_ref()
            .and_then(|id| self.scroll_offsets.get(id))
            .copied()
            .unwrap_or(0)
    }

    pub fn selected_worktree_path(&self) -> Option<&PathBuf> {
        self.worktrees.get(self.selected_worktree).map(|wt| &wt.path)
    }

    pub fn needs_session_restart(&self, key: &KeyEvent) -> bool {
        if key.code != KeyCode::Char('r')
            || self.prompt.is_some()
            || self.show_help
            || self.input_mode != InputMode::Navigation
        {
            return false;
        }
        self.selected_worktree_path()
            .and_then(|p| self.session_ids.get(p))
            .map(|id| self.exited_sessions.contains(id))
            .unwrap_or(false)
    }

    pub fn needs_session_spawn(&self, key: &KeyEvent) -> bool {
        self.prompt.is_none()
            && !self.show_help
            && key.code == KeyCode::Enter
            && self.input_mode == InputMode::Navigation
            && matches!(self.focused_view(), ViewKind::Worktrees | ViewKind::Terminal)
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

    pub fn refresh_worktrees(&mut self, worktrees: Vec<Worktree>) {
        self.worktrees = worktrees;
        if self.worktrees.is_empty() {
            self.selected_worktree = 0;
        } else if self.selected_worktree >= self.worktrees.len() {
            self.selected_worktree = self.worktrees.len() - 1;
        }
        // Sync file explorer root with current worktree
        if let Some(wt) = self.worktrees.get(self.selected_worktree) {
            self.file_explorer.set_root(wt.path.clone());
        }
    }
}
