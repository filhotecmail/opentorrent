# Contracts: US-037 — Unificar a interface na Home

Contratos internos do módulo `session_ui` (helpers de decisão — sem interface externa nova).

## `home_action_for_key(key) -> Option<HomeAction>` (novo, puro)

Decide o atalho de tecla única na Home. Parâmetros de entrada: tecla (`char` ou `Delete`) + contexto (`prompt_empty: bool`, `prompt_add_mode: bool`, `confirming_delete: bool`).

- **Entrada**: tecla + estado do prompt.
- **Saída**:
  - `Some(Pause)` quando tecla `p`/`P` e `prompt_empty && !prompt_add_mode && !confirming_delete`.
  - `Some(Resume)` quando `r`/`R` e mesmas condições.
  - `Some(Stop)` quando `x`/`X` e mesmas condições.
  - `Some(Delete)` quando `Delete` e mesmas condições.
  - `None` em qualquer outro caso (texto no prompt, modo add, confirmação, teclas comuns).
- **Contrato de teste**:
  - `p` com prompt vazio → `Pause`; `p` com `/add` digitado → `None`.
  - `r`/`R` com prompt vazio → `Resume`; com `prompt_add_mode` → `None`.
  - `x`/`X` com prompt vazio → `Stop`.
  - `Delete` com prompt vazio → `Delete`; com `confirming_delete` → `None`.
  - `a` → `None` (tecla não é atalho).

## `View` (enum reduzido)

- Variantes: `Home`, `Menu`. Nenhuma outra transição de view existe.

## `COMMANDS`

- 6 entradas: `/add`, `/pause`, `/resume`, `/delete`, `/help`, `/exit`. `/list` e `/history` não são comandos.

## `footer_hints(View::Home, false, false)`

- Deve conter `"p pausar"`, `"r retomar"`, `"x remover"`, `"Del excluir"`, `"/ comandos"`.
