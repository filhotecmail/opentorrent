# AGENTS.md

Guia de operação para agentes de IA que trabalham neste repositório.

## Projeto

**OpenTorrent** — cliente BitTorrent de linha de comando em Rust (CLI) para
Ubuntu/Linux. Baixa torrents, magnet links e arquivos `.torrent` (local ou via
URL), lista conteúdo, filtra arquivos por regex e mostra progresso em tempo
real. O motor é o `librqbit` (BitTorrent 100% Rust).

- Stack: Rust (edition 2021) + `librqbit`, `tokio`, `clap`, `anyhow`, `futures`,
  `size_format`, `tracing`, `tracing-subscriber`.
- Ferramentas de verificação: `cargo fmt`, `cargo clippy -- -D warnings`,
  `cargo test`.

## Pipeline de desenvolvimento (Spec Kit)

O projeto usa o fluxo **Spec Kit** via comandos opencode (`.opencode/commands/`):

1. `/speckit.clarify` — reduz ambiguidades do spec com perguntas dirigidas
2. `/speckit.constitution` — cria/atualiza a constituição do projeto
   (`.specify/memory/constitution.md`)
3. `/speckit.specify` — cria a especificação da feature (`specs/NNN-*/spec.md`)
4. `/speckit.plan` — gera o SDD (`.specify/templates/plan-template.md` +
   `specs/NNN-*/plan.md`, `research.md`, `data-model.md`, `contracts/`,
   `quickstart.md`)
5. `/speckit.tasks` — quebra o plano em tarefas (`tasks.md`)
6. `/speckit.checklist` — gera checklists de qualidade de requisitos
7. `/speckit.implement` — executa as tarefas do `tasks.md`
8. `/speckit.analyze` / `/speckit.converge` — análise e consolidação

Antes de iniciar qualquer fase do pipeline, consulte a constituição
(`.specify/memory/constitution.md`) e as instruções de Rust Skills
(`vendor/rust-skills/.opencode/instructions/rust-skills.md`).

## Rust Skills (obrigatório para código Rust)

O projeto integra as **Rust Skills** de `https://github.com/actionbook/rust-skills`
(vendored em `vendor/rust-skills/`, carregadas pelo opencode via
`.opencode/opencode.json` → `skills.paths`). São 38 skills com o framework de
**meta-cognição em 3 camadas** (Domínio → Design → Mecânica).

Regras de uso:

- **Antes de responder/planejar qualquer questão de Rust**, rode o
  `rust-router` para identificar a camada de entrada e a skill correta.
- **Camada 1 (mecânica)**: `m01-ownership`, `m02-resource`, `m03-mutability`,
  `m04-zero-cost`, `m05-type-driven`, `m06-error-handling`, `m07-concurrency`.
- **Camada 2 (design)**: `m09-domain`, `m10-performance`, `m11-ecosystem`,
  `m12-lifecycle`, `m13-domain-error`, `m14-mental-model`, `m15-anti-pattern`.
- **Camada 3 (domínio)**: `domain-cli` (relevante para este projeto),
  `domain-web`, `domain-fintech`, `domain-ml`, `domain-iot`, `domain-embedded`,
  `domain-cloud-native`.
- **Consultas transversais**: `coding-guidelines` (convenções P.NAM/P.FMT/P.ERR),
  `unsafe-checker` (FFI/unsafe), `rust-learner` (versões/crates), `rust-daily`.
- Sempre consulte a skill de erro ao tratar códigos de erro do compilador
  (E0382 → `m01-ownership`, E0277 → `m04-zero-cost`, etc.).

Não responda sintomas com remendos superficiais (ex.: "use `.clone()`"): trace
pelas camadas cognitivas e entregue a solução arquiteturalmente correta para o
domínio.

## Convenções

- Documentação e respostas do pipeline em **PT-BR**.
- Mudanças de governança passam por `/speckit.constitution` (versão semver).
- SDD/planos devem registrar quais skills Rust Skills foram consultadas
  (seção `Rust Skills Check` do `plan-template.md`).
