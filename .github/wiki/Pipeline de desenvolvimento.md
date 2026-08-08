# Pipeline de desenvolvimento

## Fluxo com User Stories (US)

1. Criar issue rotulada como **US** (User Story) com critérios de aceite;
2. Criar branch `feat/us-NNN-descricao-curta`;
3. Implementar seguindo as convenções;
4. Abrir Pull Request referenciando `Closes #N`;
5. CI valida: `fmt`, `clippy -D warnings`, `test`, `machete` e cobertura;
6. Merge em `master` e atualização do README (workflow `README vivo`).

## Requisitos de qualidade de issues

Toda issue deve ter **pelo menos um label** (ex.: `US`, `bug`, `enhancement`) e
**um milestone** (ex.: `v1.0`). O workflow `issue-quality.yml` comenta
automaticamente quando algo falta.

## Verificações locais

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo machete
```

## CI/CD (GitHub Actions)

| Workflow | Arquivo | O que faz |
| --- | --- | --- |
| CI | `ci.yml` | fmt, clippy, test, machete |
| Cobertura | `coverage.yml` | tarpaulin → Codecov |
| Qualidade de issues | `issue-quality.yml` | exige labels + milestone |
| Notificações | `notify.yml` | emails para filhotecmail@gmail.com |
| README vivo | `readme-live.yml` | regenera badges/estado do README |
| Dependabot | `dependabot.yml` | atualizações de deps e actions |

## Otimização de build

Consultar AGENTS.md — perfis `dev`/`release`, linker `mold`, `cargo-watch`,
`cargo-nextest`, entre outros. Referência:
<https://corrode.dev/blog/tips-for-faster-rust-compile-times>
