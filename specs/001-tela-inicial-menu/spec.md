# Feature Specification: Interface inicial centralizada com arte ASCII e menu de comandos interativo

**Feature Branch**: `feat/us-010-tela-inicial-menu`

**Created**: 2026-08-08

**Status**: Draft

**Input**: User description: "US-010 — Interface inicial centralizada com arte ASCII e menu de comandos interativo"

## Clarifications

### Session 2026-08-08

- Q: O que deve acontecer quando o usuário digita algo na caixa de entrada que não inicia com `/`? → A: Opção A - A caixa de entrada só processa comandos iniciados com `/`; qualquer outra tecla é ignorada como digitação inerte até `/` ser pressionado.
- Q: De onde a opção "Listar downloads completos" deve obter a lista de arquivos concluídos? → A: Opção C - Varrer apenas o diretório padrão `~/downloads/torrent-downloads/`, de forma independente da sessão persistida.
- Q: As opções "Adicionar Torrent a lista" e "Acompanhar progresso da lista de torrents" devem funcionar nesta US-010 ou apenas delegar/exibir aviso? → A: Opção A - Nesta US-010, essas duas opções exibem um aviso "funcionalidade disponível na próxima versão" e voltam ao menu; o fluxo real chega com a US-009. [REVISADO em 2026-08-08: o usuário solicitou implementar ambas as US juntas; as opções passam a delegar aos fluxos funcionais da US-009 na mesma execução.]
- Q: Quais teclas de atalho o menu deve aceitar para "digitação direta" das opções? → A: Opção A - Navegação com ↑/↓ + Enter, e digitação direta com letras de atalho em português: A (Adicionar), C (Acompanhar), L (Listar), S (Sair).
- Q: O que deve acontecer quando o usuário digita `/` seguido de um comando que não existe? → A: Opção C - Silenciosamente ignorar e manter o campo de entrada limpo.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Exibição da tela inicial estilizada (Priority: P1)

O usuário executa a aplicação sem parâmetros adicionais e é recebido por uma
tela centralizada com o título "OPENTORRENT" em arte ASCII estilizada e uma
caixa de entrada interativa no centro.

**Why this priority**: É a porta de entrada da nova experiência visual; todas as
demais funcionalidades dependem desta tela para serem acessadas.

**Independent Test**: Executar a aplicação sem parâmetros e verificar que o
título estilizado e a caixa de entrada são renderizados centralizados.

**Acceptance Scenarios**:

1. **Given** que o usuário executa a aplicação sem parâmetros adicionais no
   terminal, **When** a interface principal é carregada, **Then** o sistema deve
   exibir o título "OPENTORRENT" em texto estilizado centralizado e a caixa de
   entrada para interação.

---

### User Story 2 - Menu interativo de comandos ao digitar barra (Priority: P1)

Ao digitar o caractere `/` na caixa de entrada, o sistema exibe dinamicamente um
menu suspenso com as opções de navegação: adicionar torrent, acompanhar
progresso, listar downloads completos e sair.

**Why this priority**: É o mecanismo central de navegação entre as
funcionalidades; sem ele o usuário não consegue operar a interface.

**Independent Test**: Digitar `/` no campo de entrada e verificar a exibição do
menu com as quatro opções.

**Acceptance Scenarios**:

1. **Given** que o usuário está na tela inicial centralizada, **When** o usuário
   digita o caractere `/` no prompt de comando, **Then** o sistema deve abrir o
   menu suspenso contendo as opções de adição, acompanhamento, listagem
   detalhada de concluídos e saída.
2. **Given** que o menu está visível, **When** o usuário navega pelas opções com
   teclado ou digita diretamente, **Then** o sistema deve permitir a seleção
   pelas duas formas.

---

### User Story 3 - Listagem de downloads completos (Priority: P2)

Ao selecionar a opção de listar downloads completos, o usuário vê a lista
formatada com diretório, nome do arquivo, tamanho em disco e data de criação de
cada download concluído.

**Why this priority**: Fornece valor de consulta sem depender das demais
funcionalidades, mas só pode ser demonstrado após a existência de downloads.

**Independent Test**: Selecionar a opção de listagem com downloads finalizados
presentes e verificar a exibição das propriedades de cada item.

**Acceptance Scenarios**:

1. **Given** que existem downloads finalizados no diretório de destino, **When**
   o usuário seleciona a opção "Listar downloads completos" no menu, **Then** o
   sistema deve exibir a lista contendo o caminho da pasta, o nome do arquivo, o
   tamanho ocupado em disco e a data de criação de cada item.

---

### User Story 4 - Encerramento da aplicação via menu (Priority: P2)

Ao selecionar a opção de saída (Exit) no menu, a interface é fechada e o
controle retorna ao terminal do sistema operacional.

**Why this priority**: É essencial para o ciclo de vida da aplicação, mas é um
fluxo simples e de baixo risco.

**Independent Test**: Selecionar "Exit" no menu e verificar que a aplicação
encerra corretamente.

**Acceptance Scenarios**:

1. **Given** que o menu interativo está visível na tela, **When** o usuário
   seleciona a opção de saída (Exit), **Then** o sistema deve fechar a interface
   e retornar o controle ao terminal do sistema operacional.

---

### Edge Cases

- O que acontece quando o usuário digita `/` com um comando desconhecido? →
  Silenciosamente ignorar e manter o campo de entrada limpo (clarificação
  2026-08-08).
- O que acontece quando a lista de downloads completos está vazia?
- O que acontece quando o terminal é muito pequeno para renderizar a tela
  centralizada?
- Como a interface se comporta ao redimensionar o terminal?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: O sistema DEVE renderizar uma tela inicial centralizada com o
  título "OPENTORRENT" estilizado em arte ASCII ao ser executado sem parâmetros.
- **FR-002**: O sistema DEVE apresentar um campo de entrada interativo
  posicionado no centro da tela para digitação de instruções.
- **FR-003**: Ao digitar o caractere `/` no campo de entrada, o sistema DEVE
  exibir dinamicamente um menu com as opções: adicionar torrent à lista,
  acompanhar progresso da lista de torrents, listar downloads completos
  (diretório, nome, tamanho em disco e data de criação) e sair da aplicação.
- **FR-003a**: O campo de entrada DEVE processar apenas comandos iniciados com
  `/`; teclas ou texto que não iniciem com `/` são ignorados como digitação
  inerte, permanecendo o prompt sem resposta até o `/` ser pressionado.
- **FR-004**: O sistema DEVE permitir a seleção das opções do menu tanto por
  navegação de teclado quanto por digitação direta.
- **FR-004a**: A navegação de teclado DEVE usar as setas ↑/↓ e Enter para
  confirmar. A digitação direta DEVE aceitar as letras de atalho em português:
  A (Adicionar), C (Acompanhar), L (Listar), S (Sair).
- **FR-005**: Ao selecionar a opção de listagem de downloads completos, o
  sistema DEVE apresentar a lista formatada com as propriedades dos arquivos
  salvos (diretório, nome, tamanho em disco e data de criação).
- **FR-005a**: A listagem de downloads completos DEVE varrer apenas o diretório
  padrão `~/downloads/torrent-downloads/`, de forma independente da sessão
  persistida (US-009).
- **FR-006**: O sistema DEVE substituir todas as referências visuais, dicas e
  atalhos da interface genérica pelo contexto exclusivo e funcionalidades do
  OpenTorrent.
- **FR-006a**: As opções "Adicionar Torrent a lista" e "Acompanhar progresso da
  lista de torrents" DEVE delegar aos fluxos funcionais da US-009 (menu de
  inclusão e listagem de sessão) na mesma execução. [REVISADO em 2026-08-08 —
  originalmente previa aviso "próxima versão"; ambas as US são implementadas
  juntas.]
- **FR-007**: Ao selecionar a opção de saída (Exit), o sistema DEVE fechar a
  interface e retornar o controle ao terminal do sistema operacional.

### Key Entities *(include if feature involves data)*

- **Download concluído**: Item de download finalizado com atributos: diretório
  (caminho da pasta), nome do arquivo, tamanho em disco e data de criação.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: O título "OPENTORRENT" em arte ASCII e a caixa de entrada são
  exibidos centralizados em 100% das execuções sem parâmetros.
- **SC-002**: O menu interativo é exibido dinamicamente em 100% das digitações
  de `/` no campo de entrada.
- **SC-003**: As opções do menu são selecionáveis tanto por teclado quanto por
  digitação direta.
- **SC-004**: A listagem de downloads completos exibe diretório, nome, tamanho
  em disco e data de criação para cada item.
- **SC-005**: A seleção de "Exit" encerra a aplicação e retorna o controle ao
  terminal do sistema operacional.

## Assumptions

- A tela inicial é o ponto de entrada quando a aplicação é executada sem
  argumentos de linha de comando.
- A listagem de downloads completos consulta os arquivos finalizados no
  diretório de destino padrão e em diretórios customizados já persistidos.
- O menu de "Adicionar Torrent a lista" e "Acompanhar progresso" são pontos de
  acesso que delegam às funcionalidades da interface de sessão (US-009).
- O terminal alvo é um emulador de terminal moderno compatível com escape
  sequences e cores (ex.: GNOME Terminal, Windows Terminal).
- Navegação por teclado usa as setas ↑/↓ e Enter para confirmar; digitação
  direta aceita o texto do atalho da opção.
- A arte ASCII do título é um recurso textual renderizado por terminal, sem
  dependências externas de arte.
