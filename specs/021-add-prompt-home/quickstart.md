# Quickstart: US-034 — /add com prompt "insira o link:" na Home

## Pré-requisitos

- Build: `cargo check` / `cargo run` (loop de dev, constituição VIII).
- CI: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo machete`.

## Validação (smoke test via tmux)

```bash
tmux new-session -d -s ot
tmux send-keys -t ot './target/debug/opentorrent' Enter
```

### Cenário 1 — `/add` sem origem ativa o modo

```bash
tmux send-keys -t ot '/add' Enter
# esperado: prompt da Home mostra "insira o link:" (view INÍCIO, sem diálogo)
tmux send-keys -t ot 'magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567' Enter
# esperado: linha "inicializando" na Biblioteca; prompt volta ao normal; adição em background
```

### Cenário 2 — Esc cancela o modo

```bash
tmux send-keys -t ot '/add' Enter Esc
# esperado: prompt volta a "Enter a command or / for options"
```

### Cenário 3 — /add inline

```bash
tmux send-keys -t ot '/add magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567' Enter
# esperado: adição inicia direto, sem pedir o link de novo
```

### Cenário 4 — origem inválida preserva o texto

```bash
tmux send-keys -t ot '/add  not-a-real-source' Enter
# esperado: notice de erro; prompt preservado com a origem digitada
```

### Cenário 5 — regressão Session

```bash
tmux send-keys -t ot '/list' Enter 'a'
# esperado: view Include abre (fluxo legado via atalho a/A/+)
```

## Critérios de aceite

- Nenhuma adição sai da Home (view nunca muda para Include pelo fluxo do prompt).
- View Include segue funcional via `a`/`A`/`+` na Session.
- `cargo test --locked --all-targets` verde.
