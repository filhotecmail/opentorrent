#!/usr/bin/env bash
set -euo pipefail

# Atualiza as seções dinâmicas do README.md (badges e estado do projeto).
# Chamado pelo workflow .github/workflows/readme-live.yml após mudanças no repo.

REPO="${GITHUB_REPOSITORY:-filhotecmail/opentorrent}"
FILE="README.md"
[ -f "$FILE" ] || { echo "ERRO: $FILE não encontrado" >&2; exit 1; }

TMPDIR_C="${TMPDIR:-/tmp}/opencode"
mkdir -p "$TMPDIR_C"

gh api "repos/$REPO" > "$TMPDIR_C/repo.json" 2>/dev/null || exit 1
open_issues=$(jq -r '.open_issues_count' "$TMPDIR_C/repo.json")
default_branch=$(jq -r '.default_branch' "$TMPDIR_C/repo.json")

gh api "repos/$REPO/milestones?state=open" > "$TMPDIR_C/mils.json" 2>/dev/null || exit 1
milestone=$(jq -r '.[0].title // "—"' "$TMPDIR_C/mils.json")

gh api "repos/$REPO/labels" > "$TMPDIR_C/labels.json" 2>/dev/null || exit 1
nlabels=$(jq -r 'length' "$TMPDIR_C/labels.json")

latest_tag=$(gh release view --repo "$REPO" --json tagName -q .tagName 2>/dev/null || echo "—")

# --- 1) Badges dinâmicos ----------------------------------------------
ci_badge="[![CI](https://github.com/$REPO/actions/workflows/ci.yml/badge.svg?branch=$default_branch)](https://github.com/$REPO/actions/workflows/ci.yml)"
cov_badge="[![Cobertura](https://codecov.io/gh/$REPO/branch/$default_branch/graph/badge.svg)](https://codecov.io/gh/$REPO)"
release_badge="[![Release](https://img.shields.io/badge/release-$latest_tag-blue)](https://github.com/$REPO/releases)"
issues_badge="[![Issues abertas](https://img.shields.io/github/issues/$REPO)](https://github.com/$REPO/issues)"

badges="$ci_badge
$cov_badge
$release_badge
$issues_badge"

estado="| Estado | Valor |
| --- | --- |
| Branch principal | \`$default_branch\` |
| Última release | \`$latest_tag\` |
| Milestone atual | $milestone |
| Issues abertas | $open_issues |
| Labels do projeto | $nlabels |"

cargo_info="| Metadado | Valor |
| --- | --- |
| Pacote | — |
| Versão | — |
| Edição Rust | — |"
if [ -f Cargo.toml ]; then
  pkg_name=$(awk -F'=' '/^name/ {gsub(/[" ]/,"",$2); print $2; exit}' Cargo.toml)
  pkg_ver=$(awk -F'=' '/^version/ {gsub(/[" ]/,"",$2); print $2; exit}' Cargo.toml)
  rust_ed=$(awk -F'=' '/^edition/ {gsub(/[" ]/,"",$2); print $2; exit}' Cargo.toml)
  cargo_info="| Metadado | Valor |
| --- | --- |
| Pacote | \`$pkg_name\` |
| Versão | \`$pkg_ver\` |
| Edição Rust | $rust_ed |"
fi

# --- Aplicação (via Python para evitar conflito de delimitadores) -------
BADGES="$badges" ESTADO="$estado" CARGO="$cargo_info" python3 - "$FILE" <<'PY'
import os, sys

f = sys.argv[1]
with open(f, encoding="utf-8") as fh:
    content = fh.read()

def block(name):
    return os.environ[name].strip()

for marker, var in (("BADGES", "BADGES"), ("ESTADO", "ESTADO"), ("CARGO", "CARGO")):
    start = f"<!-- {marker}_START -->"
    end = f"<!-- {marker}_END -->"
    s = content.find(start)
    e = content.find(end)
    if s == -1 or e == -1:
        print(f"AVISO: marcadores {marker} não encontrados; pulando.")
        continue
    e += len(end)
    content = content[:s] + start + "\n" + block(var) + "\n" + end + content[e:]

with open(f, "w", encoding="utf-8") as fh:
    fh.write(content)

print("README.md atualizado.")
PY
