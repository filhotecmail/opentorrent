# US-016 — Redução de cintilação e renderização com Double Buffering

## Descrição

A interface interativa do terminal deve renderizar as atualizações de forma
estável, sem piscadas ou cintilações contínuas, com redesenho baseado em
diferenças (diff-rendering) e taxa de atualização controlada.

## Critérios de aceite

- **AC-1** — Eliminar chamadas contínuas de limpeza total do terminal (`clear`)
  dentro do laço principal de renderização.
- **AC-2** — Implementar renderização com buffer duplo (double buffering /
  diff-rendering): redesenhar apenas os caracteres/linhas que mudaram de estado.
- **AC-3** — O laço visual deve ter taxa de atualização controlada (frame rate
  limit, ~30 FPS) em vez de loop infinito desimpedido.
- **AC-4** — Eventos de teclado/mouse acionam redesenho sob demanda
  (event-driven redraw), sem forçar o ciclo contínuo de redesenho total.

## Implementação

### `struct Cell` + `struct Frame` (buffer de tela)

- `Cell { ch, cyan }` — um caractere + flag de destaque (ciano).
- `Frame` — grade `cols × rows` de células + posição opcional do cursor do
  terminal.
  - `put(row, col, text, cyan)` — desenha texto no buffer, ignorando o que
    cair fora dos limites.
  - `set_cursor(row, col)` — registra onde exibir o cursor do terminal
    (campo de entrada do include).
  - `diff_count(prev)` — quantas células diferem do frame anterior.
  - `apply_diff(w, prev)` — emite para o terminal apenas as células alteradas,
    agrupadas em runs contíguos na mesma linha (menos escapes).

### `Tui::render` (double buffering)

1. Desenha o estado atual da view em um `Frame` novo (nunca diretamente no
   terminal).
2. `clear` total **apenas** quando: primeiro frame, redimensionamento ou quando
   mais da metade das células mudou (troca de view). Nunca a cada iteração.
3. Caso contrário, `apply_diff` reescreve somente as células alteradas — o
   texto estático (bordas do quadro, menu, título) permanece imóvel.
4. O cursor do terminal é exibido apenas durante a digitação (view Include).

### Laço principal (`run_interactive`)

- Orçamento de frame de 33 ms (~30 FPS).
- O laço **aguarda** um evento de entrada com timeout até o fim do orçamento;
  não há loop de redesenho desimpedido.
- Redesenha apenas quando: houve evento relevante (tecla, paste, clique) **ou**
  o orçamento expirou (para atualizar progresso em tempo real).
- Movimentos de mouse não marcam "dirty" — evitam floods de render.

## Cenários de teste

### Estabilidade visual em tela estática de menu

Dado o menu principal sem interação, quando a tela permanece exibida, então o
diff entre frames idênticos é zero e nenhuma sequência é emitida para o
terminal (sem cintilação).

### Atualização fluida do progresso

Dado um download em andamento, quando o percentual muda, então apenas as
células alteradas (números/segmentos) são reescritas, mantendo bordas e texto
estáticos imóveis (verificado por `apply_diff`).

## Testes unitários

- `frame_put_writes_cells_in_bounds` — escrita dentro/fora dos limites.
- `frame_put_marks_cyan_cells` — destaque por célula.
- `frame_diff_counts_only_changed_cells` — diff conta apenas mudanças.
- `frame_apply_diff_rewrites_only_changed_run` — só a parte alterada é emitida.
- `frame_apply_diff_clears_removed_cells` — células removidas viram espaço.
- `frame_apply_diff_identical_frames_emit_nothing` — frames iguais = nada emitido.
