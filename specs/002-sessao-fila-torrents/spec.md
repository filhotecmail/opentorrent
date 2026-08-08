# Feature Specification: Persistência de sessão, gerenciamento de fila e menu interativo no terminal

**Feature Branch**: `feat/us-009-sessao-fila-menu`

**Created**: 2026-08-08

**Status**: Draft

**Input**: User description: "US-009 — Persistência de sessão, gerenciamento de fila e menu interativo no terminal"

## Clarifications

### Session 2026-08-08

- Q: Como a sessão deve ser persistida para sobreviver ao fechamento do terminal? → A: Opção A - Arquivo JSON em `~/.config/opentorrent/session.json` (padrão XDG), persistindo metainfo + estado + diretório de cada torrent.
- Q: A cada quanto tempo e em quais momentos o estado da sessão deve ser salvo? → A: Opção C - A cada mudança de estado de algum torrent (event-driven).
- Q: Quais ações devem estar disponíveis por linha (por torrent) na interface de listagem da sessão? → A: Opção A - Pausar, Retomar e Remover, acionáveis por tecla ao lado de cada linha (P/R/X).
- Q: Como o usuário deve acionar a opção "Adicionar novo torrent à fila" na interface de listagem da sessão? → A: Opção A - Tecla de atalho (A ou +) abre o menu de inclusão, exibida como dica no rodapé da listagem.
- Q: Na restauração da sessão, como tratar torrents cuja origem não está mais disponível? → A: Opção A - Persistir metainfo completa; re-adicionar por metainfo; sem metainfo → pausado com aviso.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Restauração da sessão ao abrir o programa sem parâmetros (Priority: P1)

O usuário executa o programa sem parâmetros e vê a lista completa de torrents
salvos na sessão (em andamento, pausados, parados ou concluídos), com estados e
percentuais atualizados, sem perder o progresso ao fechar o terminal.

**Why this priority**: É a base de valor da US — a persistência e restauração de
sessão garantem que nenhum download se perca.

**Independent Test**: Executar o programa sem parâmetros com downloads salvos e
verificar a restauração dos estados e percentuais.

**Acceptance Scenarios**:

1. **Given** que existem downloads adicionados previamente com diferentes
   percentuais de conclusão, **When** o usuário executa o comando principal sem
   parâmetros, **Then** o sistema deve carregar os estados persistidos e
   apresentar a lista atualizada com o progresso de cada item.

---

### User Story 2 - Acesso ao menu interativo para inclusão de novo torrent (Priority: P1)

Na mesma interface de listagem, o usuário escolhe uma opção interativa para
adicionar novos torrents à fila e vê um menu simplificado com as opções de
inserção de link/arquivo ou de retorno à listagem.

**Why this priority**: Permite gerenciar a fila sem interromper os downloads
ativos, um requisito central da US.

**Independent Test**: Abrir a interface de sessão, escolher a opção de adicionar
e verificar a exibição do menu de inclusão.

**Acceptance Scenarios**:

1. **Given** que a interface de listagem de sessão está ativa, **When** o
   usuário escolhe a opção interativa de adicionar mais torrents, **Then** o
   sistema deve apresentar as opções de entrada para digitar o link/arquivo ou
   para fechar o menu e voltar à tela anterior.

---

### User Story 3 - Adição bem-sucedida de novo item pela interface (Priority: P2)

Ao fornecer uma origem válida de torrent e confirmar, a tela é limpa, a lista é
atualizada com o novo torrent na fila e o processamento dos demais itens
continua.

**Why this priority**: É o fluxo completo de inclusão sem interromper a fila.

**Independent Test**: Adicionar um torrent válido pelo menu e verificar que ele
aparece na lista e os demais continuam processando.

**Acceptance Scenarios**:

1. **Given** que o usuário está no menu interativo de adição, **When** o usuário
   escolhe a opção de digitação, fornece uma origem válida e confirma, **Then**
   o sistema deve limpar a tela, incluir o novo torrent na lista e exibir todos
   os itens em acompanhamento contínuo.

---

### User Story 4 - Cancelamento da adição e retorno à lista (Priority: P2)

Ao escolher a opção de fechar e voltar, o sistema retorna imediatamente à
listagem sem alterar a fila atual.

**Why this priority**: Garante que o cancelamento não tenha efeitos colaterais
na fila.

**Independent Test**: Abrir o menu de adição, escolher voltar e verificar que a
lista permanece inalterada.

**Acceptance Scenarios**:

1. **Given** que o usuário abriu o menu de adição, **When** o usuário escolhe a
   opção de fechar e voltar, **Then** o sistema deve retornar imediatamente à
   exibição da lista sem alterar a fila atual.

---

### Edge Cases

- O que acontece quando o arquivo de sessão não existe (primeira execução)?
- O que acontece quando uma origem informada é inválida (magnet/torrent/URL
  corrompido)?
- O que acontece quando a lista de torrents da sessão está vazia?
- O que acontece ao fechar o terminal no meio de um download?
- Como concorrem múltiplas adições simultâneas à fila?

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: O sistema DEVE salvar o estado de todos os torrents (em
  andamento, pausados, parados ou concluídos) em uma sessão permanente.
- **FR-001a**: A persistência DEVE usar um arquivo JSON em
  `~/.config/opentorrent/session.json` (padrão XDG), contendo metainfo, estado e
  diretório de cada torrent.
- **FR-001b**: A sessão DEVE ser persistida a cada mudança de estado de algum
  torrent (event-driven), garantindo que o progresso recente não se perca no
  encerramento.
- **FR-002**: Ao ser executado sem parâmetros, o sistema DEVE restaurar a
  sessão e exibir a lista completa de torrents com seus estados e percentuais
  atualizados.
- **FR-002a**: Na restauração, o sistema DEVE re-adicionar torrents pela
  metainfo completa persistida. Torrents sem metainfo disponível DEVE ser
  marcados como pausados com aviso.
- **FR-003**: A lista de downloads exibida na sessão DEVE apresentar opções
  interativas de ação por linha para cada torrent.
- **FR-003a**: As ações por linha DEVE ser Pausar (P), Retomar (R) e Remover da
  fila (X), acionáveis por tecla ao lado de cada linha.
- **FR-004**: O sistema DEVE apresentar, na mesma interface de listagem, uma
  opção interativa para adicionar novos torrents à fila.
- **FR-004a**: A opção de adicionar torrent DEVE ser acionada pela tecla de
  atalho (A ou +), com dica exibida no rodapé da listagem.
- **FR-005**: Ao selecionar a opção de inclusão, o sistema DEVE exibir um menu
  simplificado com as opções de inserção do link/arquivo ou de retorno à
  listagem.
- **FR-006**: Ao confirmar a inclusão de um novo item, o sistema DEVE limpar a
  tela, atualizar a lista e incluir o novo torrent na fila, mantendo o
  processamento dos demais itens.
- **FR-007**: Ao escolher a opção de voltar no menu de adição, o sistema DEVE
  retornar à listagem sem alterar a fila atual.

### Key Entities *(include if feature involves data)*

- **Sessão**: Estado persistido contendo todos os torrents conhecidos.
- **Torrent de sessão**: Item com identificador (metainfo), nome, estado
  (em andamento, pausado, parado, concluído) e percentual de conclusão.
- **Fila**: Coleção ordenada de torrents gerenciada na interface de sessão.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: O estado de todos os torrents é persistido em 100% das execuções,
  sobrevivendo ao fechamento do terminal.
- **SC-002**: Ao executar sem parâmetros, a sessão é restaurada e a lista de
  torrents exibida com estados e percentuais corretos.
- **SC-003**: Novos torrents podem ser adicionados à fila sem interromper os
  downloads ativos.
- **SC-004**: O cancelamento da adição retorna à listagem sem alterar a fila.

## Assumptions

- A persistência usa um arquivo de sessão local no diretório de configuração do
  usuário (ex.: `~/.config/opentorrent/` ou equivalente do sistema).
- A restauração da sessão re-adiciona os torrents ao motor librqbit a partir da
  metainfo salva; torrents cuja origem não esteja mais disponível são marcados
  como pausados/parados.
- A interface de sessão e a tela inicial (US-010) fazem parte da mesma execução
  sem parâmetros: a tela inicial é o ponto de entrada e a sessão é a listagem
  de acompanhamento.
- O percentual exibido é derivado do progresso real do download do torrent.
- "Fechar o terminal sem perder progresso" refere-se ao estado persistido; o
  download em si é retomado pelo motor na próxima execução.
