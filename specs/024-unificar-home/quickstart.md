# Quickstart: US-037 — Unificar a interface na Home

## Pré-requisitos

- Build: `cargo check` / `cargo run` (loop de dev, constituição VIII).
- CI: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`.

## Validação (smoke test via tmux)

```bash
tmux new-session -d -s ot
tmux send-keys -t ot './target/debug/opentorrent' Enter
```

### Cenário 1 — menu de comandos com 6 itens

```bash
tmux send-keys -t ot '/'
# esperado: popup lista apenas /add, /pause, /resume, /delete, /help, /exit
tmux send-keys -t ot 'Esc'
```

### Cenário 2 — /list e /history não existem

```bash
tmux send-keys -t ot '/list' Enter
# esperado: notice "comando desconhecido: /list" (sem tela SESSÃO)
tmux send-keys -t ot '/history' Enter
# esperado: notice "comando desconhecido: /history" (sem tela COMPLETOS)
```

### Cenário 3 — atalhos de tecla única (prompt vazio)

```bash
# com um torrent ativo selecionado na Biblioteca:
tmux send-keys -t ot 'p'
# esperado: notice "pausado: <nome>"; linha muda para pausado
tmux send-keys -t ot 'r'
# esperado: notice "retomado: <nome>"
tmux send-keys -t ot 'x'
# esperado: notice "removido da fila: <nome>"
tmux send-keys -t ot 'Delete'
# esperado: prompt mostra "excluir '<nome>'? [Y] ou [N]" — Y confirma, N cancela
```

### Cenário 4 — prompt com texto não dispara atalho

```bash
tmux send-keys -t ot '/add'
# digitar 'p' com "/add" no prompt:
tmux send-keys -t ot 'p'
# esperado: prompt vira "/addp" (nenhum torrent pausa)
tmux send-keys -t ot 'Esc'
```

### Cenário 5 — a/A/+ não abrem tela

```bash
tmux send-keys -t ot 'a'
# esperado: prompt recebe 'a' (nenhuma view Incluir abre); o resto digita normal
tmux send-keys -t ot 'Backspace'
```

### Cenário 6 — Esc na Home não faz nada

```bash
tmux send-keys -t ot 'Esc'
# esperado: a Home permanece (sem sair para outra tela)
```

## Critérios de aceite

- A Home é a única tela; `/list`/`/history` não existem no menu nem executam.
- Pausar/Retomar/Parar/Excluir funcionam na Home via atalhos (prompt vazio), menu de contexto (US-036) e comandos `/pause`/`/resume`/`/delete`.
- `a`/`A`/`+` não abrem view separada.
- `cargo test --locked --all-targets` verde e `cargo clippy -- -D warnings` limpo.
