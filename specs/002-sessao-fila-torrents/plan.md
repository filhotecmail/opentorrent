# Implementation Plan: Interface interativa (US-010) + Sessão persistente e fila (US-009)

**Branch**: `feat/us-009-us-010-interface-interativa` | **Date**: 2026-08-08 |
**Specs**: [US-010](../001-tela-inicial-menu/spec.md), [US-009](spec.md)

**Input**: Feature specifications das US-009 e US-010 (implementadas em conjunto
por solicitação do usuário)

## Summary

Implementar a execução sem parâmetros como **TUI interativo**: tela inicial
centralizada com arte ASCII "OPENTORRENT" (US-010), campo de entrada que abre
menu ao digitar `/`, e opções que delegam à interface de sessão persistente
(US-009) — restauração automática via persistência nativa do librqbit,
listagem com ações por linha (P/R/X), inclusão de novos torrents (A/+) e
listagem de downloads completos (varredura do diretório padrão).

## Technical Context

**Language/Version**: Rust (edition 2021), toolchain stable local 1.97.1

**Primary Dependencies**: `librqbit 8.1.1` (persistência nativa JSON),
`crossterm 0.28.1` (eventos de teclado + terminal), `tokio`, `anyhow`,
`indicatif` (mantido para modo `add`), `size_format`

**Storage**: JSON nativo do librqbit em `~/.config/opentorrent/session`

**Testing**: `cargo test` (funções puras: varredura de completos, formatação,
parsing de comando); `cargo clippy -D warnings`; validação funcional manual do
TUI

**Target Platform**: Linux (Ubuntu), terminal moderno compatível com escape
sequences

**Project Type**: CLI interativo (TUI) + CLI clássico

**Performance Goals**: renderização responsiva (< 100ms por redraw); I/O de
persistência delegado ao store do librqbit

**Constraints**: terminal pode ser pequeno; modo interativo precisa capturar
teclado com `crossterm::event` sem conflitar com `indicatif`

**Scale/Scope**: dezenas de torrents na fila; uma origem adicionada por vez

## Constitution Check

- **VI. Rust-Skills**: cumprido — skills consultadas e registradas abaixo.
- **VII. Cargo-Skill**: cumprido — `cargo skill think`/`lookup` rodados;
  contexto `.skill/context.md` ativo.
- **VIII. Build Velocity**: cumprido — `cargo check` no loop; perfis dev/release
  já otimizados no Cargo.toml; CI com `cargo clippy -D warnings`.

## Rust Skills Check

| Skill consultada | Camada | Decisão/Impacto no design |
|------------------|--------|---------------------------|
| m01-ownership | 1 | `Arc<Session>` clonado entre tasks (stats_printer/TUI) em vez de `clone()` de dados; handles iterados por referência |
| m02-resource | 1 | `Session` como recurso RAII gerenciado pelo runtime tokio; teclado via crossterm raw mode com restore garantido |
| m07-concurrency | 1 | Canal `mpsc` para eventos de teclado; task de render separada da task de input; sem lock atravessando `.await` |
| m12-lifecycle | 2 | Restauração de sessão delegada ao `Session::new_with_opts` (lifecycle do store); cleanup de terminal em todos os caminhos de saída |
| m13-domain-error | 2 | Erros do TUI propagados via `anyhow`; mensagens de erro em lowercase sem pontuação final |
| m06-error-handling | 1 | Sem `.unwrap()` em caminho de produção; `?`/`context()` do anyhow |
| m09-domain | 2 | Enum `MenuCommand` tipa as opções do menu; `SessionItem` tipa linha da listagem |
| domain-cli | 3 | CLI sem subcomando → TUI; erros em stderr; exit code não-zero em erro |

## Project Structure

### Documentation (this feature)

```text
specs/001-tela-inicial-menu/
├── spec.md              # US-010
└── checklists/
    └── requirements.md

specs/002-sessao-fila-torrents/
├── spec.md              # US-009
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── plan.md              # este arquivo
└── checklists/
    └── requirements.md
```

### Source Code (repository root)

```text
src/
├── main.rs              # CLI (clap), runtime, roteamento add vs interativo
├── session_ui.rs        # TUI: tela inicial, campo de entrada, menu /, listagem de sessão
├── downloads.rs         # Varredura de downloads completos + formatação
└── (modo add existente permanece em main.rs)
```

**Structure Decision**: Módulo `session_ui.rs` isola toda a lógica interativa
(TUI + loop de eventos), mantendo `main.rs` enxuto (proj-01: main mínimo +
lógica em módulos). `downloads.rs` concentra a consulta de completos com testes
unitários. O modo `add` existente permanece intocado.

## Complexity Tracking

> Sem violações de constituição; nenhuma entrada necessária.

## Fases de implementação

### Phase 1 — Estrutura e CLI (US-010: tela inicial)

1. `Opts.subcommand` torna-se `Option<SubCommand>` (None → modo interativo).
2. Criar `session_ui.rs` com arte ASCII "OPENTORRENT" + campo de entrada.
3. Raw mode crossterm + loop de eventos de teclado.
4. Menu ao digitar `/` com navegação ↑/↓/Enter e atalhos A/C/L/S.

### Phase 2 — Sessão persistente (US-009)

5. Configurar `SessionPersistenceConfig::Json { folder }` em
   `~/.config/opentorrent/session`.
6. Listagem de sessão com estados, percentuais e ações P/R/X.
7. Inclusão de novos torrents via atalho A/+ (diálogo de entrada → add).
8. Opção "Listar completos" (varredura de `~/downloads/torrent-downloads/`).

### Phase 3 — Integração e validação

9. Roteamento: menu "Adicionar" → diálogo de inclusão; "Acompanhar" → listagem.
10. `cargo check`, `clippy -D warnings`, `fmt`, testes unitários.
11. Teste funcional manual das duas US.
