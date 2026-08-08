use std::{
    io::{self, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
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
use librqbit::{AddTorrent, AddTorrentResponse, ManagedTorrent, Session, TorrentStatsState};
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

/// Índice (na lista de linhas renderizadas) da primeira opção do menu.
const MENU_ITEMS_OFFSET: usize = 3;

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

/// Resolução alvo da área amostrada: 1024x768 pixels. Considerando uma célula
/// de terminal de ~8x16 px, isso equivale a 128 colunas x 48 linhas.
const FRAME_WIDTH: u16 = 128;
const FRAME_HEIGHT: u16 = 48;

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

/// Status de uma origem adicionada em segundo plano (US-014).
#[derive(Clone, Debug)]
enum PendingStatus {
    /// A origem está sendo resolvida/adicionada em background.
    Resolving,
    /// Falhou ao resolver/adicionar; a mensagem é exibida na linha.
    Error(String),
    /// Terminou com um aviso (ex.: já gerenciado); consumido como notice.
    Done(String),
}

/// Uma origem adicionada de forma assíncrona, ainda não presente na sessão.
#[derive(Clone, Debug)]
struct PendingTorrent {
    source: String,
    status: PendingStatus,
}

/// Resultado da adição em segundo plano.
#[derive(Debug)]
enum AddOutcome {
    Added,
    AlreadyManaged,
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

/// Uma célula do buffer de tela (double buffering): um caractere + se está
/// destacado em ciano.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Cell {
    ch: char,
    cyan: bool,
}

impl Cell {
    fn blank() -> Self {
        Cell {
            ch: ' ',
            cyan: false,
        }
    }
}

/// Buffer de tela (US-016): o render desenha o estado atual neste buffer e, ao
/// aplicar o frame, apenas as células alteradas em relação ao frame anterior
/// são reescritas no terminal (diff-rendering / double buffering).
struct Frame {
    grid: Vec<Cell>,
    cols: u16,
    rows: u16,
    /// Posição desejada do cursor do terminal (ex.: campo de entrada).
    cursor: Option<(u16, u16)>,
}

impl Frame {
    fn new(cols: u16, rows: u16) -> Self {
        Frame {
            grid: vec![Cell::blank(); cols as usize * rows as usize],
            cols,
            rows,
            cursor: None,
        }
    }

    fn index(&self, row: u16, col: u16) -> usize {
        row as usize * self.cols as usize + col as usize
    }

    fn cell(&self, row: u16, col: u16) -> Cell {
        if row >= self.rows || col >= self.cols {
            return Cell::blank();
        }
        self.grid[self.index(row, col)]
    }

    /// Escreve um texto a partir de `(row, col)`, fora dos limites é ignorado.
    fn put(&mut self, row: u16, col: u16, text: &str, cyan: bool) {
        for (col, ch) in (col..).zip(text.chars()) {
            if row >= self.rows || col >= self.cols {
                return;
            }
            let idx = self.index(row, col);
            let cell = &mut self.grid[idx];
            cell.ch = ch;
            cell.cyan = cyan;
        }
    }

    /// Registra a posição desejada do cursor do terminal (para exibir durante
    /// a digitação). `None` indica cursor oculto.
    fn set_cursor(&mut self, row: u16, col: u16) {
        self.cursor = Some((row, col));
    }

    /// Quantas células diferem do frame anterior.
    fn diff_count(&self, prev: &Frame) -> usize {
        self.grid
            .iter()
            .zip(prev.grid.iter())
            .filter(|(a, b)| a != b)
            .count()
    }

    /// Reescreve apenas as células alteradas em relação a `prev`, agrupando
    /// runs contíguos na mesma linha para minimizar escapes emitidos.
    fn apply_diff(&self, w: &mut impl Write, prev: &Frame) -> anyhow::Result<()> {
        for row in 0..self.rows {
            let mut col = 0u16;
            while col < self.cols {
                let cur = self.cell(row, col);
                if cur == prev.cell(row, col) {
                    col += 1;
                    continue;
                }
                // run contíguo de células alteradas com a mesma cor
                let start = col;
                let mut run = String::new();
                while col < self.cols {
                    let c = self.cell(row, col);
                    if c == prev.cell(row, col) || c.cyan != cur.cyan {
                        break;
                    }
                    run.push(c.ch);
                    col += 1;
                }
                queue!(w, cursor::MoveTo(start, row))?;
                if cur.cyan {
                    queue!(w, style::SetForegroundColor(Color::Cyan))?;
                }
                queue!(w, style::Print(run))?;
                if cur.cyan {
                    queue!(w, style::ResetColor)?;
                }
            }
        }
        Ok(())
    }
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
    /// Origens adicionadas em segundo plano, compartilhadas com as tasks.
    pending: Arc<Mutex<Vec<PendingTorrent>>>,
    /// Frame anterior renderizado (double buffering / diff-rendering).
    prev_frame: Option<Frame>,
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
            pending: Arc::new(Mutex::new(Vec::new())),
            prev_frame: None,
            running: true,
        }
    }

    /// Processa um evento de entrada (teclado, paste ou mouse). Retorna
    /// `(continuar, redesenhar)`: o primeiro indica se a UI segue aberta; o
    /// segundo, se o evento exigiu redesenho (movimentos de mouse não
    /// redesenham — evitam floods de render no diff-rendering).
    async fn handle_event(&mut self, event: Event) -> (bool, bool) {
        match event {
            Event::Key(key) => (self.handle_key(key).await.unwrap_or(false), true),
            Event::Paste(text) => {
                if self.typing {
                    // Insert pasted text at cursor position
                    let before = &self.include_input[..self.input_cursor_pos];
                    let after = &self.include_input[self.input_cursor_pos..];
                    self.include_input = format!("{before}{text}{after}");
                    self.input_cursor_pos += text.len();
                    self.notice = None;
                }
                (true, true)
            }
            Event::Mouse(mouse) => {
                if let Err(err) = self.handle_mouse(mouse).await {
                    self.notice = Some(format!("erro no clique: {err:#}"));
                }
                // Apenas cliques alteram o estado; movimentos não redesenham.
                let click = matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left));
                (true, click)
            }
            _ => (true, false),
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
            KeyCode::Enter => self.select_menu_item(),
            KeyCode::Char(c) => {
                if let Some(idx) = MENU_SHORTCUTS
                    .iter()
                    .position(|&s| s == c.to_ascii_lowercase())
                {
                    // AC-4: o destaque visual acompanha a opção acionada antes
                    // de executar o comando correspondente.
                    self.menu_index = idx;
                    self.select_menu_item();
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

    fn select_menu_item(&mut self) {
        match self.menu_index {
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
                self.submit_include();
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
            View::Menu => {
                self.mouse_menu(me.row);
                Ok(())
            }
            View::Include if !self.typing => {
                self.mouse_include(me.row);
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Clique no menu: move o cursor de seleção (indicador + destaque) para a
    /// opção clicada, em tempo real (AC-3).
    fn mouse_menu(&mut self, row: u16) {
        let Some(layout) = self.layout else {
            return;
        };
        let first_row = layout.top.saturating_add(layout.rows_offset);
        let end = first_row.saturating_add(layout.row_count as u16);
        if row < first_row || row >= end {
            return;
        }
        self.menu_index = (row - first_row) as usize;
    }

    /// Clique nas opções do diálogo de inclusão (fora do modo de digitação).
    fn mouse_include(&mut self, row: u16) {
        let Some(layout) = self.layout else {
            return;
        };
        let first_row = layout.top.saturating_add(layout.rows_offset);
        let end = first_row.saturating_add(layout.row_count as u16);
        if row < first_row || row >= end {
            return;
        }
        self.include_index = (row - first_row) as usize;
    }

    /// Maps a click on the session view to the clicked row and action button.
    ///
    /// The coordinates are relative to the `Layout` stored by the last render,
    /// so window resizes are reflected on the very next frame.
    async fn mouse_session(&mut self, col: u16, row: u16) -> anyhow::Result<()> {
        let Some(layout) = self.layout else {
            return Ok(());
        };
        let first_row = layout.top.saturating_add(layout.rows_offset);
        let end = first_row.saturating_add(layout.row_count as u16);
        if row < first_row || row >= end {
            return Ok(());
        }
        let idx = (row - first_row) as usize;

        // Sem botões renderizados (terminal estreito) ou clique fora da área
        // dos botões: apenas move a seleção para a linha clicada.
        let actions_col = layout.left.saturating_add(layout.info_width + 1);
        if !layout.show_actions || col < actions_col || col >= actions_col + ACTIONS_WIDTH {
            self.row_index = idx;
            return Ok(());
        }

        let rows = self.session_rows();
        let Some(row_data) = rows.get(idx) else {
            return Ok(());
        };

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

    /// Mantém `row_index` dentro dos limites após remoção de linhas,
    /// considerando também as origens pendentes (adição assíncrona).
    fn clamp_row_index(&mut self) {
        let remaining = self.session_rows().len() + self.pending.lock().unwrap().len();
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
        // A navegação considera também as origens pendentes (adição assíncrona).
        let rows = self.session_rows();
        let pending = self.pending.lock().unwrap().len();
        let total = rows.len() + pending;
        if total == 0 {
            self.row_index = 0;
            return;
        }
        let next = (self.row_index as isize + delta).rem_euclid(total as isize);
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
            self.clamp_row_index();
        }
        Ok(())
    }

    /// Submete a origem digitada de forma assíncrona (US-014): valida apenas a
    /// sintaxe, registra a requisição e transiciona imediatamente para a tela
    /// de acompanhamento. A resolução de metadados e as conexões de rede rodam
    /// em uma task em background, mantendo a interface responsiva.
    fn submit_include(&mut self) {
        let source = self.include_input.trim().to_string();
        self.notice = None;

        if source.is_empty() {
            self.notice = Some("informe uma origem (magnet, .torrent ou URL)".into());
            return;
        }

        // Validação síncrona apenas de sintaxe — nenhuma espera de rede aqui.
        // Em caso de erro o texto digitado é preservado para correção.
        if let Err(err) = AddTorrent::from_cli_argument(&source) {
            self.notice = Some(format!("origem inválida: {err:#}"));
            return;
        }

        self.include_input.clear();
        self.input_cursor_pos = 0;

        // Registra a requisição e exibe a linha imediatamente (AC-2/AC-3).
        self.pending.lock().unwrap().push(PendingTorrent {
            source: source.clone(),
            status: PendingStatus::Resolving,
        });
        self.view = View::Session;
        self.row_index =
            (self.session_rows().len() + self.pending.lock().unwrap().len()).saturating_sub(1);

        let session = self.session.clone();
        let pending = self.pending.clone();
        tokio::spawn(async move {
            let outcome = add_torrent_background(session, source.clone()).await;
            let mut guard = pending.lock().unwrap();
            let Some(entry) = guard.iter_mut().find(|p| p.source == source) else {
                return;
            };
            match outcome {
                Ok(AddOutcome::Added) => {
                    // O torrent real já está na sessão: remove a entrada pendente.
                    guard.retain(|p| p.source != source);
                }
                Ok(AddOutcome::AlreadyManaged) => {
                    entry.status = PendingStatus::Done("origem já gerenciada".into());
                }
                Err(err) => {
                    // AC-5: falha assíncrona → status de erro na lista, sem
                    // bloquear a navegação.
                    entry.status = PendingStatus::Error(format!("{err:#}"));
                }
            }
        });
    }

    /// Consome avisos de entradas pendentes concluídas (ex.: "já gerenciado")
    /// e remove a entrada, exibindo o aviso como notice.
    fn drain_pending_notices(&mut self) {
        let mut pending = self.pending.lock().unwrap();
        pending.retain(|p| match &p.status {
            // Aviso mais recente vence: o evento concluiu agora, então sobrescreve
            // qualquer notice anterior (ex.: "já gerenciado" após um "pausado").
            PendingStatus::Done(msg) => {
                self.notice = Some(msg.clone());
                false
            }
            _ => true,
        });
    }

    fn render(&mut self, w: &mut impl Write) -> anyhow::Result<()> {
        self.drain_pending_notices();
        let (cols, rows) = terminal::size().unwrap_or((80, 24));

        // US-016 (double buffering): o estado atual é desenhado no buffer e,
        // ao aplicar, apenas as células alteradas em relação ao frame anterior
        // são reescritas no terminal. O clear total só acontece no primeiro
        // frame, em redimensionamento ou quando quase tudo mudou (troca de
        // view) — nunca a cada iteração do laço.
        let mut frame = Frame::new(cols, rows);
        match self.view {
            View::Title => self.render_title(&mut frame, cols, rows),
            View::Menu => self.render_menu(&mut frame, cols, rows),
            View::Session => self.render_session(&mut frame, cols, rows),
            View::Include => self.render_include(&mut frame, cols, rows),
            View::Completed => self.render_completed(&mut frame, cols, rows),
        }

        if let Some(notice) = &self.notice {
            frame.put(rows.saturating_sub(1), 1, notice, false);
        }

        let total = cols as usize * rows as usize;
        let needs_full = match &self.prev_frame {
            None => true,
            Some(prev) => {
                prev.cols != cols || prev.rows != rows || frame.diff_count(prev) > total / 2
            }
        };

        if needs_full {
            queue!(w, terminal::Clear(terminal::ClearType::All), cursor::Hide)?;
            frame.apply_diff(w, &Frame::new(cols, rows))?;
        } else if let Some(prev) = &self.prev_frame {
            frame.apply_diff(w, prev)?;
        }

        // Cursor do terminal: visível apenas durante a digitação (include).
        match frame.cursor {
            Some((row, col)) => queue!(w, cursor::MoveTo(col, row), cursor::Show)?,
            None => queue!(w, cursor::Hide)?,
        }
        w.flush()?;

        self.prev_frame = Some(frame);
        Ok(())
    }

    fn render_title(&mut self, frame: &mut Frame, cols: u16, rows: u16) {
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
        self.centered_write(frame, lines, cols, rows, &highlighted);
    }

    fn render_menu(&mut self, frame: &mut Frame, cols: u16, rows: u16) {
        // Interface de navegação delimitada pelo quadro com bordas duplas.
        let (left, top, width, height) = frame_rect(cols, rows);
        let content_left = left + 2;
        let inner_width = (width as usize).saturating_sub(4);

        let mut lines: Vec<String> = Vec::new();
        lines.push("OPENTORRENT".into());
        lines.push("menu".into());
        lines.push(String::new());

        for idx in 0..MENU_ITEMS.len() {
            lines.push(menu_item_line(idx, idx == self.menu_index));
        }
        lines.push(String::new());
        lines.push("use ↑/↓ + Enter, ou digite a letra do atalho".into());

        for line in &mut lines {
            *line = Self::truncate(line, inner_width);
        }

        let content_top = center_content_top(top, height, lines.len());

        // O destaque colorido acompanha exatamente a linha com o cursor
        // `> ... <` (itens começam na linha `MENU_ITEMS_OFFSET`).
        let mut highlighted = vec![0usize];
        highlighted.push(MENU_ITEMS_OFFSET + self.menu_index);

        draw_box(frame, left, top, width, height);
        write_boxed_lines(
            frame,
            content_left,
            content_top,
            &lines,
            &highlighted,
            height,
        );

        self.layout = Some(Layout {
            top: content_top,
            left: content_left,
            rows_offset: MENU_ITEMS_OFFSET as u16,
            row_count: MENU_ITEMS.len(),
            info_width: 0,
            show_actions: false,
        });
    }

    fn render_session(&mut self, frame: &mut Frame, cols: u16, rows: u16) {
        let rows_data = self.session_rows();
        let pending = self.pending.lock().unwrap().clone();
        let mut lines: Vec<String> = Vec::new();
        lines.push("sessão de downloads".into());
        lines.push(String::new());

        // Largura da parte informativa de cada linha. Os botões de ação são
        // renderizados à direita em colunas fixas apenas quando o terminal é
        // largo o suficiente — em terminais estreitos as linhas ficam sem
        // botões para evitar estouro (que desalinharia o mapeamento do clique).
        let show_actions = cols >= ROW_INFO_WIDTH + 1 + ACTIONS_WIDTH;
        let info_width = if show_actions {
            ROW_INFO_WIDTH.min(cols.saturating_sub(1 + ACTIONS_WIDTH)) as usize
        } else {
            ROW_INFO_WIDTH.min(cols.saturating_sub(2)) as usize
        };
        let name_max = info_width.saturating_sub(31);

        if rows_data.is_empty() && pending.is_empty() {
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
                // Parte informativa com largura fixa (id em 4 dígitos) para que
                // a coluna dos botões permaneça estável entre as linhas.
                let info = format!(
                    "{} [{:>4}] {:>5.1}%  {:<11}  {}",
                    marker,
                    row.id.min(9999),
                    pct,
                    state,
                    Self::truncate(&row.name, name_max),
                );
                let line = if show_actions {
                    format!(
                        "{:<width$} {actions}",
                        info,
                        width = info_width,
                        actions = actions_string(paused),
                    )
                } else {
                    format!("{:<width$}", info, width = info_width)
                };
                lines.push(line);
            }

            // Origens adicionadas em segundo plano, ainda sem handle.
            let mut idx = rows_data.len();
            for pt in &pending {
                let marker = if idx == self.row_index { ">" } else { " " };
                let (state, detail) = match &pt.status {
                    PendingStatus::Resolving => ("inicializando", ""),
                    PendingStatus::Error(err) => ("erro", err.as_str()),
                    PendingStatus::Done(_) => continue, // já consumido no render
                };
                let mut info = format!(
                    "{} [  —] {:>5.1}%  {:<11}  {}",
                    marker,
                    0.0,
                    state,
                    Self::truncate(&pt.source, name_max),
                );
                if !detail.is_empty() {
                    info = format!("{info} — {}", Self::truncate(detail, 28));
                }
                // Mantém uma linha por entrada e o alinhamento das colunas.
                let info = Self::truncate(&info, info_width);
                let line = if show_actions {
                    format!(
                        "{:<pad$}",
                        info,
                        pad = info_width + 1 + ACTIONS_WIDTH as usize
                    )
                } else {
                    format!("{:<width$}", info, width = info_width)
                };
                lines.push(line);
                idx += 1;
            }
        }

        lines.push(String::new());
        lines.push("[P] pausar  [R] retomar  [X] remover  [A/+] adicionar  [Esc] voltar".into());
        if let Some(notice) = &self.notice {
            lines.push(notice.clone());
        }

        // Highlight the selected row (index 2 + row_index) and the title. Cada
        // entrada (sessão ou pendente) ocupa exatamente uma linha.
        let mut highlighted = vec![0usize];
        if !rows_data.is_empty() || !pending.is_empty() {
            highlighted.push(2 + self.row_index);
        }
        let (left, top) = self.centered_write(frame, lines, cols, rows, &highlighted);

        // Registra a geometria para o mapeamento de cliques do mouse.
        let total_rows = rows_data.len() + pending.len();
        if total_rows == 0 {
            self.layout = None;
        } else {
            self.layout = Some(Layout {
                top,
                left,
                rows_offset: 2,
                row_count: total_rows,
                info_width: info_width as u16,
                show_actions,
            });
        }
    }

    fn render_include(&mut self, frame: &mut Frame, cols: u16, rows: u16) {
        // Interface de entrada delimitada pelo quadro com bordas duplas, com a
        // área amostrada fixada na equivalência de 1024x768 (128x48 células).
        let (left, top, width, height) = frame_rect(cols, rows);
        let content_left = left + 2;
        let inner_width = (width as usize).saturating_sub(4);

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

        // Largura do campo de entrada respeitando os limites internos do quadro.
        let prompt = "origem: ";
        let prompt_width = prompt.chars().count();
        let max_input_width = inner_width.saturating_sub(prompt_width);

        if self.typing {
            // Quebra de linha automática restrita à largura interna do container.
            let wrapped_lines = Self::wrap_text(&self.include_input, max_input_width);
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

        // Nenhum texto ultrapassa as bordas laterais do quadro.
        for line in &mut lines {
            *line = Self::truncate(line, inner_width);
        }

        // No modo de digitação o bloco fica ancorado no topo (cursor estável);
        // fora dele, o bloco é centralizado verticalmente no interior do quadro.
        let content_top = if self.typing {
            top + 2
        } else {
            center_content_top(top, height, lines.len())
        };

        // Highlight the title and the selected option
        let mut highlighted = vec![0usize];
        highlighted.push(2 + self.include_index);

        draw_box(frame, left, top, width, height);
        write_boxed_lines(
            frame,
            content_left,
            content_top,
            &lines,
            &highlighted,
            height,
        );

        // If typing, position the cursor at the correct position in the wrapped input
        if self.typing {
            self.render_input_cursor(frame, content_left, content_top, max_input_width, height);
        } else {
            // Registra a geometria para cliques do mouse nas opções.
            self.layout = Some(Layout {
                top: content_top,
                left: content_left,
                rows_offset: 2,
                row_count: 2,
                info_width: 0,
                show_actions: false,
            });
        }
    }

    /// Registra a posição do cursor dentro do campo de entrada, relativo ao
    /// interior do quadro (bordas duplas). O cursor nunca ultrapassa a área
    /// visível do conteúdo mesmo com texto colado muito longo.
    fn render_input_cursor(
        &self,
        frame: &mut Frame,
        content_left: u16,
        content_top: u16,
        max_input_width: usize,
        frame_height: u16,
    ) {
        let prompt = "origem: ";
        let prompt_width = prompt.chars().count();

        // Estrutura das linhas do conteúdo:
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

        // Limita o cursor à última linha visível do conteúdo (dentro do quadro).
        let max_visible = (frame_height as usize).saturating_sub(3).saturating_sub(1);
        let cursor_line = cursor_line.min(max_visible.saturating_sub(input_start_line));
        let cursor_row = content_top.saturating_add((input_start_line + cursor_line) as u16);
        let cursor_col = content_left.saturating_add(cursor_col as u16);

        frame.set_cursor(cursor_row, cursor_col);
    }

    fn render_completed(&mut self, frame: &mut Frame, cols: u16, rows: u16) {
        // Consistência visual: a listagem também fica dentro do quadro.
        let (left, top, width, height) = frame_rect(cols, rows);
        let content_left = left + 2;
        let inner_width = (width as usize).saturating_sub(4);

        let mut lines: Vec<String> = Vec::new();
        lines.push("downloads completos".into());
        lines.push(format!("pasta: {}", self.output_folder.display()));
        lines.push(String::new());

        let items = match list_completed_downloads(&self.output_folder) {
            Ok(items) => items,
            Err(err) => {
                lines.push(format!("erro ao listar: {err:#}"));
                Vec::new()
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

        for line in &mut lines {
            *line = Self::truncate(line, inner_width);
        }
        let content_top = center_content_top(top, height, lines.len());

        let mut highlighted = vec![0usize];
        if items.is_empty() {
            highlighted.push(2);
        }
        draw_box(frame, left, top, width, height);
        write_boxed_lines(
            frame,
            content_left,
            content_top,
            &lines,
            &highlighted,
            height,
        );
    }

    /// Desenha as linhas centralizadas no buffer e retorna a geometria usada
    /// `(left, top)` (para o mapeamento de cliques).
    fn centered_write(
        &self,
        frame: &mut Frame,
        lines: Vec<String>,
        cols: u16,
        rows: u16,
        highlighted: &[usize],
    ) -> (u16, u16) {
        let width = lines
            .iter()
            .map(|l| l.chars().count() as u16)
            .max()
            .unwrap_or(0);
        let top = (rows.saturating_sub(lines.len() as u16)) / 2;
        let left = cols.saturating_sub(width) / 2;

        for (idx, line) in lines.iter().enumerate() {
            frame.put(
                top.saturating_add(idx as u16),
                left,
                line,
                highlighted.contains(&idx),
            );
        }
        (left, top)
    }
}

/// Texto dos botões de ação de uma linha da sessão.
fn actions_string(paused: bool) -> String {
    let toggle = if paused { "[Retomar]" } else { "[Pausar ]" };
    format!("{toggle} [Parar  ] [Excluir]")
}

/// Linha de uma opção do menu. A opção selecionada recebe o cursor `> ... <`
/// e o destaque de cor (renderizado pelo mesmo `menu_index`).
fn menu_item_line(idx: usize, selected: bool) -> String {
    if selected {
        format!(
            "> [{}] {} <",
            MENU_SHORTCUTS[idx].to_ascii_uppercase(),
            MENU_ITEMS[idx]
        )
    } else {
        format!(
            "  [{}] {}",
            MENU_SHORTCUTS[idx].to_ascii_uppercase(),
            MENU_ITEMS[idx]
        )
    }
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

/// Resolve e adiciona uma origem em segundo plano (US-014). Para magnet links a
/// resolução de metadados via DHT/trackers ocorre aqui, fora da interface.
async fn add_torrent_background(
    session: Arc<Session>,
    source: String,
) -> anyhow::Result<AddOutcome> {
    let add = AddTorrent::from_cli_argument(&source)?;
    match session.add_torrent(add, None).await? {
        AddTorrentResponse::Added(_, _) => Ok(AddOutcome::Added),
        AddTorrentResponse::AlreadyManaged(_, _) => Ok(AddOutcome::AlreadyManaged),
        _ => unreachable!("add_torrent sem list_only só retorna Added ou AlreadyManaged"),
    }
}

/// Dimensões do quadro com borda dupla, limitadas ao terminal e centralizadas.
/// A área interna corresponde à resolução fixa de 1024x768 (128x48 células).
fn frame_rect(cols: u16, rows: u16) -> (u16, u16, u16, u16) {
    // O mínimo é apenas o necessário para as duas bordas: em terminais
    // minúsculos o quadro encolhe em vez de estourar os limites da tela.
    let width = FRAME_WIDTH.min(cols.saturating_sub(2)).max(2);
    let height = FRAME_HEIGHT.min(rows.saturating_sub(2)).max(2);
    let left = cols.saturating_sub(width) / 2;
    let top = rows.saturating_sub(height) / 2;
    (left, top, width, height)
}

/// Linhas superior e inferior do quadro (bordas duplas).
fn frame_top_bottom(width: u16) -> (String, String) {
    let inner = "═".repeat(width.saturating_sub(2) as usize);
    (format!("╔{inner}╗"), format!("╚{inner}╝"))
}

/// Linha inicial do conteúdo para centralizá-lo verticalmente no quadro
/// (com margem mínima de 1 linha abaixo da borda superior).
fn center_content_top(frame_top: u16, frame_height: u16, line_count: usize) -> u16 {
    let inner = (frame_height as usize).saturating_sub(2);
    let spare = inner.saturating_sub(line_count);
    frame_top + 1 + (spare / 2) as u16
}

/// Desenha o quadro com bordas duplas no buffer, centralizado.
fn draw_box(frame: &mut Frame, left: u16, top: u16, width: u16, height: u16) {
    let (top_line, bottom_line) = frame_top_bottom(width);
    frame.put(top, left, &top_line, false);
    for row in top + 1..top + height - 1 {
        frame.put(row, left, "║", false);
        frame.put(row, left + width - 1, "║", false);
    }
    frame.put(top + height - 1, left, &bottom_line, false);
}

/// Escreve linhas de conteúdo dentro do quadro no buffer, destacando as
/// selecionadas.
fn write_boxed_lines(
    frame: &mut Frame,
    content_left: u16,
    content_top: u16,
    lines: &[String],
    highlighted: &[usize],
    frame_height: u16,
) {
    let max_lines = (frame_height as usize).saturating_sub(3);
    for (idx, line) in lines.iter().take(max_lines).enumerate() {
        frame.put(
            content_top + idx as u16,
            content_left,
            line,
            highlighted.contains(&idx),
        );
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

    // US-016: laço com redesenho sob demanda (eventos) e limite de taxa.
    // Em vez de redesenhar em loop desimpedido, o laço aguarda um evento de
    // entrada OU o fim do orçamento de frame (~30 FPS) para atualizar as
    // métricas de progresso. Com o diff-rendering, um frame sem alterações
    // não emite praticamente nenhuma sequência para o terminal.
    const FRAME_BUDGET: Duration = Duration::from_millis(33); // ~30 FPS
    let mut last_frame = Instant::now();

    let result = loop {
        // Aguarda um evento de entrada ou o fim do orçamento do frame atual.
        let elapsed = last_frame.elapsed();
        let wait = FRAME_BUDGET.saturating_sub(elapsed);
        let event = if wait.is_zero() {
            tui.keys.try_recv().ok()
        } else {
            match tokio::time::timeout(wait, tui.keys.recv()).await {
                Ok(Some(ev)) => Some(ev),
                Ok(None) | Err(_) => None,
            }
        };

        // Processa todos os eventos pendentes (drenagem), marcando se houve
        // algum que exija redesenho (tecla, paste, clique).
        let mut dirty = false;
        let mut keep = true;
        if let Some(first) = event {
            let (k, d) = tui.handle_event(first).await;
            keep = k;
            dirty |= d;
        }
        while let Ok(ev) = tui.keys.try_recv() {
            let (k, d) = tui.handle_event(ev).await;
            keep = keep && k;
            dirty |= d;
        }
        if !keep || !tui.running {
            break Ok(());
        }

        // Redesenha apenas se houve evento relevante ou o orçamento expirou
        // (para atualizar progresso em tempo real). Sem loop desimpedido.
        if dirty || last_frame.elapsed() >= FRAME_BUDGET {
            tui.render(&mut stdout)?;
            last_frame = Instant::now();
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
    fn menu_item_line_marks_selected_only() {
        assert_eq!(menu_item_line(0, true), "> [A] Adicionar torrent a lista <");
        assert_eq!(menu_item_line(0, false), "  [A] Adicionar torrent a lista");
        assert!(menu_item_line(1, true).starts_with('>'));
        assert!(menu_item_line(2, true).ends_with('<'));
        assert!(!menu_item_line(3, false).starts_with('>'));
        assert!(!menu_item_line(3, false).ends_with('<'));
        // O destaque (índice de linha) aponta para a opção selecionada.
        assert_eq!(MENU_ITEMS_OFFSET, 3);
        assert_eq!(MENU_ITEMS.len(), 4);
    }

    #[test]
    fn frame_rect_uses_1024x768_equivalence_when_terminal_is_large() {
        let (left, top, w, h) = frame_rect(200, 60);
        assert_eq!(w, FRAME_WIDTH);
        assert_eq!(h, FRAME_HEIGHT);
        assert_eq!(left, (200 - 128) / 2);
        assert_eq!(top, (60 - 48) / 2);
    }

    #[test]
    fn frame_rect_clamps_and_centers_on_small_terminal() {
        let (left, top, w, h) = frame_rect(80, 24);
        assert_eq!(w, 78);
        assert_eq!(h, 22);
        assert_eq!(left, 1);
        assert_eq!(top, 1);
    }

    #[test]
    fn frame_rect_never_exceeds_tiny_terminal() {
        let (left, top, w, h) = frame_rect(15, 7);
        assert!(w <= 15);
        assert!(h <= 7);
        assert!(left + w <= 15);
        assert!(top + h <= 7);
    }

    #[test]
    fn center_content_top_centers_block() {
        // quadro 22 linhas, conteúdo 7 linhas → interior 20, sobra 13, margem 6
        assert_eq!(center_content_top(1, 22, 7), 1 + 1 + 6);
        // conteúdo maior que o interior: fica colado na margem mínima
        assert_eq!(center_content_top(1, 22, 100), 2);
    }

    #[test]
    fn frame_borders_use_double_lines() {
        let (top, bottom) = frame_top_bottom(78);
        assert_eq!(top.chars().next(), Some('╔'));
        assert_eq!(top.chars().last(), Some('╗'));
        assert_eq!(bottom.chars().next(), Some('╚'));
        assert_eq!(bottom.chars().last(), Some('╝'));
        assert_eq!(top.chars().count(), 78);
        assert!(top.chars().all(|c| matches!(c, '╔' | '═' | '╗')));
    }

    #[test]
    fn frame_put_writes_cells_in_bounds() {
        let mut f = Frame::new(5, 3);
        f.put(1, 1, "abc", false);
        assert_eq!(f.cell(1, 1).ch, 'a');
        assert_eq!(f.cell(1, 2).ch, 'b');
        assert_eq!(f.cell(1, 3).ch, 'c');
        // fora dos limites é ignorado (sem panic)
        f.put(9, 0, "x", false);
        f.put(0, 9, "y", false);
        assert_eq!(f.cell(9, 0).ch, ' ');
    }

    #[test]
    fn frame_put_marks_cyan_cells() {
        let mut f = Frame::new(10, 2);
        f.put(0, 0, "hi", true);
        assert!(f.cell(0, 0).cyan);
        assert!(f.cell(0, 1).cyan);
        assert!(!f.cell(1, 0).cyan);
    }

    #[test]
    fn frame_diff_counts_only_changed_cells() {
        let mut a = Frame::new(10, 3);
        a.put(0, 0, "hello", false);
        let mut b = Frame::new(10, 3);
        b.put(0, 0, "hella", false);
        // apenas o último caractere difere
        assert_eq!(a.diff_count(&b), 1);
        // frames idênticos não têm diferenças
        let mut c = Frame::new(10, 3);
        c.put(0, 0, "hello", false);
        assert_eq!(a.diff_count(&c), 0);
    }

    #[test]
    fn frame_apply_diff_rewrites_only_changed_run() {
        let mut prev = Frame::new(20, 2);
        prev.put(0, 0, "status: ok", false);
        let mut cur = Frame::new(20, 2);
        cur.put(0, 0, "status: 42", false);
        let mut out = Vec::new();
        cur.apply_diff(&mut out, &prev).unwrap();
        let s = String::from_utf8(out).unwrap();
        // a parte inalterada não é reescrita
        assert!(!s.contains("status:"));
        // apenas a célula alterada é emitida
        assert!(s.contains('4'));
        assert!(s.contains('2'));
    }

    #[test]
    fn frame_apply_diff_clears_removed_cells() {
        let mut prev = Frame::new(10, 2);
        prev.put(0, 0, "abcdef", false);
        let mut cur = Frame::new(10, 2);
        cur.put(0, 0, "abc", false);
        let mut out = Vec::new();
        cur.apply_diff(&mut out, &prev).unwrap();
        let s = String::from_utf8(out).unwrap();
        // as células removidas viram espaço (limpeza) e são reescritas
        assert!(s.contains(' '));
    }

    #[test]
    fn frame_apply_diff_identical_frames_emit_nothing() {
        let mut prev = Frame::new(10, 2);
        prev.put(0, 0, "abc", false);
        let mut cur = Frame::new(10, 2);
        cur.put(0, 0, "abc", false);
        let mut out = Vec::new();
        cur.apply_diff(&mut out, &prev).unwrap();
        assert!(out.is_empty());
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
