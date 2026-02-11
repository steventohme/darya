use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use edtui::{EditorEventHandler, EditorMode, EditorState as EdtuiState, Index2, Lines as EdtuiLines};

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
    Search,
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
    /// Project search input
    SearchInput { input: String },
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
                pattern.score(haystack, &mut matcher).map(|s| (s, f.as_str()))
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
        if !self.results.is_empty() {
            self.selected = if self.selected == 0 {
                self.results.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn move_down(&mut self) {
        if !self.results.is_empty() {
            self.selected = (self.selected + 1) % self.results.len();
        }
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
        if entry.file_type().map_or(false, |ft| ft.is_file()) {
            if let Ok(rel) = entry.path().strip_prefix(root) {
                files.push(rel.to_string_lossy().to_string());
            }
        }
    }
    files.sort();
    files
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
        if !self.results.is_empty() {
            self.selected = if self.selected == 0 {
                self.results.len() - 1
            } else {
                self.selected - 1
            };
        }
    }

    pub fn move_down(&mut self) {
        if !self.results.is_empty() {
            self.selected = (self.selected + 1) % self.results.len();
        }
    }

    pub fn selected_result(&self) -> Option<&SearchResult> {
        self.results.get(self.selected)
    }
}

fn run_ripgrep(query: &str, root: &Path) -> Result<Vec<SearchResult>, String> {
    let output = std::process::Command::new("rg")
        .args(["--line-number", "--no-heading", "--color=never", "--max-count=200", query])
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
    pub search: Option<SearchViewState>,
    pub fuzzy_finder: Option<FuzzyFinderState>,
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
            search: None,
            fuzzy_finder: None,
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

        // Ctrl+P and Ctrl+F are handled in main.rs event loop
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('p') | KeyCode::Char('f'))
        {
            return;
        }

        // Fuzzy finder gets exclusive keyboard focus
        if self.fuzzy_finder.is_some() {
            self.handle_fuzzy_finder_key(key);
            return;
        }

        // Handle prompt input
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
            Prompt::SearchInput { input } => match key.code {
                KeyCode::Enter => {
                    if !input.is_empty() {
                        let query = input.clone();
                        let root = self.file_explorer.root.clone();
                        let search_state = SearchViewState::new(&query, &root);
                        self.search = Some(search_state);
                        self.prompt = None;
                        self.set_focused_view(ViewKind::Search);
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
                                self.set_focused_view(ViewKind::Editor);
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

    fn handle_nav_key(&mut self, key: KeyEvent) {
        // Ctrl+1/2/3: switch focused panel's view
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('1') => { self.set_focused_view(ViewKind::Worktrees); return; }
                KeyCode::Char('2') => { self.set_focused_view(ViewKind::Terminal); return; }
                KeyCode::Char('3') => { self.set_focused_view(ViewKind::FileExplorer); return; }
                KeyCode::Char('4') => { self.set_focused_view(ViewKind::Editor); return; }
                KeyCode::Char('5') => { self.set_focused_view(ViewKind::Search); return; }
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
            ViewKind::Search => self.handle_search_key(key),
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

    fn handle_search_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.running = false,
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
                                self.open_editor_in_other_panel();
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
