# US-020 — Tasks

## Implementação

- [x] `progress_bar(pct)`: blocos `█`/`░` + percentual adjacente (coluna de 31 chars)
- [x] `progress_color(state, finished)`: verde/amarelo/vermelho/ciano via paleta
- [x] `render_session`: colunas redefinidas (prefixo + barra + status + nome + ações) e sobreposição da barra com `put_styled` por estado
- [x] Linhas pendentes (US-014) com barra 0% + cor (ciano/vermelho)
- [x] `session_table_line`/`session_table_header` atualizados para a nova coluna (larguras idênticas)
- [x] Testes: 4 novos de barra/cor + tabela atualizada (40 no total)
- [x] Pipeline local: fmt, clippy (-D warnings), test (40), machete — OK

## Publicação

- [ ] Commit + push da branch `feat/us-020-progressbar`
- [ ] PR para master com vinculação da US
- [ ] Merge squash após CI verde
- [ ] Master: build release assinado v0.1.9 (bump automático)
- [ ] Instalar binário em `/usr/local/bin`
- [ ] Release v0.1.9 com binário + assinatura `.sig`
- [ ] Smoke test tmux da barra (blocos, cor por estado, percentual) + revisão final
