# Quickstart: US-040 — Daemon em background e deduplicação de torrents

Comandos básicos para desenvolver e testar a feature em Linux.

## Build e verificação

```bash
cargo check          # loop rápido de edição
cargo test           # testes unitários (gate)
cargo clippy -- -D warnings
cargo fmt --check
cargo build
```

## Fluxo de uso (após a implementação)

```bash
# Abrir a TUI: inicia o daemon automaticamente se não estiver rodando (transparente).
opentorrent

# Modo add com daemon ativo: delega a adição ao daemon (dedup por infohash aplicado).
opentorrent "magnet:?xt=urn:btih:..."

# Gerenciar o daemon explicitamente.
opentorrent daemon install         # instala o systemd user service (idempotente; no-op sem systemd)
opentorrent daemon status          # mostra pid + socket
opentorrent daemon stop            # encerra o daemon (systemctl --user stop ou shutdown via IPC)
opentorrent daemon start           # systemctl --user start (fallback: spawn desanexado)

# Atualização automática (release GitHub): para o service, substitui o binário
# (valida assinatura US-018 se a CA local existir) e religa.
opentorrent update

# Excluir da lista mantendo arquivos: na TUI, X/Delete → torrent some da lista,
# arquivos permanecem em ~/downloads/torrent-downloads; re-adicionar retoma (smart resume).
```

## Systemd user service

- Na primeira execução (TUI, `add`, `daemon start`), o binário instala o unit
  `~/.config/systemd/user/opentorrent-daemon.service` (`ExecStart=<bin> --daemon-headless`,
  `Restart=on-failure`) com `daemon-reload` + `enable`, se o systemd user estiver disponível.
- `systemctl --user status opentorrent-daemon` / `--user is-active opentorrent-daemon`.
- Sobreviver a logout exige `loginctl enable-linger $USER` (opcional, documentado; passo manual).
- Sem systemd (ou user manager indisponível): `daemon start` cai no spawn desanexado (comportamento antigo).

## Teste manual do daemon

1. Garantir que não há daemon: `opentorrent daemon status` → "não está rodando".
2. Rodar `opentorrent` (TUI abre; daemon é iniciado em background). Verificar um processo `opentorrent`
   desanexado: `pgrep -af opentorrent`.
3. Adicionar um magnet na TUI e observar progresso. Fechar a TUI (`/exit`). `pgrep` deve continuar
   mostrando o processo do daemon.
4. `sleep 15` (ou aguardar) e reabrir `opentorrent` → o mesmo torrent aparece com progresso maior
   (continuou baixando).
5. Adicionar o **mesmo** magnet de novo → notice "já está na lista" (sem duplicação).
6. Excluir com X (mantém arquivos) → torrent some; re-adicionar o magnet → volta e retoma (smart resume).
7. `opentorrent daemon stop` → processo encerra; `status` → "não está rodando".

## Teste automatizado sugerido (unit)

- Round-trip JSON de `DaemonRequest`/`DaemonResponse` (serializar/deserializar sem socket).
- `TorrentSnapshot` → linha da tabela (render pura, testes existentes de `session_table_line`).
- Dedup real com librqbit: `session.add_torrent(magnet)` duas vezes → 2ª retorna `AlreadyManaged`
  (teste de integração opcional, `RUN_SLOW_TESTS`, sem rede de peers).

## Observações

- Diretório/socket do daemon: `~/.config/opentorrent/daemon.sock` e `.pid` (perm. `0o700`).
- Windows: daemon desabilitado por enquanto (`cfg(target_os = "windows")`); comportamento atual preservado.
- Se o socket ficar stale (daemon morto), a próxima abertura remove e recria. Se duplo daemon for
  detectado (socket ativo), a TUI conecta no existente (não cria outro).