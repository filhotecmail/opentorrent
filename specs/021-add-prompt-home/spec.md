# Feature Specification: US-034 — /add com prompt "insira o link:" na Home

**Feature Branch**: `feat/us-034-add-prompt-home`

**Created**: 2026-08-09

**Status**: Draft

**Input**: User description: "Na tela Home, quando o usuário digita /add e dá Enter sem link, mostrar 'insira o link:' na mesma área de prompt."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Modo "insira o link:" no prompt da Home (Priority: P1)

O usuário está na Home, digita `/add` e pressiona Enter sem nenhuma origem. Em vez de abrir um diálogo separado, o prompt fixo da base muda o rótulo para "insira o link:" e permanece ativo na própria Home — o usuário digita/cola a origem (magnet, `.torrent` local ou URL) e pressiona Enter para adicionar, sem trocar de tela.

**Why this priority**: É o comportamento central da US — o `/add` passa a acontecer 100% pelo prompt da Home, eliminando a troca de contexto para a view Include no fluxo da Home.

**Independent Test**: Na Home, digitar `/add`, Enter, ver o prompt exibindo "insira o link:", colar um magnet, Enter, e permanecer na Home com a linha "inicializando" na Biblioteca e a adição rodando em segundo plano.

**Acceptance Scenarios**:
1. **Given** a Home com o prompt fixo, **When** o usuário digita `/add` e pressiona Enter sem origem, **Then** o prompt muda para "insira o link:" e a view permanece Home (sem diálogo separado).
2. **Given** o prompt no modo "insira o link:", **When** o usuário digita/cola uma origem válida e pressiona Enter, **Then** a origem é validada, a adição roda em background e o prompt volta ao estado normal da Home, com a linha "inicializando" aparecendo na Biblioteca.
3. **Given** o prompt no modo "insira o link:" com origem vazia, **When** o usuário pressiona Enter, **Then** o sistema exibe notice pedindo a origem e o modo permanece ativo.
4. **Given** o prompt no modo "insira o link:", **When** o usuário pressiona Esc, **Then** o modo é cancelado e o prompt volta ao estado normal ("Enter a command or / for options").
5. **Given** o prompt no modo "insira o link:" com origem inválida, **When** o usuário pressiona Enter, **Then** a validação de sintaxe falha, o texto digitado é preservado para correção e um notice de erro é exibido.

### User Story 2 - /add com origem inline (Priority: P1)

O usuário digita `/add magnet:...` (ou `.torrent`/URL) diretamente no prompt da Home e pressiona Enter. O sistema reconhece a origem junto do comando e adiciona imediatamente, sem pedir o link novamente.

**Why this priority**: Complementa o fluxo do US1 permitindo o atalho de uma linha só, quando o usuário já tem a origem pronta para colar.

**Independent Test**: Na Home, digitar `/add magnet:?xt=urn:btih:...`, Enter, e ver a adição iniciar em background permanecendo na Home.

**Acceptance Scenarios**:
1. **Given** o prompt da Home com `/add <origem>` e Enter pressionado, **When** a origem é válida, **Then** a adição roda em background e a Home permanece visível, com a linha da origem aparecendo imediatamente como "inicializando".
2. **Given** o prompt com `/add <origem inválida>` e Enter pressionado, **When** a validação de sintaxe falha, **Then** o prompt é preservado com a origem digitada e um notice de erro é exibido.
3. **Given** o prompt com `/add ` (espaço após o comando, sem origem) e Enter pressionado, **Then** o sistema entra no modo "insira o link:" (mesmo comportamento do US1).
4. **Given** qualquer fluxo de adição pela Home, **Then** a view Include nunca é aberta a partir da Home.

### User Story 3 - View Include preservada para atalhos da Session (Priority: P2)

O usuário que está na view Session continua podendo abrir o diálogo de inclusão via `a`/`A`/`+`, com o comportamento legado intacto (seleção origem/local, input dedicado).

**Why this priority**: Garante que a mudança de fluxo da Home não quebra o caminho de adição já existente na Session — escopo reduzido, sem remoção de código legado nesta US.

**Independent Test**: Na Session, pressionar `a` e ver o diálogo Include abrir com seleção origem/local.

**Acceptance Scenarios**:
1. **Given** a view Session, **When** o usuário pressiona `a`, `A` ou `+`, **Then** a view Include abre com o fluxo legado intacto.
2. **Given** a view Home, **When** o usuário digita `/add` e pressiona Enter, **Then** a view Include NÃO abre (o fluxo vai para o modo "insira o link:" do prompt).

---

## Technical Context

**Language/Version**: Rust 2021, toolchain stable

**Primary Dependencies**: `crossterm` (eventos, cor, cursor), `librqbit` 8.1.1 (`AddTorrent`, `Session`, `AddTorrentResponse`), `tokio` (async)

**Target Platform**: Linux terminal moderno (GNOME Terminal, Windows Terminal, etc.)

**Existing Code**: `src/session_ui.rs` — `Tui` (campos `input`, `prompt_cursor`, `view`), `handle_home_key` (US-031), `execute_command` (branch `/add` → `View::Include`), `submit_include` (validação `AddTorrent::from_cli_argument`, `PendingTorrent`, `add_torrent_background`), `render_home_prompt` (rótulo `> ` e placeholder). `View::Include` e membros `include_*`/`typing` permanecem (US-009/014).

## Design Overview

### Modo "insira o link:" no prompt da Home

- Novo campo de estado em `Tui`: `prompt_add_mode: bool` (ou equivalente). Quando ativo, o prompt da Home exibe o rótulo "insira o link:" e o texto digitado/colado é a origem a adicionar.
- `execute_command("/add")`: deixa de transicionar para `View::Include` quando o comando vem da Home/menu. Ativa o modo `prompt_add_mode`, limpa `input`, mantém `view = View::Home`.
- `handle_home_key` com `prompt_add_mode`: Enter → `submit_add_from_prompt()`; Esc → cancela o modo; caracteres → `insert_prompt_char`; Backspace → `backspace_prompt`; colar via `Event::Paste` insere no `input` da Home (reuso do fluxo US-031).
- `handle_home_key` Enter (modo normal): se `input` começa com `/add ` e tem origem após o espaço → delega para `submit_add_from_prompt()` com a origem extraída; se é exatamente `/add`/`/add ` → ativa `prompt_add_mode`; senão → `execute_command`.
- `submit_add_from_prompt`: nova versão do `submit_include` que NÃO troca de view — valida sintaxe com `AddTorrent::from_cli_argument` (preserva o prompt em erro), registra `PendingTorrent`, faz spawn de `add_torrent_background`, permanece na Home e posiciona `row_index` na nova entrada.
- `render_home_prompt`: quando `prompt_add_mode`, exibe "insira o link:" como rótulo; placeholder "Enter a command or / for options" é suprimido nesse modo.
- `footer_hints` para Home pode indicar o modo ativo (Esc cancelar).

### Inline /add

- Parse no Enter da Home: `input` trimmado que inicia com `/add ` (e tem conteúdo após o prefixo) é roteado para `submit_add_from_prompt`; `input` é limpo após a submissão.
- Validação síncrona reutiliza `AddTorrent::from_cli_argument` (mesma do `submit_include`).

### View Include preservada

- `View::Include`, `include_index`, `include_input`, `typing`, `handle_include_key`, `handle_include_typing_key`, `render_include`, `mouse_include` e `submit_include` permanecem intactos.
- Apenas o branch `/add` de `execute_command` muda (para a Home/menu). Os atalhos `a`/`A`/`+` da Session continuam abrindo `View::Include`.

### Código legado preservado

- `View::Session` e `View::Completed` não são afetados.
- `submit_include` permanece como caminho da Session (via Include).
