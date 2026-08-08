use std::{
    io::{self, Write},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use anyhow::Context;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute, queue,
    style::{self, Color},
    terminal,
};
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ManagedTorrent, Session, TorrentStatsState,
};
use size_format::SizeFormatterBinary as SF;
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
    /// Whether the include dialog is in "typing" mode (entering the source).
    typing: bool,
    /// Transient status/error message.
    notice: Option<String>,
    /// Keys received from the keyboard thread.
    keys: mpsc::UnboundedReceiver<KeyEvent>,
    running: bool,
}

impl Tui {
    fn new(
        session: Arc<Session>,
        output_folder: PathBuf,
        keys: mpsc::UnboundedReceiver<KeyEvent>,
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
            typing: false,
            notice: None,
            keys,
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
            }
            KeyCode::Backspace => {
                self.include_input.pop();
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.include_input.push(c);
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

    fn render(&self, w: &mut impl Write) -> anyhow::Result<()> {
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

    fn render_session(&self, w: &mut impl Write, cols: u16, rows: u16) -> anyhow::Result<()> {
        let rows_data = self.session_rows();
        let mut lines: Vec<String> = Vec::new();
        lines.push("sessão de downloads".into());
        lines.push(String::new());

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
                let line = format!(
                    "{} [{:>3}] {:>5.1}%  {:<11}  {} ({}/{})",
                    marker,
                    row.id,
                    pct,
                    state,
                    row.name,
                    SF::new(row.progress_bytes),
                    SF::new(row.total_bytes),
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
        self.centered_write(w, lines, cols, rows, &highlighted)?;
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
        if self.typing {
            lines.push(format!("origem: {}", self.include_input));
            lines.push("Enter para confirmar, Esc para cancelar".into());
        } else {
            lines.push("Enter para selecionar, Esc para voltar".into());
        }

        if let Some(notice) = &self.notice {
            lines.push(notice.clone());
        }

        let mut highlighted = vec![0usize];
        highlighted.push(2 + self.include_index);
        self.centered_write(w, lines, cols, rows, &highlighted)?;
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

    fn centered_write(
        &self,
        w: &mut impl Write,
        lines: Vec<String>,
        cols: u16,
        rows: u16,
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

fn key_reader(tx: mpsc::UnboundedSender<KeyEvent>) {
    loop {
        match event::read() {
            Ok(Event::Key(key)) => {
                if tx.send(key).is_err() {
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
    let _guard = RawModeGuard;

    let mut stdout = io::stdout();
    execute!(stdout, terminal::EnterAlternateScreen, cursor::Hide)?;

    let (tx, rx) = mpsc::unbounded_channel();
    std::thread::spawn(move || key_reader(tx));

    let mut tui = Tui::new(session, output_folder, rx);
    let result = loop {
        tui.render(&mut stdout)?;

        // Process all pending keys, then refresh the screen periodically.
        let mut handled = false;
        while let Ok(key) = tui.keys.try_recv() {
            handled = true;
            let keep = tui.handle_key(key).await.unwrap_or(false);
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

    execute!(stdout, terminal::LeaveAlternateScreen, cursor::Show)?;
    result
}

/// Restores the terminal state when dropped.
struct RawModeGuard;

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = terminal::disable_raw_mode();
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
}
