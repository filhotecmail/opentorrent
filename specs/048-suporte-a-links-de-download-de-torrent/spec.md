# Feature Specification: US-048 — Suporte a links de download de .torrent e drag & drop no input

**Feature Branch**: `feat/us-048-suporte-a-links-de-download-de-torrent-h`

**Created**: 2026-08-11

**Status**: Draft

**Issue**: #103

**Input**: User description: "Preciso que dê suporte a links de downloads de arquivos .torrent. O usuário pode passar por exemplo este link https://fosstorrents.com/thankyou/?name=arcolinux&cat=Latest%20Edition&id=1&hybrid=0 — esse link faz o download de um arquivo torrent. Ao identificar que o arquivo é um torrent, adicione-o na lista de transmissão. Também quero suporte a drag and drop na área do footer no campo de input para arrastar arquivos .torrent e enviá-los para a lista de processamento."

## Clarifications (Speckit Clarify)

1. **URL que retorna HTML**: A — seguir links detectados no HTML. Ao baixar a URL e receber HTML (não um .torrent), procurar por `href`/`location` que termine em `.torrent`, baixar esse link e só então adicionar se for um .torrent válido. (Ex.: fosstorrents `thankyou` → `window.location = /files/download.php?file=....torrent`.)
2. **Drag & drop**: B — tratar paste como caminho + detectar .torrent. O terminal (ex.: GNOME Terminal) cola o caminho do arquivo ao arrastar; o caminho é processado como origem e, se for um arquivo `.torrent` existente, resolvido como tal.
3. **Validação de .torrent**: A — validar bencode + infohash. Requisitar bytes, tentar parsear como torrent (`d8:announce...`) e checar infohash; aceitar qualquer URL independentemente da extensão do nome.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Adicionar torrent via URL de download (página HTML) (Priority: P1)

O usuário passa (modo add ou `/add` na TUI) um link HTTP(S) que NÃO termina em `.torrent` e cujo corpo é uma página HTML que dispara download de um `.torrent` (ex.: fosstorrents thankyou). O OpenTorrent baixa o corpo, detecta que é HTML, extrai o link de download `.torrent`, baixa-o, valida bencode+infohash e adiciona o torrent à lista de transmissão.

**Why this priority**: É o requisito central — hoje o link falha com "error decoding torrent" (item vermelho 0%).

**Independent Test**: `opentorrent add "https://fosstorrents.com/thankyou/?name=arcolinux&cat=Latest%20Edition&id=1&hybrid=0" --list` → a lista de arquivos do torrent ArcoPlasma é exibida (sem erro de decoding).

**Acceptance Scenarios**:
1. **Given** uma URL que responde HTML com link `.torrent` (fosstorrents), **When** o usuário a adiciona via `/add` ou `opentorrent add`, **Then** o torrent é resolvido e entra na lista de transmissão (item deixa de ficar vermelho/0%).
2. **Given** uma URL que responde diretamente bytes `.torrent` (sem extensão no caminho), **When** o usuário a adiciona, **Then** é validada por bencode+infohash e adicionada (sem depender da extensão).
3. **Given** uma URL que responde HTML sem nenhum link `.torrent`, **When** o usuário a adiciona, **Then** a entrada pendente mostra erro claro ("nenhum link .torrent encontrado") sem travar a navegação.
4. **Given** uma URL que responde algo que não é torrent nem HTML com link (ex.: imagem), **When** adicionada, **Then** erro claro é exibido na lista pendente.

### User Story 2 - Drag & drop de arquivo .torrent no campo de input (Priority: P1)

O usuário arrasta um arquivo `.torrent` do gerenciador de arquivos para o campo de input (footer). O terminal cola o caminho; o OpenTorrent reconhece que é um caminho de arquivo `.torrent` existente, resolve-o e adiciona o torrent à lista de processamento.

**Why this priority**: Requisito explícito; amplia a entrada de origens sem digitação.

**Independent Test**: No modo TUI, arrastar `~/Downloads/foo.torrent` para o campo de input e pressionar Enter → o torrent aparece na lista de transmissão (id do pending é limpo, entra na sessão).

**Acceptance Scenarios**:
1. **Given** a TUI aberta (View::Home), **When** o usuário arrasta um arquivo `.torrent` para o footer (terminal cola o caminho) e pressiona Enter, **Then** o torrent é adicionado à lista de processamento.
2. **Given** um caminho colado que aponta para arquivo `.torrent` inexistente, **When** o usuário adiciona, **Then** a entrada pendente mostra erro claro.
3. **Given** um caminho colado para arquivo não-`.torrent` (ex.: `.txt`), **When** adicionado, **Then** comportamento atual se mantém (erro de parse no add), com mensagem clara.

### User Story 3 - Validação por bencode + infohash (Priority: P2)

A detecção de `.torrent` não depende da extensão do nome: um payload é aceito como torrent se o corpo for bencode válido e produzir um infohash, independentemente de `Content-Disposition`/extensão.

**Why this priority**: Robustez; cobre sites que servem torrent sem extensão no caminho.

**Independent Test**: Requisitar um payload torrent com URL sem `.torrent` → adicionado com sucesso (teste unitário/integrado com bytes conhecidos).

**Acceptance Scenarios**:
1. **Given** bytes de um torrent válido (bencode), **When** validados pela rotina de resolução, **Then** retornam `Some` com o caminho final/bytes e o infohash é calculado.
2. **Given** bytes inválidos (HTML/imagem), **When** validados, **Then** retornam erro identificando o tipo (ex.: "resposta não é um torrent").

## Technical Notes (Specify)

- `librqbit` já baixa URLs e decodifica em `torrent_from_url` (`session.rs`), mas falha ao receber HTML ("error decoding torrent"). A resolução de URL de download (HTML→link `.torrent`) deve acontecer ANTES de entregar a origem ao `librqbit`, normalizando para `AddTorrent::TorrentFileBytes` ou caminho local.
- A validação de torrent pode usar `librqbit::torrent_from_bytes_ext` (reexportado publicamente por `pub use librqbit_core::torrent_metainfo::*`), que faz bencode + infohash.
- `reqwest` (já dependência) e `url` (já dependência) são suficientes para download e resolução de URL relativa; não adicionar crates novos sem necessidade (convenção do projeto).
- Ponto de entrada: função compartilhada (ex.: `resolve_source`) usada por `add_torrent_background` (TUI), `daemon::dispatch` e `main.rs` (modo add), para que os três fluxos ganhem o comportamento.
- Drag & drop: GNOME Terminal e a maioria dos terminais colam o caminho absoluto do arquivo arrastado (bracketed paste → `Event::Paste`). O fluxo atual já insere o texto do paste no input; a US garante que um caminho de arquivo `.torrent` existente seja resolvido corretamente e adicionado (Enter).
- Estados: entrada pendente `PendingStatus::Resolving` → `Added` (removida) / `Done` / `Error`. Falha assíncrona vira `PendingStatus::Error` (vermelho) sem travar navegação (AC-5).
