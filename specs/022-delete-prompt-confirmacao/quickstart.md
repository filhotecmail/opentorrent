# Quickstart: US-035 — /delete com confirmação no prompt da Home

## Pré-requisitos

- Build: `cargo check` / `cargo run` (loop de dev, constituição VIII).
- CI: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`.

## Validação (smoke test via tmux)

```bash
tmux new-session -d -s ot
tmux send-keys -t ot './target/debug/opentorrent' Enter
# adicionar um torrent de teste para haver linha na Biblioteca:
tmux send-keys -t ot '/add magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567' Enter
```

### Cenário 1 — /delete na Home pergunta no prompt

```bash
tmux send-keys -t ot '/delete' Enter
# esperado: prompt mostra "excluir '...'? [Y] ou [N]" (view INÍCIO, sem modal central)
tmux send-keys -t ot 'Y'
# esperado: linha some da Biblioteca; prompt volta ao normal; notice de exclusão
```

### Cenário 2 — N cancela

```bash
tmux send-keys -t ot '/delete' Enter 'N'
# esperado: linha permanece; notice "exclusão cancelada"; prompt volta ao normal
```

### Cenário 3 — Esc cancela

```bash
tmux send-keys -t ot '/delete' Enter Escape
# esperado: linha permanece; prompt volta ao normal
```

### Cenário 4 — tecla Delete na Session transita para a Home

```bash
tmux send-keys -t ot '/list' Enter
tmux send-keys -t ot 'Delete'
# esperado: view SESSÃO → INÍCIO; prompt pergunta "excluir '...'? [Y] ou [N]"
tmux send-keys -t ot 'Y'
# esperado: exclusão efetivada; view permanece na Home
```

### Cenário 5 — /delete via popup de comandos

```bash
tmux send-keys -t ot '/dele' Enter
# esperado: popup filtra "/delete"; Enter executa; prompt pergunta no prompt da Home
tmux send-keys -t ot 'N'
```

## Critérios de aceite

- Nenhuma exclusão abre modal central (overlay) — só o prompt da Home.
- Todos os caminhos (`/delete`, tecla `Delete`, botão Excluir do mouse) mostram a mesma pergunta com o nome do torrent.
- Com a pergunta aberta, teclas que não sejam Y/N/Esc/Ctrl+C não têm efeito.
- `cargo test --locked --all-targets` verde.
