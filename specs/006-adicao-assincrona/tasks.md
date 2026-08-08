---

description: "Task list for US-014 - Adição assíncrona de torrents"

# Tasks: US-014 (Adição assíncrona com transição imediata)

## Fase 1: Estado de pendências

- [x] T001 Criar `PendingStatus` (Resolving / Error / Done) e `PendingTorrent`
- [x] T002 Campo `pending: Arc<Mutex<Vec<PendingTorrent>>>` no `Tui`

## Fase 2: Submissão não bloqueante

- [x] T003 `submit_include`: validação síncrona apenas (sintaxe)
- [x] T004 Registra `PendingTorrent::Resolving` e transiciona para `View::Session` imediatamente
- [x] T005 Limpa `include_input` e posiciona a seleção na nova linha

## Fase 3: Task em background

- [x] T006 `add_torrent_background(session, source)`: `session.add_torrent(add, None)` fora da UI
- [x] T007 Update do status: Added → remove pendência; AlreadyManaged → Done; erro → Error
- [x] T008 Nenhum lock atravessa `.await` (lock adquirido após o await)

## Fase 4: Renderização e navegação

- [x] T009 Renderizar linhas pendentes (inicializando / erro — msg) com alinhamento dos botões
- [x] T010 `drain_pending_notices` consome `Done` como notice no render
- [x] T011 `move_row` considera sessão + pendências
- [x] T012 Highlight e `Layout.row_count` incluem pendências

## Fase 5: Validação

- [x] T013 Remoção do `add_included` bloqueante (probe list_only)
- [x] T014 Pipeline local: fmt, clippy -D warnings, test (16), machete
- [x] T015 CI remoto: fmt, clippy, test, machete, cobertura
