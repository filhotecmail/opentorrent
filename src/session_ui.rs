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
use librqbit::{AddTorrent, ManagedTorrent, Session, TorrentStatsState};
use tokio::sync::mpsc;

use crate::{
    daemon,
    downloads::{format_completed_row, history_header, list_completed_downloads},
    ipc::{DaemonRequest, DaemonResponse, DaemonState},
    ui_bottom::{self, BottomStyle},
};

/// Comandos do menu flutuante (US-031): comando e descrição funcional.
const COMMANDS: [(&str, &str); 6] = [
    ("/add", "adicionar torrent a lista"),
    ("/pause", "pausar torrent selecionado"),
    ("/resume", "retomar torrent selecionado"),
    ("/delete", "excluir torrent selecionado"),
    ("/help", "exibir ajuda de comandos"),
    ("/exit", "sair do OpenTorrent"),
];

/// Largura do prefixo da linha: marcador (2) + ID (4) + espaço (1).
const ROW_PREFIX_W: usize = 7;
/// Largura da coluna de progresso: barra de blocos + percentual adjacente.
const PROGRESS_COL_W: usize = 31;
/// Largura da coluna de status.
const STATE_W: usize = 13;
/// Largura total mínima da coluna de progresso no Card da Biblioteca (US-047):
/// alguns blocos + o percentual adjacente, para a barra continuar legível em
/// terminais estreitos.
const MIN_BAR_W: usize = 14;
/// Largura mínima da coluna de nome para que a quebra de linha seja legível.
const NAME_W_MIN: usize = 10;
/// Largura das colunas de métricas de rede (US-033): DOWN SPEED, UP SPEED, ETA
/// e RATIO, incluindo os espaços separadores. Ex.: `12.4 MiB/s 1.2 MiB/s 00:04:12   1.45`.
const METRICS_FIXED_W: usize = 38;
/// Largura fixa da parte não-nome de cada linha da tabela da sessão.
const ROW_FIXED_W: usize = ROW_PREFIX_W + PROGRESS_COL_W + 1 + STATE_W + 1;

/// Margens do layout full-window (US-031, estilo opencode): 2 colunas em
/// cada lateral. O topo/base é 1 linha, embutido nos painéis.
const LAYOUT_MARGIN_COLS: u16 = 2;

/// Intervalo de validade do cache do Histórico (US-045): a lista de completos
/// só é re-escaneada do disco após este período, evitando re-ordenar/re-desenhar
/// a cada frame.
const HISTORY_REFRESH: Duration = Duration::from_secs(2);

/// Estilo da área inferior da TUI (US-041). Para voltar à UI anterior, troque
/// para `BottomStyle::Legacy` — o render intacto (`render_home_prompt` +
/// `render_footer`) e a geometria original passam a ser usados automaticamente.
const BOTTOM_STYLE: BottomStyle = BottomStyle::Elevated;

/// The state machine driving the interactive UI.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum View {
    /// Home screen with panels (Biblioteca/Histórico) and the prompt at the
    /// bottom (US-031).
    Home,
    /// Command palette floating popup opened by typing `/` (US-031).
    Menu,
}

/// Ação de tecla única na Home (US-037), executada sobre a linha selecionada
/// da Biblioteca quando o prompt está vazio.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum HomeAction {
    /// Pausa o torrent selecionado.
    Pause,
    /// Retoma o torrent selecionado.
    Resume,
    /// Remove o torrent selecionado da fila (sem apagar arquivos).
    Remove,
    /// Abre a confirmação de exclusão definitiva (US-027).
    Delete,
}

/// Ação oferecida pelo menu de contexto da Biblioteca (US-036).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ContextAction {
    /// Pausa o torrent clicado.
    Pause,
    /// Retoma o torrent clicado.
    Resume,
    /// Remove o torrent clicado da fila (sem apagar arquivos).
    Stop,
    /// Abre a confirmação de exclusão definitiva (US-027).
    Delete,
}

/// Estado do menu de contexto aberto (US-036): item clicado da Biblioteca,
/// opção destacada e a linha da tela onde o popup é ancorado.
#[derive(Clone, Copy, Debug)]
struct ContextMenu {
    /// Índice do item clicado (torrents + pendentes, mesma indexação de
    /// `row_index`).
    entry_index: usize,
    /// Opção destacada (índice em `context_menu_options`).
    selected: usize,
    /// Linha da tela do item clicado (âncora do popup).
    anchor_row: u16,
}

#[derive(Clone)]
struct SessionRow {
    id: usize,
    name: String,
    state: TorrentStatsState,
    progress_bytes: u64,
    total_bytes: u64,
    finished: bool,
    /// Velocidade de download em MiB/s (US-033), quando a sessão está Live.
    down_speed_mbps: Option<f64>,
    /// Velocidade de upload em MiB/s (US-033), quando a sessão está Live.
    up_speed_mbps: Option<f64>,
    /// Tempo restante estimado em segundos (US-033), quando calculável.
    eta_seconds: Option<u64>,
    /// Bytes enviados (upload) acumulados na sessão (US-033).
    uploaded_bytes: u64,
    /// Handle do torrent na sessão local (modo sem daemon/testes); `None` no
    /// modo daemon (a TUI atua por `id` via IPC).
    handle: Option<Arc<ManagedTorrent>>,
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

/// Alvo de uma exclusão definitiva solicitada (US-027): um torrent real na
/// sessão (com arquivos no disco) ou uma origem pendente (ainda sem handle).
#[derive(Clone, Debug)]
enum DeleteTarget {
    /// Torrent real gerenciado pela sessão (id + nome exibido).
    Torrent { id: usize, name: String },
    /// Origem pendente, ainda sem handle — apenas descarta a requisição.
    Pending { name: String },
}

impl DeleteTarget {
    /// Nome exibido no diálogo de confirmação.
    fn name(&self) -> &str {
        match self {
            DeleteTarget::Torrent { name, .. } | DeleteTarget::Pending { name } => name,
        }
    }
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
    /// Deslocamento (em linhas) entre o topo do bloco e a primeira entrada.
    rows_offset: u16,
    /// Número de entradas renderizadas.
    row_count: usize,
    /// Linhas de tela ocupadas por cada entrada (1 = consecutivas; 2 = com
    /// linha de separação/gap entre elas).
    row_stride: u16,
}

/// Mapeamento do Card da Biblioteca (US-047): linha física da tela → índice
/// global do item (torrents + pendentes). Como os nomes podem quebrar em mais
/// de uma linha, cada entrada pode ocupar várias linhas físicas — o clique
/// resolve pelo índice da linha em vez de assumir stride fixo como o `Layout`.
struct LibraryMap {
    /// Linha da tela da primeira linha física de itens.
    top: u16,
    /// Índice global do item de cada linha física (relativa a `top`).
    rows: Vec<usize>,
}

impl LibraryMap {
    /// Índice global do item na linha física `row` (relativa à tela), ou
    /// `None` quando a linha cai fora do bloco de itens.
    fn index_at(&self, row: u16) -> Option<usize> {
        if row < self.top {
            return None;
        }
        self.rows.get((row - self.top) as usize).copied()
    }
}

/// Linha física do Card da Biblioteca (US-047): texto pronto para exibir e,
/// quando a linha carrega uma barra de progresso, o trecho colorido que é
/// sobreposto sobre o texto base.
struct PhysicalRow {
    /// Índice global do item (torrent + pendentes) ao qual a linha pertence.
    item_idx: usize,
    /// Linha de texto base (marcador/ID/barra/status/métricas/nome).
    line: String,
    /// `(texto da barra, cor do estado)` para sobrepor; `None` nas linhas de
    /// continuação de nomes quebrados e na mensagem de fila vazia.
    bar: Option<(String, Color)>,
}

/// Uma célula do buffer de tela (double buffering): um caractere + cores de
/// primeiro plano e de fundo opcionais (`None` = cor padrão do terminal).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Cell {
    ch: char,
    fg: Option<Color>,
    pub(crate) bg: Option<Color>,
}

impl Cell {
    fn blank() -> Self {
        Cell {
            ch: ' ',
            fg: None,
            bg: None,
        }
    }

    /// Caractere da célula (acessível para leitura de contraste/verificação).
    #[cfg(test)]
    pub(crate) fn ch(&self) -> char {
        self.ch
    }

    /// Cor de primeiro plano da célula.
    #[cfg(test)]
    pub(crate) fn fg(&self) -> Option<Color> {
        self.fg
    }
}

/// Paleta unificada do tema escuro profissional (US-019).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Theme {
    /// Cor das bordas de blocos/quadros.
    border: Color,
    /// Texto primário (branco/prata).
    pub(crate) text: Color,
    /// Acento/destaque (ciano).
    pub(crate) accent: Color,
    /// Cor de primeiro plano do prompt da Home (US-039): branca para garantir
    /// contraste sobre o fundo do footer (a cor de acento ciano era ilegível).
    pub(crate) home_prompt_fg: Color,
    /// Sucesso / downloads completos (verde).
    success: Color,
    /// Gauge de progresso concluído 100% (verde mais escuro, US-043).
    progress_done: Color,
    /// Gauge de progresso com downstream baixo (laranja, US-043).
    progress_low: Color,
    /// Avisos (amarelo).
    pub(crate) warning: Color,
    /// Erros (vermelho).
    pub(crate) error: Color,
    /// Texto secundário/dicas (cinza).
    pub(crate) muted: Color,
    /// Fundo do item ativo selecionado (highlight background).
    highlight_bg: Color,
    /// Fundo do Header fixo.
    header_bg: Color,
    /// Fundo do Footer fixo.
    pub(crate) footer_bg: Color,
    /// Fundo sólido do popup flutuante de comandos (US-031): evita que o
    /// conteúdo do painel por trás vaze nas linhas do popup.
    popup_bg: Color,
    /// Fundo elevado do Card de entrada (US-041): cinza escuro contrastando
    /// com o fundo padrão (None/preta) dos painéis.
    pub(crate) card_bg: Color,
    /// Barra de acento vertical do Card (US-041): azul vibrante (`#3B82F6`)
    /// quando o campo está em foco (digitação).
    pub(crate) card_accent_active: Color,
    /// Barra de acento vertical do Card (US-041): cinza neutro quando o campo
    /// está inativo (estado ocioso).
    pub(crate) card_accent_idle: Color,
    /// Barra de acento vertical do Card de Histórico (US-046): laranja,
    /// distinguindo o painel de downloads completos do Card azul do footer.
    pub(crate) card_accent_alt: Color,
    /// Barra de acento vertical do Card da Biblioteca (US-047): verde
    /// fluorescente, distinguindo a lista de processamento dos demais Cards.
    pub(crate) card_accent_library: Color,
}

pub(crate) const THEME: Theme = Theme {
    border: Color::DarkGrey,
    text: Color::White,
    accent: Color::Cyan,
    home_prompt_fg: Color::White,
    success: Color::Green,
    progress_done: Color::DarkGreen,
    progress_low: Color::Rgb {
        r: 255,
        g: 140,
        b: 0,
    },
    warning: Color::Yellow,
    error: Color::Red,
    muted: Color::DarkGrey,
    highlight_bg: Color::DarkBlue,
    header_bg: Color::Blue,
    footer_bg: Color::DarkGrey,
    popup_bg: Color::DarkGrey,
    card_bg: Color::Rgb {
        r: 24,
        g: 24,
        b: 27,
    },
    card_accent_active: Color::Rgb {
        r: 59,
        g: 130,
        b: 246,
    },
    card_accent_idle: Color::DarkGrey,
    card_accent_alt: Color::Rgb {
        r: 249,
        g: 115,
        b: 22,
    },
    card_accent_library: Color::Rgb {
        r: 57,
        g: 255,
        b: 20,
    },
};

/// Buffer de tela (US-016): o render desenha o estado atual neste buffer e, ao
/// aplicar o frame, apenas as células alteradas em relação ao frame anterior
/// são reescritas no terminal (diff-rendering / double buffering).
pub(crate) struct Frame {
    grid: Vec<Cell>,
    cols: u16,
    pub(crate) rows: u16,
    /// Posição desejada do cursor do terminal (ex.: campo de entrada).
    pub(crate) cursor: Option<(u16, u16)>,
}

impl Frame {
    pub(crate) fn new(cols: u16, rows: u16) -> Self {
        Self {
            grid: vec![Cell::blank(); cols as usize * rows as usize],
            cols,
            rows,
            cursor: None,
        }
    }

    fn index(&self, row: u16, col: u16) -> usize {
        row as usize * self.cols as usize + col as usize
    }

    pub(crate) fn cell(&self, row: u16, col: u16) -> Cell {
        if row >= self.rows || col >= self.cols {
            return Cell::blank();
        }
        self.grid[self.index(row, col)]
    }

    /// Escreve um texto a partir de `(row, col)` com destaque em ciano
    /// (`cyan = true`); fora dos limites é ignorado.
    #[cfg(test)]
    fn put(&mut self, row: u16, col: u16, text: &str, cyan: bool) {
        let fg = if cyan { Some(Color::Cyan) } else { None };
        self.put_styled(row, col, text, fg, None);
    }

    /// Escreve um texto com uma cor de primeiro plano explícita.
    fn put_colored(&mut self, row: u16, col: u16, text: &str, fg: Option<Color>) {
        self.put_styled(row, col, text, fg, None);
    }

    /// Escreve um texto com cores de primeiro plano e de fundo explícitas.
    pub(crate) fn put_styled(
        &mut self,
        row: u16,
        col: u16,
        text: &str,
        fg: Option<Color>,
        bg: Option<Color>,
    ) {
        for (col, ch) in (col..).zip(text.chars()) {
            if row >= self.rows || col >= self.cols {
                return;
            }
            let idx = self.index(row, col);
            let cell = &mut self.grid[idx];
            cell.ch = ch;
            cell.fg = fg;
            cell.bg = bg;
        }
    }

    /// Preenche uma região retangular do buffer com um caractere e cores.
    pub(crate) fn fill(
        &mut self,
        row: u16,
        col: u16,
        width: u16,
        ch: char,
        fg: Option<Color>,
        bg: Option<Color>,
    ) {
        if row >= self.rows {
            return;
        }
        // Índice base calculado fora do laço: `self.index` empresta `self`
        // imutavelmente, então o acesso mutável à grid usa o índice direto.
        let base = self.index(row, 0);
        let end = (col as usize + width as usize).min(self.cols as usize);
        for idx in col as usize..end {
            let cell = &mut self.grid[base + idx];
            cell.ch = ch;
            cell.fg = fg;
            cell.bg = bg;
        }
    }

    /// Preenche uma região retangular (várias linhas) do buffer.
    #[allow(clippy::too_many_arguments)]
    fn fill_rect(
        &mut self,
        row: u16,
        col: u16,
        height: u16,
        width: u16,
        ch: char,
        fg: Option<Color>,
        bg: Option<Color>,
    ) {
        for r in row..row.saturating_add(height) {
            self.fill(r, col, width, ch, fg, bg);
        }
    }

    /// Registra a posição desejada do cursor do terminal (para exibir durante
    /// a digitação). `None` indica cursor oculto.
    pub(crate) fn set_cursor(&mut self, row: u16, col: u16) {
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
                // run contíguo de células alteradas com as mesmas cores
                let start = col;
                let mut run = String::new();
                while col < self.cols {
                    let c = self.cell(row, col);
                    if c == prev.cell(row, col) || c.fg != cur.fg || c.bg != cur.bg {
                        break;
                    }
                    run.push(c.ch);
                    col += 1;
                }
                queue!(w, cursor::MoveTo(start, row))?;
                if let Some(bg) = cur.bg {
                    queue!(w, style::SetBackgroundColor(bg))?;
                }
                if let Some(color) = cur.fg {
                    queue!(w, style::SetForegroundColor(color))?;
                }
                queue!(w, style::Print(run))?;
                if cur.bg.is_some() || cur.fg.is_some() {
                    queue!(w, style::ResetColor)?;
                }
            }
        }
        Ok(())
    }
}

pub(crate) struct Tui {
    /// Sessão local do librqbit — `None` no modo daemon (US-040), quando a
    /// fila é renderizada a partir do snapshot ocasional do socket Unix.
    session: Option<Arc<Session>>,
    /// Último snapshot da fila recebido do daemon (modo daemon).
    snapshot: DaemonState,
    output_folder: PathBuf,
    view: View,
    /// Current command being typed in the title input field.
    input: String,
    /// Selected menu item index.
    menu_index: usize,
    /// Selected session row index.
    row_index: usize,
    /// Cursor position within the home prompt `input` (byte index).
    prompt_cursor: usize,
    /// Modo "insira o link:" do prompt da Home (US-034): ativo após `/add`
    /// sem origem; o texto digitado é a origem a adicionar.
    prompt_add_mode: bool,
    /// Transient status/error message.
    notice: Option<String>,
    /// Exclusão definitiva aguardando confirmação Y/N (US-027).
    confirming_delete: Option<DeleteTarget>,
    /// Menu de contexto aberto na Biblioteca (US-036); `None` = fechado.
    context_menu: Option<ContextMenu>,
    /// Geometria (left, top, width, height) do popup de contexto no último
    /// render (US-036), para mapear cliques do mouse nas opções.
    context_rect: Option<(u16, u16, u16, u16)>,
    /// Keys received from the keyboard thread.
    keys: mpsc::UnboundedReceiver<Event>,
    /// Geometria do último render (usada para mapear cliques do mouse).
    layout: Option<Layout>,
    /// Mapeamento do Card da Biblioteca (US-047): linha física → índice do
    /// item, considerando a quebra de linha dos nomes. `None` quando o card
    /// não está visível.
    library_map: Option<LibraryMap>,
    /// Deslocamento vertical (em linhas físicas, considerando quebras de nome)
    /// do Card da Biblioteca (US-047).
    library_scroll: usize,
    /// Região (linha do topo, altura) do Card da Biblioteca, para a rolagem
    /// com o mouse (US-047). `None` quando o card não está visível.
    library_region: Option<(u16, u16)>,
    /// Origens adicionadas em segundo plano, compartilhadas com as tasks.
    pending: Arc<Mutex<Vec<PendingTorrent>>>,
    /// Frame anterior renderizado (double buffering / diff-rendering).
    prev_frame: Option<Frame>,
    /// Posição do cursor do terminal exibida no último frame (`None` = oculto).
    prev_cursor: Option<(u16, u16)>,
    /// Cache do histórico de downloads (US-045): evita re-escanear o disco a
    /// cada frame, que fazia a lista "reordenar" continuamente enquanto a
    /// última varredura ainda era válida. `(timestamp, itens)`.
    history_cache: Option<(Instant, Vec<crate::downloads::CompletedItem>)>,
    /// Estilo da área inferior da TUI (US-041): `Legacy` mantém o prompt no
    /// fim do Body + footer; `Elevated` usa o novo Card.
    bottom_style: BottomStyle,
    /// Deslocamento vertical do grid do Card de Histórico (US-046): quantas
    /// linhas de itens foram roladas para cima.
    history_scroll: usize,
    /// Região (linha do topo, altura) do Card de Histórico, para a rolagem com
    /// o mouse (US-046). `None` quando o card não está visível.
    history_region: Option<(u16, u16)>,
    running: bool,
}

impl Tui {
    fn new(
        session: Arc<Session>,
        output_folder: PathBuf,
        keys: mpsc::UnboundedReceiver<Event>,
    ) -> Self {
        Self {
            session: Some(session),
            snapshot: DaemonState::default(),
            output_folder,
            view: View::Home,
            input: String::new(),
            menu_index: 0,
            row_index: 0,
            prompt_cursor: 0,
            prompt_add_mode: false,
            notice: None,
            confirming_delete: None,
            context_menu: None,
            context_rect: None,
            keys,
            layout: None,
            library_map: None,
            library_scroll: 0,
            library_region: None,
            pending: Arc::new(Mutex::new(Vec::new())),
            prev_frame: None,
            prev_cursor: None,
            history_cache: None,
            bottom_style: BOTTOM_STYLE,
            history_scroll: 0,
            history_region: None,
            running: true,
        }
    }

    /// Constrói a TUI no modo daemon (US-040): sem sessão local, renderizando
    /// a partir do snapshot recebido via socket Unix.
    fn new_daemon(output_folder: PathBuf, keys: mpsc::UnboundedReceiver<Event>) -> Self {
        Self {
            session: None,
            snapshot: DaemonState::default(),
            output_folder,
            view: View::Home,
            input: String::new(),
            menu_index: 0,
            row_index: 0,
            prompt_cursor: 0,
            prompt_add_mode: false,
            notice: None,
            confirming_delete: None,
            context_menu: None,
            context_rect: None,
            keys,
            layout: None,
            library_map: None,
            library_scroll: 0,
            library_region: None,
            pending: Arc::new(Mutex::new(Vec::new())),
            prev_frame: None,
            prev_cursor: None,
            history_cache: None,
            bottom_style: BOTTOM_STYLE,
            history_scroll: 0,
            history_region: None,
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
                if matches!(self.view, View::Home | View::Menu) {
                    // Paste no prompt da base (US-031): insere na posição do
                    // cursor; se começar com `/`, abre o popup de comandos.
                    let before = &self.input[..self.prompt_cursor];
                    let after = &self.input[self.prompt_cursor..];
                    self.input = format!("{before}{text}{after}");
                    self.prompt_cursor += text.len();
                    self.notice = None;
                    // A seleção do popup volta ao topo após o paste (a lista
                    // filtrada pode ter mudado de tamanho).
                    self.menu_index = 0;
                    if should_open_command_menu(&self.input)
                        && !matches!(self.view, View::Menu)
                        && !self.prompt_add_mode
                    {
                        self.view = View::Menu;
                    } else {
                        self.leave_menu_for_inline_add();
                    }
                }
                (true, true)
            }
            Event::Mouse(mouse) => {
                if let Err(err) = self.handle_mouse(mouse).await {
                    self.notice = Some(format!("erro no clique: {err:#}"));
                }
                // Cliques e a rolagem do mouse alteram o estado; movimentos
                // não redesenham.
                let click = matches!(
                    mouse.kind,
                    MouseEventKind::Down(MouseButton::Left | MouseButton::Right)
                        | MouseEventKind::ScrollUp
                        | MouseEventKind::ScrollDown
                );
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
            View::Home => self.handle_home_key(key).await?,
            View::Menu => self.handle_menu_key(key).await?,
        }
        Ok(self.running)
    }

    /// Teclas da Home (US-031): qualquer caractere vai para o prompt fixo da
    /// base; `/` (ou um prompt iniciado com `/`) abre o menu flutuante; Enter
    /// executa o comando digitado; ↑/↓ navegam na Biblioteca. No modo
    /// "insira o link:" (US-034), delega para `handle_add_prompt_key`.
    async fn handle_home_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        // US-027: com a confirmação de exclusão aberta (via /delete), apenas
        // S/N/Esc agem — mesmo estando na Home (comando do menu flutuante).
        if self.confirming_delete.is_some() {
            match key.code {
                KeyCode::Char('s')
                | KeyCode::Char('S')
                | KeyCode::Char('y')
                | KeyCode::Char('Y') => self.confirm_delete().await?,
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirming_delete = None;
                    self.notice = Some("exclusão cancelada".into());
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.running = false;
                }
                _ => {}
            }
            return Ok(());
        }

        // US-034: no modo "insira o link:", as teclas editam a origem no
        // próprio prompt da Home (sem abrir o menu de comandos).
        if self.prompt_add_mode {
            return self.handle_add_prompt_key(key).await;
        }

        // US-036: com o menu de contexto aberto, apenas ↑/↓/Enter/Esc agem.
        if self.context_menu.is_some() {
            return self.handle_context_menu_key(key).await;
        }

        // US-041: Ctrl+P abre o menu de comandos. Precisa ser tratado antes
        // das teclas únicas da Biblioteca, que capturam `p`.
        if matches!(key.code, KeyCode::Char('p') | KeyCode::Char('P'))
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.input.clear();
            self.prompt_cursor = 0;
            self.menu_index = 0;
            self.view = View::Menu;
            return Ok(());
        }

        // US-037: com o prompt vazio, teclas únicas agem na linha selecionada
        // da Biblioteca (pausar/retomar/remover/excluir).
        if let Some(action) = home_action_for_key(
            key.code,
            self.input.is_empty(),
            self.prompt_add_mode,
            self.confirming_delete.is_some(),
        ) {
            match action {
                HomeAction::Pause => self.pause_row().await?,
                HomeAction::Resume => self.resume_row().await?,
                HomeAction::Remove => self.remove_row().await?,
                HomeAction::Delete => self.request_delete(self.row_index),
            }
            return Ok(());
        }

        match key.code {
            KeyCode::Char('/') => {
                self.input = "/".into();
                self.prompt_cursor = 1;
                self.menu_index = 0;
                self.view = View::Menu;
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_prompt_char(c);
                self.notice = None;
                // Filtragem dinâmica: qualquer texto iniciado com `/` abre o
                // popup — exceto o prefixo inline "/add " (US-034), cujo
                // conteúdo após o espaço é a origem digitada no próprio prompt.
                if should_open_command_menu(&self.input) {
                    self.menu_index = 0;
                    self.view = View::Menu;
                }
            }
            KeyCode::Enter => {
                let cmd = self.input.trim().to_string();
                self.input.clear();
                self.prompt_cursor = 0;
                if let Some(source) = parse_add_command(&cmd) {
                    // `/add <origem>` inline (US-034): adiciona direto.
                    // Em caso de erro, o comando é restaurado no prompt para
                    // correção.
                    if !self.submit_add_from_prompt(&source) {
                        self.input = cmd;
                        self.prompt_cursor = self.input.len();
                    }
                } else if cmd == "/add" {
                    // `/add` sem origem: entra no modo "insira o link:".
                    self.prompt_add_mode = true;
                    self.notice = None;
                } else if cmd.starts_with('/') {
                    self.execute_command(&cmd).await?;
                }
            }
            KeyCode::Backspace => {
                self.backspace_prompt();
                // US-034: ao apagar o espaço de "/add ", a origem inline deixa
                // de existir e o prompt volta a buscar comando no popup.
                if should_open_command_menu(&self.input) {
                    self.menu_index = 0;
                    self.view = View::Menu;
                }
            }
            KeyCode::Esc => {
                self.input.clear();
                self.prompt_cursor = 0;
            }
            KeyCode::Up => self.move_row(-1),
            KeyCode::Down => self.move_row(1),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            _ => {}
        }
        Ok(())
    }

    /// Teclas do modo "insira o link:" do prompt da Home (US-034): Enter
    /// submete a origem digitada/colada, Esc cancela e volta ao prompt normal,
    /// e caracteres/backspace editam o texto da origem no próprio prompt.
    async fn handle_add_prompt_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Enter => {
                let source = self.input.clone();
                self.submit_add_from_prompt(&source);
            }
            KeyCode::Esc => {
                self.prompt_add_mode = false;
                self.input.clear();
                self.prompt_cursor = 0;
                self.notice = None;
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_prompt_char(c);
                self.notice = None;
            }
            KeyCode::Backspace => self.backspace_prompt(),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            _ => {}
        }
        Ok(())
    }

    /// Insere um caractere no prompt da Home na posição do cursor (byte index).
    fn insert_prompt_char(&mut self, c: char) {
        self.prompt_cursor = insert_char_at(&mut self.input, self.prompt_cursor, c);
    }

    /// Apaga o caractere anterior ao cursor no prompt da Home.
    fn backspace_prompt(&mut self) {
        self.prompt_cursor = backspace_char_at(&mut self.input, self.prompt_cursor);
    }

    /// Teclas do popup de comandos (US-031): ↑/↓ navegam, Enter executa o
    /// comando selecionado, Tab completa o prompt, caracteres filtram a lista
    /// conforme o texto digitado após `/` e Esc fecha.
    async fn handle_menu_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        let filtered = self.filtered_commands();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if !filtered.is_empty() {
                    self.menu_index = (self.menu_index + filtered.len() - 1) % filtered.len();
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !filtered.is_empty() {
                    self.menu_index = (self.menu_index + 1) % filtered.len();
                }
            }
            KeyCode::Enter => {
                if let Some((cmd, _)) = filtered.get(self.menu_index) {
                    self.input = cmd.clone();
                    self.prompt_cursor = self.input.len();
                    self.execute_command(cmd).await?;
                }
            }
            KeyCode::Tab => {
                // Tab completa o prompt com o comando selecionado (AC: seleção
                // via Enter ou Tab) e volta para a Home para edição/execução.
                if let Some((cmd, _)) = filtered.get(self.menu_index) {
                    self.input = cmd.clone();
                    self.prompt_cursor = self.input.len();
                    self.view = View::Home;
                }
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.insert_prompt_char(c);
                self.menu_index = 0;
                // US-034: ao digitar o espaço após "/add", o popup sai da
                // frente para a origem inline ser digitada no prompt da Home.
                self.leave_menu_for_inline_add();
            }
            KeyCode::Backspace => {
                self.backspace_prompt();
                self.menu_index = 0;
                // Fecha o popup quando o prompt volta a ficar sem `/`.
                if !self.input.starts_with('/') {
                    self.input.clear();
                    self.prompt_cursor = 0;
                    self.view = View::Home;
                }
            }
            KeyCode::Esc => {
                self.input.clear();
                self.prompt_cursor = 0;
                self.view = View::Home;
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.running = false;
            }
            _ => {}
        }
        Ok(())
    }

    /// Comandos disponíveis filtrados pelo texto digitado após `/`.
    fn filtered_commands(&self) -> Vec<(String, String)> {
        filter_commands(&self.input)
    }

    /// US-034: quando o popup de comandos está aberto e o prompt assume o
    /// prefixo inline `/add ` (origem digitada/colada após o espaço), o popup
    /// é fechado para o usuário continuar digitando a origem no prompt da Home.
    fn leave_menu_for_inline_add(&mut self) {
        if self.input.starts_with("/add ") && matches!(self.view, View::Menu) {
            self.view = View::Home;
        }
    }

    /// Executa um comando digitado/selecionado (US-031): mapeia cada comando
    /// do menu flutuante para a ação correspondente na interface.
    async fn execute_command(&mut self, cmd: &str) -> anyhow::Result<()> {
        match cmd {
            "/add" => {
                // US-034: `/add` ativa o modo "insira o link:" no próprio
                // prompt da Home — sem abrir a view Include.
                self.prompt_add_mode = true;
                self.input.clear();
                self.prompt_cursor = 0;
                self.view = View::Home;
                self.notice = None;
            }
            "/pause" => self.pause_row().await?,
            "/resume" => self.resume_row().await?,
            "/delete" => {
                // US-035: a confirmação é exibida no prompt da Home — garante
                // a view mesmo quando o /delete vem do popup de comandos.
                self.request_delete(self.row_index);
                self.view = View::Home;
            }
            "/help" => {
                let help: Vec<String> = COMMANDS
                    .iter()
                    .map(|(cmd, desc)| format!("{cmd} — {desc}"))
                    .collect();
                self.notice = Some(format!("comandos: {}", help.join(" · ")));
            }
            "/exit" => self.running = false,
            other => self.notice = Some(format!("comando desconhecido: {other}")),
        }
        Ok(())
    }

    /// Handle one mouse event, dispatching clicks to the active view.
    async fn handle_mouse(&mut self, me: MouseEvent) -> anyhow::Result<()> {
        // US-046/047: rolagem vertical dos Cards de Histórico e Biblioteca
        // (mouse wheel).
        if matches!(
            me.kind,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown
        ) {
            let step: isize = if me.kind == MouseEventKind::ScrollUp {
                -1
            } else {
                1
            };
            if let Some((top, height)) = self.library_region
                && (top..top.saturating_add(height)).contains(&me.row)
            {
                self.library_scroll = (self.library_scroll as isize + step).max(0) as usize;
            } else if let Some((top, height)) = self.history_region
                && (top..top.saturating_add(height)).contains(&me.row)
            {
                self.history_scroll = (self.history_scroll as isize + step).max(0) as usize;
            }
            return Ok(());
        }
        if !matches!(
            me.kind,
            MouseEventKind::Down(MouseButton::Left | MouseButton::Right)
        ) {
            return Ok(());
        }
        // Durante a confirmação de exclusão o mouse não interfere (US-027).
        if self.confirming_delete.is_some() {
            return Ok(());
        }
        match self.view {
            View::Menu => {
                self.mouse_menu(me.row);
                Ok(())
            }
            View::Home => {
                // US-036: com o popup de contexto aberto, o clique esquerdo
                // executa a opção clicada (dentro do popup) ou fecha (fora).
                if self.context_menu.is_some() {
                    if matches!(me.kind, MouseEventKind::Down(MouseButton::Left)) {
                        if let Some(rect) = self.context_rect {
                            let options = self.current_context_options();
                            if (rect.0..rect.0 + rect.2).contains(&me.column)
                                && let Some(idx) = context_option_at(me.row, rect, options.len())
                            {
                                let action = options[idx];
                                self.execute_context_action(action).await?;
                                return Ok(());
                            }
                        }
                        self.close_context_menu();
                    }
                    return Ok(());
                }
                // US-036: clique direito na Biblioteca abre o menu de contexto.
                if matches!(me.kind, MouseEventKind::Down(MouseButton::Right)) {
                    self.open_context_menu(me.row);
                    return Ok(());
                }
                // Clique esquerdo move a seleção para a linha clicada.
                self.mouse_home(me.row);
                Ok(())
            }
        }
    }

    /// Clique na Biblioteca da Home: move a seleção para o item clicado,
    /// resolvendo a linha física pelo `library_map` (que considera nomes
    /// quebrados em várias linhas). Cai no `layout` quando o card não existe.
    fn mouse_home(&mut self, row: u16) {
        if let Some(map) = &self.library_map {
            if let Some(idx) = map.index_at(row) {
                self.row_index = idx;
            }
            return;
        }
        let Some(layout) = self.layout else {
            return;
        };
        let first_row = layout.top.saturating_add(layout.rows_offset);
        let Some(idx) = table_row_to_index(row, first_row, layout.row_count, layout.row_stride)
        else {
            return;
        };
        self.row_index = idx;
    }

    /// Clique no menu: move o cursor de seleção (indicador + destaque) para a
    /// opção clicada, em tempo real (AC-3).
    fn mouse_menu(&mut self, row: u16) {
        let Some(layout) = self.layout else {
            return;
        };
        let first_row = layout.top.saturating_add(layout.rows_offset);
        let Some(idx) = table_row_to_index(row, first_row, layout.row_count, layout.row_stride)
        else {
            return;
        };
        self.menu_index = idx;
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
    pub(crate) fn truncate(s: &str, max: usize) -> String {
        if s.chars().count() <= max {
            return s.to_string();
        }
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }

    /// Abre o menu de contexto (US-036) sobre o item clicado da Biblioteca:
    /// resolve a linha clicada (mesmo mapeamento de `mouse_home`), passa a
    /// seleção para ela e ancora o popup. Não faz nada se o clique cai fora
    /// das linhas de itens.
    fn open_context_menu(&mut self, row: u16) {
        if let Some(map) = &self.library_map {
            let Some(idx) = map.index_at(row) else {
                return;
            };
            self.open_context_menu_at(idx, row);
            return;
        }
        let Some(layout) = self.layout else {
            return;
        };
        let first_row = layout.top.saturating_add(layout.rows_offset);
        let Some(idx) = table_row_to_index(row, first_row, layout.row_count, layout.row_stride)
        else {
            return;
        };
        self.open_context_menu_at(idx, row);
    }

    /// Ancoras o popup de contexto (US-036) sobre o item `idx` da Biblioteca.
    fn open_context_menu_at(&mut self, idx: usize, row: u16) {
        self.row_index = idx;
        self.context_menu = Some(ContextMenu {
            entry_index: idx,
            selected: 0,
            anchor_row: row,
        });
        self.context_rect = None;
    }

    /// Fecha o menu de contexto (US-036), mantendo a seleção da linha.
    fn close_context_menu(&mut self) {
        self.context_menu = None;
        self.context_rect = None;
    }

    /// Opções atuais do menu de contexto, conforme o item clicado (US-036).
    fn current_context_options(&self) -> Vec<ContextAction> {
        let entry_index = self.context_menu.map(|cm| cm.entry_index).unwrap_or(0);
        context_menu_options(entry_index, self.session_rows().len())
    }

    /// Teclas do menu de contexto aberto (US-036): ↑/↓ movem a opção
    /// destacada, Enter executa, Esc fecha; demais teclas são ignoradas.
    async fn handle_context_menu_key(&mut self, key: KeyEvent) -> anyhow::Result<()> {
        match key.code {
            KeyCode::Up => {
                let len = self.current_context_options().len();
                if let Some(cm) = &mut self.context_menu {
                    cm.selected = cycle_context_selection(cm.selected, len, -1);
                }
            }
            KeyCode::Down => {
                let len = self.current_context_options().len();
                if let Some(cm) = &mut self.context_menu {
                    cm.selected = cycle_context_selection(cm.selected, len, 1);
                }
            }
            KeyCode::Enter => {
                let options = self.current_context_options();
                if let Some(cm) = self.context_menu
                    && let Some(action) = options.get(cm.selected).copied()
                {
                    self.execute_context_action(action).await?;
                }
            }
            KeyCode::Esc => self.close_context_menu(),
            _ => {}
        }
        Ok(())
    }

    /// Executa a ação escolhida no menu de contexto (US-036): move a seleção
    /// para o item clicado, fecha o popup e despacha para a ação existente.
    async fn execute_context_action(&mut self, action: ContextAction) -> anyhow::Result<()> {
        let entry_index = self.context_menu.map(|cm| cm.entry_index).unwrap_or(0);
        self.row_index = entry_index;
        self.close_context_menu();
        match action {
            ContextAction::Pause => self.pause_row().await?,
            ContextAction::Resume => self.resume_row().await?,
            ContextAction::Stop => self.remove_row().await?,
            ContextAction::Delete => self.request_delete(entry_index),
        }
        Ok(())
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
        // US-040: modo daemon → fila a partir do snapshot vinda do socket.
        let rows = match &self.session {
            None => self
                .snapshot
                .torrents
                .iter()
                .map(|t| SessionRow {
                    id: t.id,
                    name: t.name.clone(),
                    state: state_from_display(&t.state),
                    progress_bytes: t.progress_bytes,
                    total_bytes: t.total_bytes,
                    finished: t.finished,
                    down_speed_mbps: t.down_speed_mbps,
                    up_speed_mbps: t.up_speed_mbps,
                    eta_seconds: t.eta_seconds,
                    uploaded_bytes: t.uploaded_bytes,
                    handle: None,
                })
                .collect(),
            Some(session) => {
                let rows = std::cell::RefCell::new(Vec::new());
                session.with_torrents(|torrents| {
                    for (id, handle) in torrents {
                        let stats = handle.stats();
                        // Métricas de rede (US-033): `live` só existe na sessão Live;
                        // pausado/inicializando/erro não têm velocidade nem ETA.
                        let down_speed_mbps = stats.live.as_ref().map(|l| l.download_speed.mbps);
                        let up_speed_mbps = stats.live.as_ref().map(|l| l.upload_speed.mbps);
                        let eta_seconds = if stats.finished {
                            None
                        } else {
                            stats.live.as_ref().and_then(|l| {
                                let remaining =
                                    stats.total_bytes.saturating_sub(stats.progress_bytes);
                                (l.download_speed.mbps > 0.0 && remaining > 0).then(|| {
                                    remaining / (l.download_speed.mbps * 1024.0 * 1024.0) as u64
                                })
                            })
                        };
                        rows.borrow_mut().push(SessionRow {
                            id,
                            name: handle.name().unwrap_or_else(|| format!("torrent #{id}")),
                            state: stats.state,
                            progress_bytes: stats.progress_bytes,
                            total_bytes: stats.total_bytes,
                            finished: stats.finished,
                            down_speed_mbps,
                            up_speed_mbps,
                            eta_seconds,
                            uploaded_bytes: stats.uploaded_bytes,
                            handle: Some(handle.clone()),
                        });
                    }
                });
                rows.into_inner()
            }
        };
        // US-042: ordenação automática do grid — torrents em processamento no
        // topo e, dentro de cada grupo, o mais recente inserido primeiro.
        let mut rows = rows;
        sort_rows_for_grid(&mut rows);
        rows
    }

    /// Atualiza o snapshot no modo daemon (US-040). Sem efeito no modo local.
    async fn refresh_snapshot(&mut self) {
        if self.session.is_some() {
            return;
        }
        let mut client = match daemon::connect_client().await {
            Ok(c) => c,
            Err(_) => return, // daemon indisponível: mantém o último snapshot
        };
        if let Ok(state) = client.get_state().await {
            self.snapshot = state;
        }
    }

    /// Envia uma ação ao daemon (modo daemon, US-040) e retorna a mensagem de
    /// sucesso ou o erro em `DaemonResponse::Error`.
    async fn daemon_action(&mut self, req: DaemonRequest) -> anyhow::Result<Option<String>> {
        let mut client = daemon::connect_client().await?;
        match client.request(req).await? {
            DaemonResponse::Ok { message } => Ok(Some(message)),
            DaemonResponse::AlreadyManaged => Ok(None),
            DaemonResponse::Error { message } => anyhow::bail!(message),
            other => anyhow::bail!("resposta inesperada do daemon: {other:?}"),
        }
    }

    async fn pause_row(&mut self) -> anyhow::Result<()> {
        let rows = self.session_rows();
        if let Some(row) = rows.get(self.row_index).cloned() {
            match &self.session {
                Some(session) => {
                    if let Some(handle) = &row.handle {
                        session.pause(handle).await?;
                    }
                }
                None => {
                    self.daemon_action(DaemonRequest::Pause { id: row.id })
                        .await?;
                }
            }
            self.notice = Some(format!("pausado: {}", row.name));
        } else {
            // Linha selecionada é uma origem pendente (ainda sem handle).
            self.notice = Some("torrent em inicialização — aguarde a resolução".into());
        }
        Ok(())
    }

    async fn resume_row(&mut self) -> anyhow::Result<()> {
        let rows = self.session_rows();
        if let Some(row) = rows.get(self.row_index).cloned() {
            match &self.session {
                Some(session) => {
                    if let Some(handle) = &row.handle {
                        session.clone().unpause(handle).await?;
                    }
                }
                None => {
                    self.daemon_action(DaemonRequest::Resume { id: row.id })
                        .await?;
                }
            }
            self.notice = Some(format!("retomado: {}", row.name));
        } else {
            self.notice = Some("torrent em inicialização — aguarde a resolução".into());
        }
        Ok(())
    }

    async fn remove_row(&mut self) -> anyhow::Result<()> {
        let rows = self.session_rows();
        if let Some(row) = rows.get(self.row_index).cloned() {
            match &self.session {
                Some(session) => {
                    session
                        .delete(librqbit::api::TorrentIdOrHash::Id(row.id), false)
                        .await?;
                }
                None => {
                    self.daemon_action(DaemonRequest::Stop { id: row.id })
                        .await?;
                }
            }
            self.notice = Some(format!("removido da fila: {}", row.name));
        } else {
            // Origem pendente (ainda sem handle): apenas descarta a requisição.
            let rel = self.row_index.saturating_sub(rows.len());
            let mut guard = self.pending.lock().unwrap();
            if rel < guard.len() {
                let source = guard.remove(rel).source;
                self.notice = Some(format!("removido da fila: {source}"));
            }
        }
        self.clamp_row_index();
        Ok(())
    }

    /// Solicita a exclusão definitiva do item focado (tecla Delete ou botão
    /// Excluir): registra o alvo exato e abre a confirmação Y/N (US-027).
    fn request_delete(&mut self, entry_index: usize) {
        let rows = self.session_rows();
        let target = if let Some(row) = rows.get(entry_index) {
            DeleteTarget::Torrent {
                id: row.id,
                name: row.name.clone(),
            }
        } else {
            let rel = entry_index.saturating_sub(rows.len());
            let guard = self.pending.lock().unwrap();
            let Some(entry) = guard.get(rel) else {
                return;
            };
            DeleteTarget::Pending {
                name: entry.source.clone(),
            }
        };
        self.confirming_delete = Some(target);
        self.notice = None;
    }

    /// Efetiva a exclusão confirmada: remove o torrent da sessão e apaga os
    /// arquivos do disco (`delete_files = true`); pendentes são descartados.
    async fn confirm_delete(&mut self) -> anyhow::Result<()> {
        let Some(target) = self.confirming_delete.take() else {
            return Ok(());
        };
        match target {
            DeleteTarget::Torrent { id, name } => {
                // Erro capturado como aviso (e não propagado): uma falha ao
                // apagar os arquivos não deve encerrar a TUI via handle_event.
                let result = match &self.session {
                    Some(session) => {
                        session
                            .delete(librqbit::api::TorrentIdOrHash::Id(id), true)
                            .await
                    }
                    None => match self.daemon_action(DaemonRequest::Delete { id }).await {
                        Ok(_) => Ok(()),
                        Err(err) => Err(anyhow::anyhow!(format!("{err:#}"))),
                    },
                };
                if let Err(err) = result {
                    self.notice = Some(format!("falha ao excluir: {err:#}"));
                    return Ok(());
                }
                self.notice = Some(format!("excluído: {name} (arquivos removidos)"));
            }
            DeleteTarget::Pending { name } => {
                let mut guard = self.pending.lock().unwrap();
                let before = guard.len();
                guard.retain(|p| p.source != name);
                if guard.len() == before {
                    // Já foi resolvido (virou torrent real) entre o pedido e a
                    // confirmação — não há pendente a descartar.
                    self.notice = Some(format!("{name} já não está pendente"));
                } else {
                    self.notice = Some(format!("excluído: {name} (pendente descartado)"));
                }
            }
        }
        self.clamp_row_index();
        Ok(())
    }

    /// Submete uma origem digitada/colada no prompt da Home (US-034): valida a
    /// sintaxe e, se válida, registra a requisição e dispara a adição em
    /// segundo plano — permanecendo na Home. Em erro/vazio o texto digitado é
    /// preservado para correção. Retorna `false` quando a origem não foi
    /// submetida (vazia ou inválida).
    fn submit_add_from_prompt(&mut self, source: &str) -> bool {
        let source = source.trim().to_string();
        self.notice = None;

        if source.is_empty() {
            self.notice = Some("informe uma origem (magnet, .torrent ou URL)".into());
            return false;
        }

        if let Err(err) = AddTorrent::from_cli_argument(&source) {
            self.notice = Some(format!("origem inválida: {err:#}"));
            return false;
        }

        // Origem válida: limpa o prompt, desativa o modo e inicia a adição.
        self.input.clear();
        self.prompt_cursor = 0;
        self.prompt_add_mode = false;
        self.start_add_background(source);
        true
    }

    /// Registra a origem na fila de pendentes e dispara a adição assíncrona
    /// (US-014/US-034), posicionando a seleção na nova entrada.
    fn start_add_background(&mut self, source: String) {
        self.pending.lock().unwrap().push(PendingTorrent {
            source: source.clone(),
            status: PendingStatus::Resolving,
        });
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

        // US-019 (redesign): a tela é estruturada em três seções fixas —
        // Header (topo), Body (conteúdo) e Footer (atalhos/status).
        let mut frame = Frame::new(cols, rows);
        self.render_header(&mut frame, cols);

        let body_top = 1u16;
        // US-041: o Body cede as linhas da área inferior (Card no estilo
        // Elevated; apenas o footer no Legacy). A subtração extra de 1 linha
        // garante um respiro entre o último painel e o topo do Card (evita o
        // grid de Histórico deslizando por trás do card footer).
        let geometry = ui_bottom::bottom_geometry(self.bottom_style, rows);
        let body_height = rows
            .saturating_sub(geometry.reserved)
            .saturating_sub(1)
            .max(1);
        match self.view {
            View::Home => self.render_home(&mut frame, cols, body_top, body_height),
            View::Menu => self.render_menu(&mut frame, cols, body_top, body_height),
        }

        self.render_bottom(&mut frame, cols, rows, &geometry);

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
            // Após o clear total o cursor é sempre reemitido abaixo, mesmo que
            // a posição não tenha mudado (evita cursor oculto durante a
            // digitação após redimensionamento/troca de view).
            self.prev_cursor = None;
        } else if let Some(prev) = &self.prev_frame {
            frame.apply_diff(w, prev)?;
        }

        // Cursor do terminal: visível apenas durante a digitação (include).
        // Emite apenas quando a posição/visibilidade muda — frames ociosos
        // (sem alterações) não reescrevem o cursor.
        if frame.cursor != self.prev_cursor {
            match frame.cursor {
                Some((row, col)) => queue!(w, cursor::MoveTo(col, row), cursor::Show)?,
                None => queue!(w, cursor::Hide)?,
            }
            self.prev_cursor = frame.cursor;
        }
        w.flush()?;

        self.prev_frame = Some(frame);
        Ok(())
    }

    /// Tela inicial (US-031, estilo opencode): dois painéis de linha fina —
    /// "Biblioteca de downloads" (torrents da sessão) no topo e "Histórico de
    /// downloads" (completos) no meio — com o prompt fixo na base.
    fn render_home(&mut self, frame: &mut Frame, cols: u16, body_top: u16, body_height: u16) {
        // No estilo Elevado (US-041), o Card é desenhado fora do Body (em
        // `render_bottom`); os painéis usam todo o espaço do Body. No Legacy, o
        // prompt fixo ocupa a última linha e os painéis ficam acima.
        let (prompt_row, panels_height) = match self.bottom_style {
            BottomStyle::Elevated => (0, body_height),
            BottomStyle::Legacy => (
                body_top.saturating_add(body_height).saturating_sub(1),
                body_height.saturating_sub(1).max(2),
            ),
        };
        // Divisão vertical: Biblioteca (topo, ~55%) e Histórico (~45%). Em
        // terminais muito baixos os dois painéis somam exatamente o espaço
        // disponível (sem sobrepor o prompt): o histórico encolhe primeiro e
        // desaparece abaixo de 6 linhas de painel.
        let lib_height = (panels_height * 55 / 100)
            .max(3)
            .min(panels_height.saturating_sub(3));
        let hist_height = panels_height.saturating_sub(lib_height);

        if hist_height >= 3 {
            let hist_top = body_top.saturating_add(lib_height);
            self.render_library_card(frame, cols, body_top, lib_height);
            self.render_history_card(frame, cols, hist_top, hist_height);
        } else {
            // Espaço só para a Biblioteca: o histórico é omitido.
            self.render_library_card(frame, cols, body_top, panels_height);
        }
        if self.bottom_style == BottomStyle::Legacy {
            self.render_home_prompt(frame, cols, prompt_row);
        }

        // US-036: o menu de contexto fica por cima de tudo na Home.
        if self.context_menu.is_some() {
            self.render_context_menu(frame, cols, body_top, body_height);
        }
    }

    /// Popup de contexto da Biblioteca (US-036): desenhado sobre a Home,
    /// ancorado na linha clicada, com as opções do item (Pausar/Retomar/Parar/
    /// Excluir; só Excluir para pendentes). O item destacado recebe highlight
    /// de fundo (padrão US-031). Grava `context_rect` para cliques do mouse.
    fn render_context_menu(
        &mut self,
        frame: &mut Frame,
        cols: u16,
        body_top: u16,
        body_height: u16,
    ) {
        let Some(cm) = self.context_menu else {
            return;
        };
        let options = self.current_context_options();
        let lines: Vec<String> = options
            .iter()
            .enumerate()
            .map(|(i, action)| context_item_line(context_action_label(*action), i == cm.selected))
            .collect();

        let inner_w = (cols as usize).saturating_sub(8);
        let width = lines
            .iter()
            .map(|l| Self::truncate(l, inner_w).chars().count() as u16)
            .max()
            .unwrap_or(0)
            .saturating_add(4)
            .min(cols.saturating_sub(4));
        // Altura mínima de 3 (bordas), limitada ao espaço do Body acima do
        // prompt; cada opção cabe (a altura soma as bordas sem reservar linha).
        let height = (lines.len() as u16)
            .saturating_add(2)
            .clamp(3, body_height.saturating_sub(1).max(3));
        // Ancora no item clicado, com clamp vertical dentro do Body.
        let bottom = body_top.saturating_add(body_height);
        let top = cm
            .anchor_row
            .min(bottom.saturating_sub(height))
            .max(body_top);
        let left = LAYOUT_MARGIN_COLS;
        let content_left = left + 2;
        let content_w = width.saturating_sub(4) as usize;

        // Fundo sólido do popup (evita vazar o conteúdo dos painéis por trás).
        frame.fill_rect(top, left, height, width, ' ', None, Some(THEME.popup_bg));
        draw_box(frame, left, top, width, height);

        for (idx, line) in lines.iter().enumerate() {
            let row = top + 1 + idx as u16;
            let text = Self::truncate(line, content_w);
            if idx == cm.selected {
                // O highlight cobre apenas o interior (não invade a borda).
                frame.fill(
                    row,
                    content_left,
                    width - 3,
                    ' ',
                    None,
                    Some(THEME.highlight_bg),
                );
                frame.put_styled(
                    row,
                    content_left,
                    &text,
                    Some(THEME.text),
                    Some(THEME.highlight_bg),
                );
            } else {
                frame.put_styled(
                    row,
                    content_left,
                    &text,
                    Some(THEME.text),
                    Some(THEME.popup_bg),
                );
            }
        }

        // Geometria para cliques do mouse nas opções do popup.
        self.context_rect = Some((left, top, width, height));
    }

    /// Card da Biblioteca (US-047): mesmo estilo do Card do Histórico, com a
    /// barra de acento esquerda **verde fluorescente**. Exibe a fila de
    /// torrents (ativos + pendentes) em tabela com colunas responsivas — a
    /// barra de progresso encolhe antes do nome em terminais estreitos —,
    /// nomes longos quebrados em várias linhas e rolagem vertical (mouse)
    /// com scrollbar quando o conteúdo excede a altura do card.
    fn render_library_card(&mut self, frame: &mut Frame, cols: u16, top: u16, height: u16) {
        if height < 4 {
            self.library_map = None;
            self.library_region = None;
            return;
        }
        // Fundo do Card + acento verde na margem esquerda de todas as linhas.
        frame.fill_rect(top, 0, height, cols, ' ', None, Some(THEME.card_bg));
        for r in top..top.saturating_add(height) {
            frame.put_styled(
                r,
                0,
                ui_bottom::ACCENT_BLOCK,
                Some(THEME.card_accent_library),
                Some(THEME.card_bg),
            );
        }

        // Área de conteúdo: 1 coluna de acento + margem + scrollbar à direita.
        let content_left = 3u16;
        let content_w = (cols as usize).saturating_sub(content_left as usize + 1);

        // Colunas responsivas (US-047): as métricas aparecem só quando sobra
        // espaço; a barra de progresso encolhe (até `MIN_BAR_W`) antes do nome,
        // preservando a legibilidade do nome (`NAME_W_MIN`).
        let show_metrics = content_w >= ROW_FIXED_W + METRICS_FIXED_W + 15;
        let extra_w = if show_metrics { METRICS_FIXED_W } else { 0 };
        // Colunas fixas antes da barra: marcador/ID (ROW_PREFIX_W) + espaço +
        // STATUS (STATE_W) + espaço (+ métricas quando visíveis).
        let prefix_w = ROW_PREFIX_W + 1 + STATE_W + 1 + extra_w;
        let room = content_w.saturating_sub(prefix_w);
        let bar_w = PROGRESS_COL_W
            .min(room.saturating_sub(NAME_W_MIN))
            .max(MIN_BAR_W)
            .min(room.max(1));
        let name_width = room.saturating_sub(bar_w).max(1);
        // Coluna inicial do nome (indentação das linhas de continuação).
        let name_col = prefix_w + bar_w;

        // Título + cabeçalho da tabela.
        frame.put_styled(
            top,
            content_left,
            "Biblioteca de downloads",
            Some(THEME.text),
            Some(THEME.card_bg),
        );
        let header = session_table_header(name_width, show_metrics, bar_w);
        frame.put_styled(
            top + 1,
            content_left,
            &header,
            Some(THEME.muted),
            Some(THEME.card_bg),
        );

        // Bloco físico dos itens: cada entrada gera a linha-base da tabela
        // (com o nome na coluna NOME) e, quando o nome estoura, linhas de
        // continuação indentadas. A barra colorida é desenhada por cima depois.
        let rows_data = self.session_rows();
        let pending = self.pending.lock().unwrap().clone();
        let mut physical: Vec<PhysicalRow> = Vec::new();
        let mut map_rows: Vec<usize> = Vec::new();

        if rows_data.is_empty() && pending.is_empty() {
            physical.push(PhysicalRow {
                item_idx: 0,
                line: "nenhum torrent na fila".into(),
                bar: None,
            });
            map_rows.push(0);
        } else {
            for (idx, row) in rows_data.iter().enumerate() {
                let marker = if idx == self.row_index { "> " } else { "  " };
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
                let bar = progress_bar_width(pct, bar_w);
                let bar_color = progress_color(row.state, row.finished, row.down_speed_mbps);
                let metrics = if show_metrics {
                    Some(row_metrics_text(
                        row.finished,
                        row.down_speed_mbps,
                        row.up_speed_mbps,
                        row.eta_seconds,
                        row.uploaded_bytes,
                        if row.finished {
                            row.total_bytes
                        } else {
                            row.progress_bytes
                        },
                    ))
                } else {
                    None
                };
                let base = session_table_line(
                    marker,
                    idx,
                    &bar,
                    state,
                    &row.name,
                    name_width,
                    metrics.as_deref(),
                );
                physical.push(PhysicalRow {
                    item_idx: idx,
                    line: base,
                    bar: Some((bar, bar_color)),
                });
                map_rows.push(idx);
                for chunk in wrap_text(&row.name, name_width).into_iter().skip(1) {
                    let cont = format!("{}{}", " ".repeat(name_col), chunk);
                    physical.push(PhysicalRow {
                        item_idx: idx,
                        line: cont,
                        bar: None,
                    });
                    map_rows.push(idx);
                }
            }
            // Origens pendentes (adição assíncrona) também aparecem.
            let mut idx = rows_data.len();
            for pt in &pending {
                let marker = if idx == self.row_index { "> " } else { "  " };
                let (state, color) = match &pt.status {
                    PendingStatus::Resolving => ("inicializando".to_string(), THEME.accent),
                    PendingStatus::Error(err) => {
                        // Mensagem truncada para a largura da coluna STATUS.
                        (Tui::truncate(err, STATE_W), THEME.error)
                    }
                    PendingStatus::Done(_) => continue,
                };
                let bar = progress_bar_width(0.0, bar_w);
                let metrics = if show_metrics {
                    Some(empty_metrics_text())
                } else {
                    None
                };
                let base = session_table_line(
                    marker,
                    idx,
                    &bar,
                    &state,
                    &pt.source,
                    name_width,
                    metrics.as_deref(),
                );
                physical.push(PhysicalRow {
                    item_idx: idx,
                    line: base,
                    bar: Some((bar, color)),
                });
                map_rows.push(idx);
                for chunk in wrap_text(&pt.source, name_width).into_iter().skip(1) {
                    let cont = format!("{}{}", " ".repeat(name_col), chunk);
                    physical.push(PhysicalRow {
                        item_idx: idx,
                        line: cont,
                        bar: None,
                    });
                    map_rows.push(idx);
                }
                idx += 1;
            }
        }

        // Janela rolada: título + cabeçalho + borda inferior reservam 3 linhas.
        let rows_for_items = height.saturating_sub(3) as usize;
        let total_lines = physical.len();
        let max_scroll = total_lines.saturating_sub(rows_for_items);
        self.library_scroll = self.library_scroll.min(max_scroll);
        let total_items = rows_data.len() + pending.len();
        let items_top = top + 2;

        for (i, pr) in physical
            .iter()
            .skip(self.library_scroll)
            .take(rows_for_items)
            .enumerate()
        {
            let row = items_top + i as u16;
            let selected = pr.item_idx < total_items && pr.item_idx == self.row_index;
            let bg = if selected {
                THEME.highlight_bg
            } else {
                THEME.card_bg
            };
            if selected {
                frame.fill(row, content_left, content_w as u16, ' ', None, Some(bg));
            }
            frame.put_styled(row, content_left, &pr.line, Some(THEME.text), Some(bg));
            if let Some((bar, color)) = &pr.bar {
                frame.put_styled(
                    row,
                    content_left + ROW_PREFIX_W as u16,
                    bar,
                    Some(*color),
                    Some(bg),
                );
            }
        }

        // Scrollbar vertical à direita quando o conteúdo excede o card.
        if total_lines > rows_for_items {
            let sb_col = cols.saturating_sub(1);
            let track = height - 1;
            let thumb_h = ((track as usize * rows_for_items) / total_lines).max(1) as u16;
            let thumb_offset = (track - thumb_h) as usize * self.library_scroll / max_scroll;
            for i in 0..track {
                let row = top + i;
                let in_thumb =
                    (i as usize) >= thumb_offset && (i as usize) < thumb_offset + thumb_h as usize;
                if in_thumb {
                    frame.put_styled(
                        row,
                        sb_col,
                        "█",
                        Some(THEME.card_accent_library),
                        Some(THEME.card_bg),
                    );
                } else {
                    frame.put_styled(row, sb_col, "░", Some(THEME.muted), Some(THEME.card_bg));
                }
            }
        }

        // Borda inferior do Card (US-042/046): canto `╹` + `▀`.
        let edge_row = top + height - 1;
        frame.put_styled(
            edge_row,
            0,
            ui_bottom::ACCENT_CORNER,
            Some(THEME.card_accent_idle),
            None,
        );
        frame.fill(
            edge_row,
            1,
            cols.saturating_sub(1),
            ui_bottom::EDGE_BLOCK,
            Some(THEME.card_bg),
            None,
        );

        // Mapeamento linha física → item (cliques do mouse) e região de rolagem.
        self.library_map = Some(LibraryMap {
            top: items_top,
            rows: map_rows,
        });
        self.library_region = Some((top, height));
    }

    /// Card do Histórico de downloads (US-046): mesmo estilo do Card do footer,
    /// com a barra de acento esquerda **laranja**. Exibe o grid de downloads
    /// completos dentro do card, com rolagem vertical (mouse) e scrollbar quando
    /// o conteúdo excede a altura — o grid nunca ultrapassa as bordas do card,
    /// independentemente do tamanho do terminal.
    fn render_history_card(&mut self, frame: &mut Frame, cols: u16, top: u16, height: u16) {
        if height < 4 {
            self.history_region = None;
            return;
        }
        // Fundo do Card + acento laranja na margem esquerda de todas as linhas.
        frame.fill_rect(top, 0, height, cols, ' ', None, Some(THEME.card_bg));
        for r in top..top.saturating_add(height) {
            frame.put_styled(
                r,
                0,
                ui_bottom::ACCENT_BLOCK,
                Some(THEME.card_accent_alt),
                Some(THEME.card_bg),
            );
        }

        // Área de conteúdo: reserva 1 coluna à direita para a scrollbar.
        let content_left = 3u16;
        let content_w = (cols as usize).saturating_sub(content_left as usize + 1);
        // Colunas do grid (US-045): DATA CRIAÇÃO | NOME | TAMANHO. A data e o
        // tamanho têm largura fixa; o nome absorve o restante.
        let name_w = content_w.saturating_sub(
            crate::downloads::HISTORY_DATE_W + 1 + crate::downloads::HISTORY_SIZE_W,
        );

        // Título + cabeçalho do grid.
        frame.put_styled(
            top,
            content_left,
            "Histórico de downloads",
            Some(THEME.text),
            Some(THEME.card_bg),
        );
        let header = history_header(name_w);
        frame.put_styled(
            top + 1,
            content_left,
            &header,
            Some(THEME.muted),
            Some(THEME.card_bg),
        );

        // Cache (US-045): evita re-escanear o disco a cada frame.
        let items = self.cached_completed_downloads();
        let item_texts: Vec<String> = items
            .iter()
            .map(|item| Self::truncate(&format_completed_row(item, name_w), content_w))
            .collect();

        // Janela rolada: linhas visíveis = título + cabeçalho + borda inferior.
        let rows_for_items = height.saturating_sub(3) as usize;
        let total = item_texts.len();
        let max_scroll = total.saturating_sub(rows_for_items);
        self.history_scroll = self.history_scroll.min(max_scroll);
        let items_top = top + 2;
        if item_texts.is_empty() {
            frame.put_styled(
                items_top,
                content_left,
                "nenhum download completo",
                Some(THEME.muted),
                Some(THEME.card_bg),
            );
        } else {
            for (i, text) in item_texts
                .iter()
                .skip(self.history_scroll)
                .take(rows_for_items)
                .enumerate()
            {
                frame.put_styled(
                    items_top + i as u16,
                    content_left,
                    text,
                    Some(THEME.text),
                    Some(THEME.card_bg),
                );
            }
        }

        // Scrollbar vertical à direita quando o conteúdo excede o card.
        // `max_scroll >= 1` aqui (pois `total > rows_for_items`).
        if total > rows_for_items {
            let sb_col = cols.saturating_sub(1);
            let track = height - 1;
            let thumb_h = ((track as usize * rows_for_items) / total).max(1) as u16;
            let thumb_offset = (track - thumb_h) as usize * self.history_scroll / max_scroll;
            for i in 0..track {
                let row = top + i;
                let in_thumb =
                    (i as usize) >= thumb_offset && (i as usize) < thumb_offset + thumb_h as usize;
                if in_thumb {
                    frame.put_styled(
                        row,
                        sb_col,
                        "█",
                        Some(THEME.card_accent_alt),
                        Some(THEME.card_bg),
                    );
                } else {
                    frame.put_styled(row, sb_col, "░", Some(THEME.muted), Some(THEME.card_bg));
                }
            }
        }

        // Borda inferior do Card (US-042/046): canto `╹` + `▀`.
        let edge_row = top + height - 1;
        frame.put_styled(
            edge_row,
            0,
            ui_bottom::ACCENT_CORNER,
            Some(THEME.card_accent_idle),
            None,
        );
        frame.fill(
            edge_row,
            1,
            cols.saturating_sub(1),
            ui_bottom::EDGE_BLOCK,
            Some(THEME.card_bg),
            None,
        );

        // Região do card para a rolagem com o mouse.
        self.history_region = Some((top, height));
    }

    /// Lista os downloads completos usando o cache do histórico (US-045),
    /// re-escaneando o disco apenas quando o cache expirou.
    fn cached_completed_downloads(&mut self) -> Vec<crate::downloads::CompletedItem> {
        if let Some((stamp, items)) = &self.history_cache {
            if stamp.elapsed() < HISTORY_REFRESH {
                return items.clone();
            }
        }
        let items = list_completed_downloads(&self.output_folder).unwrap_or_default();
        self.history_cache = Some((Instant::now(), items.clone()));
        items
    }

    /// Desenha a área inferior conforme o estilo (US-041): no `Elevated`,
    /// desenha o Card de entrada + badges + borda inferior; no `Legacy`,
    /// mantém o footer original.
    fn render_bottom(
        &mut self,
        frame: &mut Frame,
        cols: u16,
        rows: u16,
        geometry: &ui_bottom::BottomGeometry,
    ) {
        match self.bottom_style {
            BottomStyle::Elevated => {
                let prompt = home_prompt_label(self.prompt_add_mode);
                let width = cols.saturating_sub(2 * LAYOUT_MARGIN_COLS);
                let text_col = 3 + prompt.chars().count() as u16;
                let max_w = (width as usize).saturating_sub(text_col as usize + 2);
                let focused = self.confirming_delete.is_none();
                let data = ui_bottom::BottomData {
                    prompt,
                    input: if self.input.is_empty() && !self.prompt_add_mode {
                        "Enter a command or / for options"
                    } else {
                        &self.input
                    },
                    cursor_col: prompt_cursor_col(&self.input, self.prompt_cursor, max_w),
                    confirm_label: self
                        .confirming_delete
                        .as_ref()
                        .map(|t| confirm_delete_prompt_label(t.name())),
                    prompt_add_mode: self.prompt_add_mode,
                    menu_open: self.view == View::Menu,
                    notice: self.notice.as_deref(),
                };
                ui_bottom::render_input_card(frame, cols, geometry.input_row, &data, focused);
                ui_bottom::render_badges_row(frame, cols, geometry.badges_row, &data, focused);
                ui_bottom::render_gap_row(
                    frame,
                    cols,
                    geometry.edge_row.saturating_sub(1),
                    focused,
                );
                ui_bottom::render_edge_row(frame, cols, geometry.edge_row);
            }
            BottomStyle::Legacy => {
                self.render_footer(frame, cols, rows);
            }
        }
    }

    /// Prompt fixo da base (US-031): `> ` + texto digitado (ou placeholder) com
    /// cursor ativo, preenchendo a largura do terminal com as margens laterais.
    /// Método da TUI que exige `Arc<Session>` real (rede) → não unit-testável;
    /// excluído da cobertura (ver `coverage_nightly`).
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn render_home_prompt(&mut self, frame: &mut Frame, cols: u16, row: u16) {
        let left = LAYOUT_MARGIN_COLS;
        let width = cols.saturating_sub(2 * LAYOUT_MARGIN_COLS);
        // US-035: com a confirmação de exclusão aberta, o prompt exibe a
        // pergunta com o nome do alvo (sem input e sem cursor).
        if let Some(target) = &self.confirming_delete {
            let label = confirm_delete_prompt_label(target.name());
            let max_w = (width as usize).saturating_sub(4);
            let shown = Self::truncate(&label, max_w);
            frame.fill(row, left, width, ' ', None, Some(THEME.footer_bg));
            frame.put_styled(
                row,
                left,
                &shown,
                Some(THEME.home_prompt_fg),
                Some(THEME.footer_bg),
            );
            return;
        }
        // US-034: no modo "insira o link:", o rótulo muda e o placeholder
        // padrão é suprimido (o texto digitado é a origem a adicionar).
        let prompt = home_prompt_label(self.prompt_add_mode);
        let text = if self.input.is_empty() && !self.prompt_add_mode {
            "Enter a command or / for options".to_string()
        } else {
            self.input.clone()
        };
        let max_w = (width as usize).saturating_sub(4);
        let shown = Self::truncate(&text, max_w);
        frame.fill(row, left, width, ' ', None, Some(THEME.footer_bg));
        frame.put_styled(
            row,
            left,
            prompt,
            Some(THEME.home_prompt_fg),
            Some(THEME.footer_bg),
        );
        let text_col = left + prompt.chars().count() as u16;
        frame.put_styled(
            row,
            text_col,
            &shown,
            Some(THEME.text),
            Some(THEME.footer_bg),
        );
        // Cursor: converte o índice de bytes para coluna de caracteres (evita
        // desalinhamento com acentos/Unicode) e limita ao texto visível.
        let cursor_chars =
            prompt_cursor_col(&self.input, self.prompt_cursor, shown.chars().count());
        let cursor_col = text_col.saturating_add(cursor_chars as u16);
        frame.set_cursor(row, cursor_col);
    }

    /// Header fixo (US-019): título + versão à esquerda, view atual à direita.
    fn render_header(&self, frame: &mut Frame, cols: u16) {
        let version = env!("CARGO_PKG_VERSION");
        let title = format!(" OpenTorrent v{version} ");
        let title_len = title.chars().count() as u16;
        let label = view_label(&self.view);
        let label_text = format!(" {label} ");
        let label_len = label_text.chars().count() as u16;

        // Preenche todo o header com o fundo do tema primeiro; o título e o
        // rótulo da view são desenhados por cima (sem serem sobrescritos).
        frame.fill(0, 0, cols, ' ', None, Some(THEME.header_bg));
        frame.put_styled(0, 0, &title, Some(THEME.text), Some(THEME.header_bg));

        let label_col = cols.saturating_sub(label_len);
        if label_col > title_len {
            frame.put_styled(
                0,
                label_col,
                &label_text,
                Some(THEME.accent),
                Some(THEME.header_bg),
            );
        }
    }

    /// Footer fixo (US-019): atalhos da view à esquerda e notice/status à
    /// direita, com fundo do tema.
    fn render_footer(&self, frame: &mut Frame, cols: u16, rows: u16) {
        let row = rows.saturating_sub(1);
        frame.fill(row, 0, cols, ' ', None, Some(THEME.footer_bg));

        let hints = footer_hints(
            &self.view,
            self.prompt_add_mode,
            self.confirming_delete.is_some(),
            self.context_menu.is_some(),
        );
        frame.put_styled(row, 1, hints, Some(THEME.muted), Some(THEME.footer_bg));

        if let Some(notice) = &self.notice {
            // Erros em vermelho, avisos em amarelo (paleta unificada).
            let color = if notice.starts_with("erro") {
                THEME.error
            } else {
                THEME.warning
            };
            let text = format!(" {notice} ");
            let col = cols.saturating_sub(text.chars().count() as u16);
            frame.put_styled(row, col, &text, Some(color), Some(THEME.footer_bg));
        }
    }

    /// Popup flutuante de comandos (US-031): desenhado imediatamente acima do
    /// prompt da Home (ou acima do Card elevado, US-041), listando os comandos
    /// filtrados com a descrição alinhada à direita. O item selecionado recebe
    /// highlight de fundo (AC-3).
    fn render_menu(&mut self, frame: &mut Frame, cols: u16, body_top: u16, body_height: u16) {
        // Home por baixo (painéis + prompt); o popup sobrepõe-se acima do prompt.
        self.render_home(frame, cols, body_top, body_height);

        // US-041: no estilo Elevated o popup ancora acima do Card de entrada
        // (que fica fora do Body); no Legacy usa a linha do prompt no Body.
        let prompt_row = match self.bottom_style {
            BottomStyle::Elevated => body_top.saturating_add(body_height),
            BottomStyle::Legacy => body_top.saturating_add(body_height).saturating_sub(1),
        };
        let filtered = self.filtered_commands();
        let mut lines: Vec<String> = filtered
            .iter()
            .enumerate()
            .map(|(i, (cmd, desc))| command_item_line(cmd, desc, i == self.menu_index))
            .collect();
        if lines.is_empty() {
            lines.push("nenhum comando encontrado".into());
        }
        let inner_w = (cols as usize).saturating_sub(8);
        for line in &mut lines {
            *line = Self::truncate(line, inner_w);
        }
        let width = lines
            .iter()
            .map(|l| l.chars().count() as u16)
            .max()
            .unwrap_or(0)
            .saturating_add(4)
            .min(cols.saturating_sub(4));
        // Altura mínima de 3 (bordas) e máxima limitada pelo espaço acima do
        // prompt; cada linha de comando cabe (a altura soma as bordas).
        let height = (lines.len() as u16)
            .saturating_add(2)
            .clamp(3, prompt_row.saturating_sub(body_top).max(3));
        let left = LAYOUT_MARGIN_COLS;
        let top = prompt_row.saturating_sub(height);
        let content_left = left + 2;
        let content_w = width.saturating_sub(4) as usize;

        // Fundo sólido do popup (evita vazar o conteúdo dos painéis por trás).
        frame.fill_rect(top, left, height, width, ' ', None, Some(THEME.popup_bg));
        draw_box(frame, left, top, width, height);

        // Desenha cada linha manualmente: a selecionada com highlight de fundo
        // (AC-3) e as demais com o fundo sólido do popup. Como o popup pode ser
        // pequeno (1 item filtrado), não há reserva de linha extra para limite
        // de altura — o próprio `max_lines` cuida da janela visível.
        let max_lines = (height as usize).saturating_sub(2);
        for (idx, line) in lines.iter().take(max_lines).enumerate() {
            let row = top + 1 + idx as u16;
            let text = Self::truncate(line, content_w);
            if idx == self.menu_index {
                // O highlight cobre apenas o interior (não invade a borda direita).
                frame.fill(
                    row,
                    content_left,
                    width - 3,
                    ' ',
                    None,
                    Some(THEME.highlight_bg),
                );
                frame.put_styled(
                    row,
                    content_left,
                    &text,
                    Some(THEME.text),
                    Some(THEME.highlight_bg),
                );
            } else {
                frame.put_styled(
                    row,
                    content_left,
                    &text,
                    Some(THEME.text),
                    Some(THEME.popup_bg),
                );
            }
        }

        // Geometria para cliques do mouse nos itens do popup.
        self.layout = Some(Layout {
            top: top + 1,
            rows_offset: 0,
            row_count: filtered.len(),
            row_stride: 1,
        });
    }
}

/// Linha de um comando do popup flutuante (US-031): comando à esquerda e
/// descrição funcional alinhada à direita. O comando selecionado recebe o
/// marcador `>` (o destaque de fundo é aplicado pelo `render_menu`).
fn command_item_line(cmd: &str, desc: &str, selected: bool) -> String {
    let marker = if selected { "> " } else { "  " };
    format!("{marker}{cmd:<8} {desc}")
}

/// Comandos disponíveis filtrados pelo texto digitado após `/` (US-031): a
/// consulta vazia retorna todos; senão, apenas os comandos que a contêm.
fn filter_commands(input: &str) -> Vec<(String, String)> {
    let query = input.trim_start_matches('/');
    COMMANDS
        .iter()
        .filter(|(cmd, _)| query.is_empty() || cmd.contains(query))
        .map(|(cmd, desc)| (cmd.to_string(), desc.to_string()))
        .collect()
}

/// Rótulo do prompt fixo da Home (US-031/US-034): `> ` normal ou
/// "insira o link: " no modo de adição.
fn home_prompt_label(prompt_add_mode: bool) -> &'static str {
    if prompt_add_mode {
        "insira o link: "
    } else {
        "> "
    }
}

/// Rótulo da pergunta de confirmação de exclusão no prompt da Home (US-035):
/// `excluir '<nome>'? [Y] ou [N] `. O nome é o do alvo (`DeleteTarget::name()`).
fn confirm_delete_prompt_label(name: &str) -> String {
    format!("excluir '{name}'? [Y] ou [N] ")
}

/// Extrai a origem de um `/add <origem>` digitado inline no prompt da Home
/// (US-034). Retorna `None` quando o texto não é um `/add` com conteúdo após o
/// espaço (ex.: `/add` puro, `/add ` vazio, outros comandos).
fn parse_add_command(input: &str) -> Option<String> {
    let source = input.strip_prefix("/add ")?.trim();
    if source.is_empty() {
        None
    } else {
        Some(source.to_string())
    }
}

/// Decide se a filtragem dinâmica abre o popup de comandos dado o texto do
/// prompt da Home (US-031/US-034): abre para qualquer texto iniciado com `/`,
/// exceto o prefixo inline `/add `, cujo conteúdo após o espaço é a origem a
/// adicionar no próprio prompt.
fn should_open_command_menu(input: &str) -> bool {
    input.starts_with('/') && !input.starts_with("/add ")
}

/// Insere um caractere em `s` na posição do cursor (byte index) e retorna a
/// nova posição do cursor.
fn insert_char_at(s: &mut String, pos: usize, c: char) -> usize {
    let before = &s[..pos];
    let after = &s[pos..];
    *s = format!("{before}{c}{after}");
    pos + c.len_utf8()
}

/// Apaga o caractere anterior à posição do cursor (byte index) em `s` e
/// retorna a nova posição do cursor (0 se já estava no início).
fn backspace_char_at(s: &mut String, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let before = &s[..pos];
    if let Some((new_pos, _)) = before.char_indices().next_back() {
        *s = format!("{}{}", &before[..new_pos], &s[pos..]);
        new_pos
    } else {
        0
    }
}

/// Coluna (em caracteres) do cursor do prompt a partir do índice de bytes,
/// limitada ao número de caracteres visíveis (US-031). Converte bytes → chars
/// para não desalinhar com texto multi-byte (acentos, emoji, etc.).
fn prompt_cursor_col(input: &str, cursor_bytes: usize, max_visible: usize) -> usize {
    let bytes = cursor_bytes.min(input.len());
    input[..bytes].chars().count().min(max_visible)
}

/// Barra de progresso em blocos contínuos estilo VCL/GUI (US-020): `█` para o
/// trecho concluído e `░` para o restante, com o percentual exato adjacente
/// (ex.: `████████████░░░░░░░░░░░░░░  50.0%`). Ocupa a largura fixa da coluna
/// da tabela (`PROGRESS_COL_W`).
#[cfg(test)]
fn progress_bar(pct: f64) -> String {
    progress_bar_width(pct, PROGRESS_COL_W)
}

/// Barra de progresso com largura responsiva (US-047): mesma estética da
/// `progress_bar`, mas o chamador define a largura total `bar_w` (blocos +
/// espaço + percentual). O percentual sempre ocupa 6 caracteres
/// (`" 50.0%"`); os blocos absorvem o restante.
fn progress_bar_width(pct: f64, bar_w: usize) -> String {
    let pct = pct.clamp(0.0, 100.0);
    let pct_text = format!("{pct:>5.1}%");
    let block_w = bar_w.saturating_sub(pct_text.len() + 1); // +1 espaço antes
    let filled = ((pct / 100.0) * block_w as f64).round() as usize;
    let filled = filled.min(block_w);
    // `█`/`░` ocupam 3 bytes cada; a capacidade evita realocação por frame.
    let mut bar = String::with_capacity(bar_w * 3);
    bar.push_str(&"█".repeat(filled));
    bar.push_str(&"░".repeat(block_w - filled));
    bar.push(' ');
    bar.push_str(&pct_text);
    bar
}

/// Quebra `text` em linhas de no máximo `max_chars` caracteres, respeitando
/// limites de palavras (espaços). Palavras mais longas que a linha (ex.: URLs
/// de magnet sem espaços) são cortadas no meio. Usado pelo Card da Biblioteca
/// (US-047) para nomes longos que não cabem numa única linha.
fn wrap_text(text: &str, max_chars: usize) -> Vec<String> {
    if max_chars == 0 || text.chars().count() <= max_chars {
        return vec![text.to_string()];
    }
    let mut lines: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut cur_w = 0usize;
    for word in text.split_whitespace() {
        let chars: Vec<char> = word.chars().collect();
        let word_w = chars.len();
        if word_w <= max_chars {
            if !cur.is_empty() && cur_w + 1 + word_w <= max_chars {
                cur.push(' ');
                cur.push_str(word);
                cur_w += 1 + word_w;
            } else if cur.is_empty() {
                cur.push_str(word);
                cur_w = word_w;
            } else {
                lines.push(std::mem::take(&mut cur));
                cur.push_str(word);
                cur_w = word_w;
            }
        } else {
            // Palavra longa (magnet, URL): fecha a linha atual e corta em
            // pedaços, cada um iniciando uma nova linha.
            if !cur.is_empty() {
                lines.push(std::mem::take(&mut cur));
            }
            for chunk in chars.chunks(max_chars) {
                if !cur.is_empty() {
                    lines.push(std::mem::take(&mut cur));
                }
                cur.extend(chunk);
                cur_w = chunk.len();
            }
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Formata uma velocidade em MiB/s (US-033), ex.: `12.4 MiB/s`.
fn format_speed(mbps: f64) -> String {
    format!("{mbps:.1} MiB/s")
}

/// Formata um tempo restante em segundos como `HH:MM:SS` (US-033), ex.: `00:04:12`.
fn format_eta(seconds: u64) -> String {
    let hours = seconds / 3600;
    let minutes = (seconds % 3600) / 60;
    let secs = seconds % 60;
    format!("{hours:02}:{minutes:02}:{secs:02}")
}

/// Formata a razão de share `enviado / baixado` (US-033), ex.: `1.45`. Guarda
/// contra divisão por zero (retorna `0.00` quando nada foi baixado).
fn format_ratio(uploaded: u64, downloaded: u64) -> String {
    if downloaded == 0 {
        return "0.00".to_string();
    }
    format!("{:.2}", uploaded as f64 / downloaded as f64)
}

/// Texto das colunas de métricas de rede de uma linha da sessão (US-033):
/// DOWN SPEED, UP SPEED, ETA e RATIO, com placeholders `—` quando o torrent
/// está pausado/inicializando/erro (sem `live` stats) e `Done` no ETA quando
/// concluído. Função pura sobre as métricas (sem depender do `SessionRow`
/// inteiro) para ser testável isoladamente.
fn row_metrics_text(
    finished: bool,
    down_speed_mbps: Option<f64>,
    up_speed_mbps: Option<f64>,
    eta_seconds: Option<u64>,
    uploaded_bytes: u64,
    downloaded_bytes: u64,
) -> String {
    let (down, up, eta, ratio) = if finished {
        let ratio = format_ratio(uploaded_bytes, downloaded_bytes);
        ("—".into(), "—".into(), "Done".into(), ratio)
    } else {
        let down = down_speed_mbps
            .map(format_speed)
            .unwrap_or_else(|| "—".into());
        let up = up_speed_mbps
            .map(format_speed)
            .unwrap_or_else(|| "—".into());
        let eta = eta_seconds.map(format_eta).unwrap_or_else(|| "—".into());
        let ratio = format_ratio(uploaded_bytes, downloaded_bytes);
        (down, up, eta, ratio)
    };
    format!(" {down:<10} {up:<10} {eta:<8} {ratio:<6}")
}

/// Limite de velocidade de download (MiB/s) que define "downstream baixo" no
/// gauge (US-043): torrent Live abaixo dele fica laranja.
const LOW_DOWNSTREAM_MIBPS: f64 = 1.0;

/// Ordena as linhas do grid da Biblioteca (US-042): torrents em processamento
/// (Live, não concluído) no topo e, dentro de cada grupo, o mais recente
/// inserido (maior `id`) primeiro. Ordenação determinística e pura.
fn sort_rows_for_grid(rows: &mut [SessionRow]) {
    rows.sort_by(|a, b| {
        let a_processing = is_processing_row(a);
        let b_processing = is_processing_row(b);
        b_processing
            .cmp(&a_processing)
            .then_with(|| b.id.cmp(&a.id))
    });
}

/// `true` quando a linha representa um torrent em processamento (Live e ainda
/// não concluído) — recebe prioridade de ordenação no topo do grid (US-042).
fn is_processing_row(row: &SessionRow) -> bool {
    !row.finished && matches!(row.state, TorrentStatsState::Live)
}

/// Cor da barra de progresso conforme o estado do download (US-020) e o
/// downstream atual (US-043): vermelho para erro, laranja para downstream
/// baixo (Live < 1 MiB/s), verde escuro para 100% concluído, verde para Live
/// saudável, amarelo para pausado e ciano para inicializando.
fn progress_color(state: TorrentStatsState, finished: bool, down_speed_mbps: Option<f64>) -> Color {
    if finished {
        THEME.progress_done
    } else {
        match state {
            TorrentStatsState::Live => {
                if down_speed_mbps.unwrap_or(0.0) < LOW_DOWNSTREAM_MIBPS {
                    THEME.progress_low
                } else {
                    THEME.success
                }
            }
            TorrentStatsState::Paused => THEME.warning,
            TorrentStatsState::Error => THEME.error,
            TorrentStatsState::Initializing => THEME.accent,
        }
    }
}

/// Reconstrói `TorrentStatsState` a partir do `Display` serializado no
/// snapshot do daemon (US-040). Valores desconhecidos caem para `Initializing`
/// (renderização conservadora, sem panics).
fn state_from_display(s: &str) -> TorrentStatsState {
    match s {
        "live" => TorrentStatsState::Live,
        "paused" => TorrentStatsState::Paused,
        "error" => TorrentStatsState::Error,
        _ => TorrentStatsState::Initializing,
    }
}

/// Mapeia uma linha da tela para o índice da entrada de um bloco listado
/// (menu, diálogo ou tabela), considerando a altura ocupada por entrada
/// (`stride` linhas; 1 = consecutivas, 2 = com divisor fino). Retorna `None`
/// quando a linha está fora das entradas ou cai exatamente no divisor — um
/// clique aí não altera a seleção.
fn table_row_to_index(row: u16, first_row: u16, row_count: usize, stride: u16) -> Option<usize> {
    if row_count == 0 || stride == 0 {
        return None;
    }
    let last_row =
        first_row.saturating_add((row_count as u16).saturating_sub(1).saturating_mul(stride));
    if row < first_row || row > last_row {
        return None;
    }
    let offset = row - first_row;
    if offset % stride != 0 {
        return None;
    }
    Some((offset / stride) as usize)
}

/// Linha divisória sutil (US-028): `─` repetido pela largura da tabela,
/// separando registros sem gastar linhas em branco. A cor escura
/// (`THEME.muted`) é aplicada pelo chamador na sobreposição.
#[cfg(test)]
fn separator_line(width: usize) -> String {
    "─".repeat(width)
}

fn session_table_header(name_width: usize, show_metrics: bool, bar_w: usize) -> String {
    // Colunas numéricas com rótulos alinhados à direita (sobre os dígitos);
    // colunas de texto alinhadas à esquerda — alinhamento perfeito com os
    // valores das linhas de dados. A largura da coluna PROGRESSO acompanha a
    // barra (`bar_w`) usada nas linhas de dados.
    let mut line = format!("  {:>4} {:<bar_w$} {:<13}", "ID", "PROGRESSO", "STATUS");
    if show_metrics {
        line.push_str(&format!(
            " {:<10} {:<10} {:<8} {:<6}",
            "DOWN SPEED", "UP SPEED", "ETA", "RATIO"
        ));
    }
    line.push_str(&format!(" {:<name_width$}", "NOME"));
    line
}

/// Linha de dados da tabela estruturada da sessão (US-019/US-020). Todas as
/// colunas permanecem perfeitamente alinhadas entre si: marcador + ID, barra
/// de progresso + percentual, status, métricas de rede (opcional) e nome
/// (truncado para `name_width`). `metrics` é opcional (ex.: layout compacto
/// da Biblioteca quando o terminal não tem espaço).
fn session_table_line(
    marker: &str,
    id: usize,
    bar: &str,
    state: &str,
    name: &str,
    name_width: usize,
    metrics: Option<&str>,
) -> String {
    let name = Tui::truncate(name, name_width);
    let mut line = format!("{marker}{id:>4} {bar} {state:<13}");
    if let Some(metrics) = metrics {
        line.push_str(metrics);
    }
    line.push_str(&format!(" {name:<name_width$}"));
    line
}

/// Placeholder das colunas de métricas de rede (US-033) para origens
/// pendentes (ainda em resolução, sem stats): todos os campos em `—`.
fn empty_metrics_text() -> String {
    format!(" {:<10} {:<10} {:<8} {:<6}", "—", "—", "—", "—")
}

/// Mapeia uma tecla única para a ação correspondente na Home (US-037). Retorna
/// `None` quando o prompt não está vazio ou quando os modos `prompt_add_mode`
/// ou `confirming_delete` estão ativos (estados soberanos — a tecla volta a
/// ter o significado normal de edição/cancelamento).
fn home_action_for_key(
    key_code: KeyCode,
    prompt_empty: bool,
    prompt_add_mode: bool,
    confirming_delete: bool,
) -> Option<HomeAction> {
    if !prompt_empty || prompt_add_mode || confirming_delete {
        return None;
    }
    match key_code {
        KeyCode::Char('p') | KeyCode::Char('P') => Some(HomeAction::Pause),
        KeyCode::Char('r') | KeyCode::Char('R') => Some(HomeAction::Resume),
        KeyCode::Char('x') | KeyCode::Char('X') => Some(HomeAction::Remove),
        KeyCode::Delete => Some(HomeAction::Delete),
        _ => None,
    }
}

/// Opções do menu de contexto (US-036): torrents reais ganham as quatro ações
/// (Pausar/Retomar/Parar/Excluir); origens pendentes (sem handle) apenas
/// `Excluir`.
fn context_menu_options(entry_index: usize, session_len: usize) -> Vec<ContextAction> {
    if entry_index < session_len {
        vec![
            ContextAction::Pause,
            ContextAction::Resume,
            ContextAction::Stop,
            ContextAction::Delete,
        ]
    } else {
        vec![ContextAction::Delete]
    }
}

/// Rótulo exibido de cada ação do menu de contexto (US-036).
fn context_action_label(action: ContextAction) -> &'static str {
    match action {
        ContextAction::Pause => "Pausar",
        ContextAction::Resume => "Retomar",
        ContextAction::Stop => "Parar",
        ContextAction::Delete => "Excluir",
    }
}

/// Mapeia uma linha da tela para a opção do menu de contexto (US-036),
/// considerando a geometria do popup (borda superior em `top`, itens logo
/// abaixo). Retorna `None` quando a linha cai na borda ou fora do popup.
fn context_option_at(row: u16, rect: (u16, u16, u16, u16), option_count: usize) -> Option<usize> {
    let (_, top, _, height) = rect;
    if row < top || row >= top + height {
        return None;
    }
    let rel = row.checked_sub(top + 1)? as usize;
    (rel < option_count).then_some(rel)
}

/// Cicla a opção destacada do menu de contexto (US-036) por `delta` (±1),
/// envolvendo nos limites. Listas com 0 ou 1 opção ficam no índice 0.
fn cycle_context_selection(selected: usize, len: usize, delta: isize) -> usize {
    if len <= 1 {
        return 0;
    }
    (selected as isize + delta).rem_euclid(len as isize) as usize
}

/// Linha de uma opção do menu de contexto (US-036): marcador `>` na opção
/// destacada (o fundo é aplicado pelo render com `render_context_menu`).
fn context_item_line(label: &str, selected: bool) -> String {
    let marker = if selected { "> " } else { "  " };
    format!("{marker}{label}")
}

/// Rótulo da view atual, exibido no canto direito do Header (US-019).
fn view_label(view: &View) -> &'static str {
    match view {
        View::Home => "INÍCIO",
        View::Menu => "COMANDOS",
    }
}

/// Atalhos da view atual, exibidos no lado esquerdo do Footer (US-019).
fn footer_hints(
    view: &View,
    prompt_add_mode: bool,
    confirming_delete: bool,
    context_menu_open: bool,
) -> &'static str {
    match view {
        View::Home if context_menu_open => "↑/↓ navegar · Enter executa · Esc fecha",
        View::Home if confirming_delete => "Y exclui · N/Esc cancela",
        View::Home if prompt_add_mode => "digite/cole a origem · Enter adiciona · Esc cancela",
        View::Home => "p pausar · r retomar · x remover · Del excluir · / comandos · ↑/↓ seleciona",
        View::Menu => "↑/↓ navegar · Enter executa · Tab completa · Esc fecha",
    }
}

/// Resolve e adiciona uma origem em segundo plano (US-014). Para magnet links a
/// resolução de metadados via DHT/trackers ocorre aqui, fora da interface.
async fn add_torrent_background(
    session: Option<Arc<Session>>,
    source: String,
) -> anyhow::Result<AddOutcome> {
    let add = AddTorrent::from_cli_argument(&source)?;
    match session {
        // Modo local (sem daemon; testes): adiciona direto na sessão.
        Some(session) => match session.add_torrent(add, None).await? {
            librqbit::AddTorrentResponse::Added(_, _) => Ok(AddOutcome::Added),
            librqbit::AddTorrentResponse::AlreadyManaged(_, _) => Ok(AddOutcome::AlreadyManaged),
            _ => unreachable!("add_torrent sem list_only só retorna Added ou AlreadyManaged"),
        },
        // Modo daemon (US-040): delega ao daemon — dedup decidido por lá.
        None => {
            let mut client = daemon::connect_client().await?;
            match client.request(DaemonRequest::Add { source }).await? {
                DaemonResponse::Ok { .. } => Ok(AddOutcome::Added),
                DaemonResponse::AlreadyManaged => Ok(AddOutcome::AlreadyManaged),
                DaemonResponse::Error { message } => anyhow::bail!(message),
                other => anyhow::bail!("resposta inesperada do daemon: {other:?}"),
            }
        }
    }
}

/// Linhas superior e inferior do quadro (bordas de linha fina, estilo opencode).
fn frame_top_bottom(width: u16) -> (String, String) {
    let inner = "─".repeat(width.saturating_sub(2) as usize);
    (format!("┌{inner}┐"), format!("└{inner}┘"))
}

/// Desenha o quadro de linha fina no buffer usando a cor de borda da paleta
/// unificada (US-019/US-031 — bordas finas estilo opencode).
fn draw_box(frame: &mut Frame, left: u16, top: u16, width: u16, height: u16) {
    let (top_line, bottom_line) = frame_top_bottom(width);
    frame.put_colored(top, left, &top_line, Some(THEME.border));
    for row in top + 1..top + height - 1 {
        frame.put_colored(row, left, "│", Some(THEME.border));
        frame.put_colored(row, left + width - 1, "│", Some(THEME.border));
    }
    frame.put_colored(top + height - 1, left, &bottom_line, Some(THEME.border));
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
/// `session: Some(..)` opera em uma sessão local (modo legado/testes);
/// `session: None` é o modo daemon (US-040), renderizando via snapshot IPC.
pub async fn run_interactive(
    session: Option<Arc<Session>>,
    output_folder: PathBuf,
) -> anyhow::Result<()> {
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

    let mut tui = match session {
        Some(session) => Tui::new(session, output_folder, rx),
        None => Tui::new_daemon(output_folder, rx),
    };
    // Primeiro frame é desenhado imediatamente (sem esperar o orçamento).
    tui.render(&mut stdout)?;

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
                // Canal de entrada encerrado (thread do teclado morreu): sai.
                Ok(None) => return Ok(()),
                Err(_) => None,
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
            // US-040 (modo daemon): atualiza o snapshot da fila antes de
            // renderizar, mantendo o progresso ao vivo mesmo com a TUI aberta.
            if tui.session.is_none() {
                tui.refresh_snapshot().await;
            }
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
    fn home_prompt_fg_contrasts_with_footer_bg() {
        // US-039: o prompt da Home usa cor própria (branca) que difere do fundo
        // do footer para garantir legibilidade da confirmação/entrada.
        assert_ne!(THEME.home_prompt_fg, THEME.footer_bg);
    }

    #[test]
    fn strips_multiple_sequences() {
        let s = "\u{1b}[1;32ma\u{1b}[0m\u{1b}[31mb\u{1b}[0m";
        assert_eq!(strip_ansi(s), "ab");
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
    fn command_item_line_marks_selected_only() {
        let line = command_item_line("/add", "adicionar torrent a lista", true);
        assert!(line.starts_with("> /add"));
        assert!(line.contains("adicionar torrent a lista"));
        let unselected = command_item_line("/exit", "sair do OpenTorrent", false);
        assert!(unselected.starts_with("  /exit"));
        assert!(!unselected.starts_with('>'));
    }

    #[test]
    fn commands_cover_all_shortcuts() {
        assert_eq!(COMMANDS.len(), 6);
        for (cmd, desc) in COMMANDS {
            assert!(cmd.starts_with('/'));
            assert!(!desc.is_empty());
        }
    }

    #[test]
    fn frame_borders_use_thin_lines() {
        let (top, bottom) = frame_top_bottom(78);
        assert_eq!(top.chars().next(), Some('┌'));
        assert_eq!(top.chars().last(), Some('┐'));
        assert_eq!(bottom.chars().next(), Some('└'));
        assert_eq!(bottom.chars().last(), Some('┘'));
        assert_eq!(top.chars().count(), 78);
        assert!(top.chars().all(|c| matches!(c, '┌' | '─' | '┐')));
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
        assert_eq!(f.cell(0, 0).fg, Some(Color::Cyan));
        assert_eq!(f.cell(0, 1).fg, Some(Color::Cyan));
        assert_eq!(f.cell(1, 0).fg, None);
    }

    #[test]
    fn frame_put_colored_uses_given_color() {
        let mut f = Frame::new(10, 2);
        f.put_colored(0, 0, "hi", Some(Color::DarkYellow));
        assert_eq!(f.cell(0, 0).fg, Some(Color::DarkYellow));
        assert_eq!(f.cell(0, 1).fg, Some(Color::DarkYellow));
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
    fn progress_bar_fills_half_with_blocks() {
        let bar = progress_bar(50.0);
        // barra de 24 blocos: metade cheia + metade vazia, percentual adjacente
        assert!(bar.starts_with(&"█".repeat(12)));
        assert!(bar.contains(&"░".repeat(12)));
        assert!(bar.ends_with(" 50.0%"));
        assert_eq!(bar.chars().count(), PROGRESS_COL_W);
    }

    #[test]
    fn progress_bar_full_and_empty() {
        let full = progress_bar(100.0);
        assert!(full.starts_with(&"█".repeat(24)));
        assert!(full.ends_with("100.0%"));
        let empty = progress_bar(0.0);
        assert!(empty.starts_with(&"░".repeat(24)));
        assert!(empty.ends_with("  0.0%"));
    }

    #[test]
    fn progress_bar_clamps_out_of_range() {
        assert_eq!(progress_bar(150.0), progress_bar(100.0));
        assert_eq!(progress_bar(-10.0), progress_bar(0.0));
    }

    #[test]
    fn progress_bar_width_respects_custom_width() {
        let bar = progress_bar_width(50.0, 20);
        assert_eq!(bar.chars().count(), 20);
        assert!(bar.ends_with(" 50.0%"));
        // Largura maior preenche com mais blocos.
        let wide = progress_bar_width(50.0, 40);
        assert_eq!(wide.chars().count(), 40);
    }

    #[test]
    fn progress_bar_width_clamps_out_of_range() {
        assert_eq!(progress_bar_width(150.0, 20), progress_bar_width(100.0, 20));
        assert_eq!(progress_bar_width(-10.0, 20), progress_bar_width(0.0, 20));
    }

    #[test]
    fn wrap_text_splits_on_word_boundaries() {
        let lines = wrap_text("foo bar baz qux", 9);
        assert_eq!(lines, vec!["foo bar", "baz qux"]);
    }

    #[test]
    fn wrap_text_keeps_short_text_in_one_line() {
        assert_eq!(wrap_text("curto", 10), vec!["curto"]);
        assert_eq!(wrap_text("", 10), vec![""]);
    }

    #[test]
    fn wrap_text_cuts_long_words_without_spaces() {
        // Palavra maior que a linha (ex.: magnet) é cortada em pedaços.
        let lines = wrap_text("magnet:?xt=urn:btih:abcd", 6);
        assert!(lines.len() > 1);
        assert!(lines.iter().all(|l| l.chars().count() <= 6));
    }

    #[test]
    fn wrap_text_never_exceeds_max_chars() {
        let text = "um nome bem longo de arquivo com várias palavras no meio";
        let lines = wrap_text(text, 15);
        assert!(lines.iter().all(|l| l.chars().count() <= 15));
        assert_eq!(
            lines.join(" ").replace("  ", " ").chars().count(),
            text.chars().count()
        );
    }

    #[test]
    fn library_map_index_at_maps_screen_row_to_item() {
        let map = LibraryMap {
            top: 4,
            rows: vec![0, 0, 1, 2, 2],
        };
        // Linhas 4..=8 mapeiam; fora do bloco retorna None.
        assert_eq!(map.index_at(4), Some(0));
        assert_eq!(map.index_at(5), Some(0)); // continuação do item 0
        assert_eq!(map.index_at(6), Some(1));
        assert_eq!(map.index_at(8), Some(2));
        assert_eq!(map.index_at(3), None);
        assert_eq!(map.index_at(9), None);
    }

    #[test]
    fn library_map_index_at_empty_rows() {
        let map = LibraryMap {
            top: 0,
            rows: vec![],
        };
        assert_eq!(map.index_at(0), None);
    }

    #[test]
    fn progress_color_matches_state() {
        // Live saudável (≥ 1 MiB/s): verde.
        assert_eq!(
            progress_color(TorrentStatsState::Live, false, Some(2.0)),
            THEME.success
        );
        // Live com downstream baixo (< 1 MiB/s): laranja.
        assert_eq!(
            progress_color(TorrentStatsState::Live, false, Some(0.5)),
            THEME.progress_low
        );
        // Live sem métrica de velocidade (None): assume baixo → laranja.
        assert_eq!(
            progress_color(TorrentStatsState::Live, false, None),
            THEME.progress_low
        );
        assert_eq!(
            progress_color(TorrentStatsState::Paused, false, None),
            THEME.warning
        );
        assert_eq!(
            progress_color(TorrentStatsState::Error, false, None),
            THEME.error
        );
        assert_eq!(
            progress_color(TorrentStatsState::Initializing, false, None),
            THEME.accent
        );
        // Concluído 100%: verde mais escuro (US-043), independente da velocidade.
        assert_eq!(
            progress_color(TorrentStatsState::Live, true, Some(0.0)),
            THEME.progress_done
        );
        assert_eq!(
            progress_color(TorrentStatsState::Live, true, Some(5.0)),
            THEME.progress_done
        );
    }

    #[test]
    fn progress_color_threshold_uses_1_mib_s_boundary() {
        // No limite exato de 1 MiB/s o gauge permanece verde (>= limite).
        assert_eq!(
            progress_color(TorrentStatsState::Live, false, Some(1.0)),
            THEME.success
        );
        // Logo abaixo do limite fica laranja.
        assert_eq!(
            progress_color(TorrentStatsState::Live, false, Some(0.999)),
            THEME.progress_low
        );
    }

    #[test]
    fn sort_rows_for_grid_puts_processing_first_and_recent_first() {
        let row = |id: usize, state: TorrentStatsState, finished: bool| SessionRow {
            id,
            name: format!("torrent #{id}"),
            state,
            progress_bytes: 0,
            total_bytes: 0,
            finished,
            down_speed_mbps: None,
            up_speed_mbps: None,
            eta_seconds: None,
            uploaded_bytes: 0,
            handle: None,
        };
        let mut rows = vec![
            row(1, TorrentStatsState::Paused, false),
            row(2, TorrentStatsState::Live, false),
            row(3, TorrentStatsState::Live, false),
            row(4, TorrentStatsState::Error, false),
            row(5, TorrentStatsState::Live, true),
        ];
        sort_rows_for_grid(&mut rows);
        let ids: Vec<usize> = rows.iter().map(|r| r.id).collect();
        // Processando (Live, não concluído): topo, mais recente primeiro → 3, 2.
        assert_eq!(&ids[0..2], &[3, 2]);
        // Demais estados, também mais recente primeiro → 5 (concluído), 4, 1.
        assert_eq!(&ids[2..], &[5, 4, 1]);
    }

    #[test]
    fn sort_rows_for_grid_is_stable_for_equal_priority() {
        let row = |id: usize| SessionRow {
            id,
            name: "x".into(),
            state: TorrentStatsState::Paused,
            progress_bytes: 0,
            total_bytes: 0,
            finished: false,
            down_speed_mbps: None,
            up_speed_mbps: None,
            eta_seconds: None,
            uploaded_bytes: 0,
            handle: None,
        };
        let mut rows = vec![row(10), row(20)];
        sort_rows_for_grid(&mut rows);
        assert_eq!(rows[0].id, 20);
        assert_eq!(rows[1].id, 10);
    }

    #[test]
    fn session_table_line_aligns_columns() {
        let bar = progress_bar(50.0);
        let line = session_table_line("> ", 1, &bar, "em andamento", "arquivo.iso", 12, None);
        // marcador + ID
        assert_eq!(&line[0..6], ">    1");
        // barra ocupa a coluna de progresso a partir da coluna 7
        let chars: Vec<char> = line.chars().collect();
        let bar_seg: String = chars[7..7 + PROGRESS_COL_W].iter().collect();
        assert_eq!(bar_seg, bar);
        // status alinhado à esquerda em 13 (com o padding)
        let state_col = 7 + PROGRESS_COL_W + 1;
        let state_seg: String = chars[state_col..state_col + 13].iter().collect();
        assert_eq!(state_seg, "em andamento ");
        // nome truncado para name_width
        assert_eq!(line.chars().count(), ROW_FIXED_W + 12);
        assert!(line.contains("arquivo.iso"));
    }

    #[test]
    fn session_table_line_truncates_long_names() {
        let bar = progress_bar(100.0);
        let line = session_table_line("  ", 2, &bar, "concluído", "nome-muito-longo", 8, None);
        assert!(line.contains("nome-mu…"));
        assert_eq!(line.chars().count(), ROW_FIXED_W + 8);
    }

    #[test]
    fn session_table_line_appends_metrics_when_provided() {
        let bar = progress_bar(0.0);
        let metrics = row_metrics_text(false, Some(1.2), Some(0.5), Some(300), 100, 50);
        let line = session_table_line("  ", 3, &bar, "pausado", "x", 10, Some(&metrics));
        assert_eq!(line.chars().count(), ROW_FIXED_W + METRICS_FIXED_W + 10);
        assert!(line.contains("MiB/s"));
    }

    #[test]
    fn format_speed_uses_mib_per_s() {
        assert_eq!(format_speed(0.0), "0.0 MiB/s");
        assert_eq!(format_speed(12.35), "12.3 MiB/s");
        assert_eq!(format_speed(120.0), "120.0 MiB/s");
    }

    #[test]
    fn format_eta_is_hh_mm_ss() {
        assert_eq!(format_eta(0), "00:00:00");
        assert_eq!(format_eta(252), "00:04:12");
        assert_eq!(format_eta(3661), "01:01:01");
    }

    #[test]
    fn format_ratio_guards_division_by_zero() {
        assert_eq!(format_ratio(100, 0), "0.00");
        assert_eq!(format_ratio(0, 100), "0.00");
        assert_eq!(format_ratio(145, 100), "1.45");
        assert_eq!(format_ratio(300, 100), "3.00");
    }

    #[test]
    fn row_metrics_text_fills_columns_for_live_rows() {
        let text = row_metrics_text(false, Some(12.34), Some(2.0), Some(252), 50, 100);
        assert_eq!(text.chars().count(), METRICS_FIXED_W);
        assert!(text.contains("12.3 MiB/s"));
        assert!(text.contains("2.0 MiB/s"));
        assert!(text.contains("00:04:12"));
        assert!(text.contains("0.50"));
    }

    #[test]
    fn row_metrics_text_places_em_dash_without_live_stats() {
        let text = row_metrics_text(false, None, None, None, 0, 100);
        assert_eq!(text.chars().count(), METRICS_FIXED_W);
        // velocidade e eta em —; ratio computado mesmo pausado.
        assert_eq!(text.matches('—').count(), 3);
    }

    #[test]
    fn row_metrics_text_shows_done_eta_when_finished() {
        let text = row_metrics_text(true, Some(1.0), Some(1.0), Some(999), 200, 200);
        assert_eq!(text.chars().count(), METRICS_FIXED_W);
        assert!(text.contains("Done"));
        assert!(text.contains("1.00")); // ratio 200/200
        assert!(!text.contains("00:16:39")); // eta real ignorado
    }

    #[test]
    fn empty_metrics_text_matches_metrics_width() {
        assert_eq!(empty_metrics_text().chars().count(), METRICS_FIXED_W);
    }

    #[test]
    fn session_table_header_with_metrics_matches_data_width() {
        let bar = progress_bar(50.0);
        let header = session_table_header(12, true, PROGRESS_COL_W);
        let metrics = row_metrics_text(false, Some(1.2), Some(0.0), Some(252), 145, 100);
        let data = session_table_line(
            "  ",
            1,
            &bar,
            "em andamento",
            "arquivo.iso",
            12,
            Some(&metrics),
        );
        assert_eq!(header.chars().count(), data.chars().count());
        assert!(header.contains("DOWN SPEED"));
        assert!(header.contains("RATIO"));
        assert!(data.contains("00:04:12"));
    }

    #[test]
    fn home_footer_hints_mention_shortcuts() {
        assert!(footer_hints(&View::Home, false, false, false).contains("p pausar"));
        assert!(footer_hints(&View::Home, false, false, false).contains("r retomar"));
        assert!(footer_hints(&View::Home, false, false, false).contains("x remover"));
        assert!(footer_hints(&View::Home, false, false, false).contains("Del excluir"));
        assert!(footer_hints(&View::Home, true, false, false).contains("Esc cancela"));
        assert!(footer_hints(&View::Home, false, true, false).contains("Y exclui"));
        assert!(footer_hints(&View::Home, false, true, false).contains("N/Esc cancela"));
        // US-036: com o menu de contexto aberto, os atalhos da Home somem.
        assert!(footer_hints(&View::Home, false, false, true).contains("↑/↓ navegar"));
        assert!(footer_hints(&View::Home, false, false, true).contains("Enter executa"));
        assert!(footer_hints(&View::Home, false, false, true).contains("Esc fecha"));
        assert!(!footer_hints(&View::Home, false, false, true).contains("p pausar"));
    }

    #[test]
    fn home_action_for_key_maps_shortcuts_with_empty_prompt() {
        assert_eq!(
            home_action_for_key(KeyCode::Char('p'), true, false, false),
            Some(HomeAction::Pause)
        );
        assert_eq!(
            home_action_for_key(KeyCode::Char('P'), true, false, false),
            Some(HomeAction::Pause)
        );
        assert_eq!(
            home_action_for_key(KeyCode::Char('r'), true, false, false),
            Some(HomeAction::Resume)
        );
        assert_eq!(
            home_action_for_key(KeyCode::Char('R'), true, false, false),
            Some(HomeAction::Resume)
        );
        assert_eq!(
            home_action_for_key(KeyCode::Char('x'), true, false, false),
            Some(HomeAction::Remove)
        );
        assert_eq!(
            home_action_for_key(KeyCode::Char('X'), true, false, false),
            Some(HomeAction::Remove)
        );
        assert_eq!(
            home_action_for_key(KeyCode::Delete, true, false, false),
            Some(HomeAction::Delete)
        );
    }

    #[test]
    fn home_action_for_key_is_inert_with_typed_prompt() {
        // Com texto no prompt, as letras voltam a digitar (e Delete edita).
        assert_eq!(
            home_action_for_key(KeyCode::Char('p'), false, false, false),
            None
        );
        assert_eq!(
            home_action_for_key(KeyCode::Delete, false, false, false),
            None
        );
    }

    #[test]
    fn home_action_for_key_sovereign_states_block_shortcuts() {
        // Modo "insira o link:" (US-034): teclas editam a origem.
        assert_eq!(
            home_action_for_key(KeyCode::Char('p'), true, true, false),
            None
        );
        // Confirmação de exclusão (US-027): apenas S/N/Esc agem.
        assert_eq!(
            home_action_for_key(KeyCode::Delete, true, false, true),
            None
        );
        // Tecla fora do conjunto não gera ação.
        assert_eq!(
            home_action_for_key(KeyCode::Char('a'), true, false, false),
            None
        );
        assert_eq!(
            home_action_for_key(KeyCode::Char('/'), true, false, false),
            None
        );
    }

    #[test]
    fn context_menu_options_list_all_actions_for_torrents() {
        let options = context_menu_options(0, 3);
        assert_eq!(
            options,
            vec![
                ContextAction::Pause,
                ContextAction::Resume,
                ContextAction::Stop,
                ContextAction::Delete,
            ]
        );
        let last_real = context_menu_options(2, 3);
        assert_eq!(last_real.len(), 4);
    }

    #[test]
    fn context_menu_options_for_pending_show_only_delete() {
        // entry_index >= session_len → origem pendente (sem handle).
        let options = context_menu_options(3, 3);
        assert_eq!(options, vec![ContextAction::Delete]);
        let options = context_menu_options(9, 0);
        assert_eq!(options, vec![ContextAction::Delete]);
    }

    #[test]
    fn context_action_labels_match_shortcut_actions() {
        assert_eq!(context_action_label(ContextAction::Pause), "Pausar");
        assert_eq!(context_action_label(ContextAction::Resume), "Retomar");
        assert_eq!(context_action_label(ContextAction::Stop), "Parar");
        assert_eq!(context_action_label(ContextAction::Delete), "Excluir");
    }

    #[test]
    fn context_item_line_marks_selected_only() {
        let selected = context_item_line("Pausar", true);
        assert!(selected.starts_with("> Pausar"));
        let unselected = context_item_line("Excluir", false);
        assert!(unselected.starts_with("  Excluir"));
        assert!(!unselected.starts_with('>'));
    }

    #[test]
    fn context_option_at_maps_rows_inside_popup() {
        // Popup top=5, height=6 (borda + 4 opções + borda).
        let rect = (2, 5, 20, 6);
        assert_eq!(context_option_at(6, rect, 4), Some(0));
        assert_eq!(context_option_at(9, rect, 4), Some(3));
        // Borda superior/inferior e linhas fora do popup: None.
        assert_eq!(context_option_at(5, rect, 4), None);
        assert_eq!(context_option_at(10, rect, 4), None);
        assert_eq!(context_option_at(4, rect, 4), None);
        assert_eq!(context_option_at(30, rect, 4), None);
        // Popup com só Excluir (3 linhas): a borda inferior não é opção.
        let small = (2, 5, 20, 3);
        assert_eq!(context_option_at(6, small, 1), Some(0));
        assert_eq!(context_option_at(7, small, 1), None);
    }

    #[test]
    fn separator_line_matches_table_width() {
        // O divisor fino tem a mesma largura do cabeçalho e da linha de dados
        // (sem métricas) — as colunas permanecem alinhadas (AC-2/AC-3).
        let name_width = 12;
        let header = session_table_header(name_width, false, PROGRESS_COL_W);
        let sep = separator_line(header.chars().count());
        assert!(sep.chars().all(|c| c == '─'));
        assert_eq!(sep.chars().count(), header.chars().count());
        let data = session_table_line("  ", 1, &progress_bar(0.0), "erro", "abc", name_width, None);
        assert_eq!(sep.chars().count(), data.chars().count());
    }

    #[test]
    fn table_row_to_index_maps_with_stride() {
        // stride 2: entradas nas linhas pares do bloco; divisores ímpares ignorados.
        assert_eq!(table_row_to_index(10, 10, 5, 2), Some(0));
        assert_eq!(table_row_to_index(12, 10, 5, 2), Some(1));
        assert_eq!(table_row_to_index(11, 10, 5, 2), None); // gap
        assert_eq!(table_row_to_index(19, 10, 5, 2), None); // além da última
        assert_eq!(table_row_to_index(18, 10, 5, 2), Some(4));
        // stride 1: linhas consecutivas (menus) — comportamento original.
        assert_eq!(table_row_to_index(3, 1, 3, 1), Some(2));
        assert_eq!(table_row_to_index(4, 1, 3, 1), None);
        // contagem vazia
        assert_eq!(table_row_to_index(0, 0, 0, 2), None);
    }

    #[test]
    fn frame_fill_sets_cells_and_colors() {
        let mut f = Frame::new(10, 4);
        f.fill(2, 3, 5, ' ', None, Some(Color::DarkGrey));
        assert_eq!(f.cell(2, 2).bg, None);
        assert_eq!(f.cell(2, 3).bg, Some(Color::DarkGrey));
        assert_eq!(f.cell(2, 7).bg, Some(Color::DarkGrey));
        assert_eq!(f.cell(2, 8).bg, None);
    }

    #[test]
    fn frame_fill_rect_covers_multiple_rows() {
        let mut f = Frame::new(8, 6);
        f.fill_rect(1, 1, 3, 4, 'x', None, Some(Color::DarkGrey));
        for row in 1..4 {
            for col in 1..5 {
                assert_eq!(f.cell(row, col).ch, 'x');
            }
        }
        assert_eq!(f.cell(0, 1).ch, ' ');
        assert_eq!(f.cell(4, 1).ch, ' ');
    }

    // ---- US-031: prompt fixo, painéis e menu flutuante de comandos ----

    #[test]
    fn filtered_commands_matches_by_query() {
        // Consulta vazia (após `/`): todos os comandos.
        assert_eq!(filter_commands("/").len(), COMMANDS.len());
        // Filtro por prefixo/substring do comando.
        let filtered = filter_commands("/de");
        assert!(filtered.iter().any(|(c, _)| c == "/delete"));
        assert!(!filtered.iter().any(|(c, _)| c == "/add"));
        // Sem correspondência: lista vazia.
        assert!(filter_commands("/zzz").is_empty());
        // Texto sem barra também filtra (usado ao digitar no popup).
        assert_eq!(filter_commands("add").len(), 1);
    }

    // ---- US-034: /add com prompt "insira o link:" na Home ----

    #[test]
    fn parse_add_command_extracts_inline_source() {
        let magnet = "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567";
        assert_eq!(
            parse_add_command(&format!("/add {magnet}")),
            Some(magnet.to_string())
        );
    }

    #[test]
    fn parse_add_command_trims_extra_whitespace() {
        assert_eq!(
            parse_add_command("/add   magnet:?xt=urn:btih:abc  "),
            Some("magnet:?xt=urn:btih:abc".to_string())
        );
    }

    #[test]
    fn parse_add_command_returns_none_without_source() {
        // `/add` puro e `/add ` vazio não têm origem.
        assert_eq!(parse_add_command("/add"), None);
        assert_eq!(parse_add_command("/add "), None);
    }

    #[test]
    fn parse_add_command_returns_none_for_other_commands() {
        assert_eq!(parse_add_command("/pause"), None);
        assert_eq!(parse_add_command("/delete"), None);
        assert_eq!(parse_add_command("olá"), None);
    }

    #[test]
    fn should_open_command_menu_opens_for_command_prefixes() {
        assert!(should_open_command_menu("/"));
        assert!(should_open_command_menu("/add"));
        assert!(should_open_command_menu("/pa"));
        assert!(!should_open_command_menu(""));
    }

    #[test]
    fn should_open_command_menu_stays_closed_for_inline_add() {
        // O prefixo "/add " é a origem inline (US-034), não uma busca de comando.
        assert!(!should_open_command_menu("/add "));
        assert!(!should_open_command_menu("/add magnet:?xt=urn:btih:abc"));
    }

    #[test]
    fn home_prompt_label_changes_in_add_mode() {
        assert_eq!(home_prompt_label(false), "> ");
        assert_eq!(home_prompt_label(true), "insira o link: ");
    }

    #[test]
    fn confirm_delete_prompt_label_shows_name_and_yes_no() {
        let label = confirm_delete_prompt_label("arquivo.iso");
        assert!(label.contains("excluir 'arquivo.iso'?"));
        assert!(label.contains("[Y] ou [N]"));
        assert!(label.ends_with(' '));
    }

    #[test]
    fn confirm_delete_prompt_label_handles_pending_source() {
        let label = confirm_delete_prompt_label("magnet:?xt=urn:btih:abc");
        assert!(label.contains("excluir 'magnet:?xt=urn:btih:abc'?"));
    }

    #[test]
    fn home_footer_hints_change_in_add_mode() {
        assert!(footer_hints(&View::Home, false, false, false).contains("/ comandos"));
        assert!(footer_hints(&View::Home, true, false, false).contains("Enter adiciona"));
        assert!(footer_hints(&View::Home, true, false, false).contains("Esc cancela"));
        assert!(footer_hints(&View::Home, false, true, false).contains("Y exclui"));
    }

    #[test]
    fn insert_char_at_respects_cursor_position() {
        let mut s = String::from("abc");
        let pos = insert_char_at(&mut s, 1, 'X');
        assert_eq!(s, "aXbc");
        assert_eq!(pos, 2);
        // No fim e no início também funcionam.
        let len = s.len();
        let new_pos = insert_char_at(&mut s, len, 'Z');
        assert_eq!(new_pos, len + 1);
        assert_eq!(s, "aXbcZ");
    }

    #[test]
    fn backspace_char_at_removes_char_before_cursor() {
        let mut s = String::from("abc");
        let pos = backspace_char_at(&mut s, 3);
        assert_eq!(s, "ab");
        assert_eq!(pos, 2);
        // No início: não apaga nada.
        assert_eq!(backspace_char_at(&mut s, 0), 0);
        assert_eq!(s, "ab");
    }

    #[test]
    fn prompt_cursor_col_converts_bytes_to_chars() {
        // "ação" = 3 caracteres, 5 bytes (2 bytes por acento). Cursor no fim
        // em bytes (6? não — 5) deve resultar em coluna 3.
        assert_eq!(prompt_cursor_col("ação", 5, 10), 3);
        // Cursor no meio, após "aç" (3 bytes, 2 chars).
        assert_eq!(prompt_cursor_col("ação", 3, 10), 2);
        // Limite de visibilidade é respeitado.
        assert_eq!(prompt_cursor_col("ação", 5, 2), 2);
        // Cursor além do fim é clampado.
        assert_eq!(prompt_cursor_col("abc", 99, 10), 3);
        // ASCII puro: coluna == bytes.
        assert_eq!(prompt_cursor_col("abc", 2, 10), 2);
    }

    #[test]
    fn execute_command_notice_for_unknown() {
        // `/help` constrói um notice com todos os comandos (sem session).
        let help: Vec<String> = COMMANDS
            .iter()
            .map(|(cmd, desc)| format!("{cmd} — {desc}"))
            .collect();
        let notice = format!("comandos: {}", help.join(" · "));
        assert!(notice.contains("/add"));
        assert!(notice.contains("/exit"));
    }

    // ---- US-036/US-037: menu de contexto, atalhos e mouse (com Tui real) ----

    /// Cria uma Tui com uma sessão librqbit vazia (offline): sem DHT, sem
    /// UPnP e com porta efêmera (0..1) — nada de rede é usado nos testes.
    async fn test_tui_async() -> (Tui, Arc<Session>, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "opentorrent-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let session = librqbit::Session::new_with_opts(
            dir.clone(),
            librqbit::SessionOptions {
                disable_dht: true,
                disable_dht_persistence: true,
                enable_upnp_port_forwarding: false,
                fastresume: false,
                listen_port_range: Some(0..1),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        let (_tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (Tui::new(session.clone(), dir.clone(), rx), session, dir)
    }

    /// Layout padrão dos testes: bloco no topo 2 com 1 linha de offset e 3
    /// entradas consecutivas (linhas 3..=5).
    fn test_layout() -> Layout {
        Layout {
            top: 2,
            rows_offset: 1,
            row_count: 3,
            row_stride: 1,
        }
    }

    fn right_click(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn left_click(col: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: col,
            row,
            modifiers: KeyModifiers::empty(),
        }
    }

    #[test]
    fn cycle_context_selection_wraps_around_limits() {
        assert_eq!(cycle_context_selection(0, 4, 1), 1);
        assert_eq!(cycle_context_selection(3, 4, 1), 0);
        assert_eq!(cycle_context_selection(0, 4, -1), 3);
        assert_eq!(cycle_context_selection(1, 4, -1), 0);
        // Listas de 0/1 opção ficam no índice 0.
        assert_eq!(cycle_context_selection(0, 1, 1), 0);
        assert_eq!(cycle_context_selection(0, 0, 1), 0);
    }

    #[tokio::test]
    async fn open_context_menu_anchors_popup_on_clicked_row() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.layout = Some(test_layout());
        tui.open_context_menu(3); // primeira entrada (linha 3)
        let cm = tui.context_menu.expect("popup deve abrir");
        assert_eq!(cm.entry_index, 0);
        assert_eq!(cm.anchor_row, 3);
        assert_eq!(cm.selected, 0);
        assert_eq!(tui.row_index, 0);
        assert_eq!(tui.context_rect, None);
    }

    #[tokio::test]
    async fn open_context_menu_ignores_clicks_outside_rows() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        // Sem layout definido o clique é ignorado.
        tui.open_context_menu(3);
        assert!(tui.context_menu.is_none());
        // Acima e abaixo do bloco de entradas (linhas 3..=5).
        tui.layout = Some(test_layout());
        tui.open_context_menu(2);
        assert!(tui.context_menu.is_none());
        tui.open_context_menu(6);
        assert!(tui.context_menu.is_none());
        // Fora do bloco não altera um popup já aberto.
        tui.open_context_menu(3);
        assert!(tui.context_menu.is_some());
        tui.open_context_menu(2);
        assert!(tui.context_menu.is_some());
    }

    #[tokio::test]
    async fn close_context_menu_clears_anchor_and_rect() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.layout = Some(test_layout());
        tui.open_context_menu(3);
        tui.context_rect = Some((2, 3, 13, 3));
        tui.close_context_menu();
        assert!(tui.context_menu.is_none());
        assert!(tui.context_rect.is_none());
    }

    #[tokio::test]
    async fn mouse_home_moves_selection_to_clicked_row() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.layout = Some(test_layout());
        tui.row_index = 0;
        tui.mouse_home(5); // terceira entrada
        assert_eq!(tui.row_index, 2);
        tui.mouse_home(0); // fora do bloco: nada muda
        assert_eq!(tui.row_index, 2);
    }

    #[tokio::test]
    async fn mouse_home_uses_library_map_with_wrapped_names() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        // Item 0 ocupa 2 linhas físicas (nome quebrado); item 1, uma.
        tui.library_map = Some(LibraryMap {
            top: 10,
            rows: vec![0, 0, 1],
        });
        tui.row_index = 0;
        tui.mouse_home(11); // continuação do item 0
        assert_eq!(tui.row_index, 0);
        tui.mouse_home(12); // item 1
        assert_eq!(tui.row_index, 1);
        tui.mouse_home(13); // fora do bloco: nada muda
        assert_eq!(tui.row_index, 1);
    }

    #[tokio::test]
    async fn open_context_menu_uses_library_map_on_clicked_row() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.library_map = Some(LibraryMap {
            top: 2,
            rows: vec![0, 0, 1],
        });
        tui.open_context_menu(3); // continuação do item 0
        let cm = tui.context_menu.expect("popup deve abrir");
        assert_eq!(cm.entry_index, 0);
        assert_eq!(cm.anchor_row, 3);
        assert_eq!(tui.row_index, 0);
        tui.open_context_menu(4); // item 1
        let cm = tui.context_menu.expect("popup deve reabrir");
        assert_eq!(cm.entry_index, 1);
        assert_eq!(tui.row_index, 1);
    }

    #[tokio::test]
    async fn mouse_menu_moves_menu_selection() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.view = View::Menu;
        tui.layout = Some(Layout {
            top: 10,
            rows_offset: 0,
            row_count: 3,
            row_stride: 1,
        });
        tui.menu_index = 0;
        tui.mouse_menu(11);
        assert_eq!(tui.menu_index, 1);
    }

    #[tokio::test]
    async fn request_delete_targets_pending_entry() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.pending.lock().unwrap().push(PendingTorrent {
            source: "magnet:?xt=urn:btih:abc".into(),
            status: PendingStatus::Resolving,
        });
        tui.request_delete(0); // sessão vazia → entrada 0 é o pendente
        match tui.confirming_delete {
            Some(DeleteTarget::Pending { name }) => assert!(name.contains("btih:abc")),
            _ => panic!("esperava DeleteTarget::Pending"),
        }
        assert!(tui.notice.is_none());
    }

    #[tokio::test]
    async fn request_delete_out_of_bounds_is_noop() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.request_delete(5); // sem pendentes e sem torrents
        assert!(tui.confirming_delete.is_none());
    }

    #[tokio::test]
    async fn confirm_delete_discards_pending_entry() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.pending.lock().unwrap().push(PendingTorrent {
            source: "magnet:?xt=urn:btih:xyz".into(),
            status: PendingStatus::Resolving,
        });
        tui.confirming_delete = Some(DeleteTarget::Pending {
            name: "magnet:?xt=urn:btih:xyz".into(),
        });
        tui.confirm_delete().await.unwrap();
        assert!(tui.pending.lock().unwrap().is_empty());
        assert!(tui.confirming_delete.is_none());
        assert!(tui.notice.as_deref().unwrap().contains("excluído"));
    }

    #[tokio::test]
    async fn confirm_delete_without_target_is_noop() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.confirm_delete().await.unwrap();
        assert!(tui.notice.is_none());
    }

    #[tokio::test]
    async fn context_menu_esc_closes_popup() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.layout = Some(test_layout());
        tui.open_context_menu(3);
        tui.handle_context_menu_key(KeyEvent::from(KeyCode::Esc))
            .await
            .unwrap();
        assert!(tui.context_menu.is_none());
    }

    #[tokio::test]
    async fn context_menu_enter_requests_delete_for_pending() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.pending.lock().unwrap().push(PendingTorrent {
            source: "magnet:?xt=urn:btih:abc".into(),
            status: PendingStatus::Resolving,
        });
        tui.layout = Some(test_layout());
        tui.open_context_menu(3);
        // Sessão vazia → única opção é Excluir; Enter a executa.
        tui.handle_context_menu_key(KeyEvent::from(KeyCode::Enter))
            .await
            .unwrap();
        assert!(tui.context_menu.is_none());
        assert!(matches!(
            tui.confirming_delete,
            Some(DeleteTarget::Pending { .. })
        ));
    }

    #[tokio::test]
    async fn context_menu_single_option_navigation_is_noop() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.layout = Some(test_layout());
        tui.open_context_menu(3); // sessão vazia → só Excluir
        tui.handle_context_menu_key(KeyEvent::from(KeyCode::Down))
            .await
            .unwrap();
        tui.handle_context_menu_key(KeyEvent::from(KeyCode::Up))
            .await
            .unwrap();
        assert_eq!(tui.context_menu.unwrap().selected, 0);
    }

    #[tokio::test]
    async fn render_context_menu_records_clickable_rect() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.layout = Some(test_layout());
        tui.open_context_menu(3);
        let mut frame = Frame::new(80, 30);
        tui.render_context_menu(&mut frame, 80, 1, 28);
        // Popup com 1 opção ("  Excluir"): width = 9 + 4 = 13, height = 3,
        // ancorado na linha clicada (3).
        let rect = tui.context_rect.expect("rect deve ser gravado");
        assert_eq!(rect, (2, 3, 13, 3));
    }

    #[tokio::test]
    async fn render_context_menu_without_open_popup_is_noop() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.context_rect = Some((2, 3, 13, 3));
        let mut frame = Frame::new(80, 30);
        tui.render_context_menu(&mut frame, 80, 1, 28);
        assert!(tui.context_menu.is_none());
        assert_eq!(tui.context_rect, Some((2, 3, 13, 3)));
    }

    #[tokio::test]
    async fn mouse_ignores_non_click_events() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.layout = Some(test_layout());
        let move_event = MouseEvent {
            kind: MouseEventKind::Moved,
            column: 5,
            row: 3,
            modifiers: KeyModifiers::empty(),
        };
        tui.handle_mouse(move_event).await.unwrap();
        assert!(tui.context_menu.is_none());
        assert_eq!(tui.row_index, 0);
    }

    #[tokio::test]
    async fn mouse_ignores_clicks_during_delete_confirmation() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.layout = Some(test_layout());
        tui.confirming_delete = Some(DeleteTarget::Pending {
            name: "magnet:?xt=urn:btih:abc".into(),
        });
        tui.handle_mouse(right_click(5, 3)).await.unwrap();
        assert!(tui.context_menu.is_none());
    }

    #[tokio::test]
    async fn mouse_right_click_opens_context_menu() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.layout = Some(test_layout());
        tui.handle_mouse(right_click(5, 3)).await.unwrap();
        let cm = tui.context_menu.expect("clique direito deve abrir o popup");
        assert_eq!(cm.entry_index, 0);
        assert_eq!(tui.row_index, 0);
    }

    #[tokio::test]
    async fn mouse_left_click_outside_context_menu_selects_row() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.layout = Some(test_layout());
        tui.handle_mouse(left_click(5, 5)).await.unwrap();
        assert_eq!(tui.row_index, 2);
        assert!(tui.context_menu.is_none());
    }

    #[tokio::test]
    async fn mouse_left_click_inside_context_menu_executes_option() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.pending.lock().unwrap().push(PendingTorrent {
            source: "magnet:?xt=urn:btih:abc".into(),
            status: PendingStatus::Resolving,
        });
        tui.layout = Some(test_layout());
        tui.open_context_menu(3);
        tui.context_rect = Some((2, 4, 20, 3)); // popup: item na linha 5
        tui.handle_mouse(left_click(3, 5)).await.unwrap();
        // A opção clicada (Excluir) foi executada e o popup fechado.
        assert!(tui.context_menu.is_none());
        assert!(matches!(
            tui.confirming_delete,
            Some(DeleteTarget::Pending { .. })
        ));
    }

    #[tokio::test]
    async fn mouse_left_click_outside_context_menu_closes_it() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.layout = Some(test_layout());
        tui.open_context_menu(3);
        tui.context_rect = Some((2, 4, 20, 3));
        tui.handle_mouse(left_click(60, 8)).await.unwrap(); // fora do popup
        assert!(tui.context_menu.is_none());
    }

    #[tokio::test]
    async fn mouse_left_click_without_rect_closes_context_menu() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.layout = Some(test_layout());
        tui.open_context_menu(3); // context_rect ainda None
        tui.handle_mouse(left_click(5, 5)).await.unwrap();
        assert!(tui.context_menu.is_none());
    }

    #[tokio::test]
    async fn mouse_right_click_in_menu_view_moves_selection() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.view = View::Menu;
        tui.layout = Some(Layout {
            top: 10,
            rows_offset: 0,
            row_count: 3,
            row_stride: 1,
        });
        tui.menu_index = 0;
        tui.handle_mouse(right_click(5, 11)).await.unwrap();
        assert_eq!(tui.menu_index, 1);
    }

    #[tokio::test]
    async fn ctrl_p_opens_command_menu_from_status_bar() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        let key = KeyEvent {
            code: KeyCode::Char('p'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        };
        tui.handle_home_key(key).await.unwrap();
        assert_eq!(tui.view, View::Menu);
        assert!(tui.input.is_empty());
        // Todos os comandos ficam visíveis com o prompt vazio.
        assert_eq!(tui.filtered_commands().len(), COMMANDS.len());
    }

    // ---- US-046: Card do Histórico com acento laranja e rolagem ----

    fn history_items(n: usize) -> Vec<crate::downloads::CompletedItem> {
        (0..n)
            .map(|i| crate::downloads::CompletedItem {
                path: std::path::PathBuf::from(format!("/tmp/f{i}.iso")),
                name: format!("arquivo-{i}.iso"),
                size: 1_000_000 + i as u64,
                modified: std::time::SystemTime::UNIX_EPOCH,
            })
            .collect()
    }

    #[tokio::test]
    async fn history_card_draws_orange_accent_title_and_grid() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.history_cache = Some((std::time::Instant::now(), history_items(10)));
        let mut frame = Frame::new(80, 20);
        tui.render_history_card(&mut frame, 80, 8, 6);
        // Acento laranja na margem esquerda de todas as linhas do card.
        assert_eq!(frame.cell(8, 0).ch(), '▌');
        assert_eq!(frame.cell(8, 0).fg(), Some(THEME.card_accent_alt));
        assert_eq!(frame.cell(12, 0).ch(), '▌');
        // Título na primeira linha do card.
        let title: String = (3..40).map(|c| frame.cell(8, c).ch()).collect();
        assert!(title.starts_with("Histórico de downloads"));
        // Borda inferior `▀` na última linha do card.
        assert_eq!(frame.cell(13, 1).ch(), '▀');
        // 10 itens > 3 linhas visíveis → scrollbar presente (thumb no topo).
        assert_eq!(frame.cell(8, 79).ch(), '█');
        assert_eq!(tui.history_region, Some((8, 6)));
    }

    #[tokio::test]
    async fn history_card_empty_state_shows_message() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.history_cache = Some((std::time::Instant::now(), Vec::new()));
        let mut frame = Frame::new(80, 20);
        tui.render_history_card(&mut frame, 80, 8, 6);
        let row: String = (3..40).map(|c| frame.cell(10, c).ch()).collect();
        assert!(row.contains("nenhum download completo"));
        assert_eq!(frame.cell(8, 0).fg(), Some(THEME.card_accent_alt));
    }

    #[tokio::test]
    async fn mouse_wheel_scrolls_history_card_and_clamps() {
        let (mut tui, _session, _dir) = test_tui_async().await;
        tui.history_cache = Some((std::time::Instant::now(), history_items(20)));
        tui.history_region = Some((8, 6));
        let wheel = |kind: MouseEventKind| MouseEvent {
            kind,
            column: 40,
            row: 10,
            modifiers: KeyModifiers::empty(),
        };
        // Scroll para baixo dentro do card rola para baixo.
        tui.handle_mouse(wheel(MouseEventKind::ScrollDown))
            .await
            .unwrap();
        assert_eq!(tui.history_scroll, 1);
        // Scroll para cima volta a 0 e não fica negativa.
        tui.handle_mouse(wheel(MouseEventKind::ScrollUp))
            .await
            .unwrap();
        assert_eq!(tui.history_scroll, 0);
        tui.handle_mouse(wheel(MouseEventKind::ScrollUp))
            .await
            .unwrap();
        assert_eq!(tui.history_scroll, 0);
        // Fora da região do card não rola.
        tui.handle_mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 40,
            row: 2,
            modifiers: KeyModifiers::empty(),
        })
        .await
        .unwrap();
        assert_eq!(tui.history_scroll, 0);
        // O render clampa o deslocamento ao máximo (20 itens - 3 visíveis).
        tui.history_scroll = 100;
        let mut frame = Frame::new(80, 20);
        tui.render_history_card(&mut frame, 80, 8, 6);
        assert_eq!(tui.history_scroll, 17);
    }
}
