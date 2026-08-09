# US-029 — Verificação assíncrona de atualizações e releases ao consultar a versão

## História

Como **Usuário do sistema**, eu quero que `opentorrent --version` consulte o
repositório remoto para verificar se existe uma versão mais recente, para saber
imediatamente qual é a versão instalada e se há atualizações disponíveis, com uma
sugestão clara do comando para atualização.

## Critérios de aceite

- [x] `--version` (e `-V`) exibem a versão instalada no binário.
- [x] Chamada HTTP à API `https://api.github.com/repos/filhotecmail/opentorrent/releases/latest`.
- [x] Timeout curto (máx. 3s) via `tokio::time::timeout` — não trava o terminal.
- [x] Comparação SemVer (`semver` crate): versão remota superior → aviso
      `Nova versão disponível: vX.Y.Z (atual: vA.B.C)`.
- [x] Sugestão explícita do comando de atualização
      (`curl -L -o opentorrent https://github.com/.../download/vX.Y.Z/opentorrent-vX.Y.Z-linux-x86_64 && chmod +x opentorrent`).
- [x] Versão local mais recente → `você está na versão mais recente (vA.B.C)`.
- [x] Falha de conexão/timeout/JSON → apenas a versão local, sem erros visíveis
      (falha graciosa: `fetch_latest_release()` retorna `None`).

## Cenários de teste

### Exibição da versão instalada estando na versão mais recente

- **Dado** binário local v0.1.15 e última release no repositório v0.1.15
- **Quando** o usuário executa `opentorrent --version`
- **Então** o sistema exibe `OpenTorrent v0.1.15` e a mensagem indicando que o
  sistema já está na versão mais recente.

### Detecção de nova versão e sugestão de atualização

- **Dado** versão instalada v0.1.10 e release v0.1.12 publicada
- **Quando** o usuário executa `opentorrent --version`
- **Então** o sistema exibe a versão atual, alerta sobre a v0.1.12 disponível e
  apresenta o comando de atualização correspondente.

### Execução do comando em ambiente offline ou com falha na rede

- **Dado** máquina sem conexão ou requisição com timeout
- **Quando** o usuário executa `opentorrent --version`
- **Então** o sistema exibe apenas a versão local, ignorando a checagem remota
  sem gerar exceptions ou logs de erro no terminal.

## Implementação

- `Opts.check_version` (`--version`/`-V`) substitui o flag automático do clap.
- `run_version_check()`: imprime a versão, `build_runtime()` + `block_on`.
- `fetch_latest_release()`: GET na API com `User-Agent`, `error_for_status`,
  `serde_json::Value["tag_name"]`, timeout de 3s — `None` em qualquer falha.
- `has_update(tag, current)`: SemVer estrita, malformados → `false`.
- `update_command(tag)`: curl de download do asset + `chmod +x`.
- Deps `reqwest` (json+rustls-tls), `semver`, `serde_json` — já presentes no
  Cargo.lock via librqbit (zero novas crates no grafo além das features).
- 4 testes (`has_update_*`, `update_command_targets_release_asset`).
