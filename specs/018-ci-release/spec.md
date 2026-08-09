# US-030 — Correção do pipeline de CI/CD para geração de artefatos de release alinhados à branch master

## História

Como **Desenvolvedor / Mantenedor** do projeto opentorrent, eu quero que a
automação de CI/CD compile e publique o binário de release a partir do commit
mais recente da branch master (ou da tag de versão disparada), para que os
usuários que instalarem via curl/script recebam o executável contendo todas as
correções de bugs, melhorias de TUI e a versão exata sinalizada.

## Critérios de aceite

- [x] Workflow `release.yml` com checkout do hash exato da tag/branch
      (`fetch-depth: 0`).
- [x] Sanitização de caches antigos do Cargo (`cargo clean` no job de release;
      o job não restaura cache de builds anteriores) — recompilação total.
- [x] Validação pré-publicação: `./opentorrent --version` e conferência da
      versão contra a tag (saída gravada em arquivo, sem pipe/broken pipe).
- [x] README e instalação sem hardcode: `/releases/latest/download/opentorrent-linux-x86_64`
      (asset com nome estável publicado pelo workflow, além do versionado).
- [x] Binário do GitHub Releases reflete as mudanças mais recentes da árvore
      (compilado no runner a partir do commit da tag).

## Cenários de teste

### Validação de versão e binário em novo disparo de tag de release

- **Dado** um novo commit na master com a tag `v0.1.X`
- **Quando** o pipeline de CI/CD é acionado (push de tag)
- **Então** o workflow valida `--version` com a versão da tag e anexa o binário
  correspondente no GitHub Releases.

### Instalação via curl em ambiente limpo de usuário

- **Dado** uma release atualizada publicada no repositório
- **Quando** o usuário executa o comando de instalação via `curl` da documentação
- **Então** o binário baixado e movido para o PATH executa com todas as
  correções de TUI e indica a versão correta instalada.

## Implementação

- `.github/workflows/release.yml`: gatilhos `push: tags: ['v*']` +
  `workflow_dispatch` (input `tag`); job `release` com checkout `fetch-depth: 0`,
  `cargo clean` (AC-2), build `--release --verbose`, validação de versão (AC-3)
  e publicação via `softprops/action-gh-release` com assets
  `opentorrent-$TAG-linux-x86_64` + `opentorrent-linux-x86_64` (AC-4).
- `Cargo.toml`: `edition = "2024"` (Rust atual); `metadata.rs` com
  `#[unsafe(link_section)]` (exigência da edição 2024); `collapsible_if`
  corrigido.
- README: Estado do projeto atualizado (release/versão), Estrutura do projeto e
  Dependências revisados, instalação via `/releases/latest` sem hardcode.
