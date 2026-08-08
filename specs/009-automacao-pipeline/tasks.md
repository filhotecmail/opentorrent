# US-017 — Tarefas

## Concluídas

- [x] Criar `scripts/us-pipeline.sh` com os subcomandos `start`, `finish` e
      `state`.
- [x] `start`: sincroniza a branch principal, cria a branch de trabalho a partir
      dela e registra a issue (com label US + milestone) (AC-1/AC-2).
- [x] `finish`: valida o pipeline (fmt/clippy/test/machete), commit, push e PR
      com `closes #N` vinculando a issue (AC-3/AC-4/AC-5).
- [x] Fechamento automático da issue via `closes #N` no corpo do PR (AC-6).
- [x] Documentação da US (specs/009).
