//! Protocolo IPC do daemon (US-040): socket Unix + JSON.
//!
//! Enquadramento: `u32` little-endian com o comprimento do payload + payload
//! JSON (UTF-8). Cada conexão processa uma requisição e responde antes da
//! próxima (request → response por conexão).

use std::{
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Tamanho máximo aceito de um frame (64 KiB) para evitar DoS no socket.
const MAX_FRAME_LEN: u32 = 64 * 1024;

/// Requisição do cliente (TUI ou CLI `add`) ao daemon (US-040).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DaemonRequest {
    /// Adiciona uma origem (magnet, URL `.torrent` ou arquivo local).
    /// Dedup por infohash é decidido pelo daemon (fonte da verdade = sessão).
    Add { source: String },
    /// Pausa o torrent.
    Pause { id: usize },
    /// Retoma o torrent.
    Resume { id: usize },
    /// Remove da fila mantendo os arquivos no disco.
    Stop { id: usize },
    /// Exclui definitivamente (remove da sessão e apaga os dados).
    Delete { id: usize },
    /// Retorna o snapshot atual da fila.
    GetState,
    /// Pede ao daemon para encerrar.
    Shutdown,
}

/// Resposta do daemon ao cliente (US-040).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DaemonResponse {
    /// Sucesso incondicional (ex.: pause/resume/stop) com mensagem.
    Ok { message: String },
    /// Dedup: origem já gerenciada pelo infohash (notice "já está na lista").
    AlreadyManaged,
    /// Snapshot atual da fila (resposta ao `GetState`).
    /// `Option` permite evoluir o protocolo; nesta US é sempre `Some`.
    State { snapshot: Option<DaemonState> },
    /// Erro propagado sem stack trace interno, em pt-BR.
    Error { message: String },
}

/// Snapshot serializável da fila para a TUI renderizar (US-040).
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonState {
    pub torrents: Vec<TorrentSnapshot>,
    /// Nº de torrents completos.
    pub finished: usize,
    /// Bytes totais baixados na sessão.
    pub downloaded_total: u64,
    /// Downloads webseed em andamento (US-049). Vazio quando nenhum ativo;
    /// `#[serde(default)]` mantém compatibilidade evolutiva do protocolo.
    #[serde(default)]
    pub webseed: Vec<WebseedSnapshot>,
}

/// Linha serializável de um torrent na fila (US-040). Espelha os campos que a
/// TUI já renderizava via `SessionRow`; `state` é o `Display` de
/// `TorrentStatsState` (ex.: `live`, `paused`, `error`, `initializing`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TorrentSnapshot {
    pub id: usize,
    pub name: String,
    pub state: String,
    pub progress_bytes: u64,
    pub total_bytes: u64,
    pub finished: bool,
    pub down_speed_mbps: Option<f64>,
    pub up_speed_mbps: Option<f64>,
    pub eta_seconds: Option<u64>,
    pub uploaded_bytes: u64,
}

/// Linha serializável de um download webseed em andamento (US-049). Os campos
/// espelham `WebseedProgress` do módulo `webseed`; `state` é um texto de domínio
/// (`Downloading`, `Done` ou mensagem de `Error`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WebseedSnapshot {
    pub id: usize,
    pub source: String,
    pub name: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub done_files: usize,
    pub total_files: usize,
    pub state: String,
}

/// Serializa um frame (u32 LE + payload) em `buf`.
pub fn encode_frame(out: &mut Vec<u8>, payload: &[u8]) -> io::Result<()> {
    let len = u32::try_from(payload.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame payload too large"))?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
    Ok(())
}

/// Decodifica o comprimento de um frame a partir dos 4 primeiros bytes do
/// cabeçalho. Retorna o comprimento esperado do payload.
pub fn frame_len(header: &[u8]) -> io::Result<u32> {
    debug_assert_eq!(header.len(), 4);
    let len = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    if len > MAX_FRAME_LEN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    Ok(len)
}

/// Serializa e enquadra uma mensagem `T` (serde) em `buf`.
pub fn encode_message<T: Serialize>(out: &mut Vec<u8>, msg: &T) -> io::Result<()> {
    let payload =
        serde_json::to_vec(msg).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    encode_frame(out, &payload)
}

/// Decodifica o payload (após o cabeçalho de 4 bytes) em uma mensagem `T`.
pub fn decode_message<T: for<'de> Deserialize<'de>>(payload: &[u8]) -> io::Result<T> {
    serde_json::from_slice(payload).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

/// Cliente IPC do daemon (US-040): conecta ao socket Unix, envia uma
/// `DaemonRequest` e aguarda a resposta correspondente. Unix-only (socket
/// Unix não existe no Windows — a TUI lá roda em modo local).
#[cfg(unix)]
pub struct DaemonClient {
    stream: tokio::net::UnixStream,
}

#[cfg(unix)]
impl DaemonClient {
    /// Conecta ao socket do daemon. Retorna `Err` se o daemon não estiver
    /// rodando (ou o socket estiver stale).
    pub async fn connect(socket_path: &Path) -> io::Result<Self> {
        Ok(DaemonClient {
            stream: tokio::net::UnixStream::connect(socket_path).await?,
        })
    }

    /// Envia uma requisição e aguarda a resposta do daemon.
    pub async fn request(&mut self, req: DaemonRequest) -> anyhow::Result<DaemonResponse> {
        let mut buf = Vec::with_capacity(64);
        encode_message(&mut buf, &req)?;
        self.stream.write_all(&buf).await?;
        // A resposta é lida de forma streaming: 4 bytes de cabeçalho + payload.
        let mut header = [0u8; 4];
        self.stream.read_exact(&mut header).await?;
        let len = frame_len(&header)? as usize;
        let mut payload = vec![0u8; len];
        self.stream.read_exact(&mut payload).await?;
        Ok(decode_message(&payload)?)
    }

    /// Conveniência: solicita o snapshot atual da fila.
    pub async fn get_state(&mut self) -> anyhow::Result<DaemonState> {
        match self.request(DaemonRequest::GetState).await? {
            DaemonResponse::State { snapshot } => Ok(snapshot.unwrap_or_default()),
            DaemonResponse::Error { message } => anyhow::bail!(message),
            other => anyhow::bail!("resposta inesperada do daemon: {other:?}"),
        }
    }
}

/// Caminhos do daemon (socket e pidfile), a partir do diretório de
/// configuração (`~/.config/opentorrent`, US-038).
pub struct DaemonPaths {
    pub socket: PathBuf,
    pub pid: PathBuf,
}

impl DaemonPaths {
    /// Monta os caminhos dentro do diretório de configuração da aplicação.
    pub fn new(config_folder: &Path) -> Self {
        DaemonPaths {
            socket: config_folder.join("daemon.sock"),
            pid: config_folder.join("daemon.pid"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_request() {
        let reqs = [
            DaemonRequest::Add {
                source: "magnet:?xt=urn:btih:abc".to_string(),
            },
            DaemonRequest::Pause { id: 0 },
            DaemonRequest::Resume { id: 1 },
            DaemonRequest::Stop { id: 2 },
            DaemonRequest::Delete { id: 3 },
            DaemonRequest::GetState,
            DaemonRequest::Shutdown,
        ];
        for req in reqs {
            let mut buf = Vec::new();
            encode_message(&mut buf, &req).unwrap();
            let body = &buf[4..];
            let decoded: DaemonRequest = decode_message(body).unwrap();
            assert_eq!(decoded, req);
        }
    }

    #[test]
    fn round_trip_response_state() {
        let resp = DaemonResponse::State {
            snapshot: Some(DaemonState {
                torrents: vec![TorrentSnapshot {
                    id: 0,
                    name: "ubuntu.iso".to_string(),
                    state: "Downloading".to_string(),
                    progress_bytes: 42,
                    total_bytes: 100,
                    finished: false,
                    down_speed_mbps: Some(1.5),
                    up_speed_mbps: None,
                    eta_seconds: Some(60),
                    uploaded_bytes: 0,
                }],
                finished: 0,
                downloaded_total: 42,
                webseed: vec![WebseedSnapshot {
                    id: 0,
                    source: "https://archive.org/download/x/x.torrent".to_string(),
                    name: "x".to_string(),
                    downloaded_bytes: 10,
                    total_bytes: 100,
                    done_files: 1,
                    total_files: 3,
                    state: "Downloading".to_string(),
                }],
            }),
        };
        let mut buf = Vec::new();
        encode_message(&mut buf, &resp).unwrap();
        let body = &buf[4..];
        let decoded: DaemonResponse = decode_message(body).unwrap();
        assert_eq!(decoded, resp);
    }

    #[test]
    fn frame_len_rejects_oversized() {
        let mut header = (MAX_FRAME_LEN + 1).to_le_bytes();
        assert!(frame_len(&header).is_err());
        header = MAX_FRAME_LEN.to_le_bytes();
        assert_eq!(frame_len(&header).unwrap(), MAX_FRAME_LEN);
    }

    #[test]
    fn frame_header_le() {
        let mut buf = Vec::new();
        encode_frame(&mut buf, b"abcd").unwrap();
        let len = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        assert_eq!(len, 4);
    }

    #[test]
    fn names_with_newlines_survive_json() {
        let req = DaemonRequest::Add {
            source: "nome\ncom\nquebras".to_string(),
        };
        let mut buf = Vec::new();
        encode_message(&mut buf, &req).unwrap();
        let decoded: DaemonRequest = decode_message(&buf[4..]).unwrap();
        assert_eq!(decoded, req);
    }
}
