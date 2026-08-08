# Contribuindo

## Convenções de código

- **Idioma:** código, mensagens, documentação e commits em **português do Brasil**.
- **Código:** Rust claro, sem comentários desnecessários, sem `.unwrap()` em
  caminhos de produção (use `?`/`context()` do `anyhow`).
- **Erros:** mensagens em letra minúscula, sem pontuação final.
- **Async:** nunca segurar `Mutex`/`RwLock` guard atravessando `.await`.
- **UI:** `indicatif` para progresso (stderr), `crossterm` para mouse.
- **Build limpo:** `cargo clippy -- -D warnings` sem warnings.

## Fluxo de commit

1. `cargo fmt --all`
2. `cargo clippy --all-targets --all-features -- -D warnings`
3. `cargo test --locked`
4. `cargo machete`
5. Commit com mensagem descritiva (`feat:`, `fix:`, `docs:`, `ci:`, `deps:`, ...)
   e referência à issue quando aplicável (`Closes #N`).

## Labels do projeto

`US`, `bug`, `enhancement`, `documentation`, `urgent`, `ready`, `in-progress`,
`review`, `blocked`, `ci`, `infra`, `milestone`, `question`, `help wanted`,
`good first issue`, `duplicate`, `invalid`, `wontfix`.

## Recursos

- **Issues:** <https://github.com/filhotecmail/opentorrent/issues>
- **Milestones:** <https://github.com/filhotecmail/opentorrent/milestones>
- **Wiki:** esta página e demais artigos.
