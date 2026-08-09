# US-027 — Exibição de ações para torrents em inicialização e exclusão com atalho Delete

## História

Como **Usuário do sistema**, eu quero que os torrents no estado "inicializando"
exibam o menu completo de ações `[Pausar ] [Parar ] [Excluir]` e que a tecla
`Delete` remova o torrent e seus arquivos físicos do disco, para ter controle
total de gerenciamento sobre qualquer item da lista (mesmo antes da conclusão
dos metadados) e remover arquivos rapidamente via teclado.

## Critérios de aceite

- [x] Itens com status `inicializando` ou pendentes de metadados renderizam a
      coluna `AÇÕES` com `[Pausar ] [Parar ] [Excluir]` alinhada com os demais.
- [x] Tecla `Delete` sobre a linha focada aciona a rotina de exclusão do item.
- [x] A exclusão via `Delete` remove a entrada da sessão e apaga os arquivos do
      armazenamento local (`delete_files = true`).
- [x] Confirmação rápida (modal Y/N) antes da remoção definitiva dos dados.
- [x] A lista atualiza e re-indexa os IDs imediatamente após a remoção
      (`clamp_row_index` + re-leitura da sessão a cada frame).

## Cenários de teste

### Exibição de menu de ações em torrent no estado inicializando

- **Dado** que um link foi adicionado e está no status `0.0% inicializando`
- **Quando** a tabela da sessão é renderizada
- **Então** a coluna de ações exibe `[Pausar ] [Parar ] [Excluir]` alinhada com
  o restante dos itens (inclusive origens pendentes em resolução).

### Remoção de torrent e exclusão de arquivo físico via atalho Delete

- **Dado** que o cursor está na linha de um torrent ativo ou em inicialização
- **Quando** o usuário pressiona `Delete` e confirma (`S`)
- **Então** o torrent é removido da sessão do motor e os arquivos associados no
  diretório de downloads são apagados do disco.

## Implementação

- `DeleteTarget` enum (Torrent com id/nome ou Pending com nome) + campo
  `Tui::confirming_delete`.
- `request_delete(entry_index)`: registra o alvo exato e abre a confirmação.
- `confirm_delete()`: `session.delete(Id, true)` (apaga arquivos) ou descarta a
  origem pendente; sempre `clamp_row_index`.
- `handle_session_key`: intercepta S/Y/N/Esc durante a confirmação; `Delete`
  chama `request_delete(self.row_index)`.
- Modal `render_confirm_delete`: diálogo com bordas duplas centralizado sobre o
  Body escurecido (overlay), com nome truncado e prompt `[S]im [N]ão`.
- Botão `Excluir` do mouse passa a usar a mesma confirmação; pendentes mostram
  aviso ao tentar pausar/parar.
- Pendentes em `Resolving` renderizam a coluna de ações completa quando o
  terminal é largo o suficiente.
- `remove_row` ('X') também descarta pendentes; footer ganha `[Del] excluir`.
