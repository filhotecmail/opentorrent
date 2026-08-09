# US-028 — Tasks

## Implementação

- [x] Helper `separator_line(width)` (linha `─` na largura da tabela).
- [x] `table_width` compartilhado (cabeçalho + separador do cabeçalho + divisores).
- [x] Data loop: substituir gap em branco por `separator_line(table_width)`.
- [x] Pending loop: idem.
- [x] Coletar índices dos divisores (`seps`) e sobrepor com `THEME.muted`.
- [x] Highlight cobre apenas a linha ativa (divisores nunca destacados).
- [x] Mouse/matemática de linhas preservadas (stride 2; cliques no divisor inertes).
- [x] Comentários e nomes de testes atualizados (gap → divisor fino).

## Testes

- [x] `separator_line_matches_table_width`
- [x] `session_row_line_accounts_for_thin_separator` (renomeado)
- [x] Suíte existente (49 testes) verde + clippy `-D warnings` + machete.

## Publicação

- [ ] Specs em `specs/016-separadores-tabela/`
- [ ] Commit + push da branch `feat/us-028-separadores-tabela`
- [ ] PR para `master` com CI verde e merge
- [ ] Build release assinado v0.1.15 + instalação + release no GitHub
