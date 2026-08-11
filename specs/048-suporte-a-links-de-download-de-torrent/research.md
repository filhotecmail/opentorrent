# Research: US-048 — Suporte a links de download de .torrent e drag & drop no input

## Problema

O comando `opentorrent add "https://fosstorrents.com/thankyou/?name=arcolinux&cat=Latest%20Edition&id=1&hybrid=0"` (e o `/add` na TUI) falha: o `librqbit` chama `torrent_from_url` → `torrent_from_bytes`, tenta decodificar o corpo (HTML) como bencode e retorna "error decoding torrent". Na TUI, a entrada pendente vira `PendingStatus::Error` — linha **vermelha, 0%**.

**Causa raiz (camada domínio → design → mecânica)**: o fluxo assume que toda URL http(s) serve o corpo `.torrent` diretamente. A web de torrents usa páginas de "thankyou"/redirect (JS `window.location`, `<meta refresh>`, `<a href>`) que entregam o `.torrent` num segundo request. O OpenTorrent não resolve essa indireção.

## Evidência empírica (2026-08-11)

- `https://fosstorrents.com/thankyou/?name=arcolinux&cat=Latest%20Edition&id=1&hybrid=0` → `HTTP/2 200`, `content-type: text/html; charset=UTF-8`, corpo HTML com `<script>function startDownload() {window.location = "/files/download.php?file=arcoplasma-v25.05.01-x86_64.iso.torrent";}</script>` e `<body onload="startDownload();">`.
- `https://fosstorrents.com/files/download.php?file=arcoplasma-v25.05.01-x86_64.iso.torrent` → `HTTP/2 200`, `content-type: application/x-bittorrent`, `content-disposition: attachment; filename=...iso.torrent`, corpo `d8:announce36:udp://fosstorrents.com:6969/announ...` (bencode válido, 81614 bytes).

## Decisões de design

1. **`resolve_source(source)`** — função async compartilhada que classifica a origem:
   - **http(s) URL** → `download_for_torrent(url)`:
     - baixa bytes com `reqwest`;
     - tenta `librqbit::torrent_from_bytes_ext::<ByteBuf>(&bytes)` → se OK, `ResolvedSource::TorrentBytes(bytes)`;
     - senão se o corpo parece HTML → extrai candidatos a link `.torrent` (`window.location`, `location.href`, `<a href>`, `href=`) — resolve-join relativo — baixa cada um até validar;
     - senão → erro de domínio claro ("resposta não é um torrent", "nenhum link .torrent encontrado na página").
   - **senão** → `ResolvedSource::CliArgument(source)` (delega a `AddTorrent::from_cli_argument`, preserva magnet/infohash/caminho local).
   - Caminho local `.torrent` existente (incl. colado por drag & drop) já é resolvido por `from_cli_argument` → `TorrentFileBytes`; nada muda.
2. **Formato de saída** — `enum ResolvedSource<'a> { CliArgument(Cow<'a, str>), TorrentBytes(bytes::Bytes) }`, convertido para `AddTorrent` no ponto de chamada. Evita re-baixar/manipular strings (type-09).
3. **Validação** — reuso da API pública do librqbit (`torrent_from_bytes_ext`, reexportada), sem crate novo. `bytes::Bytes` e `url::Url` já existem no gráfico de deps (via `librqbit`/`reqwest`); confirmar import direto (`bytes` pode não estar declarado → usar `Vec<u8>` disfarçado via `bytes::Bytes` se disponível; senão `Into<Bytes>`).
4. **Html link extraction** — sem parser HTML (crate novo evita: const. VIII). Heurística: procurar por substrings de URL em atributos `href`, `window.location`, `location.href` terminando em `.torrent`, usando `url::Url::parse` + `join`. Amostras reais (fosstorrents) confirmam padrão simples.
5. **Pontos de integração** (3 call sites de add):
   - `src/session_ui.rs::add_torrent_background` (TUI local);
   - `src/daemon.rs::dispatch` (daemon — fonte da verdade);
   - `src/main.rs` (modo add non-interativo — probe e download).
   Todos drenam o `AddTorrent` que o `librqbit` aceita. `update.rs` intocado.
6. **Drag & drop** — GNOME Terminal/Konkale colam o caminho absoluto via bracketed paste → `Event::Paste` → já inserido no `self.input` (US-031). Enter → `submit_add_from_prompt` → `from_cli_argument` lê o arquivo. Nenhuma mudança de infra de input; testes garantem que caminho colado `.torrent` entra na lista.

## Riscos / trade-offs

- **Falsos positivos na extração HTML**: limitar a candidatos que terminam em `.torrent` e validar com `torrent_from_bytes_ext` (rejeição natural).
- **Payload grande**: `reqwest.bytes()` em memória; tamanhos típicos de `.torrent` são <1 MiB. Sem streaming (YAGNI).
- **Redirecionamentos HTTP (3xx)**: `reqwest` segue por padrão — já cobririam o fosstorrents se o servidor respondesse 302; aqui é JS redirect, coberto pela extração HTML.
- **Sem dep nova**: `bytes` é transitiva de `reqwest`/`librqbit`; se não for importável diretamente por não estar em `[dependencies]`, adicioná-la é justificado (já no lock, custo zero de build) ou usar `Vec<u8>` e `From<Vec<u8>> for Bytes`.