# Feature Specification: US-033 — Métricas na Biblioteca, histórico tabular e /add 100% via prompt

**Feature Branch**: `feat/us-033-metricas-biblioteca-add-prompt`

**Created**: 2026-08-09

**Status**: Draft

**Input**: User description: "US-033 — exibir métricas em tempo real na Biblioteca (DOWN SPEED, UP SPEED, ETA, RATIO), transformar o Histórico em tabela com ações e eliminar a tela secundária de adição (View::Include), fazendo o /add acontecer 100% pelo prompt fixo da base"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Métricas em tempo real na Biblioteca (Priority: P1)

O usuário acompanha a Biblioteca de downloads da Home e vê, além da barra de progresso e do status, as métricas de rede em tempo real: velocidade de download, velocidade de upload, tempo restante estimado (ETA) e razão de share (RATIO = enviado / baixado).

**Why this priority**: É o coração da US — métricas de rede são o valor principal que o usuário busca ao monitorar downloads ativos.

**Independent Test**: Renderizar a Biblioteca com pelo menos um torrent em andamento e verificar a presença das colunas DOWN SPEED, UP SPEED, ETA e RATIO com valores formatados.

**Acceptance Scenarios**:
1. **Given** um torrent em andamento na sessão, **When** a Biblioteca é renderizada, **Then** cada linha exibe DOWN SPEED (ex.: `12.4 MiB/s`), UP SPEED (ex.: `1.2 MiB/s`), ETA (ex.: `00:04:12`) e RATIO (ex.: `1.45`).
2. **Given** um torrent pausado ou inicializando, **When** a linha é renderizada, **Then** as colunas de velocidade/ETA exibem placeholder (`—`) em vez de valores falsos.
3. **Given** um torrent concluído (100%), **When** a linha é renderizada, **Then** ETA exibe `Done` e as velocidades voltam a `—`.
4. **Given** um torrent com upload e nenhum download concluído, **When** RATIO é calculado, **Then** exibe `0.00` (sem divisão por zero).

### User Story 2 - Histórico de downloads tabular com ações (Priority: P1)

O usuário abre o painel de Histórico e encontra uma tabela estruturada com as colunas ID, NOME, TAMANHO, RATIO, DATA CONCLUSÃO e AÇÕES — com a possibilidade de excluir um download concluído diretamente pelo painel.

**Why this priority**: O Histórico deixa de ser uma lista de texto crua e passa a permitir gerenciamento (exclusão) sem sair da Home.

**Independent Test**: Renderizar o Histórico com downloads concluídos e verificar cabeçalho tabular com as seis colunas e o botão `[Excluir]` na linha selecionada.

**Acceptance Scenarios**:
1. **Given** que existem downloads concluídos no diretório de saída, **When** o painel de Histórico é renderizado, **Then** cada linha exibe ID, NOME, TAMANHO (ex.: `700.00MiB`), RATIO (ou `—` quando a sessão não tem registro), DATA CONCLUSÃO (`%d/%m/%Y %H:%M`) e a coluna AÇÕES.
2. **Given** uma linha selecionada no Histórico, **When** o usuário aciona a ação Excluir, **Then** o sistema pede confirmação e, confirmado, remove o arquivo do disco e da lista.
3. **Given** um histórico sem downloads, **When** o painel é renderizado, **Then** exibe a mensagem "nenhum download completo".

### User Story 3 - /add 100% via prompt fixo da base (Priority: P1)

O usuário adiciona torrents sem sair da Home: selecionar `/add` no menu flutuante preenche o prompt com `/add ` e ativa o cursor; digitando a origem (magnet, `.torrent` local ou URL) e pressionando Enter a adição acontece em segundo plano, com a interface permanecendo na Home.

**Why this priority**: Elimina a tela secundária legada (View::Include), unificando a adição no prompt — menos contextos, menos código morto.

**Independent Test**: Na Home, digitar `/add`, selecionar Enter, ver o prompt preenchido com `/add `, colar um magnet, Enter → permanecer na Home com a linha "inicializando" na Biblioteca e notice de sucesso/falha.

**Acceptance Scenarios**:
1. **Given** o menu flutuante de comandos aberto, **When** o usuário navega até `/add` e pressiona Enter, **Then** o prompt fixo é preenchido com `/add ` e o cursor permanece ativo na Home (sem mudar de tela).
2. **Given** o prompt com `/add <origem>` e Enter pressionado, **When** a origem é válida, **Then** a adição roda em background e a Home permanece visível, com a linha da origem aparecendo imediatamente na Biblioteca como "inicializando".
3. **Given** o prompt com `/add` (sem origem) e Enter pressionado, **Then** o sistema exibe notice pedindo a origem (não navega).
4. **Given** o prompt com `/add <origem inválida>` e Enter pressionado, **When** a validação de sintaxe falha, **Then** o prompt é preservado com a origem digitada e um notice de erro é exibido.
5. **Given** qualquer fluxo de adição, **Then** `View::Include` não existe mais na enumeração de telas.

---

## Technical Context

**Language/Version**: Rust 2021, toolchain stable

**Primary Dependencies**: `crossterm` (eventos, cor, cursor), `librqbit` 8.1.1 (stats de rede: `LiveStats`, `Speed`, `TorrentStats`), `tokio` (async), `size_format` (tamanhos), `chrono` (data de conclusão)

**Target Platform**: Linux terminal moderno (GNOME Terminal, Windows Terminal, etc.)

**Existing Code**: `src/session_ui.rs` — `Tui`, `View` (Home/Menu/Session/Include/Completed), `SessionRow` (id/name/state/progress/total/finished/handle), `session_table_header`/`session_table_line`, `render_history_panel` (lista texto crua via `format_completed`), `submit_include` (tela secundária `View::Include`), `render_home_prompt` (prompt fixo), `request_delete`/`confirm_delete` (US-027). `src/downloads.rs` — `CompletedItem`, `list_completed_downloads`, `format_completed`.

## Design Overview

### Métricas na Biblioteca

- `SessionRow` ganha campos: `down_speed_mbps: Option<f64>`, `up_speed_mbps: Option<f64>`, `eta_seconds: Option<u64>`, `uploaded_bytes: u64`.
- Fonte dos dados: `handle.stats()` → `stats.live` (`Option<LiveStats>`). Quando `live` é `Some` e `finished` é `false`: `download_speed.mbps`, `upload_speed.mbps` e ETA calculado como `(total_bytes - progress_bytes) / (down_speed_mbps * 1024 * 1024)`. Quando `live` é `None` (pausado/inicializando) ou `finished` é `true`: placeholders `—`/`Done`.
- RATIO = `uploaded_bytes / progress_bytes` (ou `/ total_bytes` quando finished), com guarda contra divisão por zero → `0.00`.
- Formatação: velocidade `{:.1} MiB/s` (ex.: `12.4 MiB/s`), ETA `HH:MM:SS` (ex.: `00:04:12`), RATIO `{:.2}`.
- Tabela: `session_table_header`/`session_table_line` ganham parâmetros de métricas, mantendo o alinhamento de colunas já testado (US-019/020).

### Histórico tabular

- `render_history_panel` deixa de usar `format_completed` e passa a montar tabela com `history_table_header`/`history_table_line`.
- Colunas: ID (índice), NOME, TAMANHO (`SF::new(size)`), RATIO (da sessão quando o torrent correspondente está finished e ainda na sessão; senão `—`), DATA CONCLUSÃO (`modified.format("%d/%m/%Y %H:%M")`), AÇÕES.
- Exclusão: reutiliza o fluxo US-027 (`request_delete`/`confirm_delete`) com um novo alvo `DeleteTarget::File { path, name }` que, confirmado, executa `std::fs::remove_file` e atualiza a lista.

### /add via prompt (remoção de View::Include)

- `execute_command("/add")` deixa de transicionar para `View::Include`: preenche `self.input = "/add "` (com espaço, para digitação contínua), `prompt_cursor = 5`, `view = View::Home`, e fica no modo de digitação do prompt.
- `handle_home_key` Enter: se o prompt começa com `/add` e tem conteúdo após o espaço, delega para `submit_add_from_prompt` (nova versão assíncrona do `submit_include` que NÃO troca de view); se só `/add`/`/add `, exibe notice pedindo origem.
- `submit_add_from_prompt`: valida sintaxe com `AddTorrent::from_cli_argument` (preserva o prompt em erro), registra `PendingTorrent`, faz spawn de `add_torrent_background` e mantém `view = View::Home`, posicionando o cursor na última linha (nova entrada) e `row_index` na origem pendente.
- Remoção completa de `View::Include` e de todos os membros/funções órfãos: `include_index`, `include_input`, `typing`, `handle_include_key`, `handle_include_typing_key`, `render_include`, `render_input_cursor`, `mouse_include`, `submit_include` e os branches de `view_label`/`footer_hints`/`handle_key`/`render`.
- Ações equivalentes dos atalhos (`a`/`+` em Session) passam a preencher o prompt com `/add ` e retornar para a Home.

### Código legado preservado

- `View::Session` e `View::Completed` permanecem como telas de acompanhamento detalhado (US-009/010), agora com métricas também na sessão (reuso de `session_table_line`).
