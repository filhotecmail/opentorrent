# Contracts: US-040 — Daemon em background e deduplicação de torrents

Contratos do IPC (socket Unix + JSON). Enquadramento: **tamanho-prefixado** (u32 LE little-endian + payload
UTF-8 JSON) para evitar colisão com `\n` em nomes de arquivos.

## Protocolo

```
[ len: u32 LE ][ payload: JSON bytes (DaemonRequest) ]   -> request (cliente)
[ len: u32 LE ][ payload: JSON bytes (DaemonResponse) ]  -> response (daemon)
```

Cada requisição recebe exatamente **uma** resposta, na mesma conexão, antes da próxima requisição. Conexão
curta por operação (apenas uma requisição por conexão) ou reuso com pipelining serial — decisão de
implementação; o contrato garante resposta antes da próxima requisição na mesma conexão.

## Tipos (serde)

### `DaemonRequest` (tagged enum)

```json
{ "Add": { "source": "magnet:?xt=urn:btih:..." } }
{ "Pause":    { "id": 0 } }
{ "Resume":   { "id": 1 } }
{ "Stop":     { "id": 2 } }
{ "Delete":   { "id": 3 } }
{ "GetState": null }
{ "Shutdown": null }
```

### `DaemonResponse` (tagged enum)

```json
{ "Ok": { "message": "pausado: foo" } }
{ "AlreadyManaged": null }
{ "State": { "snapshot": { "torrents": [], "finished": 0, "downloaded_total": 0 } } }
{ "Error": { "message": "torrent não encontrado (id=9)" } }
```

### `DaemonState` / `TorrentSnapshot`

```json
{
  "torrents": [
    {
      "id": 0,
      "name": "ubuntu-24.04.iso",
      "state": "Downloading",
      "progress_bytes": 1048576,
      "total_bytes": 4294967296,
      "finished": false,
      "down_speed_mbps": 2.5,
      "up_speed_mbps": 0.1,
      "eta_seconds": 1800,
      "uploaded_bytes": 0
    }
  ],
  "finished": 0,
  "downloaded_total": 1048576
}
```

## Casos de uso (spec → contrato)

| Cenário da spec | Requisição | Resposta esperada |
|-----------------|-----------|-------------------|
| US-1: reconectar mostra progresso | `GetState` | `State` com snapshot atual (progresso > fechamento). |
| US-2: dedup (mesmo infohash na lista) | `Add` (2ª vez, mesmo magnet/URL/arquivo) | `AlreadyManaged` — nenhuma linha nova. |
| US-2: dedup (torrent completo na lista) | `Add` (mesma origem) | `AlreadyManaged`. |
| US-2: após excluir (X, arquivos mantidos) | `Add` (mesma origem) | `Ok` — re-adiciona e retoma (smart resume). |
| US-3: auto-start | — (TUI entra sem daemon) | daemon spawnado; `GetState` → `State`. |
| US-4: excluir mantém arquivos | `Stop { id }` | `Ok` — torrent fora da lista; arquivos intactos. |

## CLI (modos externos, fora do IPC)

| Comando | Efeito |
|---------|--------|
| `opentorrent daemon install` | Instala o systemd user service (unit `~/.config/systemd/user/opentorrent-daemon.service`, `daemon-reload` + `enable`); idempotente; no-op sem systemd. |
| `opentorrent daemon start` | `systemctl --user start` (fallback: spawn desanexado `--daemon-headless`). |
| `opentorrent daemon stop` | `systemctl --user stop` (fallback: `Shutdown` via IPC). |
| `opentorrent daemon status` | pid + socket. |
| `opentorrent update` | Para o service, baixa release GitHub, valida assinatura (US-018, se CA local), substitui binário (temp+rename) e religa. |

## Garantias

- **Dedup é decidido no daemon** (fonte da verdade = sessão librqbit). A TUI/CLI apenas encaminham `Add`.
- Erros: `Error` com mensagem simples (pt-BR), nunca stack trace interno.
- Socket: `~/.config/opentorrent/daemon.sock`, diretório `0o700`; conexões apenas do mesmo usuário.
- `GetState` é `null` payload (serde `#[serde(deny_unknown_fields)]` recomendada nos enums).