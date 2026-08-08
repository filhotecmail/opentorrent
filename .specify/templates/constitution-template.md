# [PROJECT_NAME] Constitution
<!-- Example: Spec Constitution, TaskFlow Constitution, etc. -->

## Core Principles

### [PRINCIPLE_1_NAME]
<!-- Example: I. Library-First -->
[PRINCIPLE_1_DESCRIPTION]
<!-- Example: Every feature starts as a standalone library; Libraries must be self-contained, independently testable, documented; Clear purpose required - no organizational-only libraries -->

### [PRINCIPLE_2_NAME]
<!-- Example: II. CLI Interface -->
[PRINCIPLE_2_DESCRIPTION]
<!-- Example: Every library exposes functionality via CLI; Text in/out protocol: stdin/args → stdout, errors → stderr; Support JSON + human-readable formats -->

### [PRINCIPLE_3_NAME]
<!-- Example: III. Test-First (NON-NEGOTIABLE) -->
[PRINCIPLE_3_DESCRIPTION]
<!-- Example: TDD mandatory: Tests written → User approved → Tests fail → Then implement; Red-Green-Refactor cycle strictly enforced -->

### [PRINCIPLE_4_NAME]
<!-- Example: IV. Integration Testing -->
[PRINCIPLE_4_DESCRIPTION]
<!-- Example: Focus areas requiring integration tests: New library contract tests, Contract changes, Inter-service communication, Shared schemas -->

### [PRINCIPLE_5_NAME]
<!-- Example: V. Observability, VI. Versioning & Breaking Changes, VII. Simplicity -->
[PRINCIPLE_5_DESCRIPTION]
<!-- Example: Text I/O ensures debuggability; Structured logging required; Or: MAJOR.MINOR.BUILD format; Or: Start simple, YAGNI principles -->

### VI. Rust-Skills (NON-NEGOTIABLE)
<!-- Projeto Rust: integra https://github.com/actionbook/rust-skills (clone em ~/rust-skills/). -->
Todo planejamento, design ou implementação de código Rust MUST consultar as
Rust Skills via `rust-router` e aplicar o framework de meta-cognição em 3 camadas
(Domínio → Design → Mecânica), usando as skills da camada identificada
(`m01`–`m07` mecânica, `m09`–`m15` design, `domain-*` domínio) e
`coding-guidelines` antes de propor arquitetura, tipos, ownership/borrowing,
concorrência ou escolha de crates. Plano SDD/`plan.md` MUST registrar as skills
consultadas na seção `Rust Skills Check`. Remendos superficiais a sintomas de
compilação (ex.: `.clone()` sem análise de ownership) são PROIBIDOS sem análise
cognitiva.

### VII. Cargo-Skill (NON-NEGOTIABLE)
<!-- Projeto Rust: integra https://lib.rs/crates/cargo-skill (contexto ativo por camadas). -->
O agente MUST usar o `cargo-skill` como fonte de contexto ativo de regras Rust:
carregar `.skill/context.md` quando existir e invocar `cargo skill lookup`/
`think`/`write`/`review`/`refactor`/`debug` conforme a tarefa (consulta pontual,
raciocínio de design ou execução). O índice de regras (Layer 1) da seção
`# Rust Skill Reference` do `AGENTS.md` é a referência offline obrigatória antes
de propor regras de estilo, tipos, ownership, performance ou testes.

## [SECTION_2_NAME]
<!-- Example: Additional Constraints, Security Requirements, Performance Standards, etc. -->

[SECTION_2_CONTENT]
<!-- Example: Technology stack requirements, compliance standards, deployment policies, etc. -->

## [SECTION_3_NAME]
<!-- Example: Development Workflow, Review Process, Quality Gates, etc. -->

[SECTION_3_CONTENT]
<!-- Example: Code review requirements, testing gates, deployment approval process, etc. -->

## Governance
<!-- Example: Constitution supersedes all other practices; Amendments require documentation, approval, migration plan -->

[GOVERNANCE_RULES]
<!-- Example: All PRs/reviews must verify compliance; Complexity must be justified; Use [GUIDANCE_FILE] for runtime development guidance -->

**Version**: [CONSTITUTION_VERSION] | **Ratified**: [RATIFICATION_DATE] | **Last Amended**: [LAST_AMENDED_DATE]
<!-- Example: Version: 2.1.1 | Ratified: 2025-06-13 | Last Amended: 2025-07-16 -->
