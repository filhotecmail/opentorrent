# Requisitos Checklist: US-040 — Daemon em background e deduplicação de torrents

**Purpose**: Checklist de qualidade de requisitos — verificação de completude, testabilidade e consistência da US-040.
**Created**: 2026-08-10
**Feature**: [spec.md](../../../specs/025-daemon-background-deduplica-torrents/spec.md)

## User Stories (completude)

- [ ] CHK001 US-1 (P1): download continua em background com TUI fechada — comportamento definido nos 4 Acceptance Scenarios.
- [ ] CHK002 US-2 (P1): dedup por infohash — 4 cenários cobrem: processando, completo, excluído, origens diferentes/mesmo infohash.
- [ ] CHK003 US-3 (P1): auto-start do daemon — 3 cenários cobrem: parado, já rodando, modo add.
- [ ] CHK004 US-4 (P2): excluir mantém arquivos + readição retoma — 2 cenários.
- [ ] CHK005 Prioridades coerentes: P1 = núcleo (daemon/dedup/auto-start), P2 = absorve exclusão (complementar ao dedup).

## Rastreabilidade (spec ↔ plano ↔ contratos ↔ tarefas)

- [ ] CHK006 Cada Acceptance Scenario da spec tem contrato no `contracts/README.md`.
- [ ] CHK007 Cada contrato tem implementação mapeada em `tasks.md` (T004-T018).
- [ ] CHK008 `data-model.md` define todos os tipos do IPC (`DaemonRequest`/`DaemonResponse`/`DaemonState`/`TorrentSnapshot`).
- [ ] CHK009 `research.md` registra: gap do daemon (main.rs:377), dedup nativo (`AlreadyManaged`), IPC (tokio `UnixListener`), riscos+mmitigações.
- [ ] CHK010 `quickstart.md` permite reproduzir manualmente as 4 user stories (passos 1-7).

## Testabilidade

- [ ] CHK011 Round-trip JSON de `DaemonRequest`/`DaemonResponse` tem teste unitário (T003).
- [ ] CHK012 Dedup por infohash testável (T013) com 3 origens → mesmo infohash → `AlreadyManaged`.
- [ ] CHK013 Snapshot→linhas da tabela reutiliza funções puras existentes (`session_table_line`), cobertas por unit tests.
- [ ] CHK014 Smart resume retoma arquivos parciais após exclusão (T018) — verificação manual/`RUN_SLOW_TESTS`.
- [ ] CHK015 Não há locks `Mutex` guard cruzando `.await` no daemon (async-02, audível por review).

## Consistência técnica

- [ ] CHK016 `serde`/`serde_json` presentes; `tokio` recebe `net`+`io-util` (T001) — sem deps novas pesadas.
- [ ] CHK017 Socket/pidfile sob `~/.config/opentorrent` com permissão `0o700` (segurança, spec).
- [ ] CHK018 Socket stale e duplo daemon mitigados (T004/T014).
- [ ] CHK019 Windows preserva build e comportamento atual (`cfg(target_os = "windows")`), daemon como follow-up.
- [ ] CHK020 Erros do daemon propagados sem stack trace (contrato `Error`, pt-BR).

## Qualidade / Gates

- [ ] CHK021 `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` verdes (T019-T020).
- [ ] CHK022 Constituição IV (Rust-Skills) e VIII (Build Velocity) atendidas sem violações documentadas.
- [ ] CHK023 Nenhum `.unwrap()`/`.expect()` em caminhos produtivos novos (regra anti-01/02) — exceto `expect` para invariantes com msg (err-06).
- [ ] CHK024 PR para master com "Closes #96" via `create-new-us.sh execute-pipeline-gh`.

## Notes

- Check items off as completed: `[x]`
- US-4 é requisito do usuário original ("depois de excluir da lista"), não puramente dedup — manter como P2 mas **obrigatório** no escopo.
- A decisão de "o que é a lista" (sessão librqbit vs lista persistida distinta) está registrada em plan.md (Open Questions) — se divergir, atualizar spec/data-model ANTES da implementação.