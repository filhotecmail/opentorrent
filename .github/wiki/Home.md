# Bem-vindo à Wiki do OpenTorrent

O **OpenTorrent** é um cliente BitTorrent de linha de comando (CLI) para
Ubuntu/Linux escrito em Rust. Esta wiki documenta arquitetura, uso, pipeline de
desenvolvimento e convenções do projeto.

## Navegação

- [[Home]] — visão geral
- [[Arquitetura]] — como o projeto é estruturado
- [[CLI e Uso]] — comandos e exemplos
- [[Pipeline de desenvolvimento]] — Spec Kit, CI/CD e fluxo de US
- [[Contribuindo]] — convenções, padrões e boas práticas

## Resumo rápido

- **Linguagem/stack:** Rust (edition 2021) + `librqbit`, `tokio`, `clap`,
  `anyhow`, `indicatif`, `crossterm`.
- **Motor:** `librqbit` (BitTorrent 100% Rust).
- **Verificação:** `cargo fmt`, `cargo clippy -- -D warnings`, `cargo test`.
- **CI:** GitHub Actions (`.github/workflows/`).
