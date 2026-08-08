use std::{
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use crossterm::{
    cursor,
    event::{
        self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton, MouseEvent,
        MouseEventKind,
    },
    execute, queue,
    style::{self, Color},
    terminal,
};
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session, TorrentStatsState,
};
use tokio::sync::mpsc;

use crate::downloads::{format_completed, list_completed_downloads};

const BANNER: [&str; 5] = [
    "  ___   _ __   ____  _  _  _____  ___   ____   ____   ____  _  _  _____",
    " / _ \\ |  __| |  _ \\ | \\/ | |_   _| / _ \\ |  _ \\ |  _ \\ |  _ \\ | \\/ | |_   _|",
    "| | | || |_   | |_) || \\/ |   | |  | | | || |_) || |_) || |_) || \\/ |   | |",
    "| |_| ||  _|  |  _ < | |\\/| |  | |  | |_| ||  _ < |  _ < |  _ < | |\\/| |  | |",
    " \\___/ |_|    |_| \\_\\ |_|  |_|  |_|   \\___/ |_| \\_\\ |_| \\_\\ |_| \\_\\ |_|  |_|  |_|",
];

const MENU_ITEMS: [&str; 4] = [
    "Adicionar torrent a lista",
    "Acompanhar progresso",
    "Listar downloads completos",
    "Sair",
];

const MENU_SHORTCUTS: [char; 4] = ['a', 'c', 'l', 's'];

/// Largura de cada botão de ação renderizado nas linhas da sessão (ex.: `[Pausar ]`).
const ACTION_BTN_WIDTH: u16 = 9;
/// Espaço entre botões de ação.
const ACTION_GAP: u16 = 1;
/// Largura total ocupada pelos três botões de ação.
const ACTIONS_WIDTH: u16 = 3 * ACTION_BTN_WIDTH + 2 * ACTION_GAP;
/// Início da área do botão "parar" (após o primeiro botão + espaço).
const ACTION_1_START: u16 = ACTION_BTN_WIDTH + ACTION_GAP;
/// Início da área do botão "excluir" (após dois botões + dois espaços).
const ACTION_2_START: u16 = 2 * (ACTION_BTN_WIDTH + ACTION_GAP);
/// Largura máxima da parte informativa da linha da sessão (antes dos botões).
const ROW_INFO_WIDTH: u16 = 60;

/// The state machine driving the interactive UI.
enum View {
    /// Title screen with ASCII art and the command input field.
    Title,
    /// Dropdown menu opened by typing `/`.
    Menu,
    /// US-009 session list with per-row actions.
    Session,
    /// US-009 simplified include dialog (type source / back).
    Include,
    /// US-010 completed downloads list.
    Completed,
}

struct SessionRow {
    id: usize,
    name: String,
    state: TorrentStatsState,
    progress_bytes: u64,
    total_bytes: u64,
    finished: bool,
    handle: Arc<ManagedTorrent>,
}

/// Geometria do último render, usada para mapear cliques do mouse para ações.
/// Recalculada a cada frame, considerando o tamanho atual do terminal.
#[derive(Clone, Copy, Debug)]
struct Layout {
    /// Linha da tela onde o bloco de conteúdo começa.
    top: u16,
    /// Coluna da tela onde o bloco de conteúdo começa.
    left: u16,
    /// Deslocamento (em linhas) entre o topo do bloco e a primeira entrada.
    rows_offset: u16,
    /// Número de entradas renderizadas.
    row_count: usize,
    /// Largura da parte informativa da linha (define a coluna dos botões).
    info_width: u16,
    /// Se a sessão exibe botões de ação clicáveis.
    show_actions: bool,
}

struct Tui {
    session: Arc<Session>,
    output_folder: PathBuf,
    view: View,
    /// Current command being typed in the title input field.
    input: String,
    /// Selected menu item index.
    menu_index: usize,
    /// Selected session row index.
    row_index: usize,
    /// Selected include-dialog option.
    include_index: usize,
    /// The torrent source typed in the include dialog.
    include_input: String,
    /// Cursor position within `include_input` (byte index).
    input_cursor_pos: usize,
    /// Whether the include dialog is in "typing" mode (entering the source).
    typing: bool,
    /// Transient status/error message.
    notice: Option<String>,
    /// Keys received from the keyboard thread.
    keys: mpsc::UnboundedReceiver<Event>,
    /// Geometria do último render (usada para mapear cliques do mouse).
    layout: Option<Layout>,
    running: bool,
}

impl Tui {
    fn new(
        session: Arc<Session>,
        output_folder: PathBuf,
        keys: mpsc::UnboundedReceiver<Event>,
    ) -> Self {
        Self {
            session,
            output_folder,
            view: View::Title,
            input: String::new(),
            menu_index: 0,
            row_index: 0,
            include_index: 0,
            include_input: String::new(),
            input_cursor_pos: 0,
            typing: false,
            notice: None,
            keys,
            layout: None,
            running: true,
        }
    }

    /// Handle one key event, returning `true` if the UI should keep running.
    async fn handle_key(&mut self, key: KeyEvent) -> anyhow::Result<bool> {
        if key.kind == KeyEventKind::Release {
            return Ok(true);
        }

        match self.view {
            View::Title => self.handle_title_key(key)?,
            View::Menu => self.handle_menu_key(key)?,
            View::Session => self.handle_session_key(key).await?,
            View::Include => self.handle_include_key(key).await?,
            View::Completed => self.handle_completed_key(key)?,
        }
        Ok(self.running)
    }

    fn handle_title_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Char('/') => {
                self.input = "/".into();
                self.menu_index = 0;
                self.view = View::Menu;
            }
            // FR-003a: keys that do not start with `/` are inert input.
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_menu_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                self.menu_index = (self.menu_index + MENU_ITEMS.len() - 1) % MENU_ITEMS.len();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                self.menu_index = (self.menu_index + 1) % MENU_ITEMS.len();
            }
            KeyCode::Enter => self.select_menu_item(self.menu_index),
            KeyCode::Char(c) => {
                if let Some(idx) = MENU_SHORTCUTS
                    .iter()
                    .position(|&s| s == c.to_ascii_lowercase())
                {
                    self.select_menu_item(idx);
                }
                // FR: unknown commands are silently ignored, field stays clean.
            }
            KeyCode::Esc => {
                self.input.clear();
                self.view = View::Title;
            }
            _ => {}
        }
        Ok(())
    }

    fn select_menu_item(&mut self, idx: usize) {
        match idx {
            0 => {
                self.include_index = 0;
                self.include_input.clear();
                self.view = View::Include;
            }
            1 => {
                self.row_index = 0;
                self.view = View::Session;
            }
            2 => self.view = View::Completed,
            3 => self.running = false,
            _ => unreachable!("menu index out of range"),
        }
    }

    async fn handle_session_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.move_row(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_row(1),
            KeyCode::Char('p') | KeyCode::Char('P') => self.pause_row().await?,
            KeyCode::Char('r') | KeyCode::Char('R') => self.resume_row().await?,
            KeyCode::Char('x') | KeyCode::Char('X') => self.remove_row().await?,
            KeyCode::Char('a') | KeyCode::Char('A') | KeyCode::Char('+') => {
                self.include_index = 0;
                self.include_input.clear();
                self.view = View::Include;
            }
            KeyCode::Esc => self.view = View::Menu,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_include_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        if self.typing {
            return self.handle_include_typing_key(key).await;
        }

        match key.code {
            KeyCode::Up | KeyCode::Char('k') => self.include_index = 0,
            KeyCode::Down | KeyCode::Char('j') => self.include_index = 1,
            KeyCode::Enter => match self.include_index {
                0 => {
                    self.typing = true;
                    self.include_input.clear();
                    self.input_cursor_pos = 0;
                    self.notice = None;
                }
                1 => self.view = View::Session,
                _ => unreachable!("include index out of range"),
            },
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            KeyCode::Esc => self.view = View::Session,
            _ => {}
        }
        Ok(())
    }

    async fn handle_include_typing_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Enter => {
                self.typing = false;
                self.add_included().await?;
            }
            KeyCode::Esc => {
                self.typing = false;
                self.include_input.clear();
                self.input_cursor_pos = 0;
            }
            KeyCode::Backspace => {
                if self.input_cursor_pos > 0 {
                    // Remove character before cursor
                    let before = &self.include_input[..self.input_cursor_pos];
                    let after = &self.include_input[self.input_cursor_pos..];
                    // Find the previous character boundary
                    if let Some((new_pos, _)) = before.char_indices().next_back() {
                        self.include_input = format!("{}{}", &before[..new_pos], after);
                        self.input_cursor_pos = new_pos;
                    }
                }
            }
            KeyCode::Delete => {
                if self.input_cursor_pos < self.include_input.len() {
                    // Remove character at cursor
                    let before = &self.include_input[..self.input_cursor_pos];
                    let after = &self.include_input[self.input_cursor_pos..];
                    if let Some((next_pos, _)) = after.char_indices().next() {
                        self.include_input = format!("{}{}", before, &after[next_pos..]);
                    } else {
                        self.include_input = before.to_string();
                    }
                }
            }
            KeyCode::Left => {
                if self.input_cursor_pos > 0 {
                    // Move cursor one character left
                    let before = &self.include_input[..self.input_cursor_pos];
                    if let Some((new_pos, _)) = before.char_indices().next_back() {
                        self.input_cursor_pos = new_pos;
                    } else {
                        self.input_cursor_pos = 0;
                    }
                }
            }
            KeyCode::Right => {
                if self.input_cursor_pos < self.include_input.len() {
                    // Move cursor one character right
                    let after = &self.include_input[self.input_cursor_pos..];
                    if let Some((next_pos, _)) = after.char_indices().next() {
                        self.input_cursor_pos += next_pos;
                    } else {
                        self.input_cursor_pos = self.include_input.len();
                    }
                }
            }
            KeyCode::Home => {
                self.input_cursor_pos = 0;
            }
            KeyCode::End => {
                self.input_cursor_pos = self.include_input.len();
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+A = Home
                self.input_cursor_pos = 0;
            }
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+E = End
                self.input_cursor_pos = self.include_input.len();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Insert character at cursor position
                let before = &self.include_input[..self.input_cursor_pos];
                let after = &self.include_input[self.input_cursor_pos..];
                self.include_input = format!("{}{}{}", before, c, after);
                self.input_cursor_pos += c.len_utf8();
                self.notice = None;
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_completed_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Esc => self.view = View::Menu,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle one mouse event, dispatching clicks to the active view.
    async fn handle_mouse(&mut self, me: MouseEvent) -> anyhow::Result<()> {
        if me.kind != MouseEventKind::Down(MouseButton::Left) {
            return Ok(());
        }
        match self.view {
            View::Session => self.mouse_session(me.column, me.row).await,
            _ => Ok(()),
        }
    }

    /// Maps a click on the session view to the clicked row and action button.
    ///
    /// The coordinates are relative to the `Layout` stored by the last render,
    /// so window resizes are reflected on the very next frame.
    async fn mouse_session(&mut self, col: u16, row: u16) -> anyhow::Result<()> {
        let Some(layout) = self.layout else {
            return Ok(());
        };
        if !layout.show_actions {
            return Ok(());
        }
        let first_row = layout.top.saturating_add(layout.rows_offset);
        let end = first_row.saturating_add(layout.row_count as u16);
        if row < first_row || row >= end {
            return Ok(());
        }
        let idx = (row - first_row) as usize;

        let rows = self.session_rows();
        let Some(row_data) = rows.get(idx) else {
            return Ok(());
        };

        let actions_col = layout.left.saturating_add(layout.info_width + 1);
        if col < actions_col || col >= actions_col + ACTIONS_WIDTH {
            // Clique fora dos botões: apenas move a seleção para a linha.
            self.row_index = idx;
            return Ok(());
        }
        let Some(button) = action_button_at(col - actions_col) else {
            return Ok(());
        };
        match button {
            0 => {
                let paused = matches!(
                    row_data.state,
                    TorrentStatsState::Paused | TorrentStatsState::Error
                );
                if paused {
                    self.session.clone().unpause(&row_data.handle).await?;
                    self.notice = Some(format!("retomado: {}", row_data.name));
                } else {
                    self.session.pause(&row_data.handle).await?;
                    self.notice = Some(format!("pausado: {}", row_data.name));
                }
            }
            1 => {
                self.session
                    .delete(librqbit::api::TorrentIdOrHash::Id(row_data.id), false)
                    .await?;
                self.notice = Some(format!("parado: {}", row_data.name));
                self.clamp_row_index();
            }
            2 => {
                self.session
                    .delete(librqbit::api::TorrentIdOrHash::Id(row_data.id), true)
                    .await?;
                self.notice = Some(format!("excluído: {}", row_data.name));
                self.clamp_row_index();
            }
            _ => {}
        }
        Ok(())
    }

    /// Mantém `row_index` dentro dos limites após remoção de linhas.
    fn clamp_row_index(&mut self) {
        let remaining = self.session_rows().len();
        if remaining == 0 {
            self.row_index = 0;
        } else if self.row_index >= remaining {
            self.row_index = remaining - 1;
        }
    }

    /// Trunca o texto para caber em `max` caracteres, adicionando "…" se necessário.
    fn truncate(s: &str, max: usize) -> String {
        if s.chars().count() <= max {
            return s.to_string();
        }
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }

    /// Wrap text to fit within the given width (soft wrap by words).
    /// Returns a vector of visual lines.
    fn wrap_text(text: &str, width: usize) -> Vec<String> {
        if width == 0 {
            return vec![String::new()];
        }
        let mut lines = Vec::new();
        let mut current_line = String::new();

        for word in text.split_inclusive(' ') {
            // If adding this word would exceed width, start a new line
            if current_line.chars().count() + word.chars().count() > width {
                if !current_line.is_empty() {
                    lines.push(current_line);
                    current_line = String::new();
                }
                // If a single word is longer than width, hard-break it
                if word.chars().count() > width {
                    let mut remaining = word;
                    while remaining.chars().count() > width {
                        let (line_part, rest) = remaining.split_at(
                            remaining
                                .char_indices()
                                .nth(width)
                                .map(|(i, _)| i)
                                .unwrap_or(remaining.len()),
                        );
                        lines.push(line_part.to_string());
                        remaining = rest;
                    }
                    current_line = remaining.to_string();
                } else {
                    current_line.push_str(word);
                }
            } else {
                current_line.push_str(word);
            }
        }

        if !current_line.is_empty() || lines.is_empty() {
            lines.push(current_line);
        }

        lines
    }

    fn move_row(&mut self, delta: isize) {
        let rows = self.session_rows();
        if rows.is_empty() {
            self.row_index = 0;
            return;
        }
        let len = rows.len() as isize;
        let next = (self.row_index as isize + delta).rem_euclid(len);
        self.row_index = next as usize;
    }

    fn session_rows(&self) -> Vec<SessionRow> {
        let rows = std::cell::RefCell::new(Vec::new());
        self.session.with_torrents(|torrents| {
            for (id, handle) in torrents {
                let stats = handle.stats();
                rows.borrow_mut().push(SessionRow {
                    id,
                    name: handle.name().unwrap_or_else(|| format!("torrent #{id}")),
                    state: stats.state,
                    progress_bytes: stats.progress_bytes,
                    total_bytes: stats.total_bytes,
                    finished: stats.finished,
                    handle: handle.clone(),
                });
            }
        });
        let mut rows = rows.into_inner();
        rows.sort_by_key(|r| r.id);
        rows
    }

    async fn pause_row(&mut self) -> anyhow::Result<()> {
        let rows = self.session_rows();
        if let Some(row) = rows.get(self.row_index) {
            self.session.pause(&row.handle).await?;
            self.notice = Some(format!("pausado: {}", row.name));
        }
        Ok(())
    }

    async fn resume_row(&mut self) -> anyhow::Result<()> {
        let rows = self.session_rows();
        if let Some(row) = rows.get(self.row_index) {
            self.session.clone().unpause(&row.handle).await?;
            self.notice = Some(format!("retomado: {}", row.name));
        }
        Ok(())
    }

    async fn remove_row(&mut self) -> anyhow::Result<()> {
        let rows = self.session_rows();
        if let Some(row) = rows.get(self.row_index) {
            self.session
                .delete(librqbit::api::TorrentIdOrHash::Id(row.id), false)
                .await?;
            self.notice = Some(format!("removido da fila: {}", row.name));
            let remaining = self.session_rows().len();
            if remaining == 0 {
                self.row_index = 0;
            } else if self.row_index >= remaining {
                self.row_index = remaining - 1;
            }
        }
        Ok(())
    }

    async fn add_included(&mut self) -> anyhow::Result<()> {
        let source = self.include_input.trim().to_string();
        if source.is_empty() {
            self.notice = Some("informe uma origem (magnet, .torrent ou URL)".into());
            return Ok(());
        }

        self.notice = Some("resolvendo origem...".into());
        let add = AddTorrent::from_cli_argument(&source)
            .with_context(|| format!("origem inválida: {source}"));
        let add = match add {
            Ok(add) => add,
            Err(err) => {
                self.notice = Some(format!("origem inválida: {err:#}"));
                return Ok(());
            }
        };

        // Probe with list_only to resolve magnets and learn the output folder.
        let probe_opts = AddTorrentOptions {
            list_only: true,
            ..Default::default()
        };

        let result = self.session.add_torrent(add, Some(probe_opts)).await;

        let (info, only_files, output_folder, torrent_bytes) = match result {
            Ok(AddTorrentResponse::ListOnly(l)) => {
                (l.info, l.only_files, l.output_folder, l.torrent_bytes)
            }
            Ok(AddTorrentResponse::AlreadyManaged(id, handle)) => {
                self.notice = Some(format!(
                    "já gerenciado (id={id}, hash={:?})",
                    handle.info_hash()
                ));
                return Ok(());
            }
            Ok(_) => unreachable!("list_only can only return ListOnly"),
            Err(err) => {
                self.notice = Some(format!("falha ao resolver: {err:#}"));
                return Ok(());
            }
        };

        // Skip fully downloaded torrents; resume partials.
        let overwrite = crate::existing_state(&output_folder, &info, only_files.as_deref())?;
        let overwrite = match overwrite {
            crate::ExistingState::Complete => {
                self.notice = Some("torrent já completamente baixado".into());
                return Ok(());
            }
            crate::ExistingState::Partial => true,
            crate::ExistingState::Missing => false,
        };

        let add_opts = AddTorrentOptions {
            only_files,
            overwrite,
            output_folder: Some(output_folder.to_string_lossy().into_owned()),
            ..Default::default()
        };

        match self
            .session
            .add_torrent(AddTorrent::TorrentFileBytes(torrent_bytes), Some(add_opts))
            .await
        {
            Ok(AddTorrentResponse::Added(_, handle)) => {
                self.notice = Some(format!("adicionado, hash={:?}", handle.info_hash()));
                self.view = View::Session;
                self.row_index = self.session_rows().len().saturating_sub(1);
                self.include_input.clear();
            }
            Ok(AddTorrentResponse::AlreadyManaged(id, handle)) => {
                self.notice = Some(format!(
                    "já gerenciado (id={id}, hash={:?})",
                    handle.info_hash()
                ));
            }
            Ok(_) => unreachable!("re-add can only return Added or AlreadyManaged"),
            Err(err) => {
                self.notice = Some(format!("falha ao adicionar: {err:#}"));
            }
        }
        Ok(())
    }

    fn render(&mut self, w: &mut impl Write) -> anyhow::Result<()> {
        let (cols, rows) = terminal::size().unwrap_or((80, 24));
        queue!(w, terminal::Clear(terminal::ClearType::All), cursor::Hide)?;

        match self.view {
            View::Title => self.render_title(w, cols, rows)?,
            View::Menu => self.render_menu(w, cols, rows)?,
            View::Session => self.render_session(w, cols, rows)?,
            View::Include => self.render_include(w, cols, rows)?,
            View::Completed => self.render_completed(w, cols, rows)?,
        }

        if let Some(notice) = &self.notice {
            queue!(
                w,
                cursor::MoveTo(1, rows.saturating_sub(1)),
                style::SetForegroundColor(Color::DarkYellow),
                style::Print(notice),
                style::ResetColor,
            )?;
        }
        w.flush()?;
        Ok(())
    }

    fn render_title(&self, w: &mut impl Write, cols: u16, rows: u16) -> anyhow::Result<()> {
        let mut lines: Vec<String> = Vec::new();
        for line in BANNER.iter() {
            lines.push(line.to_string());
        }
        lines.push(String::new());
        lines.push("Bem-vindo ao OpenTorrent".into());
        lines.push("digite / para abrir o menu".into());
        lines.push(String::new());

        let prompt = format!("{} {}", ">", self.input);
        lines.push(prompt);
        lines.push(String::new());
        lines.push("atalhos: A adicionar | C acompanhar | L listar | S sair".into());

        let highlighted: Vec<usize> = (0..5).collect();
        self.centered_write(w, lines, cols, rows, &highlighted)?;
        Ok(())
    }

    fn render_menu(&self, w: &mut impl Write, cols: u16, rows: u16) -> anyhow::Result<()> {
        let mut lines: Vec<String> = Vec::new();
        lines.push("OPENTORRENT".into());
        lines.push("menu".into());
        lines.push(String::new());

        for (idx, item) in MENU_ITEMS.iter().enumerate() {
            let label = if idx == self.menu_index {
                format!(
                    "> [{}] {} <",
                    MENU_SHORTCUTS[idx].to_ascii_uppercase(),
                    item
                )
            } else {
                format!("  [{}] {}", MENU_SHORTCUTS[idx].to_ascii_uppercase(), item)
            };
            lines.push(label);
        }
        lines.push(String::new());
        lines.push("use ↑/↓ + Enter, ou digite a letra do atalho".into());

        let mut highlighted = vec![0usize];
        highlighted.push(2 + self.menu_index);
        self.centered_write(w, lines, cols, rows, &highlighted)?;
        Ok(())
    }

    fn render_session(&mut self, w: &mut impl Write, cols: u16, rows: u16) -> anyhow::Result<()> {
        let rows_data = self.session_rows();
        let mut lines: Vec<String> = Vec::new();
        lines.push("sessão de downloads".into());
        lines.push(String::new());

        // Largura da parte informativa de cada linha: os botões de ação são
        // renderizados à direita em colunas fixas, recalculadas dinamicamente.
        let info_width =
            ROW_INFO_WIDTH.min(cols.saturating_sub(ACTIONS_WIDTH + 6).max(30)) as usize;
        let name_max = info_width.saturating_sub(30);

        if rows_data.is_empty() {
            lines.push("nenhum torrent na fila".into());
        } else {
            for (idx, row) in rows_data.iter().enumerate() {
                let marker = if idx == self.row_index { ">" } else { " " };
                let state = if row.finished {
                    "concluído"
                } else {
                    match row.state {
                        TorrentStatsState::Live => "em andamento",
                        TorrentStatsState::Paused => "pausado",
                        TorrentStatsState::Error => "erro",
                        TorrentStatsState::Initializing => "inicializando",
                    }
                };
                let pct = if row.total_bytes == 0 {
                    0.0
                } else {
                    row.progress_bytes as f64 / row.total_bytes as f64 * 100.0
                };
                let paused = matches!(
                    row.state,
                    TorrentStatsState::Paused | TorrentStatsState::Error
                );
                let info = format!(
                    "{} [{:>3}] {:>5.1}%  {:<11}  {}",
                    marker,
                    row.id,
                    pct,
                    state,
                    Self::truncate(&row.name, name_max),
                );
                let line = format!(
                    "{:<width$} {actions}",
                    info,
                    width = info_width,
                    actions = actions_string(paused),
                );
                lines.push(line);
            }
        }

        lines.push(String::new());
        lines.push("[P] pausar  [R] retomar  [X] remover  [A/+] adicionar  [Esc] voltar".into());
        if let Some(notice) = &self.notice {
            lines.push(notice.clone());
        }

        // Highlight the selected row (index 2 + row_index) and the title.
        let mut highlighted = vec![0usize];
        if !rows_data.is_empty() {
            highlighted.push(2 + self.row_index);
        }
        let (left, top) = self.centered_write(w, lines, cols, rows, &highlighted)?;

        // Registra a geometria para o mapeamento de cliques do mouse.
        if rows_data.is_empty() {
            self.layout = None;
        } else {
            self.layout = Some(Layout {
                top,
                left,
                rows_offset: 2,
                row_count: rows_data.len(),
                info_width: info_width as u16,
                show_actions: true,
            });
        }
        Ok(())
    }

    fn render_include(&self, w: &mut impl Write, cols: u16, rows: u16) -> anyhow::Result<()> {
        let mut lines: Vec<String> = Vec::new();
        lines.push("adicionar torrent a fila".into());
        lines.push(String::new());

        let options = [
            "digitar a origem (magnet, .torrent ou URL)",
            "voltar a listagem",
        ];
        for (idx, opt) in options.iter().enumerate() {
            let marker = if idx == self.include_index { ">" } else { " " };
            lines.push(format!("{marker} {opt}"));
        }

        lines.push(String::new());

        // Calculate available width for the input field (leave margins)
        let prompt = "origem: ";
        let prompt_width = prompt.chars().count();
        let max_input_width = cols.saturating_sub(prompt_width as u16 + 4).max(20) as usize;

        if self.typing {
            // Wrap the input text
            let wrapped_lines = Self::wrap_text(&self.include_input, max_input_width);
            let _input_line_count = wrapped_lines.len().max(1);

            // Render the prompt + first line (or empty)
            if wrapped_lines.is_empty() {
                lines.push(prompt.to_string());
            } else {
                for (i, wrapped) in wrapped_lines.iter().enumerate() {
                    if i == 0 {
                        lines.push(format!("{prompt}{wrapped}"));
                    } else {
                        // Continuation lines: indent to align with prompt
                        let indent = " ".repeat(prompt_width);
                        lines.push(format!("{indent}{wrapped}"));
                    }
                }
            }

            lines.push(String::new());
            lines.push("Enter para confirmar, Esc para cancelar".into());
        } else {
            lines.push("Enter para selecionar, Esc para voltar".into());
        }

        if let Some(notice) = &self.notice {
            lines.push(notice.clone());
        }

        // Highlight the title and the selected option
        let mut highlighted = vec![0usize];
        highlighted.push(2 + self.include_index);

        // Render the base lines first
        self.render_include_base(w, cols, rows, &lines, &highlighted)?;

        // If typing, position the cursor at the correct position in the wrapped input
        if self.typing {
            self.render_input_cursor(w, cols, rows, &lines)?;
        }

        Ok(())
    }

    /// Render the base lines for include view (without cursor positioning)
    fn render_include_base(
        &self,
        w: &mut impl Write,
        cols: u16,
        rows: u16,
        lines: &[String],
        highlighted: &[usize],
    ) -> anyhow::Result<()> {
        let width = lines
            .iter()
            .map(|l| l.chars().count() as u16)
            .max()
            .unwrap_or(0);
        let top = (rows.saturating_sub(lines.len() as u16)) / 2;
        let left = cols.saturating_sub(width) / 2;

        for (idx, line) in lines.iter().enumerate() {
            queue!(w, cursor::MoveTo(left, top.saturating_add(idx as u16)),)?;
            if highlighted.contains(&idx) {
                queue!(
                    w,
                    style::SetForegroundColor(Color::Cyan),
                    style::Print(line),
                    style::ResetColor,
                )?;
            } else {
                queue!(w, style::Print(line))?;
            }
        }
        Ok(())
    }

    /// Render the cursor at the correct position within the wrapped input field
    fn render_input_cursor(
        &self,
        w: &mut impl Write,
        cols: u16,
        rows: u16,
        lines: &[String],
    ) -> anyhow::Result<()> {
        let prompt = "origem: ";
        let prompt_width = prompt.chars().count();
        let max_input_width = cols.saturating_sub(prompt_width as u16 + 4).max(20) as usize;

        // Find the line index where the input starts (after options + blank lines)
        // lines structure:
        // 0: "adicionar torrent a fila"
        // 1: ""
        // 2: option 0
        // 3: option 1
        // 4: ""
        // 5+: wrapped input lines (starts at line 5)
        let input_start_line = 5;

        // Calculate cursor position within the wrapped text
        let wrapped = Self::wrap_text(&self.include_input, max_input_width);
        let input_line_count = wrapped.len().max(1);

        // Find which visual line and column the cursor is on
        let mut cursor_line = 0;
        let mut cursor_col = prompt_width;
        let mut chars_before_cursor = self.input_cursor_pos;

        for (i, wrapped_line) in wrapped.iter().enumerate() {
            let line_len = wrapped_line.chars().count();
            if chars_before_cursor <= line_len {
                cursor_line = i;
                cursor_col = prompt_width + chars_before_cursor;
                break;
            }
            chars_before_cursor -= line_len;
        }

        // If cursor is past all wrapped lines, put it at the end of the last line
        if cursor_line >= input_line_count {
            cursor_line = input_line_count - 1;
            let last_line = wrapped.last().map(|l| l.chars().count()).unwrap_or(0);
            cursor_col = prompt_width + last_line.min(chars_before_cursor);
        }

        let width = lines
            .iter()
            .map(|l| l.chars().count() as u16)
            .max()
            .unwrap_or(0);
        let top = (rows.saturating_sub(lines.len() as u16)) / 2;
        let left = cols.saturating_sub(width) / 2;

        let cursor_row = top.saturating_add((input_start_line + cursor_line) as u16);
        let cursor_col = left.saturating_add(cursor_col as u16);

        queue!(w, cursor::MoveTo(cursor_col, cursor_row), cursor::Show,)?;

        Ok(())
    }

    fn render_completed(&self, w: &mut impl Write, cols: u16, rows: u16) -> anyhow::Result<()> {
        let mut lines: Vec<String> = Vec::new();
        lines.push("downloads completos".into());
        lines.push(format!("pasta: {}", self.output_folder.display()));
        lines.push(String::new());

        let items = match list_completed_downloads(&self.output_folder) {
            Ok(items) => items,
            Err(err) => {
                lines.push(format!("erro ao listar: {err:#}"));
                self.centered_write(w, lines, cols, rows, &[])?;
                return Ok(());
            }
        };

        if items.is_empty() {
            lines.push("nenhum download completo".into());
        } else {
            for item in &items {
                lines.push(format_completed(item));
            }
        }

        lines.push(String::new());
        lines.push("Esc para voltar ao menu".into());

        let mut highlighted = vec![0usize];
        if items.is_empty() {
            highlighted.push(2);
        }
        self.centered_write(w, lines, cols, rows, &highlighted)?;
        Ok(())
    }

    /// Desenha as linhas centralizadas e retorna a geometria usada `(left, top)`.
    fn centered_write(
        &self,
        w: &mut impl Write,
        lines: Vec<String>,
        cols: u16,
        rows: u16,
        highlighted: &[usize],
    ) -> anyhow::Result<(u16, u16)> {
        let width = lines
            .iter()
            .map(|l| l.chars().count() as u16)
            .max()
            .unwrap_or(0);
        let top = (rows.saturating_sub(lines.len() as u16)) / 2;
        let left = cols.saturating_sub(width) / 2;

        for (idx, line) in lines.iter().enumerate() {
            queue!(w, cursor::MoveTo(left, top.saturating_add(idx as u16)),)?;
            if highlighted.contains(&idx) {
                queue!(
                    w,
                    style::SetForegroundColor(Color::Cyan),
                    style::Print(line),
                    style::ResetColor,
                )?;
            } else {
                queue!(w, style::Print(line))?;
            }
        }
        Ok((left, top))
    }
}

/// Texto dos botões de ação de uma linha da sessão.
fn actions_string(paused: bool) -> String {
    let toggle = if paused { "[Retomar]" } else { "[Pausar ]" };
    format!("{toggle} [Parar  ] [Excluir]")
}

/// Se `offset` cai dentro da área do botão que começa em `start`.
fn in_action_button(offset: u16, start: u16) -> bool {
    (start..start + ACTION_BTN_WIDTH).contains(&offset)
}

/// Mapeia o deslocamento horizontal a partir do início dos botões de ação para
/// o índice do botão (0 = pausar/retomar, 1 = parar, 2 = excluir).
/// Retorna `None` quando o clique cai fora da área delimitada (incluindo os
/// espaços entre botões, que são intencionalmente inertes).
fn action_button_at(offset: u16) -> Option<usize> {
    if in_action_button(offset, 0) {
        Some(0)
    } else if in_action_button(offset, ACTION_1_START) {
        Some(1)
    } else if in_action_button(offset, ACTION_2_START) {
        Some(2)
    } else {
        None
    }
}

/// Remove ANSI escape sequences added by crossterm style helpers.
#[cfg(test)]
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // Skip until the end of the escape sequence ('m' for SGR).
            for n in chars.by_ref() {
                if n == 'm' {
                    break;
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn key_reader(tx: mpsc::UnboundedSender<Event>) {
    loop {
        match event::read() {
            Ok(Event::Key(key)) => {
                if tx.send(Event::Key(key)).is_err() {
                    break;
                }
            }
            Ok(Event::Paste(text)) => {
                if tx.send(Event::Paste(text)).is_err() {
                    break;
                }
            }
            Ok(Event::Mouse(mouse)) => {
                if tx.send(Event::Mouse(mouse)).is_err() {
                    break;
                }
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
}

/// Run the interactive TUI. Expects the terminal to be in a clean state.
pub async fn run_interactive(session: Arc<Session>, output_folder: PathBuf) -> anyhow::Result<()> {
    terminal::enable_raw_mode().context("failed to enable raw mode")?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        terminal::EnterAlternateScreen,
        cursor::Hide,
        event::EnableBracketedPaste,
        event::EnableMouseCapture
    )?;
    let _guard = RawModeGuard;

    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || key_reader(tx));

    let mut tui = Tui::new(session, output_folder, rx);
    let result = loop {
        tui.render(&mut stdout)?;

        // Process all pending keys, then refresh the screen periodically.
        let mut handled = false;
        while let Ok(event) = tui.keys.try_recv() {
            handled = true;
            let keep = match event {
                Event::Key(key) => tui.handle_key(key).await.unwrap_or(false),
                Event::Paste(text) => {
                    if tui.typing {
                        // Insert pasted text at cursor position
                        let before = &tui.include_input[..tui.input_cursor_pos];
                        let after = &tui.include_input[tui.input_cursor_pos..];
                        tui.include_input = format!("{}{}{}", before, text, after);
                        tui.input_cursor_pos += text.len();
                        tui.notice = None;
                    }
                    true
                }
                Event::Mouse(mouse) => {
                    if let Err(err) = tui.handle_mouse(mouse).await {
                        tui.notice = Some(format!("erro no clique: {err:#}"));
                    }
                    true
                }
                _ => true,
            };
            if !keep {
                break;
            }
        }
        if !tui.running {
            break Ok(());
        }
        if !handled {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };

    execute!(
        stdout,
        terminal::LeaveAlternateScreen,
        cursor::Show,
        event::DisableBracketedPaste,
        event::DisableMouseCapture
    )?;
    result
}

/// Restores the terminal state when dropped.
struct RawModeGuard;

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
        let _ = execute!(
            io::stdout(),
            event::DisableBracketedPaste,
            event::DisableMouseCapture
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_sgr_escape_sequences() {
        let colored = "\u{1b}[36mhello\u{1b}[0m world".to_string();
        assert_eq!(strip_ansi(&colored), "hello world");
    }

    #[test]
    fn strips_multiple_sequences() {
        let s = "\u{1b}[1;32ma\u{1b}[0m\u{1b}[31mb\u{1b}[0m";
        assert_eq!(strip_ansi(s), "ab");
    }

    #[test]
    fn wrap_text_keeps_short_lines_untouched() {
        let lines = Tui::wrap_text("olá mundo", 30);
        assert_eq!(lines, vec!["olá mundo"]);
    }

    #[test]
    fn wrap_text_breaks_long_lines_on_spaces() {
        let lines = Tui::wrap_text("aaaa bbbb cccc", 6);
        assert_eq!(lines, vec!["aaaa ", "bbbb ", "cccc"]);
    }

    #[test]
    fn wrap_text_hard_breaks_single_long_word() {
        let lines = Tui::wrap_text("abcdefghij", 4);
        assert_eq!(lines, vec!["abcd", "efgh", "ij"]);
    }

    #[test]
    fn wrap_text_handles_width_zero() {
        let lines = Tui::wrap_text("qualquer coisa", 0);
        assert_eq!(lines, vec![""]);
    }

    #[test]
    fn wrap_text_preserves_whole_text_when_wrapped() {
        let source =
            "magnet:?xt=urn:btih:abcdef0123456789abcdef0123456789abcdef01&dn=arquivo+de+teste";
        let width = 30;
        let lines = Tui::wrap_text(source, width);
        let joined: String = lines.join("");
        assert_eq!(joined, source);
        assert!(lines.iter().all(|l| l.chars().count() <= width));
    }

    #[test]
    fn truncate_keeps_short_text() {
        assert_eq!(Tui::truncate("curto", 20), "curto");
    }

    #[test]
    fn truncate_shortens_long_text() {
        assert_eq!(
            Tui::truncate("abcdefghijklmnopqrstuvwxyz", 10),
            "abcdefghi…"
        );
    }

    #[test]
    fn actions_string_shows_resume_when_paused() {
        assert_eq!(actions_string(true), "[Retomar] [Parar  ] [Excluir]");
        assert_eq!(actions_string(false), "[Pausar ] [Parar  ] [Excluir]");
    }

    #[test]
    fn action_button_mapping_covers_buttons_only() {
        // botão pausar/retomar
        assert_eq!(action_button_at(0), Some(0));
        assert_eq!(action_button_at(8), Some(0));
        // espaço entre botões não dispara ação
        assert_eq!(action_button_at(9), None);
        // botão parar
        assert_eq!(action_button_at(10), Some(1));
        assert_eq!(action_button_at(18), Some(1));
        // espaço entre botões não dispara ação
        assert_eq!(action_button_at(19), None);
        // botão excluir
        assert_eq!(action_button_at(20), Some(2));
        assert_eq!(action_button_at(28), Some(2));
        // fora da área dos botões
        assert_eq!(action_button_at(29), None);
        assert_eq!(action_button_at(100), None);
    }
}
