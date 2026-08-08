#!/usr/bin/env bash
# Assinatura digital do binário de release (US-018).
#
# Gera uma Autoridade Certificadora (CA) local e um certificado X.509 com
# extensão de uso estendido para assinatura de código (`codeSigning`), assina o
# binário release com RSA-SHA256 (assinatura desanexada) e valida a integridade
# do executável contra a CA raiz. Qualquer falha de verificação retorna erro
# (código de saída != 0), fazendo a esteira falhar.
#
# Uso:
#   ./scripts/sign-release.sh [init|sign|verify] [BINÁRIO]
#     init    — cria a CA local e o certificado de code signing (se não existirem)
#     sign    — assina o binário e verifica a assinatura (padrão quando o
#               argumento é omitido junto com build)
#     verify  — apenas verifica a assinatura de um binário
#
# As chaves privadas e os certificados ficam em $SIGN_DIR (fora do controle de
# versão; padrão: ~/.local/share/opentorrent/signing), com permissões restritas.
set -euo pipefail

cd "$(dirname "$0")/.."

# Diretório seguro para chaves/certificados (AC-4: fora do versionamento).
SIGN_DIR="${SIGN_DIR:-$HOME/.local/share/opentorrent/signing}"
CA_KEY="$SIGN_DIR/ca.key"
CA_CERT="$SIGN_DIR/ca.crt"
CA_SERIAL="$SIGN_DIR/ca.srl"
CS_KEY="$SIGN_DIR/code-signing.key"
CS_CSR="$SIGN_DIR/code-signing.csr"
CS_CERT="$SIGN_DIR/code-signing.crt"
CS_EXT="$SIGN_DIR/code-signing.ext"
CS_PUB="$SIGN_DIR/code-signing.pub"

BIN="${2:-target/release/opentorrent}"
SIG="${BIN}.sig"

# Sanidade do ambiente
command -v openssl >/dev/null 2>&1 || {
  echo "erro: openssl não encontrado (instale com: sudo apt install openssl)" >&2
  exit 1
}
# Nota: -addext/-extfile exigem OpenSSL >= 1.1.1 (padrão no Ubuntu 20.04+).
[ -f "$BIN" ] || {
  echo "erro: binário não encontrado: $BIN (rode 'cargo build --release' antes)" >&2
  exit 1
}

# ---------------------------------------------------------------------------
# init — cria a CA local e o certificado de code signing (AC-1)
# ---------------------------------------------------------------------------
init_signing() {
  if [ -f "$CA_CERT" ] && [ -f "$CS_CERT" ] && [ -f "$CS_PUB" ]; then
    echo "assinatura: CA e certificado de code signing já existem em $SIGN_DIR"
    return
  fi

  mkdir -p "$SIGN_DIR"
  chmod 700 "$SIGN_DIR"

  # CA raiz local (autoassinada) — basicConstraints CA:TRUE
  echo "assinatura: gerando CA local em $SIGN_DIR"
  openssl req -x509 -newkey rsa:3072 -sha256 -nodes \
    -keyout "$CA_KEY" -out "$CA_CERT" -days 3650 \
    -subj "/CN=OpenTorrent Local CA/O=OpenTorrent" \
    -addext "basicConstraints=critical,CA:TRUE" \
    -addext "keyUsage=critical,keyCertSign,cRLSign"
  chmod 600 "$CA_KEY"

  # Certificado de assinatura de código emitido pela CA local,
  # com extendedKeyUsage=codeSigning (AC-1)
  echo "assinatura: gerando certificado de code signing (extendedKeyUsage=codeSigning)"
  openssl genrsa -out "$CS_KEY" 3072
  chmod 600 "$CS_KEY"
  openssl req -new -key "$CS_KEY" -out "$CS_CSR" \
    -subj "/CN=OpenTorrent Code Signing/O=OpenTorrent"

  cat > "$CS_EXT" <<'EOF'
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=codeSigning
EOF

  openssl x509 -req -in "$CS_CSR" \
    -CA "$CA_CERT" -CAkey "$CA_KEY" -CAcreateserial \
    -out "$CS_CERT" -days 825 -sha256 -extfile "$CS_EXT"

  # Chave pública do certificado (usada na verificação)
  openssl x509 -in "$CS_CERT" -pubkey -noout > "$CS_PUB"

  rm -f "$CS_CSR" "$CS_EXT"
}

# ---------------------------------------------------------------------------
# sign — assina o binário e verifica a assinatura (AC-2/AC-3)
# ---------------------------------------------------------------------------
sign_release() {
  init_signing

  echo "assinatura: assinando $BIN (RSA-SHA256)"
  openssl dgst -sha256 -sign "$CS_KEY" -out "$SIG" "$BIN"

  verify_release
  echo "assinatura: $SIG"
}

# ---------------------------------------------------------------------------
# verify — valida a assinatura e a integridade contra a CA raiz (AC-3)
# ---------------------------------------------------------------------------
verify_release() {
  [ -f "$SIG" ] || {
    echo "erro: arquivo de assinatura não encontrado: $SIG" >&2
    exit 1
  }

  # 1. O certificado de assinatura é emitido pela CA local?
  openssl verify -CAfile "$CA_CERT" -purpose any "$CS_CERT" >/dev/null 2>&1 || {
    echo "erro: certificado de assinatura não é confiável pela CA local" >&2
    exit 1
  }

  # 2. O certificado possui a extensão codeSigning?
  if ! openssl x509 -in "$CS_CERT" -noout -text 2>/dev/null | grep -q "Code Signing"; then
    echo "erro: certificado sem extensão codeSigning" >&2
    exit 1
  fi

  # 3. A assinatura do binário bate com a chave pública do certificado?
  #    (stderr do openssl é preservado aqui para facilitar o diagnóstico)
  if ! openssl dgst -sha256 -verify "$CS_PUB" -signature "$SIG" "$BIN"; then
    echo "erro: assinatura inválida — o binário foi alterado ou a assinatura não corresponde" >&2
    exit 1
  fi

  echo "assinatura: verificação OK — integridade e autenticidade confirmadas para $BIN"
}

case "${1:-sign}" in
  init) init_signing ;;
  sign) sign_release ;;
  verify) verify_release ;;
  *)
    echo "uso: $0 [init|sign|verify] [BINÁRIO]" >&2
    exit 1
    ;;
esac
