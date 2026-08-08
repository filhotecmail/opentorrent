# US-018 — Assinatura digital do binário de release com certificado de Code Signing e CA local

## Descrição

O binário da aplicação gerado em modo release deve ser automaticamente assinado
utilizando uma chave de Code Signing derivada de uma Autoridade Certificadora
(CA) local, garantindo integridade, autenticidade e rastreabilidade do
executável.

## Critérios de aceite

- **AC-1** — O pipeline de compilação deve gerar uma CA local e um certificado
  X.509 com `extendedKeyUsage = codeSigning`, caso não existam.
- **AC-2** — O binário compilado em release (`target/release/opentorrent`) deve
  ser assinado digitalmente, gerando seu arquivo de assinatura verificável.
- **AC-3** — O processo de assinatura deve validar a integridade do executável
  após a compilação, falhando a esteira se a verificação não for bem-sucedida.
- **AC-4** — Chaves privadas e certificados de assinatura armazenados de forma
  segura e fora do controle de versão do repositório.

## Implementação

### `scripts/sign-release.sh`

Script com três subcomandos:

- **`init`** — Cria (se não existirem) a CA local autoassinada e o certificado
  de code signing emitido por ela (AC-1):
  - CA: RSA 3072, `basicConstraints=CA:TRUE`, `keyUsage=keyCertSign,cRLSign`,
    validade 3650 dias.
  - Certificado: RSA 3072, `basicConstraints=CA:FALSE`,
    `keyUsage=digitalSignature`, `extendedKeyUsage=codeSigning`, validade 825
    dias.
- **`sign`** — Gera a assinatura RSA-SHA256 desanexada (`binário.sig`) e chama
  `verify` (AC-2).
- **`verify`** — Valida contra a CA raiz (AC-3):
  1. o certificado é emitido pela CA local (`openssl verify`);
  2. o certificado possui a extensão `Code Signing`;
  3. a assinatura do binário bate com a chave pública do certificado
     (`openssl dgst -verify`) — qualquer falha retorna exit ≠ 0.

### Integração no `build.sh`

Após `cargo build --release`, o `build.sh` executa
`./scripts/sign-release.sh sign` — a assinatura e a verificação são parte da
esteira de build release; falha aborta o build (`set -euo pipefail`).

### Armazenamento seguro (AC-4)

- Chaves/certificados em `$SIGN_DIR` (padrão
  `~/.local/share/opentorrent/signing/`), com permissão de diretório `700` e
  chaves privadas `600`.
- Fora do controle de versão: diretório `.signing/` e `*.sig` adicionados ao
  `.gitignore`.

## Cenários de teste

### Geração de assinatura e verificação bem-sucedida

Dado o binário `target/release/opentorrent` compilado, quando a rotina de
assinatura é executada com o certificado e a chave privada, então o arquivo
`opentorrent.sig` é criado e a verificação com a CA raiz retorna sucesso.

### Detecção de alteração e falha na verificação

Dado o binário assinado com conteúdo alterado posteriormente, quando a
verificação de assinatura é disparada contra a CA raiz, então o sistema rejeita
a assinatura e reporta erro de integridade (exit ≠ 0).
