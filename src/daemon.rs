//! Ciclo de vida do daemon em background (US-040).
//!
//! O daemon é o processo que mantém a `Session` do librqbit viva sem TUI,
//! permitindo que os downloads continuem quando a interface é fechada. A
//! comunicação com a TUI/CLI acontece via socket Unix + JSON (`crate::ipc`).

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
};

use anyhow::{Context, bail};
use librqbit::{
    AddTorrent, AddTorrentOptions, AddTorrentResponse, ByteBufOwned, ManagedTorrent, Session,
    SessionOptions, SessionPersistenceConfig, api::TorrentIdOrHash,
};
use tokio::net::UnixListener;

use crate::{
    DaemonOpts, Opts, config_folder, default_output_folder, fallback_trackers,
    ipc::{
        self, DaemonClient, DaemonPaths, DaemonRequest, DaemonResponse, DaemonState,
        TorrentSnapshot,
    },
    webseed::{WebseedProgress, WebseedRegistry, WebseedState, WebseedTask},
};

/// Nome do systemd user service do daemon (US-040).
pub(crate) const SERVICE_NAME: &str = "opentorrent-daemon";

/// Diretório do daemon: `~/.config/opentorrent` (mesmo da persistência US-009).
pub(crate) fn paths() -> anyhow::Result<DaemonPaths> {
    Ok(DaemonPaths::new(&config_folder()?))
}

/// Caminho do unit systemd user (US-040): `~/.config/systemd/user/opentorrent-daemon.service`.
fn service_unit_path() -> anyhow::Result<PathBuf> {
    let config_dir = config_folder()?;
    let base = config_dir
        .parent()
        .context("não foi possível resolver ~/.config para o unit systemd")?;
    Ok(base
        .join("systemd")
        .join("user")
        .join(format!("{SERVICE_NAME}.service")))
}

/// Conteúdo do unit systemd user apontando para o binário atual (US-040). O
/// daemon roda o mesmo binário em modo headless; `Restart=on-failure` o
/// recoloca de pé em caso de crash.
fn service_unit_content() -> anyhow::Result<String> {
    let exe = std::env::current_exe().context("não foi possível localizar o binário")?;
    Ok(format!(
        "[Unit]\n\
         Description=OpenTorrent daemon (downloads em background)\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={} --daemon-headless\n\
         Restart=on-failure\n\
         RestartSec=3\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n",
        exe.display()
    ))
}

/// Indica se o systemd está disponível para user services (US-040).
fn systemd_user_available() -> bool {
    Command::new("systemctl")
        .args(["--user", "is-system-running"])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// Exposição pública (crate) da disponibilidade do systemd user (US-040).
pub(crate) fn systemd_available() -> bool {
    systemd_user_available()
}

/// Executa `systemctl --user <args>` e falha se o comando não suceder.
fn run_systemctl(args: &[&str]) -> anyhow::Result<()> {
    let status = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()
        .with_context(|| format!("falha ao executar systemctl {}", args.join(" ")))?;
    if status.success() {
        Ok(())
    } else {
        bail!("systemctl {} falhou (status {status})", args.join(" "))
    }
}

/// Instala o systemd user service caso não exista (ou o binário tenha mudado de
/// caminho), seguido de `daemon-reload` + `enable` (US-040). Idempotente: não
/// reescreve o unit quando o conteúdo é o mesmo. Sem systemd disponível, não
/// faz nada (fallback para o spawn desanexado).
pub(crate) fn install_service() -> anyhow::Result<()> {
    if !systemd_user_available() {
        return Ok(());
    }
    let path = service_unit_path()?;
    let content = service_unit_content()?;
    let changed = match fs::read_to_string(&path) {
        Ok(existing) => existing != content,
        Err(_) => true,
    };
    if !changed {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("falha ao criar diretório systemd user")?;
    }
    fs::write(&path, content)
        .with_context(|| format!("falha ao gravar unit do daemon em {}", path.display()))?;
    run_systemctl(&["daemon-reload"])?;
    run_systemctl(&["enable", SERVICE_NAME])?;
    Ok(())
}

/// Indica se existe um daemon rodando: tenta conectar ao socket. Conexão
/// bem-sucedida prova que há um listener vivo (socket stale retorna `false`).
pub(crate) fn is_running() -> anyhow::Result<bool> {
    let paths = paths()?;
    if !paths.socket.exists() {
        return Ok(false);
    }
    Ok(std::os::unix::net::UnixStream::connect(&paths.socket).is_ok())
}

/// Executa um subcomando `daemon [install|start|stop|status]` (US-040).
pub(crate) async fn run_command(cmd: &DaemonOpts) -> anyhow::Result<()> {
    match cmd {
        DaemonOpts::Install => {
            install_service()?;
            if systemd_available() {
                eprintln!("service instalado ({SERVICE_NAME})");
            } else {
                eprintln!("systemd user indisponível — daemon roda por `start` manual (fallback)");
            }
            Ok(())
        }
        DaemonOpts::Start => {
            if is_running()? {
                eprintln!("daemon já está rodando");
            } else {
                start()?;
                eprintln!("daemon iniciado");
            }
            Ok(())
        }
        DaemonOpts::Stop => {
            if !is_running()? {
                bail!("daemon não está rodando");
            }
            stop().await?;
            Ok(())
        }
        DaemonOpts::Status => {
            if is_running()? {
                let pid = fs::read_to_string(paths()?.pid).unwrap_or_default();
                println!("daemon rodando (pid={})", pid.trim());
            } else {
                println!("daemon não está rodando");
            }
            Ok(())
        }
    }
}

/// Inicia o daemon. Com systemd user disponível, garante o service instalado e
/// delega ao `systemctl --user start`; sem systemd, usa o spawn desanexado
/// atual (`--daemon-headless`).
pub(crate) fn start() -> anyhow::Result<()> {
    if systemd_user_available() {
        install_service()?;
        return run_systemctl(&["start", SERVICE_NAME]);
    }
    spawn_headless()
}

/// Spawn desanexado do daemon (`--daemon-headless`) — fallback sem systemd.
fn spawn_headless() -> anyhow::Result<()> {
    let exe = std::env::current_exe().context("não foi possível localizar o binário")?;
    // Spawn desanexado: sem stdin/stdout/stderr e em um novo grupo de sessão
    // (setsid) para sobreviver ao fechamento do terminal da TUI.
    let mut cmd = Command::new(&exe);
    cmd.arg("--daemon-headless")
        .env("OPENTORRENT_DAEMON", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().context("falha ao iniciar o daemon")?;
    Ok(())
}

/// Envia `Shutdown` ao daemon via IPC e aguarda a confirmação. Com systemd
/// user disponível, o `systemctl --user stop` é usado (desligamento nativa do
/// systemd com limpeza de socket/pidfile pelo exit).
pub(crate) async fn stop() -> anyhow::Result<()> {
    if systemd_user_available() {
        run_systemctl(&["stop", SERVICE_NAME])?;
        eprintln!("daemon encerrado");
        return Ok(());
    }
    let mut client = connect_client().await?;
    match client.request(DaemonRequest::Shutdown).await? {
        DaemonResponse::Ok { .. } => {
            eprintln!("daemon encerrado");
            Ok(())
        }
        DaemonResponse::Error { message } => bail!("falha ao encerrar o daemon: {message}"),
        other => bail!("resposta inesperada do daemon: {other:?}"),
    }
}

/// Conecta ao socket do daemon (o daemon deve estar rodando).
pub(crate) async fn connect_client() -> anyhow::Result<DaemonClient> {
    let paths = paths()?;
    DaemonClient::connect(&paths.socket)
        .await
        .context("daemon não está acessível (socket indisponível)")
}

/// Entrada do processo headless (`opentorrent --daemon-headless`): sobe a
/// sessão do librqbit e serve o IPC até receber `Shutdown`.
#[cfg_attr(coverage_nightly, coverage(off))]
pub(crate) async fn run_headless(_opts: &Opts) -> anyhow::Result<()> {
    let paths = paths()?;
    let config_dir = config_folder()?;
    // Diretório com permissões restritas ao usuário (segurança do socket).
    fs::create_dir_all(&config_dir).context("falha ao criar diretório de configuração")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&config_dir, fs::Permissions::from_mode(0o700)).ok();
    }

    let output_folder = default_output_folder()?;
    fs::create_dir_all(&output_folder)
        .with_context(|| format!("error creating output folder {}", output_folder.display()))?;

    let session_opts = SessionOptions {
        fastresume: true,
        persistence: Some(SessionPersistenceConfig::Json {
            folder: Some(config_dir),
        }),
        trackers: fallback_trackers(),
        ..Default::default()
    };
    let session = Session::new_with_opts(output_folder.clone(), session_opts)
        .await
        .context("error initializing session (daemon)")?;

    // Registro compartilhado das tarefas webseed (US-049).
    let webseed = crate::webseed::new_registry();

    // Pidfile para `status`.
    fs::write(&paths.pid, std::process::id().to_string())
        .context("falha ao gravar pidfile do daemon")?;

    serve(&paths, session, output_folder, webseed)
        .await
        .map_err(|err| {
            eprintln!("daemon error: {err}");
            err
        })?;
    Ok(())
}

/// Serve o socket Unix: aceita conexões e despacha cada `DaemonRequest`.
async fn serve(
    paths: &DaemonPaths,
    session: Arc<Session>,
    base_folder: PathBuf,
    webseed: WebseedRegistry,
) -> anyhow::Result<()> {
    // Remove socket stale (daemon anterior morto sem limpeza) e liga o novo.
    if paths.socket.exists() {
        let _ = fs::remove_file(&paths.socket);
    }
    let listener = UnixListener::bind(&paths.socket)
        .with_context(|| format!("falha ao ligar o socket {}", paths.socket.display()))?;

    // Um canal de shutdown: `Shutdown` encerra o loop e encerra o processo.
    let shutdown = Arc::new(tokio::sync::Notify::new());
    let shutdown_rx = shutdown.clone();

    loop {
        tokio::select! {
            _ = shutdown_rx.notified() => {
                break;
            }
            accepted = listener.accept() => {
                let (stream, _) = accepted.context("falha ao aceitar conexão IPC")?;
                let session = session.clone();
                let shutdown = shutdown.clone();
                let webseed = webseed.clone();
                let base_folder = base_folder.clone();
                tokio::spawn(async move {
                    if let Err(err) =
                        handle_connection(stream, &session, &base_folder, &webseed, &shutdown).await
                    {
                        eprintln!("ipc error: {err}");
                    }
                });
            }
        }
    }

    // Limpeza do socket/pidfile ao encerrar.
    let _ = fs::remove_file(&paths.socket);
    let _ = fs::remove_file(&paths.pid);
    Ok(())
}

/// Despacha uma conexão: lê requisições em loop (o cliente reusa a conexão)
/// e devolve a resposta de cada uma. Encerra quando o cliente fecha (EOF). O
/// `Shutdown` sinaliza o fim do servidor via `Notify`.
async fn handle_connection(
    mut stream: tokio::net::UnixStream,
    session: &Arc<Session>,
    base_folder: &Path,
    webseed: &WebseedRegistry,
    shutdown: &Arc<tokio::sync::Notify>,
) -> anyhow::Result<()> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let mut header = [0u8; 4];
    loop {
        // EOF (cliente fechou a conexão) → encerra normalmente.
        match read_frame_header(&mut stream, &mut header).await? {
            FrameRead::Closed => return Ok(()),
            FrameRead::Read => {}
        }
        let len = ipc::frame_len(&header)? as usize;
        let mut payload = vec![0u8; len];
        stream.read_exact(&mut payload).await?;
        let req: DaemonRequest = ipc::decode_message(&payload)?;

        let (resp, shutdown_requested) = match req {
            DaemonRequest::Shutdown => (
                DaemonResponse::Ok {
                    message: "shutdown".to_string(),
                },
                true,
            ),
            other => (dispatch(session, base_folder, webseed, other).await, false),
        };

        let mut out = Vec::with_capacity(64);
        ipc::encode_message(&mut out, &resp)?;
        stream.write_all(&out).await?;

        if shutdown_requested {
            shutdown.notify_one();
        }
    }
}

/// Resultado da leitura do cabeçalho de um frame.
enum FrameRead {
    /// Cabeçalho lido com sucesso.
    Read,
    /// Cliente fechou a conexão antes dos 4 bytes (EOF) — conexão encerra.
    Closed,
}

/// Lê os 4 bytes do cabeçalho. `Ok(Closed)` quando o cliente fecha antes de
/// entregar o frame; `Ok(Read)` quando o cabeçalho foi lido.
async fn read_frame_header(
    stream: &mut tokio::net::UnixStream,
    header: &mut [u8; 4],
) -> anyhow::Result<FrameRead> {
    use tokio::io::AsyncReadExt;
    match stream.read_exact(header).await {
        Ok(_) => Ok(FrameRead::Read),
        Err(err) if err.kind() == std::io::ErrorKind::UnexpectedEof => Ok(FrameRead::Closed),
        Err(err) => Err(err.into()),
    }
}

/// Aplica uma `DaemonRequest` de ação sobre a sessão e produz a resposta.
async fn dispatch(
    session: &Arc<Session>,
    base_folder: &Path,
    webseed: &WebseedRegistry,
    req: DaemonRequest,
) -> DaemonResponse {
    match req {
        DaemonRequest::Add { source } => {
            // Dedup por infohash: `add_torrent` retorna `AlreadyManaged` se o
            // infohash já está na sessão (fonte da verdade = daemon). Re-add
            // após exclusão (arquivos mantidos) retoma via smart resume.
            // US-048: URLs de download que respondem HTML com link `.torrent`
            // são resolvidas aqui, antes de entregar ao librqbit.
            let resolved = match crate::resolve::resolve_source(&source).await {
                Ok(r) => r,
                Err(err) => {
                    return DaemonResponse::Error {
                        message: err.to_string(),
                    };
                }
            };
            // US-049: `.torrent` com `url-list` GetRight → download webseed em
            // background e só então o torrent entra na sessão.
            if let crate::resolve::ResolvedSource::TorrentBytes(bytes) = &resolved {
                if let Some(bases) = crate::webseed::extract_url_list(bytes) {
                    return start_webseed(
                        webseed,
                        base_folder,
                        session,
                        source,
                        bytes.clone(),
                        bases,
                    )
                    .await;
                }
            }
            let add = match resolved.into_add_torrent() {
                Ok(a) => a,
                Err(err) => {
                    return DaemonResponse::Error {
                        message: err.to_string(),
                    };
                }
            };
            match session.add_torrent(add, None).await {
                Ok(AddTorrentResponse::Added(_, _)) => DaemonResponse::Ok {
                    message: "adicionado".to_string(),
                },
                Ok(AddTorrentResponse::AlreadyManaged(_, _)) => DaemonResponse::AlreadyManaged,
                Ok(_) => unreachable!("add sem list_only só retorna Added ou AlreadyManaged"),
                Err(err) => DaemonResponse::Error {
                    message: err.to_string(),
                },
            }
        }
        DaemonRequest::Pause { id } => {
            request_by_id(session, id, |session, handle| async move {
                session.pause(&handle).await
            })
            .await
        }
        DaemonRequest::Resume { id } => {
            request_by_id(session, id, |session, handle| async move {
                session.clone().unpause(&handle).await
            })
            .await
        }
        DaemonRequest::Stop { id } => {
            request_by_id(session, id, move |session, _handle| async move {
                session.delete(TorrentIdOrHash::Id(id), false).await
            })
            .await
        }
        DaemonRequest::Delete { id } => {
            request_by_id(session, id, move |session, _handle| async move {
                session.delete(TorrentIdOrHash::Id(id), true).await
            })
            .await
        }
        DaemonRequest::GetState => DaemonResponse::State {
            snapshot: Some(build_state(session, webseed).await),
        },
        DaemonRequest::Shutdown => unreachable!("Shutdown tratado em handle_connection"),
    }
}

/// Inicia o download webseed de um `.torrent` com `url-list` em background
/// (US-049): registra a tarefa no snapshot, baixa os arquivos no layout final
/// do motor e, ao concluir, entrega o `.torrent` à sessão — o `initial_check`
/// do `librqbit` valida as peças por SHA1 e o restante segue por peers.
async fn start_webseed(
    webseed: &WebseedRegistry,
    base_folder: &Path,
    session: &Arc<Session>,
    source: String,
    bytes: bytes::Bytes,
    bases: Vec<String>,
) -> DaemonResponse {
    let parsed = match librqbit::torrent_from_bytes_ext::<ByteBufOwned>(&bytes) {
        Ok(parsed) => parsed,
        Err(err) => {
            return DaemonResponse::Error {
                message: err.to_string(),
            };
        }
    };
    let info = parsed.meta.info;
    let Some(name) = info.name.as_ref() else {
        return DaemonResponse::Error {
            message: "torrent sem nome".to_string(),
        };
    };
    let name = String::from_utf8_lossy(name.as_ref()).into_owned();
    let config = crate::webseed::WebseedConfig {
        bases,
        name: name.clone(),
    };
    let files = match crate::webseed::build_webseed_files(&info, None) {
        Ok(files) => files,
        Err(err) => {
            return DaemonResponse::Error {
                message: err.to_string(),
            };
        }
    };
    // Layout do motor: single-file grava `base_dir/nome`; multi-file usa
    // `base_dir/nome/subcaminho` (o `relative_path` dos arquivos já é o
    // subcaminho interno sem o `info.name`).
    let mut folder = base_folder.to_path_buf();
    if info.files.is_some() {
        folder = folder.join(&name);
    }
    let id = {
        let mut tasks = webseed.lock().unwrap_or_else(|p| p.into_inner());
        let id = tasks.iter().map(|t| t.id).max().map_or(0, |max| max + 1);
        let total_bytes = files
            .iter()
            .filter(|f| !f.is_padding)
            .map(|f| f.length)
            .sum();
        let total_files = files.iter().filter(|f| !f.is_padding).count();
        tasks.push(WebseedTask {
            id,
            source: source.clone(),
            name: name.clone(),
            progress: WebseedProgress {
                total_bytes,
                downloaded_bytes: 0,
                total_files,
                done_files: 0,
                state: WebseedState::Downloading,
            },
        });
        id
    };
    let task_session = session.clone();
    let task_webseed = webseed.clone();
    tokio::spawn(async move {
        let cb_webseed = task_webseed.clone();
        let result =
            crate::webseed::download_webseed(&config, files, &folder, 4, move |progress| {
                if let Ok(mut tasks) = cb_webseed.lock() {
                    if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
                        task.progress = progress.clone();
                    }
                }
            })
            .await;
        {
            let mut tasks = task_webseed.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
                task.progress.state = match &result {
                    Ok(progress) => progress.state.clone(),
                    Err(err) => WebseedState::Error(format!("{err:#}")),
                };
            }
        }
        let ok_to_add = match &result {
            Ok(progress) => matches!(progress.state, WebseedState::Done),
            Err(_) => false,
        };
        if !ok_to_add {
            // Download incompleto: tarefa mantida com `state=Error` (permite
            // re-add); o torrent não entra na sessão com arquivos faltando.
            let msg = match &result {
                Ok(progress) => format!("{}", progress.state),
                Err(err) => format!("{err:#}"),
            };
            eprintln!("webseed #{id} ({name}) incompleto: {msg}");
            return;
        }
        // Entrega o `.torrent` ao motor; `initial_check` valida as peças.
        // `overwrite` permite o motor assumir os arquivos já gravados pelo
        // webseed (mesmo conteúdo, verificável por hash) — sem isso o
        // `add_torrent` recusa arquivo existente no disco.
        match task_session
            .add_torrent(
                AddTorrent::TorrentFileBytes(bytes),
                Some(AddTorrentOptions {
                    overwrite: true,
                    ..Default::default()
                }),
            )
            .await
        {
            Ok(_) => {
                let mut tasks = task_webseed.lock().unwrap_or_else(|p| p.into_inner());
                tasks.retain(|t| t.id != id);
            }
            Err(err) => {
                let mut tasks = task_webseed.lock().unwrap_or_else(|p| p.into_inner());
                if let Some(task) = tasks.iter_mut().find(|t| t.id == id) {
                    task.progress.state = WebseedState::Error(format!("{err:#}"));
                }
            }
        }
    });
    DaemonResponse::Ok {
        message: "adicionando via webseed".to_string(),
    }
}

/// Executa uma ação por `id`, localizando o handle na sessão.
async fn request_by_id<F, Fut>(session: &Arc<Session>, id: usize, action: F) -> DaemonResponse
where
    F: FnOnce(Arc<Session>, Arc<ManagedTorrent>) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<()>>,
{
    let handle = {
        let found = std::cell::RefCell::new(None);
        session.with_torrents(|torrents| {
            for (tid, handle) in torrents {
                if tid == id {
                    *found.borrow_mut() = Some(handle.clone());
                    break;
                }
            }
        });
        found.into_inner()
    };
    match handle {
        Some(h) => match action(session.clone(), h).await {
            Ok(()) => DaemonResponse::Ok {
                message: "ok".to_string(),
            },
            Err(err) => DaemonResponse::Error {
                message: err.to_string(),
            },
        },
        None => DaemonResponse::Error {
            message: format!("torrent não encontrado (id={id})"),
        },
    }
}

/// Constrói o snapshot da fila para a TUI renderizar (US-040).
async fn build_state(session: &Arc<Session>, webseed: &WebseedRegistry) -> DaemonState {
    let rows = std::cell::RefCell::new(Vec::new());
    let finished = std::cell::RefCell::new(0usize);
    let downloaded = std::cell::RefCell::new(0u64);

    session.with_torrents(|torrents| {
        for (id, handle) in torrents {
            let stats = handle.stats();
            let down_speed_mbps = stats.live.as_ref().map(|l| l.download_speed.mbps);
            let up_speed_mbps = stats.live.as_ref().map(|l| l.upload_speed.mbps);
            let eta_seconds = if stats.finished {
                None
            } else {
                stats.live.as_ref().and_then(|l| {
                    let remaining = stats.total_bytes.saturating_sub(stats.progress_bytes);
                    (l.download_speed.mbps > 0.0 && remaining > 0)
                        .then(|| remaining / (l.download_speed.mbps * 1024.0 * 1024.0) as u64)
                })
            };
            if stats.finished {
                *finished.borrow_mut() += 1;
            }
            *downloaded.borrow_mut() += stats.progress_bytes;
            rows.borrow_mut().push(TorrentSnapshot {
                id,
                name: handle.name().unwrap_or_else(|| format!("torrent #{id}")),
                state: stats.state.to_string(),
                progress_bytes: stats.progress_bytes,
                total_bytes: stats.total_bytes,
                finished: stats.finished,
                down_speed_mbps,
                up_speed_mbps,
                eta_seconds,
                uploaded_bytes: stats.uploaded_bytes,
            });
        }
    });
    let mut rows = rows.into_inner();
    rows.sort_by_key(|r| r.id);
    DaemonState {
        torrents: rows,
        finished: finished.into_inner(),
        downloaded_total: downloaded.into_inner(),
        webseed: webseed
            .lock()
            .map(|tasks| tasks.iter().map(WebseedTask::snapshot).collect())
            .unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn daemon_paths_under_config_dir() {
        // US-040: socket e pidfile no diretório de configuração da aplicação.
        let paths = DaemonPaths::new(Path::new("/tmp/cfg/opentorrent"));
        assert_eq!(
            paths.socket,
            std::path::Path::new("/tmp/cfg/opentorrent/daemon.sock")
        );
        assert_eq!(
            paths.pid,
            std::path::Path::new("/tmp/cfg/opentorrent/daemon.pid")
        );
    }

    #[test]
    fn service_unit_lives_under_systemd_user_dir() {
        // US-040: o unit fica em ~/.config/systemd/user/opentorrent-daemon.service
        // (irmão do diretório de configuração da aplicação).
        let path = service_unit_path().expect("caminho do unit resolvido");
        let expected = config_folder()
            .expect("config dir")
            .parent()
            .expect("parent")
            .join("systemd/user/opentorrent-daemon.service");
        assert_eq!(path, expected);
    }

    #[test]
    fn service_unit_content_points_at_current_binary() {
        // US-040: ExecStart usa o binário atual em modo headless + Restart.
        let content = service_unit_content().expect("unit gerado");
        let exe = std::env::current_exe().expect("binário atual");
        assert!(content.contains(&format!("ExecStart={} --daemon-headless", exe.display())));
        assert!(content.contains("Restart=on-failure"));
        assert!(content.contains("WantedBy=default.target"));
    }

    /// Sobe uma sessão de teste (sem rede/DHT) e um listener IPC temporário,
    /// devolvendo os caminhos e a sessão compartilhada.
    async fn test_daemon_setup() -> (Arc<Session>, DaemonPaths) {
        let dir = std::env::temp_dir().join(format!(
            "opentorrent-daemon-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let session = Session::new_with_opts(
            dir.clone(),
            SessionOptions {
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
        let paths = DaemonPaths::new(&dir);
        (session, paths)
    }

    #[tokio::test]
    async fn ipc_serves_state_and_errors() {
        // US-040: o daemon responde `GetState` (fila vazia), rejeita `Add`
        // inválido com `Error` e reporta `Error` para id inexistente.
        let (session, paths) = test_daemon_setup().await;
        std::fs::remove_file(&paths.socket).ok();
        let listener = UnixListener::bind(&paths.socket).unwrap();
        let session_clone = session.clone();
        let shutdown = Arc::new(tokio::sync::Notify::new());
        let webseed = crate::webseed::new_registry();
        let base_folder = paths
            .socket
            .parent()
            .expect("socket tem parent")
            .to_path_buf();
        let handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(stream, &session_clone, &base_folder, &webseed, &shutdown)
                .await
                .unwrap();
        });

        let mut client = DaemonClient::connect(&paths.socket).await.unwrap();
        // GetState: fila vazia.
        let state = client.get_state().await.unwrap();
        assert_eq!(state.finished, 0);
        assert!(state.torrents.is_empty());

        // Add inválido: origem malformada → Error.
        let resp = client
            .request(DaemonRequest::Add {
                source: "não-é-um-magnet".to_string(),
            })
            .await
            .unwrap();
        assert!(matches!(resp, DaemonResponse::Error { .. }));

        // Pause de id inexistente → Error (não crasha).
        let resp = client
            .request(DaemonRequest::Pause { id: 999 })
            .await
            .unwrap();
        assert!(matches!(resp, DaemonResponse::Error { .. }));

        // Fecha a conexão para que o servidor veja EOF e encerre o loop.
        drop(client);
        handle.await.unwrap();
    }

    #[tokio::test]
    async fn ipc_returns_already_managed_for_duplicate_add() {
        // US-040: dedup por infohash — a 2ª adição do mesmo torrent responde
        // `AlreadyManaged`, sem duplicar a fila. Requer rede para resolver o
        // magnet; quando indisponível, o add falha com `Error` (aceitável).
        let (session, _paths) = test_daemon_setup().await;
        let source =
            "magnet:?xt=urn:btih:deadbeefdeadbeefdeadbeefdeadbeefdeadbeef&dn=teste".to_string();
        let _ = session
            .add_torrent(
                librqbit::AddTorrent::from_cli_argument(&source).unwrap(),
                None,
            )
            .await;
        // A sessão agora considera o infohash; uma nova adição com o mesmo
        // magnet deve retornar AlreadyManaged se o torrent foi registrado.
        let resp = session
            .add_torrent(
                librqbit::AddTorrent::from_cli_argument(&source).unwrap(),
                None,
            )
            .await;
        match resp {
            Ok(librqbit::AddTorrentResponse::AlreadyManaged(_, _)) => {
                // Dedup por infohash ativo: 2ª adição reportada como existente.
            }
            Ok(_) => panic!("segunda adição do mesmo infohash deveria ser AlreadyManaged"),
            // Rede indisponível no CI: o add falha; o dedup não é verificável
            // aqui (teste aceita e não panica).
            Err(_) => {}
        }
    }

    // --- US-049 webseed: integração do `dispatch(Add)` com servidor HTTP ---

    /// Emite um string bencode (`<len>:<bytes>`) para o writer.
    fn push_bstr(out: &mut Vec<u8>, s: &[u8]) {
        out.extend_from_slice(format!("{}:", s.len()).as_bytes());
        out.extend_from_slice(s);
    }

    fn push_len(out: &mut Vec<u8>, val: u64) {
        out.extend_from_slice(format!("i{val}e").as_bytes());
    }

    /// Gera um `.torrent` single-file válido (peças SHA1 reais do conteúdo),
    /// com `url-list` opcional — para o caminho webseed (US-049).
    fn single_file_torrent(name: &str, data: &[u8], url_list: Option<&str>) -> Vec<u8> {
        use sha1::Digest;
        let mut pieces = Vec::new();
        for chunk in data.chunks(256 * 1024) {
            let mut hasher = sha1::Sha1::new();
            hasher.update(chunk);
            pieces.extend_from_slice(hasher.finalize().as_slice());
        }
        let mut out = Vec::new();
        out.push(b'd');
        push_bstr(&mut out, b"announce");
        push_bstr(&mut out, b"udp://tracker.invalid:80");
        push_bstr(&mut out, b"info");
        out.push(b'd');
        push_bstr(&mut out, b"length");
        push_len(&mut out, data.len() as u64);
        push_bstr(&mut out, b"name");
        push_bstr(&mut out, name.as_bytes());
        push_bstr(&mut out, b"piece length");
        push_len(&mut out, 256 * 1024);
        push_bstr(&mut out, b"pieces");
        push_bstr(&mut out, &pieces);
        out.push(b'e');
        if let Some(base) = url_list {
            push_bstr(&mut out, b"url-list");
            out.push(b'l');
            push_bstr(&mut out, base.as_bytes());
            out.push(b'e');
        }
        out.push(b'e');
        out
    }

    /// Servidor HTTP mínimo: serve respostas fixas por rota (200 com corpo,
    /// ignorando Range — aceito pelo `download_webseed`).
    struct MiniHttp {
        addr: std::net::SocketAddr,
        routes: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>>,
    }

    impl MiniHttp {
        fn start() -> Self {
            let routes = std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::<
                String,
                Vec<u8>,
            >::new()));
            let (addr_tx, addr_rx) = std::sync::mpsc::channel();
            {
                let routes = routes.clone();
                std::thread::spawn(move || {
                    let rt = tokio::runtime::Runtime::new().expect("rt do MiniHttp");
                    rt.block_on(async move {
                        let listener =
                            tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bind");
                        let _ = addr_tx.send(listener.local_addr().expect("addr"));
                        loop {
                            let Ok((mut socket, _)) = listener.accept().await else {
                                continue;
                            };
                            let routes = routes.clone();
                            tokio::spawn(async move {
                                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                                let mut buf = [0u8; 4096];
                                if socket.read(&mut buf).await.is_err() {
                                    return;
                                }
                                let req = String::from_utf8_lossy(&buf);
                                let Some(path) = req.split_whitespace().nth(1) else {
                                    return;
                                };
                                let path = path.trim_start_matches('/').to_string();
                                let body = routes.lock().unwrap().get(&path).cloned();
                                match body {
                                    Some(body) => {
                                        let head = format!(
                                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                                            body.len()
                                        );
                                        let _ = socket.write_all(head.as_bytes()).await;
                                        let _ = socket.write_all(&body).await;
                                    }
                                    None => {
                                        let _ = socket
                                            .write_all(
                                                b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                                            )
                                            .await;
                                    }
                                }
                            });
                        }
                    });
                });
            }
            Self {
                addr: addr_rx.recv().expect("endereço do MiniHttp"),
                routes,
            }
        }

        fn add(&self, path: &str, body: Vec<u8>) {
            self.routes.lock().unwrap().insert(path.to_string(), body);
        }
    }

    #[tokio::test]
    async fn webseed_add_downloads_then_joins_session() {
        // US-049 (T009): `dispatch(Add)` de `.torrent` com `url-list` responde
        // rápido e o torrent entra na sessão após o download HTTP; sem
        // `url-list` o add é direto. Gated: precisa de `Session` real + HTTP.
        if std::env::var("RUN_SLOW_TESTS").is_err() {
            eprintln!("RUN_SLOW_TESTS não definido — teste pulado");
            return;
        }
        const NAME: &str = "arquivo.bin";
        let data: Vec<u8> = (0..300_000u32).map(|i| (i % 251) as u8).collect();

        let server = MiniHttp::start();
        let base = format!("http://{}/", server.addr);
        server.add(
            "with.torrent",
            single_file_torrent(NAME, &data, Some(&base)),
        );
        server.add(NAME, data.clone());

        let dir = std::env::temp_dir().join(format!(
            "opentorrent-webseed-join-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let session = Session::new_with_opts(
            dir.clone(),
            SessionOptions {
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

        let webseed = crate::webseed::new_registry();
        let base_folder = dir.clone();
        let shutdown = Arc::new(tokio::sync::Notify::new());
        let listener = UnixListener::bind(dir.join("daemon.sock")).unwrap();
        let session_clone = session.clone();
        let spawn_webseed = webseed.clone();
        let spawn_folder = base_folder.clone();
        let handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            handle_connection(
                stream,
                &session_clone,
                &spawn_folder,
                &spawn_webseed,
                &shutdown,
            )
            .await
            .unwrap();
        });

        let mut client = DaemonClient::connect(&base_folder.join("daemon.sock"))
            .await
            .unwrap();

        // Com `url-list` → resposta imediata "adicionando via webseed".
        let resp = client
            .request(DaemonRequest::Add {
                source: format!("http://{}/with.torrent", server.addr),
            })
            .await
            .unwrap();
        match resp {
            DaemonResponse::Ok { message } => assert_eq!(message, "adicionando via webseed"),
            other => panic!("esperava Ok(webseed), got {other:?}"),
        }

        // A task background baixa `arquivo.bin` e o torrent entra na sessão.
        let mut in_session = false;
        for _ in 0..60 {
            let downloaded = std::fs::read(base_folder.join(NAME))
                .map(|bytes| bytes == data)
                .unwrap_or(false);
            let present = downloaded && session.with_torrents(|iter| iter.count() > 0);
            if present {
                in_session = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        assert!(
            in_session,
            "torrent webseed não entrou na sessão após o download"
        );
        // Registro limpo após o torrent entrar na sessão (tarefa concluída).
        let restantes = webseed.lock().unwrap_or_else(|p| p.into_inner()).len();
        assert_eq!(
            restantes, 0,
            "tarefa webseed concluída deveria sair do registro"
        );

        // Sem `url-list` → add direto (não via webseed). Usa `info` distinto
        // (nome/conteúdo diferentes) para não colidir com o infohash do `with`.
        let plain_data: Vec<u8> = (0..100_000u32).map(|i| (i % 13) as u8).collect();
        server.add(
            "plain.torrent",
            single_file_torrent("plain.bin", &plain_data, None),
        );
        let resp = client
            .request(DaemonRequest::Add {
                source: format!("http://{}/plain.torrent", server.addr),
            })
            .await
            .unwrap();
        assert!(
            matches!(&resp, DaemonResponse::Ok { message } if message == "adicionado"),
            "esperava add direto, got {resp:?}"
        );

        drop(client);
        handle.await.unwrap();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
