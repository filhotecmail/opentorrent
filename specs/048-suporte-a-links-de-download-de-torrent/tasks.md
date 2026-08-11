# Tasks: US-048 — Suporte a links de download de .torrent e drag & drop no input

**Input**: Design documents from `/specs/048-suporte-a-links-de-download-de-torrent/`

**Prerequisites**: plan.md (required), spec.md (required), research.md, data-model.md, contracts/README.md

**Tests**: Testes unitários/resolução são parte da US e OBRIGATÓRIOS (spec organization, artefatos do
Spec Kit). Gate: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`.

**Organization**: Tasks agrupadas: (1) módulo de resolução + testes; (2) integração nos 3 pontos de add;
(3) drag & drop; (4) validação final e ciclo release.

## Phase 1: Módulo de resolução (core)

**Purpose**: `src/resolve.rs` — pipeline download → validação bencode → extração de link `.torrent` do HTML

- [ ] T001 Adicionar `bytes = "1"` a `[dependencies]` (Cargo.toml) — já no lock (1.12.1) via reqwest/librqbit; custo de build zerado. Conferir `reqwest::Client` para download compartilhado.
- [ ] T002 Criar `src/resolve.rs` com `enum ResolvedSource<'a> { CliArgument(Cow<'a, str>), TorrentBytes(bytes::Bytes) }` e `ResolvedSource::to_add_torrent()` (data-model.md); registrar `pub mod resolve;` em main.rs.
- [ ] T003 [P] `async fn fetch_bytes(client: &reqwest::Client, url: &url::Url) -> anyhow::Result<bytes::Bytes>` — GET com `User-Agent: opentorrent`, `error_for_status`, `.bytes()`. Teste unitário com server HTTP local (`tokio` + `TcpListener` simples) ou `httpmock` se já disponível — senão teste via binário/regressão.
- [ ] T004 [P] Validação de payload: usar `librqbit::torrent_from_bytes_ext::<ByteBuf>(&bytes)` (reexportado) para decidir "é torrent". Testes: bytes bencode válido (ex.: pequeno .torrent de fixture gerado por `librqbit::create_torrent` ou bytes hard-coded) → `Ok`; HTML/imagem/garbage → `Err`.
- [ ] T005 [P] `fn extract_torrent_links(html: &str, base: &url::Url) -> Vec<url::Url>` — heurística sem parser HTML: coletar valores de atributos (`href="..."`, `window.location = "..."`, `location.href = "..."`), `url::Url::parse` + `.join(base)`, manter os que terminam em `.torrent`. Testes: HTML do fosstorrents (fixture), HTML com href relativo, sem link → vazio.
- [ ] T006 [P] `async fn download_for_torrent(client, url) -> anyhow::Result<Option<bytes::Bytes>>` — baixa; valida; se HTML → itera candidatos validando cada um (primeiro ok vence); retorna `None`/`Err` de domínio ("nenhum link .torrent encontrado na página" / "resposta não é um torrent").
- [ ] T007 [P] `pub async fn resolve_source<'a>(client, source: &'a str) -> anyhow::Result<ResolvedSource<'a>>` — roteia: http(s) → `download_for_torrent`; senão `CliArgument`. Testes: URL direta torrent, URL HTML com link, magnet/path sem I/O.

**Checkpoint**: `resolve.rs` compila e testes de resolução passam; `cargo check` limpo.

## Phase 2: Integração nos pontos de add

**Purpose**: TUI local, daemon e modo add usam `resolve_source` antes do `add_torrent`

- [ ] T008 `add_torrent_background` (session_ui.rs:2916): criar/obter `reqwest::Client`, chamar `resolve_source` no modo local e no daemon (daemon recebe a origem já resolvida como bytes? — decisão: envio da origem original ao daemon, que resolve internamente para manter fonte da verdade e dedup). Se modo local → `ResolvedSource::to_add_torrent()` → `session.add_torrent`; daemon → manter `DaemonRequest::Add { source }` (a resolução ocorre no daemon, T009).
- [ ] T009 `daemon.rs::dispatch` (`DaemonRequest::Add`): chamar `resolve_source` ANTES de `session.add_torrent`; falha de resolução → `DaemonResponse::Error { message }` (contexto de domínio). Teste de integração (RUN_SLOW_TESTS): dispatch Add com URL que responde HTML→torrent adiciona; com HTML sem link → Error.
- [ ] T010 `main.rs` (modo add): no fluxo `run_add`, para cada origem http(s) chamar `resolve_source` e usar o resultado em vez de `AddTorrent::from_cli_argument` direto (probe `list_only` e download). Regressão manual q\doc quickstart.

**Checkpoint**: os 3 fluxos resolvem URL de download; `cargo test` + `cargo clippy -- -D warnings` limpos.

## Phase 3: Drag & drop (paste de caminho .torrent)

**Purpose**: garantir que caminho colado/arrastado de `.torrent` entra na lista

- [ ] T011 Confirmar fluxo `Event::Paste` (session_ui.rs:678) já insere o caminho no `self.input`; `submit_add_from_prompt` → `resolve_source` → `CliArgument` → `from_cli_argument` (lê o arquivo). Adicionar teste unitário de `parse_add_command`/`submit_add_from_prompt` com caminho `.torrent` (fixture temporária em `temp_dir`).
- [ ] T012 Teste manual (quickstart): arrastar arquivo `.torrent` na TUI (GNOME Terminal) → Enter → entra na lista; caminho inexistente → erro claro.

**Checkpoint**: drag & drop validado.

## Phase 4: Validação final e ciclo release

- [ ] T013 Rodar gate completo: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`.
- [ ] T014 Regressão canônica (`--list` fosstorrents) e modo CLI add sem `--list`.
- [ ] T015 Via `scripts/create-new-us.sh execute-pipeline-gh "feat: suporte a links de download .torrent e drag & drop (US-048)"`: validação → commit → push → PR "Closes #103" → CI/CD → release (build.sh) → instalar para o usuário testar.

## Checkpoints finais

- `cargo check` os 3 pontos integram `resolve_source`;
- gate verde (fmt/clippy/test/machete);
- release publicado e binário instalado para teste do usuário.