# Feature Specification: US-031 — Interface TUI com painéis divididos, caixa de entrada na base e menu de comandos flutuante

**Feature Branch**: `feat/us-031-tui-paineis-comandos`

**Created**: 2026-08-09

**Status**: Draft

**Input**: User description: "US-031 — Interface TUI com painéis divididos (Biblioteca/Histórico), caixa de entrada na base e menu de comandos flutuante estilo autocompletar"

**Referência de estilo**: TUI do repositório `anomalyco/opencode` (OpenTUI) — layout full-window reativo, tema escuro com fundo em toda a tela, prompt fixo na base e palette de comandos flutuante com filtragem.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Divisão da tela principal em painéis de Biblioteca, Histórico e Caixa de Entrada (Priority: P1)

O usuário inicia o OpenTorrent em modo interativo TUI. A tela inicial deve ser dividida em três seções: quadro superior de Biblioteca, quadro intermediário de Histórico e campo de input delimitado no rodapé.

**Why this priority**: É a base do novo layout; sem os painéis o restante da US não se sustenta.

**Independent Test**: Renderizar a Home e verificar a presença dos títulos "Biblioteca de downloads" e "Histórico de downloads" e da linha de prompt na base.

**Acceptance Scenarios**:
1. **Given** que o aplicativo é iniciado em modo interativo, **When** a interface desenha a tela inicial, **Then** o layout deve apresentar o quadro superior de Biblioteca, o quadro intermediário de Histórico e o campo de input delimitado no rodapé.

### User Story 2 - Layout full-window reativo (estilo opencode) (Priority: P1)

O layout deve ocupar todo o terminal com margens padronizadas (1-2 caracteres), recalculado a cada redimensionamento da janela — em vez de um quadro fixo 128x48.

**Why this priority**: O usuário pediu explicitamente a reatividade à tela/resolução do opencode.

**Independent Test**: Redimensionar o terminal e verificar que o conteúdo se ajusta às novas dimensões sem estourar as bordas.

**Acceptance Scenarios**:
1. **Given** um terminal de qualquer tamanho, **When** a TUI é renderizada, **Then** o conteúdo ocupa a área disponível respeitando margens laterais/superior/inferior.
2. **Given** que a janela é redimensionada durante a execução, **When** o próximo frame é desenhado, **Then** as coordenadas de layout são recalculadas com as novas dimensões.

### User Story 3 - Abertura do menu flutuante de comandos ao digitar barra (Priority: P1)

Com o cursor no campo de entrada do rodapé, ao digitar `/` o sistema deve renderizar o painel flutuante de autocompletar acima do prompt, listando os comandos do OpenTorrent com suas descrições.

**Why this priority**: É o mecanismo central de navegação por comandos.

**Independent Test**: Digitar `/` e verificar a abertura do popup com os comandos `/add /list /pause /resume /delete /help /exit`.

**Acceptance Scenarios**:
1. **Given** que o cursor está no campo de entrada, **When** o usuário digita `/`, **Then** o sistema renderiza o painel flutuante acima do prompt com os comandos e descrições.
2. **Given** que o usuário continua digitando após a barra (ex.: `/li`), **When** o texto é filtrado, **Then** apenas os comandos correspondentes permanecem visíveis (filtragem dinâmica).

### User Story 4 - Seleção de comando via navegação no popup flutuante (Priority: P1)

No menu flutuante, o usuário navega com ↑/↓ (ou j/k), destaca o item com cor de fundo realçada e seleciona com Enter ou Tab.

**Why this priority**: Define a interação completa do popup.

**Independent Test**: Navegar no popup e pressionar Enter para preencher/executar o comando.

**Acceptance Scenarios**:
1. **Given** o menu flutuante visível, **When** o usuário navega com ↑/↓ e pressiona Enter, **Then** o comando selecionado preenche a caixa de entrada para execução imediata.
2. **Given** o menu flutuante visível, **When** o usuário pressiona Tab, **Then** o comando selecionado completa o texto do prompt.
3. **Given** o item selecionado no popup, **Then** ele exibe destaque de fundo (highlight background), conforme o padrão de seleção ativa.

---

## Technical Context

**Language/Version**: Rust 2021, toolchain stable

**Primary Dependencies**: `crossterm` (eventos, cor, cursor), `librqbit` (sessão de torrents), `tokio` (async)

**Target Platform**: Linux terminal moderno (GNOME Terminal, Windows Terminal, etc.)

**Existing Code**: `src/session_ui.rs` — `Tui`, `View` (Title/Menu/Session/Include/Completed), `Frame` (double buffering), `Theme` (US-019), tabela de sessão com barra em blocos (US-020/028), exclusão com Delete (US-027).

## Design Overview

### Novas views

- `View::Home` (substitui `View::Title`): tela inicial com dois painéis de linha fina —
  1. **Biblioteca de downloads** (topo): tabela compacta dos torrents da sessão (barra em blocos + nome).
  2. **Histórico de downloads** (meio): downloads concluídos (100%) com nome e tamanho.
  3. **Prompt fixo na base** (rodapé, acima do footer): `> ` + texto digitado + cursor ativo; placeholder "Enter a command or / for options" quando vazio.
- `View::Menu`: popup flutuante de comandos desenhado acima do prompt (em vez do menu centralizado atual).

### Comandos

```
COMMANDS: [(&str, &str); 7] = [
    ("/add",    "adicionar torrent a lista"),
    ("/list",   "acompanhar progresso"),
    ("/pause",  "pausar torrent selecionado"),
    ("/resume", "retomar torrent selecionado"),
    ("/delete", "excluir torrent selecionado"),
    ("/help",   "exibir ajuda de comandos"),
    ("/exit",   "sair do OpenTorrent"),
];
```

### Layout full-window reativo

- `frame_rect`/`body_frame`: margens laterais de 2 colunas e 1 linha (topo/base), ocupando todo o terminal; dimensões recalculadas a cada `render()` (que já lê `terminal::size()` por frame).
- `draw_box`/`frame_top_bottom`: bordas de linha fina `┌─┐│└┘` (estilo opencode) no lugar das duplas `╔═╗║╚╝`.

### Interação

- Home: qualquer caractere é inserido no prompt; `/` abre o popup; `Enter` executa o comando digitado; `Esc` limpa; `Backspace` apaga.
- Menu (popup): `↑/↓`/`j/k` navegam, `Enter` executa o comando selecionado, `Tab` completa o prompt, caracteres filtram a lista, `Backspace` apaga/refiltra, `Esc` fecha.
- O destaque (highlight background) acompanha o item selecionado do popup.
