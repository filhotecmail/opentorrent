# Research: US-034 — /add com prompt "insira o link:" na Home

## Desconhecidos do Technical Context

Não há NEEDS CLARIFICATION restantes (resolvidos com o usuário na especificação). Descobertas de código:

### Fluxo atual de `/add`

- `execute_command("/add")` (session_ui.rs:649-654) transiciona para `View::Include`. **Decisão**: muda para ativar `prompt_add_mode` sem trocar de view.
- `handle_home_key` Enter (session_ui.rs:548-555): `input.trim()` → se inicia com `/`, chama `execute_command`. **Decisão**: antes de `execute_command`, extrai origem inline com `parse_add_command`; `/add`/`/add ` sem origem ativa o modo.
- `submit_include` (session_ui.rs:1199-1250): valida `AddTorrent::from_cli_argument`, registra `PendingTorrent`, spawn de `add_torrent_background`. **Decisão**: novo `submit_add_from_prompt(&str)` reusa a mesma sequência sem trocar de view.

### Paste no prompt da Home

- `handle_event` `Event::Paste` (session_ui.rs:466-480): insere em `input` na posição de `prompt_cursor` para `View::Home`/`Menu`. **Decisão**: com `prompt_add_mode` ativo e view Home, o paste já cai nesse branch — sem alteração adicional.

### Testabilidade

- `Tui::new` exige `Arc<Session>` (librqbit), não construível em teste unitário puro. **Decisão**: lógica de extração inline (`parse_add_command`) e formatação do placeholder ficam em funções puras testáveis; transições de estado testadas via helper puro de decisão (ex.: `add_command_target(&str)`) quando viável.

### Alternativas consideradas

- **Enum de modo de prompt** (`PromptMode::{Command, AddLink}`) em vez de `bool`: rejeitado — um único modo extra, bool é suficiente e consistente com `typing`/`confirming_delete` existentes (YAGNI, `m05`).
- **Reusar `submit_include`**: rejeitado — ele troca para `View::Session`; o novo fluxo deve permanecer na Home.

## Decisões consolidadas

- **Decision**: `prompt_add_mode: bool` em `Tui`.
- **Rationale**: estado mínimo, consistente com padrões existentes; custo zero de render.
- **Alternatives considered**: enum de modo (descartado por escopo).

- **Decision**: função pura `parse_add_command(input: &str) -> Option<String>` para extrair origem inline.
- **Rationale**: testável sem session; roteia o Enter da Home.
- **Alternatives considered**: parse inline no handler (menos testável).

- **Decision**: `submit_add_from_prompt(&mut self, source: &str)` reusando `PendingTorrent` + `add_torrent_background`.
- **Rationale**: adição assíncrona padrão US-014 sem lock através de `.await`.
- **Alternatives considered**: `submit_include` (troca de view; descartado).

## Descoberta durante o smoke test — filtragem dinâmica × inline

A US-031 abre o popup de comandos no primeiro `/` digitado, e a partir daí os
caracteres são despachados para `handle_menu_key` — não para `handle_home_key`.
Como consequência, digitar `/add magnet:...` por extenso travava no popup
("nenhum comando encontrado", Enter sem efeito), inviabilizando o inline da
US2. Resolução em `src/session_ui.rs`:

- Helper puro `should_open_command_menu(input)`: `input.starts_with('/') &&
  !input.starts_with("/add ")` — a origem inline não é busca de comando.
- `leave_menu_for_inline_add()`: no `handle_menu_key` Char(c), ao digitar o
  espaço de `/add ` o popup fecha (`view = View::Home`) e a origem continua
  sendo digitada no prompt da Home; mesma transição aplicada ao `Event::Paste`
  quando o popup estava aberto.
- `handle_home_key` Backspace: ao apagar o espaço de `/add ` o popup reabre
  (consistente com a US-031).
- `handle_home_key` Enter com origem inválida restaura `input` no prompt para
  correção (AC US2-2) — `submit_add_from_prompt` retorna `bool`.
