# Quickstart: US-036 — Menu de contexto com clique direito na Biblioteca

## Pré-requisitos

- Build: `cargo check` / `cargo run` (loop de dev, constituição VIII).
- CI: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`.
- Requer a US-037 (unificação na Home) entregue: o menu atua na Biblioteca da Home.

## Validação (smoke test via tmux)

```bash
tmux new-session -d -s ot
tmux set-option -g mouse on
tmux send-keys -t ot './target/debug/opentorrent' Enter
```

> Clique direito: com `tmux set -g mouse on`, um clique físico com o botão
> direito envia `Down(MouseButton::Right)` ao app (ou use
> `tmux send-keys -t ot xterm-mouse` com as sequências de escape SGR).

### Cenário 1 — clique direito abre o popup

```bash
# clique direito sobre um torrent da Biblioteca
# esperado: popup sobre a linha com Pausar/Retomar/Parar/Excluir; linha fica selecionada
```

### Cenário 2 — navegar e executar

```bash
tmux send-keys -t ot 'Down' 'Down' 'Enter'
# esperado: 3ª opção (Parar) executada; popup fecha; notice "removido da fila: <nome>"
```

### Cenário 3 — Esc fecha sem executar

```bash
# clique direito → Esc
# esperado: popup fecha; nenhuma ação; seleção da linha mantida
```

### Cenário 4 — clique fora fecha

```bash
# clique direito → clique esquerdo fora do popup (ex.: no Histórico)
# esperado: popup fecha sem executar
```

### Cenário 5 — pendente mostra só Excluir

```bash
# adicionar um magnet e, antes de resolver, clique direito na linha "inicializando"
# esperado: popup com apenas Excluir
```

### Cenário 6 — Excluir abre a confirmação

```bash
# clique direito → ↑↑↑ (Pausar) ou navegar até Excluir → Enter
# para Excluir: clique direito → Enter (única opção em pendente) → Enter
# esperado: prompt da Home "excluir '<nome>'? [Y] ou [N]"; Y confirma, N cancela
```

## Critérios de aceite

- Clique direito na Biblioteca abre o popup sobre a linha clicada e a seleciona.
- ↑/↓/Enter/Esc e clique nas opções funcionam conforme o contrato.
- Pendentes mostram apenas `Excluir`.
- `Excluir` aciona a confirmação Y/N da US-035.
- `cargo test --locked --all-targets` verde e `cargo clippy -- -D warnings` limpo.
