# Feature Specification: Processamento assíncrono e transição fluida na adição de novos torrents

**Feature Branch**: `feat/us-014-adicao-assincrona`

**Status**: Implementado

**Input**: User story US-014 — Adicionar um novo link/arquivo de torrent em segundo plano e transicionar imediatamente para a tela de acompanhamento.

## Contexto

Antes da US-014, a confirmação da origem no diálogo de inclusão chamava `session.add_torrent` com probe `list_only`, que **bloqueia** a interface enquanto resolve metadados de magnet links via DHT/trackers (pode levar segundos). A US-014:

- Valida apenas a sintaxe no momento da submissão (`AddTorrent::from_cli_argument`, síncrono e barato).
- Registra a requisição em uma lista de pendências (`PendingTorrent`) e transiciona **imediatamente** para a tela de sessão.
- Executa a resolução/adicição em uma task `tokio::spawn` em background; a linha da lista exibe `inicializando` enquanto isso.
- Falhas assíncronas atualizam a linha para `erro: <mensagem>` sem bloquear a navegação.

## Critérios de aceite

1. Nenhuma validação bloqueante nem espera de rede no momento da submissão — `submit_include` só faz parse de sintaxe.
2. Transição imediata para a tela de Acompanhar Progresso — `view = View::Session` no mesmo frame.
3. Novo torrent figura imediatamente na lista com status inicial — linha pendente `inicializando`.
4. Resolução de metadados/redes em rotina assíncrona — `add_torrent_background` numa task.
5. Falha assíncrona → status de erro na linha, navegação desimpedida — `PendingStatus::Error`.

## Cenários de teste

- Digitar/colar um magnet extenso e confirmar → transição instantânea para a sessão com a linha `inicializando`.
- A task obtém os metadados em background → a linha pendente some e o torrent real aparece com título/progresso atualizados em tempo real.
- Link inacessível → a linha muda para `erro: <motivo>`; setas/Enter continuam funcionando.

## Notas técnicas

- `Session::add_torrent` com magnet **bloqueia** até resolver o infohash (confirmado no fonte do librqbit 8.1.1, `resolve_magnet().await`), por isso a chamada vive na task e nunca na UI.
- Estado compartilhado: `Arc<Mutex<Vec<PendingTorrent>>>`; nenhum lock atravessa `.await` (o lock é adquirido após o await, no update).
- `drain_pending_notices` consome avisos (`Done`) no início de cada render, exibindo-os como notice.
- Navegação (`move_row`) considera sessão + pendências; ações `p/r/x` em linha pendente são inofensivas (`rows.get(idx)` → `None`).
