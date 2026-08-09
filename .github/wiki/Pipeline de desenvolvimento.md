# Pipeline de desenvolvimento

## Fluxo com User Stories (US)

O ciclo é automatizado por `./scripts/create-new-us.sh`:

```bash
# 1. Sincroniza master, cria a branch local e abre a Issue (com milestone + label)
./scripts/create-new-us.sh start US-038 "Título da US" "descrição opcional"

# 2. (aplicar as alterações na branch criada)

# 3. Valida (fmt/clippy/test/machete), commit, push e abre o PR com "Closes #N"
./scripts/create-new-us.sh execute-pipeline-gh "mensagem de commit"
```

1. `start US-038 "título"` — cria a branch `feat/us-038-slug` a partir de
   `master` e abre a issue rotulada como **US** com milestone;
2. Implementar seguindo as convenções;
3. `execute-pipeline-gh` — valida e abre o Pull Request referenciando
   `Closes #N`;
4. CI valida: `fmt`, `clippy -D warnings`, `test`, `machete` e cobertura;
5. Merge em `master` e atualização do README (workflow `README vivo`).

## Requisitos de qualidade de issues

Toda issue deve ter **pelo menos um label** (ex.: `US`, `bug`, `enhancement`) e
**um milestone** (ex.: `v1.0`). O workflow `issue-quality.yml` comenta
automaticamente quando algo falta — o `create-new-us.sh` já garante ambos.

## Verificações locais

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --locked
cargo machete
```

## Build, assinatura e release

- **`./build.sh [release|debug] [build|...]`** — compila com bump automático de
  versão patch em `Cargo.toml`; em `release` assina digitalmente o binário.
- **`./scripts/sign-release.sh [init|sign|verify]`** — assinatura digital
  (US-018): CA local + certificado `extendedKeyUsage=codeSigning` em
  `~/.local/share/opentorrent/signing` (fora do git), assinatura RSA-SHA256
  desanexada (`<bin>.sig`) e validação de integridade contra a CA raiz.

## CI/CD (GitHub Actions)

| Workflow | Arquivo | O que faz |
| --- | --- | --- |
| CI | `ci.yml` | fmt, clippy, test, machete |
| Cobertura | `coverage.yml` | tarpaulin → Codecov |
| Qualidade de issues | `issue-quality.yml` | exige labels + milestone |
| Notificações | `notify.yml` | emails para filhotecmail@gmail.com |
| README vivo | `readme-live.yml` | regenera badges/estado do README |
| Release | `release.yml` | publica release no push de tag `v*` / bump na master (US-030) |
| Dependabot | `dependabot.yml` | atualizações de deps e actions |

## Otimização de build

Consultar AGENTS.md — perfis `dev`/`release`, linker `mold`, `cargo-watch`,
`cargo-nextest`, entre outros. Referência:
<https://corrode.dev/blog/tips-for-faster-rust-compile-times>
