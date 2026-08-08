# Quickstart: US-009 + US-010

**Date**: 2026-08-08

## Como executar

```bash
# Modo interativo (TUI): tela inicial + menu
cargo run

# Modo clássico: adicionar e baixar
cargo run -- add <torrent>
```

## Fluxos cobertos

| Ação | Resultado |
|------|-----------|
| `opentorrent` (sem args) | Tela inicial centralizada com arte ASCII "OPENTORRENT" + campo de entrada |
| Digitar `/` | Menu suspenso: Adicionar / Acompanhar / Listar completos / Sair |
| `A` | Menu de inclusão: digitar origem ou voltar |
| `C` | Listagem de sessão com estados, percentuais e ações P/R/X |
| `L` | Lista downloads completos de `~/downloads/torrent-downloads/` |
| `S` | Sai da aplicação |

## Atalhos do menu

- Navegação: `↑`/`↓` + `Enter`
- Digitação direta: `A`, `C`, `L`, `S`

## Persistência de sessão

- Arquivos em `~/.config/opentorrent/session` (gerenciados pelo librqbit)
- Restauração automática ao abrir o modo interativo

## Validação

```bash
cargo check
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
cargo test
```
