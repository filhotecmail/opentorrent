// US-039/coverage: habilita `#[coverage(off)]` apenas no job de cobertura
// (nightly + cfg `coverage_nightly`). Em builds normais o atributo é removido.
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    io::{IsTerminal, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{Context, bail};
use clap::Parser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, MouseButton, MouseEvent,
        MouseEventKind,
    },
    execute, terminal,
};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ByteBufOwned, ListOnlyResponse,
    ManagedTorrent, Session, SessionOptions, TorrentMetaV1Info, TorrentStatsState,
    api::TorrentIdOrHash,
};
use size_format::SizeFormatterBinary as SF;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

#[cfg(unix)]
mod daemon;
mod downloads;
#[cfg(unix)]
mod ipc;
mod metadata;
mod session_ui;
mod ui_bottom;
mod update;

const MSG_WIDTH: u16 = 40;
const BAR_WIDTH: u16 = 20;
const ACTION_BTN_WIDTH: u16 = 9;
const ACTION_GAP: u16 = ACTION_BTN_WIDTH + 1;
const ACTIONS_COL: u16 = MSG_WIDTH + 1 + 1 + BAR_WIDTH + 1 + 1;

struct MultiProgressWriter(MultiProgress);

impl Write for MultiProgressWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        for line in String::from_utf8_lossy(buf).lines() {
            self.0.println(line).map_err(std::io::Error::other)?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn print_line(multi: &MultiProgress, msg: &str) {
    multi.suspend(|| {
        let _ = writeln!(std::io::stdout(), "{msg}");
    });
}

/// O diretório de saída padrão quando `-o` não é informado (US-038): resolve a
/// pasta pessoal de forma cross-platform e monta o caminho de downloads.
pub(crate) fn default_output_folder() -> anyhow::Result<PathBuf> {
    Ok(default_output_folder_from(&home_dir()?))
}

/// Monta o caminho padrão de downloads a partir da pasta pessoal.
/// Windows: `%USERPROFILE%\Downloads\torrent-downloads`; demais SOs:
/// `$HOME/downloads/torrent-downloads`.
fn default_output_folder_from(home: &Path) -> PathBuf {
    let mut path = home.to_path_buf();
    #[cfg(target_os = "windows")]
    path.push("Downloads");
    #[cfg(not(target_os = "windows"))]
    path.push("downloads");
    path.push("torrent-downloads");
    path
}

/// Resolve a pasta pessoal do usuário de forma cross-platform (US-038): usa
/// `$HOME` no Unix e `%USERPROFILE%` no Windows (fallback automático do crate
/// `dirs`) — a inicialização não aborta se `HOME` não estiver definida.
fn home_dir() -> anyhow::Result<PathBuf> {
    dirs::home_dir().context("could not determine the user home directory")
}

/// Diretório de dados de configuração da aplicação (US-038): `%APPDATA%\opentorrent`
/// no Windows e `~/.config/opentorrent` no Unix (via `dirs::config_dir()`).
/// Unix-only: no Windows é usado apenas pelo daemon, que não existe.
#[cfg(unix)]
pub(crate) fn config_folder() -> anyhow::Result<PathBuf> {
    dirs::config_dir()
        .context("could not determine the user config directory")
        .map(|dir| dir.join("opentorrent"))
}

/// Trackers públicos de fallback aplicados somente no Windows (US-039): o
/// firewall do Windows costuma bloquear o DHT, e um magnet sem `&tr=` que não
/// resolve metadados via DHT fica preso em "inicializando" sem mensagem de
/// erro. Os trackers globais da sessão (SessionOptions::trackers) garantem
/// fontes de peers mesmo com o DHT indisponível.
#[cfg(target_os = "windows")]
const FALLBACK_TRACKERS: &[&str] = &[
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://open.demonii.com:1337/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.openbittorrent.com:6969/announce",
    "http://tracker.openbittorrent.com:80/announce",
    "udp://exodus.desync.com:6969/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://explodie.org:6969/announce",
    "udp://tracker.tiny-vps.com:6969/announce",
    "udp://tracker.cyberia.is:6969/announce",
    "udp://tracker.dler.org:6969/announce",
    "https://tracker.tamersunion.org:443/announce",
];

/// Trackers globais da sessão (US-039): conjunto não vazio no Windows como
/// fallback para o DHT bloqueado pelo firewall, e vazio nas demais plataformas,
/// onde o DHT já resolve magnets sem trackers — o comportamento do Linux fica
/// inalterado.
#[cfg(target_os = "windows")]
fn fallback_trackers() -> HashSet<url::Url> {
    FALLBACK_TRACKERS
        .iter()
        .filter_map(|s| url::Url::parse(s).ok())
        .collect()
}

#[cfg(not(target_os = "windows"))]
fn fallback_trackers() -> HashSet<url::Url> {
    HashSet::new()
}

/// Whether the torrent's files already exist in the given output folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExistingState {
    /// No file exists on disk yet.
    Missing,
    /// At least one file exists, but not all of them are fully downloaded.
    Partial,
    /// All files exist and have their expected size.
    Complete,
}

pub(crate) fn existing_state(
    output_folder: &Path,
    info: &TorrentMetaV1Info<ByteBufOwned>,
    only_files: Option<&[usize]>,
) -> anyhow::Result<ExistingState> {
    let mut any_existing = false;
    let mut all_complete = true;
    for (idx, file) in info.iter_file_details()?.enumerate() {
        if file.attrs().padding {
            continue;
        }
        if only_files.is_some_and(|files| !files.contains(&idx)) {
            continue;
        }
        let path = output_folder.join(file.filename.to_pathbuf()?);
        match std::fs::metadata(&path) {
            Ok(meta) => {
                any_existing = true;
                if meta.len() < file.len {
                    all_complete = false;
                }
            }
            Err(_) => all_complete = false,
        }
    }
    Ok(match (any_existing, all_complete) {
        (false, _) => ExistingState::Missing,
        (true, true) => ExistingState::Complete,
        (true, false) => ExistingState::Partial,
    })
}

/// OpenTorrent: download torrents and magnet links from the terminal.
#[derive(Parser)]
#[command(author, about)]
struct Opts {
    /// The console log level (trace, debug, info, warn, error, off). Defaults to off for a clean interface.
    #[arg(long, short = 'v', default_value = "off")]
    log_level: String,

    /// Print the installed version and check for a newer release (US-029).
    #[arg(long = "version", short = 'V')]
    check_version: bool,

    /// Internal: run as the background daemon (US-040). Hidden flag used by
    /// `opentorrent daemon start` to spawn a fully detached process.
    #[cfg(unix)]
    #[arg(long, hide = true)]
    daemon_headless: bool,

    #[command(subcommand)]
    subcommand: Option<SubCommand>,
}

#[derive(Parser)]
enum SubCommand {
    /// Download one or more torrents (magnet links, .torrent files or http(s) URLs).
    Add(AddOpts),

    /// Manage the background daemon (US-040), which keeps downloads alive
    /// even when the interactive UI is closed.
    #[cfg(unix)]
    Daemon {
        #[command(subcommand)]
        cmd: DaemonOpts,
    },

    /// Update OpenTorrent to the latest GitHub release (US-040): stops the
    /// daemon service, replaces the binary and restarts it.
    Update,
}

#[cfg(unix)]
#[derive(clap::Subcommand)]
enum DaemonOpts {
    /// Install the systemd user service (US-040) if not present (idempotent).
    Install,
    /// Start the daemon in the background if it is not running.
    Start,
    /// Stop the running daemon.
    Stop,
    /// Show whether the daemon is running and where its socket is.
    Status,
}

#[derive(Parser)]
struct AddOpts {
    /// The magnet link, local .torrent file, or http(s) URL to a .torrent file.
    #[arg(required = true, value_name = "TORRENT")]
    torrent: Vec<String>,

    /// The output folder to write to. Defaults to ~/downloads/torrent-downloads/.
    #[arg(short = 'o', long)]
    output_folder: Option<String>,

    /// The sub folder within the output folder to write to.
    #[arg(short = 's', long)]
    sub_folder: Option<String>,

    /// If set, only download files whose name matches this regex.
    #[arg(short = 'r', long = "filename-re")]
    only_files_matching_regex: Option<String>,

    /// Only list the torrent contents, do not download anything.
    #[arg(short = 'l', long)]
    list: bool,

    /// Allow writing on top of existing files.
    #[arg(long)]
    overwrite: bool,

    /// Exit once all torrents finish downloading.
    #[arg(short = 'e', long)]
    exit_on_finish: bool,

    /// A comma-separated list of initial peers (host:port).
    #[arg(long = "initial-peers")]
    initial_peers: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let opts = Opts::parse();

    // US-029: `--version`/`-V` imprime a versão instalada e verifica se há uma
    // release mais recente (com timeout curto e falha graciosa offline).
    if opts.check_version {
        return run_version_check();
    }

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new(format!(
            "librqbit={},opentorrent={}",
            opts.log_level, opts.log_level
        ))
    });

    let multi = MultiProgress::new();
    let log_multi = multi.clone();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(move || MultiProgressWriter(log_multi.clone()))
        .init();

    let mouse_enabled = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    if mouse_enabled {
        let _ = execute!(std::io::stdout(), EnableMouseCapture);
    }

    let runtime = build_runtime()?;

    match runtime.block_on(run(opts, multi)) {
        Ok(()) => {
            disable_mouse(mouse_enabled);
            std::process::exit(0);
        }
        Err(err) => {
            disable_mouse(mouse_enabled);
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}

fn disable_mouse(mouse_enabled: bool) {
    if mouse_enabled {
        let _ = execute!(std::io::stdout(), DisableMouseCapture);
    }
}

/// Constrói o runtime tokio compartilhado (time + io habilitados).
fn build_runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .enable_io()
        .build()
        .context("failed to build tokio runtime")
}

/// URL da API de releases do GitHub usada pela checagem de versão (US-029).
pub(crate) const RELEASES_LATEST_URL: &str =
    "https://api.github.com/repos/filhotecmail/opentorrent/releases/latest";

/// US-029: `--version`/`-V` — imprime a versão instalada e consulta o
/// repositório remoto por uma release mais recente. Falhas de rede/timeout
/// apenas omitem o aviso, nunca geram erros visíveis (falha graciosa).
fn run_version_check() -> anyhow::Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    println!("OpenTorrent v{current}");

    let runtime = build_runtime()?;

    // Falha de rede/timeout/resposta inválida (None): apenas a versão local.
    if let Some(tag) = runtime.block_on(fetch_latest_release()) {
        if update::has_update(&tag, current) {
            println!("Nova versão disponível: {tag} (atual: v{current})");
            println!("Atualize com: {}", update::update_command());
        } else {
            println!("você está na versão mais recente (v{current})");
        }
    }
    Ok(())
}

/// Busca o `tag_name` da última release publicada no GitHub, com timeout curto
/// (máx. 3s). Retorna `None` em qualquer falha (rede, timeout, status, JSON).
pub(crate) async fn fetch_latest_release() -> Option<String> {
    let client = reqwest::Client::new();
    let request = async {
        let resp = client
            .get(RELEASES_LATEST_URL)
            .header("User-Agent", "opentorrent")
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?;
        let json: serde_json::Value = resp.json().await.ok()?;
        json.get("tag_name")?.as_str().map(String::from)
    };
    tokio::time::timeout(Duration::from_secs(3), request)
        .await
        .ok()?
}

/// Despacha o comando para o modo interativo ou para o modo aditivo. Entrypoint
/// de rede/TUI não unit-testável → excluído da cobertura (ver `coverage_nightly`).
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run(opts: Opts, multi: MultiProgress) -> anyhow::Result<()> {
    // US-040: modo internal do daemon headless (spawned por `daemon start`).
    #[cfg(unix)]
    if opts.daemon_headless {
        return daemon::run_headless(&opts).await;
    }

    match opts.subcommand {
        Some(SubCommand::Add(add)) => run_add(add, multi).await,
        #[cfg(unix)]
        Some(SubCommand::Daemon { cmd }) => daemon::run_command(&cmd).await,
        Some(SubCommand::Update) => update::run_update().await,
        // US-009/US-010: without a subcommand, open the interactive session UI.
        None => run_interactive().await,
    }
}

/// Run the interactive session UI (no subcommand). Entrypoint de rede/TUI não
/// unit-testável → excluído da cobertura (ver `coverage_nightly`).
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_interactive() -> anyhow::Result<()> {
    let output_folder = default_output_folder()?;
    std::fs::create_dir_all(&output_folder)
        .with_context(|| format!("error creating output folder {}", output_folder.display()))?;

    // US-040: a sessão vive em um daemon de background. Na primeira execução
    // instala o systemd user service (idempotente, sem systemd → no-op) e
    // inicia o daemon se não estiver rodando; a TUI renderiza via snapshot IPC.
    #[cfg(unix)]
    {
        daemon::install_service()?;
        if !daemon::is_running()? {
            daemon::start()?;
            wait_for_daemon().await?;
        }
        session_ui::run_interactive(None, output_folder).await
    }
    // Windows (sem socket Unix/systemd): a TUI roda em modo local, com a
    // sessão do librqbit no próprio processo (comportamento pré-US-040).
    #[cfg(not(unix))]
    {
        let session_opts = SessionOptions {
            trackers: fallback_trackers(),
            ..Default::default()
        };
        let session = Session::new_with_opts(output_folder.clone(), session_opts)
            .await
            .context("error initializing session")?;
        session_ui::run_interactive(Some(session), output_folder).await
    }
}

/// Aguarda o daemon abrir o socket após o spawn (poll curto, US-040).
#[cfg(unix)]
async fn wait_for_daemon() -> anyhow::Result<()> {
    for _ in 0..50 {
        if daemon::is_running()? {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    bail!("o daemon não abriu o socket a tempo")
}

/// Entrypoint de rede/TUI não unit-testável → excluído da cobertura (ver
/// `coverage_nightly`).
#[cfg_attr(coverage_nightly, coverage(off))]
async fn run_add(opts: AddOpts, multi: MultiProgress) -> anyhow::Result<()> {
    if opts.list {
        print_line(&multi, "listing torrent contents");
    }

    let initial_peers = match opts.initial_peers.as_deref() {
        Some(s) => Some(
            s.split(',')
                .map(|addr| {
                    addr.parse()
                        .with_context(|| format!("invalid address {addr}, expected host:port"))
                })
                .collect::<anyhow::Result<Vec<_>>>()?,
        ),
        None => None,
    };

    let session_opts = SessionOptions {
        // US-039: no Windows, trackers públicos de fallback (DHT bloqueado).
        trackers: fallback_trackers(),
        ..Default::default()
    };

    let output_folder = match opts.output_folder.as_deref() {
        Some(o) => PathBuf::from(o),
        None => default_output_folder()?,
    };
    std::fs::create_dir_all(&output_folder)
        .with_context(|| format!("error creating output folder {}", output_folder.display()))?;
    print_line(
        &multi,
        &format!("output folder: {}", output_folder.display()),
    );
    let session = Session::new_with_opts(output_folder, session_opts)
        .await
        .context("error initializing session")?;

    let (mouse_tx, mouse_rx) = mpsc::unbounded_channel();
    if std::io::stdin().is_terminal() {
        std::thread::spawn(move || mouse_reader(mouse_tx));
    }

    librqbit::librqbit_spawn(
        "stats_printer",
        tracing::trace_span!("stats_printer"),
        stats_printer(session.clone(), multi.clone(), mouse_rx),
    );

    let mut added = false;
    let mut any_handled = false;
    let mut handles = Vec::new();

    for torrent in &opts.torrent {
        print_line(&multi, &format!("processing {torrent}..."));
        let probe_opts = AddTorrentOptions {
            only_files_regex: opts.only_files_matching_regex.clone(),
            list_only: true,
            sub_folder: opts.sub_folder.clone(),
            initial_peers: initial_peers.clone(),
            ..Default::default()
        };

        // Resolve the torrent metainfo (including for magnets, via peers) and
        // the final output folder, so we can check what already exists on disk.
        let (info, only_files, output_folder, torrent_bytes) = match session
            .add_torrent(AddTorrent::from_cli_argument(torrent)?, Some(probe_opts))
            .await
        {
            Ok(AddTorrentResponse::ListOnly(ListOnlyResponse {
                info,
                only_files,
                output_folder,
                torrent_bytes,
                ..
            })) => (info, only_files, output_folder, torrent_bytes),
            Ok(AddTorrentResponse::AlreadyManaged(id, handle)) => {
                print_line(
                    &multi,
                    &format!("already managed (id={id}, hash={:?})", handle.info_hash()),
                );
                any_handled = true;
                continue;
            }
            Ok(_) => unreachable!("list_only can only return ListOnly"),
            Err(err) => {
                print_line(&multi, &format!("failed: {err}"));
                continue;
            }
        };

        if opts.list {
            for (idx, file) in info.iter_file_details()?.enumerate() {
                let included = match &only_files {
                    Some(files) => files.contains(&idx),
                    None => true,
                };
                print_line(
                    &multi,
                    &format!(
                        "[{idx}] {:?} ({}){}",
                        file.filename,
                        SF::new(file.len),
                        if included { "" } else { ", will skip" }
                    ),
                );
            }
            continue;
        }

        // Smart resume: only start a download when the target files are missing
        // or incomplete. Fully downloaded torrents are reported and skipped.
        let existing = existing_state(&output_folder, &info, only_files.as_deref())?;
        let overwrite = match existing {
            ExistingState::Complete if opts.overwrite => true,
            ExistingState::Complete => {
                print_line(
                    &multi,
                    &format!(
                        "torrent already fully downloaded in {}",
                        output_folder.display()
                    ),
                );
                any_handled = true;
                continue;
            }
            ExistingState::Partial => {
                print_line(
                    &multi,
                    &format!(
                        "partial files found in {}, resuming download",
                        output_folder.display()
                    ),
                );
                true
            }
            ExistingState::Missing => opts.overwrite,
        };

        // Re-add using the already-resolved torrent bytes, so magnets are not
        // re-resolved, and pin the same output folder that was inspected above.
        let add_opts = AddTorrentOptions {
            only_files,
            overwrite,
            output_folder: Some(output_folder.to_string_lossy().into_owned()),
            initial_peers: initial_peers.clone(),
            ..Default::default()
        };

        match session
            .add_torrent(AddTorrent::TorrentFileBytes(torrent_bytes), Some(add_opts))
            .await
        {
            Ok(AddTorrentResponse::AlreadyManaged(id, handle)) => {
                print_line(
                    &multi,
                    &format!("already managed (id={id}, hash={:?})", handle.info_hash()),
                );
                any_handled = true;
            }
            Ok(AddTorrentResponse::Added(_, handle)) => {
                print_line(&multi, &format!("added, hash={:?}", handle.info_hash()));
                added = true;
                any_handled = true;
                handles.push(handle);
            }
            Ok(_) => unreachable!("re-add with overwrite can only return Added or AlreadyManaged"),
            Err(err) => {
                print_line(&multi, &format!("failed: {err}"));
            }
        }
    }

    if opts.list {
        return Ok(());
    }

    if !added {
        if any_handled {
            print_line(&multi, "nothing to download, all torrents already handled");
            return Ok(());
        }
        bail!("no torrents were added");
    }

    if opts.exit_on_finish {
        let results = futures::future::join_all(handles.iter().map(|h| h.wait_until_completed()));
        let results = tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                print_line(&multi, "interrupted by Ctrl+C, shutting down");
                return Ok(());
            }
            r = results => r,
        };
        if results.iter().any(|r| r.is_err()) {
            bail!("some downloads failed");
        }
        print_line(&multi, "all downloads completed");
    } else {
        tokio::signal::ctrl_c().await?;
        print_line(&multi, "interrupted by Ctrl+C, shutting down");
    }

    Ok(())
}

fn mouse_reader(tx: mpsc::UnboundedSender<MouseEvent>) {
    loop {
        match event::read() {
            Ok(Event::Mouse(event)) => {
                if tx.send(event).is_err() {
                    break;
                }
            }
            Ok(_) => continue,
            Err(_) => break,
        }
    }
}

fn actions_string(paused: bool) -> String {
    let toggle = if paused { "[Retomar]" } else { "[Pausar ]" };
    format!("{toggle} [Parar  ] [Excluir]")
}

struct TorrentBar {
    handle: Arc<ManagedTorrent>,
    bar: ProgressBar,
    main_file: Option<(usize, u64)>,
}

fn build_bar(multi: &MultiProgress, id: usize, total_bytes: u64, name: &str) -> ProgressBar {
    let bar = multi.add(ProgressBar::new(total_bytes.max(1)));
    bar.set_style(
        ProgressStyle::with_template(&format!(
            "{{msg:<{MSG_WIDTH}!}} [{{bar:{BAR_WIDTH}.cyan/blue}}] {{prefix}}"
        ))
        .unwrap()
        .progress_chars("=>-"),
    );
    bar.set_message(format!("[{id}] {name}"));
    bar.set_prefix(actions_string(false));
    bar
}

async fn handle_click(
    session: &Arc<Session>,
    bars: &Mutex<HashMap<usize, TorrentBar>>,
    bar_order: &Mutex<Vec<usize>>,
    row: u16,
    column: u16,
    term_rows: u16,
) -> anyhow::Result<()> {
    // Read the clicked bar within a short lock scope, then drop the guards
    // before any `.await` (no locks held across await points).
    let (id, handle, is_paused) = {
        let order = bar_order.lock().unwrap();
        let n = order.len() as u16;
        if n == 0 || term_rows == 0 {
            return Ok(());
        }

        // The progress bars are drawn at the bottom of the terminal, one line
        // each, in the order the bars were added (bar_order). The topmost bar
        // starts at row `term_rows - n` (1-based mouse rows), so the clicked
        // bar index is the offset from that row.
        let top_bar_row = term_rows.saturating_sub(n) + 1;
        let row = row.saturating_sub(1) + 1;
        if row < top_bar_row || row > term_rows {
            return Ok(());
        }
        let bar_idx = (row - top_bar_row) as usize;
        let Some(&id) = order.get(bar_idx) else {
            return Ok(());
        };

        let bars_guard = bars.lock().unwrap();
        let Some(tb) = bars_guard.get(&id) else {
            return Ok(());
        };

        let is_paused = matches!(
            tb.handle.stats().state,
            TorrentStatsState::Paused | TorrentStatsState::Error
        );

        (id, tb.handle.clone(), is_paused)
    };

    let col = column.saturating_sub(1) + 1;
    let action_col = ACTIONS_COL + 1;

    if (action_col..action_col + ACTION_BTN_WIDTH).contains(&col) {
        if is_paused {
            session.clone().unpause(&handle).await?;
        } else {
            session.pause(&handle).await?;
        }
    } else if (action_col + ACTION_GAP..action_col + ACTION_GAP + ACTION_BTN_WIDTH).contains(&col) {
        session.delete(TorrentIdOrHash::Id(id), false).await?;
    } else if (action_col + 2 * ACTION_GAP..action_col + 2 * ACTION_GAP + ACTION_BTN_WIDTH)
        .contains(&col)
    {
        session.delete(TorrentIdOrHash::Id(id), true).await?;
    }

    Ok(())
}

async fn stats_printer(
    session: Arc<Session>,
    multi: MultiProgress,
    mut mouse_rx: mpsc::UnboundedReceiver<MouseEvent>,
) -> anyhow::Result<()> {
    let bars: Mutex<HashMap<usize, TorrentBar>> = Mutex::new(HashMap::new());
    let bar_order: Mutex<Vec<usize>> = Mutex::new(Vec::new());

    loop {
        let seen = RefCell::new(Vec::<usize>::new());

        session.with_torrents(|torrents| {
            for (idx, torrent) in torrents {
                seen.borrow_mut().push(idx);
                let stats = torrent.stats();
                let name = torrent.name().unwrap_or_else(|| format!("torrent #{idx}"));

                let mut bars = bars.lock().unwrap();
                let mut bar_order = bar_order.lock().unwrap();
                match bars.entry(idx) {
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        let file_infos = torrent
                            .with_metadata(|m| m.file_infos.clone())
                            .unwrap_or_default();
                        let main_file = file_infos
                            .iter()
                            .enumerate()
                            .max_by_key(|(_, fi)| fi.len)
                            .map(|(i, fi)| (i, fi.len));

                        let bar = build_bar(&multi, idx, stats.total_bytes, &name);
                        entry.insert(TorrentBar {
                            handle: torrent.clone(),
                            bar,
                            main_file,
                        });
                        bar_order.push(idx);
                    }
                    std::collections::hash_map::Entry::Occupied(_) => {}
                }

                let tb = bars.get(&idx).unwrap();
                let is_paused = matches!(
                    stats.state,
                    TorrentStatsState::Paused | TorrentStatsState::Error
                );
                tb.bar.set_prefix(actions_string(is_paused));

                let (pos, len) = match tb.main_file {
                    Some((i, file_len)) => {
                        (stats.file_progress.get(i).copied().unwrap_or(0), file_len)
                    }
                    None => (stats.progress_bytes, stats.total_bytes),
                };
                tb.bar.set_length(len.max(1));
                tb.bar.set_position(pos);
                if stats.finished {
                    tb.bar.finish();
                }
            }
        });

        // Remove bars for torrents that no longer exist (deleted by click).
        {
            let mut bars = bars.lock().unwrap();
            let mut bar_order = bar_order.lock().unwrap();
            bar_order.retain(|id| {
                if seen.borrow().contains(id) {
                    true
                } else {
                    if let Some(tb) = bars.remove(id) {
                        multi.remove(&tb.bar);
                    }
                    false
                }
            });
        }

        // Drain pending mouse events.
        let (_, term_rows) = terminal::size().unwrap_or((0, 0));
        loop {
            match mouse_rx.try_recv() {
                Ok(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    row,
                    column,
                    ..
                }) => {
                    if let Err(err) =
                        handle_click(&session, &bars, &bar_order, row, column, term_rows).await
                    {
                        print_line(&multi, &format!("click error: {err}"));
                    }
                }
                Ok(_) => continue,
                Err(_) => break,
            }
        }

        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_output_folder_uses_platform_downloads_dir() {
        let folder = default_output_folder_from(Path::new("/home/usuario"));
        #[cfg(target_os = "windows")]
        assert_eq!(
            folder,
            Path::new("/home/usuario/Downloads/torrent-downloads")
        );
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            folder,
            Path::new("/home/usuario/downloads/torrent-downloads")
        );
    }

    #[test]
    fn default_output_folder_resolves_cross_platform() {
        // US-038: a resolução não depende da variável `$HOME`; o crate `dirs`
        // usa `%USERPROFILE%` no Windows (fallback automático) e o passwd no
        // Unix quando a variável está ausente.
        let folder = default_output_folder().expect("pasta pessoal resolvida");
        assert!(folder.ends_with("torrent-downloads"));
    }

    #[test]
    #[cfg(unix)]
    fn config_folder_is_opentorrent_under_config_dir() {
        // US-038: %APPDATA%\opentorrent no Windows / ~/.config/opentorrent no Unix.
        let folder = config_folder().expect("diretório de configuração resolvido");
        assert_eq!(folder.file_name().unwrap(), "opentorrent");
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn fallback_trackers_off_on_non_windows() {
        // US-039: a lista de trackers de fallback é aplicada apenas no Windows;
        // nas demais plataformas a sessão mantém o comportamento padrão.
        assert!(fallback_trackers().is_empty());
    }
}
