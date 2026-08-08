use std::{
    cell::RefCell,
    collections::HashMap,
    io::{IsTerminal, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use anyhow::{bail, Context};
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
    api::TorrentIdOrHash, AddTorrent, AddTorrentOptions, AddTorrentResponse, ListOnlyResponse,
    ManagedTorrent, Session, SessionOptions, TorrentStatsState,
};
use size_format::SizeFormatterBinary as SF;
use tokio::sync::mpsc;
use tracing_subscriber::EnvFilter;

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

/// OpenTorrent: download torrents and magnet links from the terminal.
#[derive(Parser)]
#[command(version, author, about)]
struct Opts {
    /// The console log level (trace, debug, info, warn, error, off). Defaults to off for a clean interface.
    #[arg(long, short = 'v', default_value = "off")]
    log_level: String,

    #[command(subcommand)]
    subcommand: SubCommand,
}

#[derive(Parser)]
enum SubCommand {
    /// Download one or more torrents (magnet links, .torrent files or http(s) URLs).
    Add(AddOpts),
}

#[derive(Parser)]
struct AddOpts {
    /// The magnet link, local .torrent file, or http(s) URL to a .torrent file.
    #[arg(required = true, value_name = "TORRENT")]
    torrent: Vec<String>,

    /// The output folder to write to. Defaults to the current folder.
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

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .enable_io()
        .build()
        .context("failed to build tokio runtime")?;

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

async fn run(opts: Opts, multi: MultiProgress) -> anyhow::Result<()> {
    match opts.subcommand {
        SubCommand::Add(add) => run_add(add, multi).await,
    }
}

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

    let session_opts = SessionOptions::default();

    let output_folder = opts
        .output_folder
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_default();
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
    let mut handles = Vec::new();

    for torrent in &opts.torrent {
        print_line(&multi, &format!("processing {torrent}..."));
        let torrent_opts = || AddTorrentOptions {
            only_files_regex: opts.only_files_matching_regex.clone(),
            overwrite: opts.overwrite,
            list_only: opts.list,
            sub_folder: opts.sub_folder.clone(),
            initial_peers: initial_peers.clone(),
            ..Default::default()
        };

        match session
            .add_torrent(
                AddTorrent::from_cli_argument(torrent)?,
                Some(torrent_opts()),
            )
            .await
        {
            Ok(AddTorrentResponse::AlreadyManaged(id, handle)) => {
                print_line(
                    &multi,
                    &format!("already managed (id={id}, hash={:?})", handle.info_hash()),
                );
                continue;
            }
            Ok(AddTorrentResponse::ListOnly(ListOnlyResponse {
                info, only_files, ..
            })) => {
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
            Ok(AddTorrentResponse::Added(_, handle)) => {
                print_line(&multi, &format!("added, hash={:?}", handle.info_hash()));
                added = true;
                handles.push(handle);
            }
            Err(err) => {
                print_line(&multi, &format!("failed: {err}"));
                continue;
            }
        }
    }

    if opts.list {
        return Ok(());
    }

    if !added {
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
