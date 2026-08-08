# Arquitetura

O OpenTorrent é um binário único Rust. A estrutura de diretórios:

```text
opentorrent/
├── Cargo.toml      # Manifesto: nome, versão, dependências e perfis de build
├── Cargo.lock      # Versões exatas travadas (commitado, pois é um binário)
├── src/
│   └── main.rs     # CLI (clap) + sessão de download (librqbit) + UI (indicatif/crossterm)
├── .github/
│   ├── workflows/  # CI, cobertura, notificações, qualidade de issues, README vivo
│   └── dependabot.yml
└── scripts/
    └── update-readme.sh  # gera as seções dinâmicas do README
```

## Camadas

1. **CLI (clap)** — parse de argumentos: `add`, `list`, `-s`, `-r`, `--overwrite`,
   `--list`, `-o`, etc.
2. **Sessão (librqbit)** — `Session::new`, `add_torrent` (arquivo `.torrent`,
   URL ou magnet), `get_torrents`, `wait_for_completion`, etc.
3. **UI (indicatif + crossterm)** — `MultiProgress` com barras por torrent,
   interação por mouse (pausar/retomar/parar/excluir), `print_line` via
   `multi.suspend`.

## Decisões-chave

- **Diretório padrão:** `~/downloads/torrent-downloads/` quando `-o` não é
  informado.
- **Retomada inteligente:** probe com `list_only: true` resolve a metainfo uma
  única vez; compara tamanhos em disco para decidir entre `Missing`/`Partial`/
  `Complete`.
- **Perfis de build:** `dev` otimizado para iteração rápida e `release` com
  `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`.
