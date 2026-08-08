# Feature Specification: Melhoria na entrada de texto do prompt de origem e ajuste no tratamento de colar (Paste)

**Feature Branch**: `feat/us-011-input-improvements`

**Created**: 2026-08-08

**Status**: Draft

**Input**: User description: "US-011 — Melhoria na entrada de texto do prompt de origem e ajuste no tratamento de colar (Paste)"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Posicionamento automático do cursor ao entrar no modo de digitação (Priority: P1)

O usuário está no menu de adição de torrent e confirma a escolha de digitar a origem. A interface deve abrir o campo de edição com o cursor ativo e posicionado imediatamente após o rótulo de entrada.

**Why this priority**: É a base da usabilidade do fluxo de inclusão; sem foco correto o usuário precisa clicar ou navegar manualmente.

**Independent Test**: Entrar no modo de digitação e verificar se o cursor está posicionado no campo de entrada.

**Acceptance Scenarios**:
1. **Given** que o usuário está no menu de adição de torrent, **When** o usuário confirma a escolha de digitar a origem, **Then** a interface deve abrir o campo de edição com o cursor ativo e posicionado imediatamente após o rótulo de entrada.

---

### User Story 2 - Colagem de link magnético longo com quebra automática de linha (Priority: P1)

O campo de entrada de origem está ativo com o cursor posicionado. O usuário cola uma URL de torrent ou link magnético extenso. O texto deve ser ajustado e distribuído em múltiplas linhas respeitando os limites da tela, mantendo o layout organizado e legível sem achatar a interface.

**Why this priority**: Links magnéticos podem ter centenas de caracteres; sem quebra automática a interface quebra visualmente.

**Independent Test**: Colar um link magnético de 500+ caracteres no campo de entrada e verificar quebra de linha visual e layout preservado.

**Acceptance Scenarios**:
1. **Given** que o campo de entrada de origem está ativo com o cursor posicionado, **When** o usuário cola uma URL de torrent ou link magnético extenso, **Then** o texto deve ser ajustado e distribuído em múltiplas linhas respeitando os limites da tela, mantendo o layout organizado e legível sem achatar a interface.

---

### User Story 3 - Preservação do rodapé e elementos da UI durante expansão do texto (Priority: P1)

O texto colado expande verticalmente no campo de entrada. As instruções de ação e rodapé da tela devem permanecer fixas em suas posições abaixo do campo de entrada, acompanhando a expansão vertical do texto colado.

**Why this priority**: Garante que o usuário sempre veja as dicas de navegação (Enter para confirmar, Esc para cancelar) independentemente do tamanho do texto.

**Independent Test**: Colar texto longo, verificar que o rodapé permanece visível e na posição correta.

**Acceptance Scenarios**:
1. **Given** que o texto colado expande verticalmente no campo de entrada, **When** o campo cresce para múltiplas linhas, **Then** as instruções de ação e rodapé da tela devem permanecer fixas em suas posições abaixo do campo de entrada, acompanhando a expansão vertical do texto colado.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Ao transicionar para o modo de digitação de origem (Include view → typing mode), o cursor DEVE ser posicionado automaticamente no campo de entrada, imediatamente após o rótulo "origem: ".

- **FR-002**: O campo de entrada DEVE suportar quebra de linha visual automática (soft wrap) quando o texto excede a largura do terminal, distribuindo o texto em múltiplas linhas de exibição.

- **FR-003**: A colagem de texto longo (via terminal paste / bracketed paste mode) NÃO DEVE desalinhar ou sobrescrever elementos da interface (título, menu, rodapé).

- **FR-004**: O rodapé com instruções ("Enter para confirmar, Esc para cancelar") DEVE permanecer fixo abaixo do campo de entrada, movendo-se para baixo conforme o campo expande verticalmente.

- **FR-005**: O campo de entrada DEVE ter largura máxima respeitando as margens laterais do terminal (ex.: 80% da largura ou largura total menos padding).

- **FR-006**: Backspace e navegação por setas DEVEM funcionar corretamente em texto com múltiplas linhas visuais (soft wrap).

---

### Non-Functional Requirements

- **NFR-001**: A renderização não deve piscar (flicker) durante redraws do campo de entrada.

- **NFR-002**: O cursor deve permanecer visível e na posição correta após cada keystroke ou operação de paste.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Em 100% das transições para modo de digitação, o cursor aparece posicionado no campo de entrada.
- **SC-002**: Links magnéticos de 500+ caracteres colados no campo são exibidos com quebra visual em múltiplas linhas sem quebrar o layout.
- **SC-003**: O rodapé permanece visível e na posição correta durante toda a edição.

---

## Assumptions

- O terminal alvo suporta bracketed paste mode (a maioria dos terminais modernos suporta).
- A largura mínima do terminal para operação é 60 colunas.
- O crossterm 0.28 já reporta eventos de paste via `Event::Paste` quando bracketed paste mode está habilitado.

---

## Technical Notes

### crossterm Bracketed Paste Mode
```rust
use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use crossterm::terminal::{enable_raw_mode, disable_raw_mode};

// Habilita bracketed paste mode para receber Event::Paste
execute!(stdout, event::EnableBracketedPaste)?;
```

### Soft Wrap Logic
O campo de entrada armazena o texto como uma única `String` lógica. Na renderização, calcula-se quantas linhas visuais o texto ocupa dado a largura disponível:
```rust
fn wrap_text(text: &str, width: usize) -> Vec<String> {
    // word wrap simples: quebra em espaços quando possível
}
```

### Cursor Positioning
A posição do cursor (linha, coluna) na tela é derivada do índice do caractere no texto lógico + offset do prompt "origem: ".

---

## Edge Cases

- Terminal redimensionado durante edição → re-calcular wrap e reposicionar cursor.
- Paste de texto com quebras de linha literais (`\n`) → tratar como quebras explícitas.
- Texto colado maior que altura do terminal → scroll ou truncamento com indicação.
- Backspace no início de linha visual (não lógica) → mover para final da linha anterior.