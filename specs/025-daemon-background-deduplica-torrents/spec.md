# Feature Specification: US-040 — Daemon em background e deduplicação de torrents

**Feature Branch**: `feat/us-040-suporte-a-daemon-em-background-e-dedupli`

**Created**: 2026-08-10

**Status**: Draft

**Input**: User description: "Suporte a daemon: quero que os downloads continuem em background mesmo com a aplicação fechada; ao reabrir, a lista/sessão mostra o progresso atualizado. E dedup: se adicionar na lista um torrent que já existe na lista (de processamento ou processado) mas que esteja na lista, nada deve fazer, a menos que o usuário tenha excluído ele da lista."

## Clarifications (Speckit Clarify)

1. **Modelo**: A — daemon de background (processo único continua baixando com a TUI fechada; a TUI reconecta e renderiza ao vivo).
2. **Comunicação**: B — daemon expõe o estado via socket Unix + JSON; a TUI conecta e renderiza ao vivo.
3. **Dedup**: A — identidade por infohash (re-adiciona apenas se o torrent foi excluído da lista).
4. **Excluir da lista (X/Delete)**: A — remove da lista mantendo os arquivos no disco (re-adicionar retoma o download).
5. **Início do daemon**: A — a TUI inicia o daemon automaticamente quando ele não está rodando (transparente).
6. **Service manager (follow-up US-040)**: A — o daemon é instalado como **systemd user service** (`~/.config/systemd/user/opentorrent-daemon.service`), criado automaticamente pelo binário na primeira execução (instalação) se não existir; `start`/`stop`/`status` delegam ao `systemctl --user`.
7. **Ciclo de atualização (follow-up US-040)**: A — novo comando `opentorrent update`: para o service, baixa/substitui o binário da release GitHub (validando a assinatura US-018 quando disponível), reescreve o service se necessário e religa o daemon.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Downloads continuam com a TUI fechada (Priority: P1)

O usuário inicia o OpenTorrent (sem subcomando) e adiciona um torrent. Ao fechar a aplicação (Ctrl+C ou `/exit`), os downloads continuam em background. Ao reabrir o OpenTorrent, a Biblioteca mostra a sessão persistida com o progresso atualizado do que ocorreu enquanto a TUI esteve fechada.

**Why this priority**: É o núcleo da feature — o daemon desacopla o download da presença da TUI.

**Independent Test**: Abrir o cliente, adicionar um magnet, fechar com Ctrl+C, aguardar ~10s, reabrir → o mesmo torrent aparece na Biblioteca com progresso maior que no fechamento (o daemon continuou baixando).

**Acceptance Scenarios**:
1. **Given** o daemon em execução e a TUI fechada, **When** o usuário reabre o OpenTorrent, **Then** a TUI se conecta ao daemon existente e mostra a sessão atual (sem duplicar o processo).
2. **Given** o daemon em execução, **When** o usuário fecha a TUI com `/exit` ou Ctrl+C, **Then** apenas a TUI encerra; o daemon permanece baixando.
3. **Given** um torrent em download e o daemon em execução, **When** o tempo passa sem TUI aberta, **Then** o download progride (verificável ao reconectar).
4. **Given** o daemon anteriormente em execução, **When** o usuário reinicia a máquina, **Then** ao reabrir o OpenTorrent o daemon é iniciado novamente e a sessão persistida é retomada (fastresume).

### User Story 2 - Deduplicação por infohash (Priority: P1)

O usuário adiciona um torrent já presente na lista (processando ou já processado/completo). Nada acontece: nenhuma duplicação, apenas um aviso de que já está na lista. Se o torrent foi excluído da lista (mantendo arquivos), pode ser re-adicionado.

**Why this priority**: Requisito explícito; usa a identidade canônica (infohash) que já existe no librqbit.

**Independent Test**: Com um torrent ativo, executar `/add` com o mesmo magnet (ou outro magnet do mesmo infohash, ou o mesmo `.torrent` via URL) → mensagem "já está na lista" e nenhuma linha nova.

**Acceptance Scenarios**:
1. **Given** um torrent processando na lista, **When** o usuário adiciona a mesma origem, **Then** a mensagem `já está na lista` é exibida e nenhuma linha duplicada aparece.
2. **Given** um torrent completo (processado) na lista, **When** o usuário adiciona a mesma origem, **Then** nada acontece (mesmo fluxo de já gerenciado), preservando o estado na lista.
3. **Given** um torrent excluído da lista (X/Delete, arquivos mantidos no disco), **When** o usuário o adiciona novamente, **Then** ele é re-adicionado e o download retoma (smart resume).
4. **Given** duas origens diferentes (ex.: magnet e `.torrent`) com o **mesmo infohash**, **When** a segunda é adicionada, **Then** ela é tratada como candidata a duplicata (já gerenciada).

### User Story 3 - Daemon inicia automaticamente (Priority: P1)

Ao abrir o OpenTorrent sem subcomando, se o daemon não estiver rodando, ele é iniciado automaticamente em background e a TUI conecta. Nenhum passo manual é necessário.

**Why this priority**: Decisão de clarificação A — experiência transparente.

**Independent Test**: Com o daemon parado, executar o binário sem flags → a TUI abre e a sessão funciona; verificar que há um processo `opentorrent` em execução em background após a TUI ser aberta.

**Acceptance Scenarios**:
1. **Given** o daemon parado, **When** o usuário abre o OpenTorrent, **Then** o daemon é iniciado (processo background) e a TUI conecta automaticamente.
2. **Given** o daemon já rodando, **When** o usuário abre uma segunda TUI, **Then** a TUI conecta ao daemon existente (sem iniciar um segundo daemon).
3. **Given** o daemon em execução, **When** o usuário roda `opentorrent <torrent>` (modo add), **Then** o torrent é delegado ao daemon (dedup por infohash aplicado) ou o modo add atual continua funcionando sem conflito.

### User Story 4 - Excluir da lista mantém arquivos (Priority: P2)

A ação de excluir (X/Delete) remove o torrent da lista mantendo os arquivos no disco. O torrent excluído não é mais considerado "na lista" para fins de dedup (pode ser re-adicionado).

**Why this priority**: Complementa o dedup — define quando a re-adição é permitida.

**Acceptance Scenarios**:
1. **Given** um torrent na lista, **When** o usuário exclui com X/Delete (sem apagar arquivos), **Then** ele some da lista e os arquivos permanecem no diretório de saída.
2. **Given** um torrent excluído da lista, **When** o usuário o adiciona novamente, **Then** o smart resume detecta os arquivos existentes e retoma o download.

### User Story 5 - Daemon instalado como systemd user service (Priority: P1)

Ao instalar a aplicação (primeira execução do binário na máquina — TUI, modo add, ou `daemon start`), o systemd **user service** é criado automaticamente caso não exista: `~/.config/systemd/user/opentorrent-daemon.service` apontando para o binário atual com `--daemon-headless`, seguido de `systemctl --user daemon-reload` + `enable`. O daemon passa a ser gerenciado pelo systemd, e `start`/`stop`/`status` delegam ao `systemctl --user` (com fallback para o spawn desanexado atual quando o systemd não estiver disponível).

**Why this priority**: Requisito explícito do usuário — "ao instalar a aplicação, criar automaticamente o Daemon service caso não tenha sido criado".

**Independent Test**: Com o service ausente, executar `opentorrent daemon status`/TUI → o unit `opentorrent-daemon.service` é criado e `systemctl --user is-active opentorrent-daemon` reflete o estado.

**Acceptance Scenarios**:
1. **Given** o service inexistente, **When** o usuário executa qualquer comando do binário (primeira execução), **Then** o unit systemd user é escrito, `daemon-reload` executado e o service habilitado.
2. **Given** o service já instalado, **When** o usuário executa novamente, **Then** nada é reescrito sem necessidade (idempotente; reescreve apenas se o caminho do binário mudou).
3. **Given** o service instalado, **When** o usuário roda `opentorrent daemon start`, **Then** o systemd inicia o daemon (`systemctl --user start`).
4. **Given** o service instalado, **When** o usuário roda `opentorrent daemon stop`, **Then** o systemd encerra o daemon (`systemctl --user stop`).
5. **Given** o systemd indisponível (não-Linux/user sem systemd), **When** o usuário roda `daemon start`, **Then** o fallback atual (spawn desanexado) é usado, sem erro.

### User Story 6 - Atualização automática para/atualiza/religa (Priority: P1)

O comando `opentorrent update` consulta a release mais recente no GitHub (mesmo fluxo do `--version`, US-029), para o service do daemon se estiver rodando, baixa o novo binário, valida a assinatura digital quando a CA local existir (US-018), substitui o binário, reescreve o service se o caminho mudou, e religa o daemon. O `--version` passa a sugerir `opentorrent update` em vez de curl manual.

**Why this priority**: Requisito explícito do usuário — "em caso de atualização, deve parar o daemon, atualizar o daemon service e subir novamente".

**Independent Test**: Com uma versão antiga instalada e uma release nova disponível, executar `opentorrent update` → service parado, binário substituído, `opentorrent --version` mostra a nova versão, service religado.

**Acceptance Scenarios**:
1. **Given** uma release mais recente disponível, **When** o usuário roda `opentorrent update`, **Then** o download é feito, a assinatura validada (se CA presente), o binário substituído e a versão atualizada reportada.
2. **Given** o daemon rodando via service, **When** o comando update é executado, **Then** o service é parado antes da substituição e religado depois (sem processo órfão).
3. **Given** o update baixa um binário inválido (verificação de assinatura habilitada e falha), **Then** a substituição é abortada e o binário/versão atuais são preservados.
4. **Given** já na versão mais recente, **When** o usuário roda `opentorrent update`, **Then** a mensagem indica que está atualizado e nada é substituído.
5. **Given** falha de rede durante o download, **Then** `update` aborta graciosamente e mantém o binário atual intacto.

## Technical Context

**Language/Version**: Rust 2021, toolchain stable

**Primary Dependencies**: `librqbit` 8.1.1 (Session, `AddTorrent`, `AddTorrentResponse`, `TorrentStatsState`, `SessionPersistenceConfig`), `tokio`, `crossterm`, `clap`, `anyhow`

**Target Platform**: Linux (Unix sockets); manter compatibilidade de build cross-platform (Windows) sem exigir o daemon

**Existing Code**:
- `src/main.rs` (883 linhas) — `Opts`/`SubCommand::Add`, `run_interactive()` cria `Session` com `SessionPersistenceConfig::Json` em `~/.config/opentorrent` e delega a `session_ui::run_interactive`. `run_add()` gerencia o modo aditivo (CLI de um shot), incluindo o tratamento `AddTorrentResponse::AlreadyManaged` (linhas 479-486).
- `src/session_ui.rs` (3302 linhas) — TUI. `View` enum (Home, Menu), `Tui` struct com `session: Arc<Session>`, loop de eventos com `FRAME_BUDGET` (~30 FPS), `add_torrent_background` (linha 2101) que retorna `AddOutcome::{Added, AlreadyManaged}`, `PendingTorrent`/`PendingStatus`, ações `HomeAction`/`ContextAction` (Pause/Resume/Remove/Delete), `run_interactive()` (linha 2218) — entrypoint da TUI que assume a sessão pronta.
- `src/downloads.rs` — `CompletedItem`, `list_completed_downloads()` (histórico de completos por varredura do diretório).
- `src/metadata.rs` — `project_note()` (rígido embutido no binário).

## Design Overview

### Arquitetura: processo único do daemon + IPC por socket Unix

O binário `opentorrent` passa a suportar três modos:

1. **Modo daemon (background)** — processo sem TUI que mantém a `Session` do librqbit viva e escuta um socket Unix (`~/.config/opentorrent/daemon.sock`) em JSON. Ao conectar um cliente (TUI ou comando add), o daemon continua gerenciando a sessão e responde requisições (adicionar, pausar, retomar, excluir, estado da fila).
2. **Modo TUI (interativo)** — abre a interface atual. Na entrada, verifica se o daemon está rodando (socket existente/ativo). Se não, inicia o daemon (processo filho `opentorrent daemon`, desanexado) e conecta a ele via IPC.
3. **Modo add (CLI de um shot)** — se o daemon estiver rodando, delega a adição a ele via IPC (retorna o resultado e o dedup é aplicado no lado do daemon); se não estiver rodando, mantém o comportamento atual.

A **fonte da verdade é a `Session` dentro do daemon** (com `SessionPersistenceConfig::Json`). A persistência já existente (`~/.config/opentorrent`) garante a retomada/carregamento da sessão ao iniciar o daemon. A TUI se torna um cliente read/write do estado do daemon, renderizando a partir do snapshot JSON recebido em vez de consultar a `Session` diretamente.

### Componentes (módulos novos)

- **`src/daemon.rs`** — ciclo de vida do daemon: instalação do systemd user service (`~/.config/systemd/user/opentorrent-daemon.service`), `start`/`stop`/`status` delegando ao `systemctl --user` (com fallback para spawn desanexado), detecção do socket, pidfile (`~/.config/opentorrent/daemon.pid`).
- **`src/ipc.rs`** — protocolo de comunicação: tipos de mensagens (request/response) em JSON sobre socket Unix; enums `DaemonRequest` (Add, Pause, Resume, Delete, RemoveFromList, GetState), `DaemonResponse` (State/Ok/AlreadyManaged/Error), snapshot serializável da fila (id, infohash, nome, estado, bytes, velocidades) para a TUI renderizar.
- **`src/update.rs`** — comando `update`: verifica release GitHub, para o service, baixa o binário, valida assinatura (US-018), substitui, religa o service.
- **`src/session_ui.rs`** — a TUI passa a consumir o snapshot via IPC em vez de `session.with_torrents`. Ações da TUI viram requisições ao daemon.
- **`src/main.rs`** — despacho dos modos: `daemon` (subcomando novo), `update` (subcomando novo), `add` (delegação opcional), interativo (auto-start do daemon).

### Deduplicação

O librqbit já retorna `AddTorrentResponse::AlreadyManaged(id, handle)` para um infohash já presente na sessão. Como a sessão vive no daemon, **todo** `add` passa por ele e o dedup por infohash é automático e consistente (magnet/URL/arquivo → mesmo infohash). A exclusão da lista remove o torrent da sessão (`session.delete(id, false)`), liberando o infohash para re-adição; os arquivos permanecem no disco (smart resume trata a retomada).

### Limitações cross-platform

O daemon usa socket Unix (Linux/macOS) e systemd user service (Linux). No Windows, sem systemd: o `daemon start` usa o fallback de spawn desanexado atual, e o IPC pode usar named pipes ou o daemon pode ser desabilitado com fallback para o comportamento atual (sessão em processo da TUI, que encerra ao fechar). O escopo desta US prioriza Linux; a porta Windows completa entra como follow-up. A compilação cross-platform deve ser preservada (`cfg(target_os)`).

### Cybersecurity / Segurança

- O socket Unix deve ter permissões restritas ao usuário (`0o700` no diretório; socket com permissões padrão do daemon) para evitar que outros usuários controlem a sessão.
- Não confiar no conteúdo do JSON como origem de caminhos arbitrarios — apenas com validações existentes (output_folder já validado pelo usuário autenticado/owner).
- Mensagens de erro do daemon propagadas à TUI em JSON (sem expor stack traces internos).
- A atualização (`update`) valida a assinatura digital do binário baixado quando a CA local da US-018 existir, abortando a substituição em caso de falha de verificação.
- O unit systemd é escrito em `~/.config/systemd/user/` (por usuário, sem sudo), com `ExecStart` apontando para o binário do próprio usuário.

### Rust Skills Check

Skills consultadas para o plano: `domain-cli`, `m07-concurrency` (tokio/Send-Sync/await), `m12-lifecycle` (ciclo de vida do daemon, RAII para socket/guards), `m01-ownership` (Arc<Session> compartilhado), `m06-error-handling` (anyhow + respostas de erro JSON), `m11-ecosystem` (Cargo deps: `serde_json`, `tokio::net::UnixListener`), `m12-lifecycle`/`m02-resource` (gestão do subprocesso systemctl no `update`, substituição atômica do binário).

## Out of Scope

- Interface web/móvel para o daemon.
- Enfileiramento/agendamento de múltiplos downloads paralelos controlados pela TUI (manter o comportamento atual do librqbit).
- Múltiplos usuários/contas (o daemon é por usuário do SO, via diretório de config).
- Porta completa do daemon no Windows (follow-up).
- `loginctl enable-linger` automático (daemon em user session sobrevive apenas durante a sessão de login; linger fica documentado no quickstart como passo manual opcional).