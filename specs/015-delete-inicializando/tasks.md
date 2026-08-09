# US-027 — Tasks

## Implementação

- [x] `DeleteTarget` (Torrent/Pending) + campo `confirming_delete` no `Tui`.
- [x] `handle_session_key`: intercepta S/Y/N/Esc durante a confirmação.
- [x] `KeyCode::Delete` → `request_delete(row_index)`.
- [x] `request_delete` captura o alvo exato (id do torrent ou pendente).
- [x] `confirm_delete`: remove da sessão com `delete_files=true` ou descarta
      pendente; `clamp_row_index` (re-indexação).
- [x] Modal `render_confirm_delete` centralizado com overlay e bordas duplas.
- [x] Botão `Excluir` do mouse usa a mesma confirmação; pausar/parar em
      pendentes exibe aviso.
- [x] Pendentes `Resolving` renderizam coluna de ações (AC-1).
- [x] `remove_row` ('X') descarta pendentes; footer com `[Del] excluir`.
- [x] Mouse bloqueado durante a confirmação.

## Testes

- [x] `confirm_delete_modal_asks_yes_no`
- [x] `confirm_delete_modal_truncates_long_names`
- [x] `session_footer_hints_mention_delete_shortcut`
- [x] Suíte existente (47 testes) verde + clippy `-D warnings` + machete.

## Publicação

- [ ] Specs em `specs/015-delete-inicializando/`
- [ ] Commit + push da branch `feat/us-027-delete-inicializando`
- [ ] PR para `master` com CI verde e merge
- [ ] Build release assinado v0.1.12 + instalação + release no GitHub
