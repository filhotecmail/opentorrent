# US-030 — Tasks

## Implementação

- [x] Workflow `.github/workflows/release.yml` (push de tag v* + workflow_dispatch).
- [x] Checkout com `fetch-depth: 0` (hash exato da tag/branch — AC-1).
- [x] `cargo clean` + sem cache de builds antigos (AC-2).
- [x] Build `--release --verbose`.
- [x] Validação pré-publicação `--version` vs tag (AC-3, sem pipe).
- [x] Assets `opentorrent-$TAG-linux-x86_64` + `opentorrent-linux-x86_64` (AC-4).
- [x] Publicação via `softprops/action-gh-release` (target commit + body com curl).
- [x] `Cargo.toml`: `edition = "2024"` + ajustes exigidos (`#[unsafe(link_section)]`, `collapsible_if`).
- [x] README: Estado do projeto, Estrutura, Dependências e Instalação
      (`/releases/latest`, sem hardcode `v0.1.5`).

## Testes

- [x] YAML válido (python yaml).
- [x] Simulação da validação de versão (tag confere; tag divergente rejeitada).
- [x] Suíte existente (53 testes) verde + clippy `-D warnings` + machete + fmt.
- [ ] Validação E2E: push da tag v0.1.17 dispara o workflow e publica o release.
- [ ] Instalação via curl `/releases/latest/download/opentorrent-linux-x86_64`.

## Publicação

- [ ] Specs em `specs/018-ci-release/`
- [ ] Commit + push da branch `feat/us-030-ci-release`
- [ ] PR para `master` com CI verde e merge
- [ ] Bump/build release v0.1.17, commit, push + tag → workflow Release
- [ ] Verificação do release gerado pelo pipeline + instalação curl
