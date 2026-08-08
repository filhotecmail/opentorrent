use std::{
    cell::RefCell, collections::HashMap, io::Write, path::PathBuf, sync::Arc, time::Duration,
};

use anyhow::{bail, Context};
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ListOnlyResponse, Session, SessionOptions,
    TorrentStatsState,
};
use size_format::SizeFormatterBinary as SF;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;

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

/// OpenTorrent: download torrents and magnet links from the terminal.
#[derive(Parser)]
#[command(version, author, about)]
struct Opts {
    /// The console log level (trace, debug, info, warn, error)
    #[arg(long, short = 'v', default_value = "info")]
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

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .enable_io()
        .build()
        .context("failed to build tokio runtime")?;

    match runtime.block_on(run(opts, multi)) {
        Ok(()) => std::process::exit(0),
        Err(err) => {
            error!("{err:#}");
            std::process::exit(1)
        }
    }
}

async fn run(opts: Opts, multi: MultiProgress) -> anyhow::Result<()> {
    match opts.subcommand {
        SubCommand::Add(add) => run_add(add, multi).await,
    }
}

async fn run_add(opts: AddOpts, multi: MultiProgress) -> anyhow::Result<()> {
    if opts.list {
        info!("listing torrent contents");
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

    librqbit::librqbit_spawn(
        "stats_printer",
        tracing::trace_span!("stats_printer"),
        stats_printer(session.clone(), multi),
    );

    let mut added = false;
    let mut handles = Vec::new();

    for torrent in &opts.torrent {
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
                info!(
                    "torrent {torrent:?} already managed, id={id}, hash={:?}",
                    handle.info_hash()
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
                    info!(
                        "file {:?}, size {}{}",
                        file.filename,
                        SF::new(file.len),
                        if included { "" } else { ", will skip" }
                    );
                }
                continue;
            }
            Ok(AddTorrentResponse::Added(_, handle)) => {
                added = true;
                handles.push(handle);
            }
            Err(err) => {
                error!("error adding {torrent:?}: {err:#}");
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
                warn!("received Ctrl+C, shutting down");
                return Ok(());
            }
            r = results => r,
        };
        if results.iter().any(|r| r.is_err()) {
            bail!("some downloads failed");
        }
        info!("all downloads completed");
    } else {
        tokio::signal::ctrl_c().await?;
        warn!("received Ctrl+C, shutting down");
    }

    Ok(())
}

struct TorrentBars {
    status: ProgressBar,
    files: Vec<ProgressBar>,
}

async fn stats_printer(session: Arc<Session>, multi: MultiProgress) -> anyhow::Result<()> {
    let bars: RefCell<HashMap<usize, TorrentBars>> = RefCell::new(HashMap::new());

    loop {
        session.with_torrents(|torrents| {
            for (idx, torrent) in torrents {
                let stats = torrent.stats();
                let name = torrent.name().unwrap_or_else(|| format!("torrent #{idx}"));

                let mut bars = bars.borrow_mut();
                let tb = bars.entry(idx).or_insert_with(|| {
                    let status = multi.add(ProgressBar::new_spinner());
                    status.set_style(
                        ProgressStyle::with_template("{spinner:.green} {msg}")
                            .unwrap()
                            .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈"),
                    );
                    status.enable_steady_tick(Duration::from_millis(100));

                    let file_infos = torrent
                        .with_metadata(|m| m.file_infos.clone())
                        .unwrap_or_default();
                    let files = file_infos
                        .into_iter()
                        .map(|fi| {
                            let bar = multi.add(ProgressBar::new(fi.len));
                            bar.set_style(
                                ProgressStyle::with_template(
                                    "{msg} [{bar:34.cyan/blue}] {pos}/{len} ({percent}%)",
                                )
                                .unwrap()
                                .progress_chars("=>-"),
                            );
                            bar.set_message(format!(
                                "[{}] {}",
                                idx,
                                fi.relative_filename.display()
                            ));
                            bar
                        })
                        .collect::<Vec<_>>();

                    TorrentBars { status, files }
                });

                if let TorrentStatsState::Initializing = stats.state {
                    let total = stats.total_bytes;
                    let progress = stats.progress_bytes;
                    let pct = if total == 0 {
                        0.0
                    } else {
                        (progress as f64 / total as f64) * 100.0
                    };
                    tb.status
                        .set_message(format!("[{idx}] {name}: inicializando {pct:.2}%"));
                    continue;
                }

                let (live, _live_stats) = match (torrent.live(), stats.live.as_ref()) {
                    (Some(live), Some(_live_stats)) => (live, _live_stats),
                    _ => continue,
                };
                let down_speed = live.down_speed_estimator();
                let up_speed = live.up_speed_estimator();
                let total = stats.total_bytes;
                let progress = stats.progress_bytes;
                let downloaded_pct = if stats.finished {
                    100.0
                } else {
                    (progress as f64 / total as f64) * 100.0
                };
                let eta = match down_speed.time_remaining() {
                    Some(d) => format!(", ETA: {d:?}"),
                    None => String::new(),
                };

                if stats.finished {
                    tb.status.set_message(format!(
                        "[{idx}] {name}: concluído — {downloaded_pct:.2}% ({}/{})",
                        SF::new(progress),
                        SF::new(total),
                    ));
                    tb.status.finish();
                } else {
                    tb.status.set_message(format!(
                        "[{idx}] {name}: {downloaded_pct:.2}% ({}/{}) ↓{:.2} MiB/s ↑{:.2} MiB/s{}",
                        SF::new(progress),
                        SF::new(total),
                        down_speed.mbps(),
                        up_speed.mbps(),
                        eta,
                    ));
                }

                for (fi, bar) in tb.files.iter().enumerate() {
                    if let Some(prog) = stats.file_progress.get(fi) {
                        bar.set_position(*prog);
                    }
                    if stats.finished {
                        bar.finish();
                    }
                }
            }
        });
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
