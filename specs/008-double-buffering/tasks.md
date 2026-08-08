# US-016 — Tarefas

## Concluídas

- [x] Implementar `Cell` e `Frame` (buffer de tela) com `put`, `set_cursor`,
      `diff_count` e `apply_diff`.
- [x] Reescrever `Tui::render` para desenhar no buffer e aplicar diff contra o
      frame anterior (AC-2).
- [x] Remover o `terminal::Clear(All)` contínuo do laço; manter clear apenas em
      primeiro frame, redimensionamento ou troca de view (AC-1).
- [x] Laço principal com orçamento de frame (~30 FPS) e redesenho sob demanda
      por eventos (AC-3/AC-4).
- [x] Cursor do terminal exibido apenas durante a digitação.
- [x] Testes unitários do diff-rendering (27 testes no total).
- [x] Pipeline local: fmt, clippy (-D warnings), test, machete — OK.
