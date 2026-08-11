# Contracts: US-048 — Suporte a links de download de .torrent e drag & drop no input

Contratos internos do pipeline de resolução de origem (`src/resolve.rs`). Nenhum protocolo externo novo.

## 1. `resolve_source(source: &str, client: &reqwest::Client) -> Result<ResolvedSource<'_>, anyhow::Error>`

- Para **origens não-http(s)** (magnet `magnet:`, infohash de 40 hex, caminho de arquivo local, `-`):
  retorna `Ok(ResolvedSource::CliArgument(Cow::Borrowed(source)))` — sem I/O. O chamador delega a
  `AddTorrent::from_cli_argument`, preservando o comportamento atual.
- Para **URLs http(s)**:
  - **Contrato forte**: nunca retorna uma origem ainda-resolúvel; ou `TorrentBytes` (bytes validados como
    `.torrent`) ou `Err` com mensagem de domínio clara.
  - Download com `client.get(url).header("User-Agent", "opentorrent")`.
  - Validação por `librqbit::torrent_from_bytes_ext::<ByteBuf>(&bytes)` (bencode + infohash). Ok →
    `TorrentBytes`.
  - Falha de validação:
    - corpo parecendo HTML → extrair candidatos (atributos `href`, `window.location`, `location.href`),
      resolver-join relativo contra a URL base, manter apenas os que terminam em `.torrent`; baixar cada um
      e validar; primeiro ok → `TorrentBytes`, senão `Err("nenhum link .torrent encontrado na página")`.
    - senão → `Err("resposta não é um torrent")`.
  - Erros de rede/HTTP (timeout, 4xx/5xx) → `Err("falha ao baixar a URL: {erro}")`.

## 2. `ResolvedSource::to_add_torrent(self) -> Result<AddTorrent<'_>, anyhow::Error>`

- `CliArgument(s)` → `AddTorrent::from_cli_argument(&s)` (lê arquivo local quando aplicável).
- `TorrentBytes(b)` → `AddTorrent::TorrentFileBytes(b)` (sem erro).

## 3. Pontos de chamada

- `session_ui.rs::add_torrent_background` (TUI local): substitui `AddTorrent::from_cli_argument(&source)`
  por `let src = resolve_source(&source, &client).await?; let add = src.to_add_torrent()?;`.
- `daemon.rs::dispatch` (`DaemonRequest::Add`): idem, antes de `session.add_torrent`.
- `main.rs` (modo add): idem no probe (`list_only`) e no download — a origem resolvida alimenta
  `session.add_torrent`.

## 4. Drag & drop

- O terminal entrega o caminho absoluto via bracketed paste → `crossterm::Event::Paste` → inserido no
  `self.input` (fluxo US-031 já existente, **inalterado**).
- Enter → `submit_add_from_prompt` → `resolve_source` → caminho `.torrent` existente vira
  `CliArgument` → `from_cli_argument` lê o arquivo. Contrato: caminho de arquivo `.torrent` colado/dropeado
  entra na lista de processamento.

## Regressão canônica

```bash
cargo run -- add "https://fosstorrents.com/thankyou/?name=arcolinux&cat=Latest%20Edition&id=1&hybrid=0" --list
```

Deve imprimir a lista de arquivos do torrent (sem "error decoding torrent").