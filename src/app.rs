use std::collections::HashMap;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::Theme;
use crate::event::AppEvent;
use crate::worktree::types::Worktree;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    Navigation,
    Terminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panel {
    Sidebar,
    Terminal,
}

/// Overlay prompts for user input
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prompt {
    /// Creating a worktree: user types a branch name
    CreateWorktree { input: String },
    /// Confirming worktree deletion
    ConfirmDelete { worktree_name: String },
}

pub struct App {
    pub running: bool,
    pub input_mode: InputMode,
    pub active_panel: Panel,
    pub worktrees: Vec<Worktree>,
    pub selected_worktree: usize,
    /// Maps worktree path -> session ID for worktrees with active sessions
    pub session_ids: HashMap<PathBuf, String>,
    /// Currently active session ID (the one being displayed)
    pub active_session_id: Option<String>,
    /// Active prompt overlay (if any)
    pub prompt: Option<Prompt>,
    /// Status message to show briefly
    pub status_message: Option<String>,
    pub theme: Theme,
}

impl App {
    pub fn new(worktrees: Vec<Worktree>, theme: Theme) -> Self {
        Self {
            running: true,
            input_mode: InputMode::Navigation,
            active_panel: Panel::Sidebar,
            worktrees,
            selected_worktree: 0,
            session_ids: HashMap::new(),
            active_session_id: None,
            prompt: None,
            status_message: None,
            theme,
        }
    }

    pub fn handle_event(&mut self, event: &AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(*key),
            AppEvent::Resize(_w, _h) => {}
            AppEvent::PtyOutput { .. } => {}
            AppEvent::Tick => {}
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        // Ctrl+c always quits (unless in a prompt)
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            if self.prompt.is_some() {
                self.prompt = None;
                return;
            }
            self.running = false;
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
        match self.active_panel {
            Panel::Sidebar => self.handle_sidebar_key(key),
            Panel::Terminal => self.handle_terminal_panel_nav_key(key),
        }
    }

    fn handle_sidebar_key(&mut self, key: KeyEvent) {
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
                self.active_panel = Panel::Terminal;
            }
            _ => {}
        }
    }

    fn handle_terminal_panel_nav_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Tab => {
                self.active_panel = Panel::Sidebar;
            }
            KeyCode::Char('i') | KeyCode::Enter => {
                if self.active_session_id.is_some() {
                    self.input_mode = InputMode::Terminal;
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

    fn handle_terminal_key(&mut self, key: KeyEvent) {
        // Esc exits terminal mode back to navigation
        if key.code == KeyCode::Esc {
            self.input_mode = InputMode::Navigation;
        }
        // In terminal mode, keys get forwarded to PTY (handled in main loop)
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
        }
    }

    pub fn selected_worktree_path(&self) -> Option<&PathBuf> {
        self.worktrees.get(self.selected_worktree).map(|wt| &wt.path)
    }

    pub fn needs_session_spawn(&self, key: &KeyEvent) -> bool {
        self.prompt.is_none()
            && self.active_panel == Panel::Sidebar
            && key.code == KeyCode::Enter
            && self.input_mode == InputMode::Navigation
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
    }
}
