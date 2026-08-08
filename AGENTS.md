# AGENTS.md

Guia de operação para agentes de IA que trabalham neste repositório.

## Projeto

**OpenTorrent** — cliente BitTorrent de linha de comando em Rust (CLI) para
Ubuntu/Linux. Baixa torrents, magnet links e arquivos `.torrent` (local ou via
URL), lista conteúdo, filtra arquivos por regex e mostra progresso em tempo
real. O motor é o `librqbit` (BitTorrent 100% Rust).

- Stack: Rust (edition 2021) + `librqbit`, `tokio`, `clap`, `anyhow`, `futures`,
  `size_format`, `tracing`, `tracing-subscriber`.
- Ferramentas de verificação: `cargo fmt`, `cargo clippy -- -D warnings`,
  `cargo test`.

## Pipeline de desenvolvimento (Spec Kit)

O projeto usa o fluxo **Spec Kit** via comandos opencode (`.opencode/commands/`):

1. `/speckit.clarify` — reduz ambiguidades do spec com perguntas dirigidas
2. `/speckit.constitution` — cria/atualiza a constituição do projeto
   (`.specify/memory/constitution.md`)
3. `/speckit.specify` — cria a especificação da feature (`specs/NNN-*/spec.md`)
4. `/speckit.plan` — gera o SDD (`.specify/templates/plan-template.md` +
   `specs/NNN-*/plan.md`, `research.md`, `data-model.md`, `contracts/`,
   `quickstart.md`)
5. `/speckit.tasks` — quebra o plano em tarefas (`tasks.md`)
6. `/speckit.checklist` — gera checklists de qualidade de requisitos
7. `/speckit.implement` — executa as tarefas do `tasks.md`
8. `/speckit.analyze` / `/speckit.converge` — análise e consolidação

Antes de iniciar qualquer fase do pipeline, consulte a constituição
(`.specify/memory/constitution.md`) e as instruções de Rust Skills
(`~/rust-skills/.opencode/instructions/rust-skills.md`).

## Rust Skills (obrigatório para código Rust)

O projeto integra as **Rust Skills** de `https://github.com/actionbook/rust-skills`
(clonadas em `~/rust-skills/` — fora do repositório, não versionadas — e
carregadas pelo opencode via `.opencode/opencode.json` → `skills.paths`). São 38
skills com o framework de **meta-cognição em 3 camadas** (Domínio → Design →
Mecânica).

Setup (uma vez):

```bash
git clone --depth 1 https://github.com/actionbook/rust-skills.git ~/rust-skills
# atualizar depois:
git -C ~/rust-skills pull
```

Regras de uso:

- **Antes de responder/planejar qualquer questão de Rust**, rode o
  `rust-router` para identificar a camada de entrada e a skill correta.
- **Camada 1 (mecânica)**: `m01-ownership`, `m02-resource`, `m03-mutability`,
  `m04-zero-cost`, `m05-type-driven`, `m06-error-handling`, `m07-concurrency`.
- **Camada 2 (design)**: `m09-domain`, `m10-performance`, `m11-ecosystem`,
  `m12-lifecycle`, `m13-domain-error`, `m14-mental-model`, `m15-anti-pattern`.
- **Camada 3 (domínio)**: `domain-cli` (relevante para este projeto),
  `domain-web`, `domain-fintech`, `domain-ml`, `domain-iot`, `domain-embedded`,
  `domain-cloud-native`.
- **Consultas transversais**: `coding-guidelines` (convenções P.NAM/P.FMT/P.ERR),
  `unsafe-checker` (FFI/unsafe), `rust-learner` (versões/crates), `rust-daily`.
- Sempre consulte a skill de erro ao tratar códigos de erro do compilador
  (E0382 → `m01-ownership`, E0277 → `m04-zero-cost`, etc.).

Não responda sintomas com remendos superficiais (ex.: "use `.clone()`"): trace
pelas camadas cognitivas e entregue a solução arquiteturalmente correta para o
domínio.

## Convenções

- Documentação e respostas do pipeline em **PT-BR**.
- Mudanças de governança passam por `/speckit.constitution` (versão semver).
- SDD/planos devem registrar quais skills Rust Skills foram consultadas
  (seção `Rust Skills Check` do `plan-template.md`).

## Cargo-Skill (contexto ativo por camadas)

O projeto usa o **cargo-skill** (`https://lib.rs/crates/cargo-skill`) como índice
de regras Rust em camadas, complementar às Rust Skills. Ele escreve um contexto
efêmero em `.skill/context.md` (gitignored) que define quais camadas o agente
deve usar na sessão.

Setup (uma vez):

```bash
cargo install cargo-skill
cargo skill init            # detecta repo + agentes, garante .skill/ no .gitignore
```

Regras de uso no pipeline:

- **Quando existe `.skill/context.md`**, carregue-o como contexto ativo da
  sessão antes de escrever/analisar código Rust.
- **Consulta pontual de regra**: `cargo skill lookup <prefixo>` (ou shorthand
  `cargo skill own`, `cargo skill err`, etc.). Prefixos: `own`, `err`, `mem`,
  `api`, `async`, `opt`, `type`, `perf`, `test`, `doc`, `name`, `proj`, `lint`,
  `anti`.
- **Raciocínio (design)**: `cargo skill think` (camadas 1+2).
- **Execução (implementar)**: `cargo skill write` (camadas 1+2+3).
- **Workflows dirigidos**: `cargo skill review`, `cargo skill refactor`,
  `cargo skill debug`.
- **Limpeza**: `cargo skill clear`; **estado**: `cargo skill status`.
- O índice completo de regras (Layer 1) está versionado na seção
  `# Rust Skill Reference` abaixo — consulte-o offline sem custo de I/O.

## Otimização de Tempos de Compilação

Referência oficial: <https://corrode.dev/blog/tips-for-faster-rust-compile-times>
( Matthias Endler, atualizado em 2026-03-14 ). guidelines abaixo são obrigatórios
para o loop de desenvolvimento local e para CI; princípio **VIII da constituição**
(`Build Velocity`, NON-NEGOTIABLE) determina as regras mínimas.

### Loop de desenvolvimento (ordem por impacto)

1. **`rustup update`** — mantenha toolchain atualizada; o rustc ganhou ~30-40% de
   velocidade YoY, alguns projetos até 45%+. Custo quase zero, ganho garantido.
2. **`cargo check` no lugar de `cargo build`** — 2-3x mais rápido; faz type/borrow
   check sem gerar binário. Parar com `cargo build` no loop de edição. Combinar
   com `cargo watch` (`cargo install cargo-watch`) para re-executar em mudança de
   arquivo; `cargo watch -c` limpa a tela antes de cada run.
3. **Remover dependências não usadas** periodicamente:
   ```bash
   cargo install cargo-machete && cargo machete
   cargo install cargo-shear
   cargo install cargo-udeps --locked   # nightly, detecta feature-gated unused
   ```
   Combinar os três (cada um tem falsos positivos/negativos) para máxima cobertura.
4. **Atualizar dependências**:
   - `cargo update` — semver-compatible.
   - `cargo outdated -wR` — versões incompatíveis; corrigir código.
   - `cargo tree --duplicate` — consolidar duplicações a uma versão.
   - `cargo audit` — vulnerabilidades e crates deprecados.
5. **`cargo build --timings`** — identificar crates lentos; o gráfico mostra
   crates esperando CPU (vermelho), inativos (azul), ativos (verde). Foco na crate
   bottlenecks para melhorar paralelismo.
6. **`cargo rustc -- -Zself-profile`** — perfilar a compilação; visualizar com
   flamegraph ou Chrome profiler (`chrome://tracing`).
7. **`cargo llvm-lines`** — identificar funções genéricas que mais geram código LLVM
   (`drop_in_place`, `merge_sort`, `RawVec::reserve_internal`, etc.). Reduzir
   monomorfização explícita onde possível (ver "Generics" abaixo).
8. **`cargo-features-manager`** — `cargo install cargo-features-manager && cargo features prune`
   para desativar features não usadas de dependências (ex.: `tokio` tem múltiplas,
   `bindgen` habilita `clap` por padrão para uso de binário, geralmente não
   necessário para uso como biblioteca). Após pruning, conferir `Cargo.lock` diff.
9. **Feature-gating de código caro** (`[features] json = ["serde_json"]`) — mesmo
   dentro do crate. Compilar só o caminho necessário na sua feature de trabalho.
10. **Diagnóstico de rebuilds misteriosos**:
    ```bash
    export CARGO_LOG="cargo::core::compiler::fingerprint=info"
    cargo build -vv | rg "dirty|Dirty"
    ```
    Causas comuns: diferenças em `CC`/`CXX`/`VIRTUAL_ENV`/`PATH` entre Makefile,
    rust-analyzer e CI; feature flag mismatches; perfis diferentes; timestamps de
    arquivos gerados. Uniformizar ambiente entre ferramentas (ver
    `rust-analyzer.check.extraEnv` em `.vscode/settings.json`).

### Substituir dependências pesadas

`cargo tree` mostra o peso de cada dependência; `cargo-bloat --time` informa o
tempo de build por crate. Trocas comuns:

| Crate       | Alternativa            |
|-------------|------------------------|
| `serde`     | `miniserde`, `nanoserde` |
| `reqwest`   | `ureq`                 |
| `clap`      | `lexopt`               |

Split de crates grandes em **workspaces** (`[workspace]`) — só recompila crates
alterados; usado por `servo`, `vector`, `bevy`. `cargo-hakari` resolve
"workspace-hack" quando uma dep aparece com features diferentes entre crates
(pode reduzir builds consecutivos em ~50%). Ver docs em
<https://docs.rs/cargo-hakari>.

### Linker rápido

Verificar gargalo de link com `cargo +nightly rustc --bin <bin> -- -Z time-passes`.
Se `link`/`run_linker` for dominante, trocar o linker:

| Linker | Plataforma  | Pronto p/ produção | Notas                                  |
|--------|-------------|-------------------|----------------------------------------|
| `lld`  | Linux/macOS | Sim               | Drop-in para o linker do sistema       |
| `mold` | Linux       | Sim               | Otimizado para Linux                   |
| `zld`  | macOS       | Não (deprecated)  | Drop-in para `ld` da Apple             |

Configurar em `.cargo/config.toml`:
```toml
[target.x86_64-unknown-linux-gnu]
rustflags = ["-C", "link-arg=-fuse-ld=mold"]
```

### Perfis de compilação para velocidade de dev

```toml
[profile.dev]
debug = 0              # sem debuginfo: builds mais rápidos; backtraces só nome de fn
split-debuginfo = "unpacked"  # macOS: builds incrementais muito mais rápidos
strip = "debuginfo"

[profile.dev.build-override]
opt-level = 3           # acelerar proc-macros e build scripts (rodam em cada build)

[profile.dev.package."*"]
opt-level = 3           # dependências compiladas com O3 uma vez, cacheadas
```

**Linux**: usar `debug = 0` **e** `strip = "debuginfo"`.
**macOS**: `debug = 0` (rustc usa `strip` externo); `split-debuginfo = "unpacked"` para builds incrementais.
**Windows**: testar ambos para ver qual é mais rápido.

Nota: alterar `debug`/`strip` invalida cache e pode forçar rebuild completo.
Se precisar de linha em backtrace, use `split-debuginfo = "unpacked"` como trade-off.

### Proc-macros

- `[profile.dev.build-override] opt-level = 3` (acima) — acelera compilação e
  execução repetida de proc-macros.
- `cargo expand` — ver código expandido; não escala para codebases grandes.
- `RUSTFLAGS="-Zmacro-stats" cargo +nightly build` — quantifica overhead de macros
  e ajuda a focar otimização. Já produziu ganhos em `bevy` e `arbitrary`.
- Feature-gate `serde` no crate compartilhado: torná-lo `optional = true` e usar
  `#[cfg(feature = "serde")]`/`#[cfg_attr(feature = "serde", derive(...))]`.
  Ativar `features = ["serde"]` só nas leaf crates que serializam. Evita esperar
  serde compilar ao buildar partes do projeto que não precisam de serialização.
- `watt` (experimental) — pré-compila macros para Wasm, poupando downstream users
  de compilar a lógica da macro (~20s p/ macro complexa → ~0.3s por proc-macro
  crate + ~3s runtime compartilhado).

### Generics — inner non-generic function

Padrão std (`std::fs::read_to_string`): wrapper genérico delega para função interna
non-generic que compila uma única vez:

```rust
pub fn parse<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    fn inner(path: &Path) -> io::Result<Vec<u8> { /* trabalho real */ }
    inner(path.as_ref())
}
```

Evita monomorfização explosiva quando a função é usada com muitos tipos.

### Cranelift (backend alternativo, nightly)

`rustc_codegen_cranelift` — backend paralelo ao LLVM, builds dev mais rápidos.
Binários funcionais mas não tão otimizados. Para experimentar:
```bash
rustup component add rustc-codegen-cranelift-preview --toolchain nightly
RUSTFLAGS="-C codegen-backend=cranelift" cargo +nightly build
```

### Frontend paralelo (nightly)

`RUSTFLAGS="-Z threads=8" cargo +nightly build` — ativa frontend paralelo;
benchmarks mostram até ~50% de redução em tempo de compilação (varia por código).
Para tornar default, adicionar a `~/.cargo/config.toml`:
```toml
[build]
rustflags = ["-Z", "threads=8"]
```

### Cache de dependências

- **`sccache`** (Mozilla) — cacheia crates compilados. Útil quando múltiplos
  projetos compartilham deps nas mesmas versões (ex.: build servers). Ganho
  marginal em setup single-project.
- **`cargo-add-dynamic`** (`cargo install cargo-add-dynamic`) — converte dep em
  dylib (`[lib] crate-type = ["dylib"]`). Poupa tempo de link ao mudar só seu
  código; a dep só recompila ao mudar feature/versão. Ex.:
  ```bash
  cargo add-dynamic polars --features csv-file,lazy,list,describe,rows,fmt,strings,temporal
  ```
- **`criner`** — baixa offline todos os crates de crates.io (~50GB) para builds
  sem rede. Útil em conexões lentas.

### Filesystem / hardware

- Linux: montar volume `ext4` com
  `-o noauto_da_alloc,data=writeback,lazytime,journal_async_commit,commit=999,nobarrier`
  no `target/` — guarda no page cache, writeback depois. Tratar como scratch
  (dados podem ser corrompidos em crash). Alternativa a `tmpfs` (limitado a RAM+swap).
- `mold`/`lld` eliminam gargalo de link (linkers single-threaded são lentos).
- Compilar em scratch disk (RAM-backed) se I/O for bottleneck.
- Hardware: many-core (AMD Ryzen Threadripper + 32GB RAM) ou Apple M-series
  (M1/M2/M3 Max). Codespaces/Northflank para offload cloud se necessário.

### Testes

- **`cargo-nextest`** (`cargo install cargo-nextest && cargo nextest run`) — até
  60% mais rápido que `cargo test` graças a paralelismo. Benchmarks de referência
  (do post): crucible 3.38x, guppy 2.29x, mdBook 2.31x, meilisearch 1.96x,
  omicron 2.19x, tokio 2.09x.
- **Combinar testes de integração num único binário** (em `tests/main.rs` com
  `mod`): o rustc cria um binário por arquivo em `tests/` e cada um é linkado
  separadamente. Consolidar poupa link time (até ~50%). Trade-off: expor tipos
  internos (`pub`) para acesso entre módulos.
- **Tests lentos sob feature flag**:
  ```rust
  #[test]
  fn integration_with_real_torrent() {
      if std::env::var("RUN_SLOW_TESTS").is_err() { return; }
      // ... teste caro ...
  }
  ```
  Rodar local só com `RUN_SLOW_TESTS=1 cargo nextest run`.

### Caches de CI / dependências

- **GitHub Actions** — `Swatinem/rust-cache@v2` como step único; cacheia
  `~/.cargo` e `target/` entre runs.
- **`cargo-chef`** para Docker — plano em 3 stages (`planner` → `cacher` →
  `builder`) cacheia dependências em layer separada do código. Reportado 5x
  speed-up (~10min → ~2min, codebase 14k LOC, 500 deps).
- **Earthly** — Satellites (build runners cacheados) eliminam transfer de cache.
  `IMPORT github.com/earthly/lib/rust` abstrai cache config. Ver
  <https://earthly.dev/rust>.
- **CI tweaks**:
  - `CARGO_INCREMENTAL= 0` — incremental em CI (from-scratch builds) só adiciona overhead.
  - `[profile.dev] debug = 0` e `strip = "debuginfo"` — debuginfo em CI é ruído.
  - `RUSTFLAGS: -D warnings` globalmente (não `#![deny(warnings)]` in-code).
  - Separar step de compile (`cargo test --no-run --locked`) do step de run
    (`cargo test -- --nocapture --quiet`) para medir onde o tempo vai.
  - Runner mais rápido: `ubicloud` (Linux), `buildjet`, `runs-on` — mais cores,
    menos tempo de build.

### IDE (rust-analyzer / VS Code)

- **Separar `targetDir` do rust-analyzer** em `settings.json`:
  ```json
  { "rust-analyzer.cargo.targetDir": true }
  ```
  Builda em `target/rust-analyzer` sem interferir com `cargo run`. Relatos mostram
  30s → 3.8s em alguns casos. Evita invalidação de cache do `cargo build` por
  causa do rust-analyzer.
- Configurar `rust-analyzer.check.extraEnv` com environment matching do Make/CI
  (`CC`, `CXX`, `VIRTUAL_ENV`, `PATH`) para evitar rebuilds por fingerprint mismatch.
- Fechar projetos não relacionados — cada janela VS Code roda sua própria cópia
  do rust-analyzer.
- Poucos breakpoints: cada breakpoint atrasa debug sessions.

### macOS específico

- `split-debuginfo = "unpacked"` no `[profile.dev]` — já default em nightly. Até
  70% de redução(reportado).
- Excluir terminal do Gatekeeper (Security & Privacy → Developer Tools, habilitar
  terminal/iTerm/Ghostty). `sudo spctl developer-mode enable-terminal`. Poucos
  segundos por iteração.

### Windows específico

- **Dev Drive** (Windows 11) — filesystem otimizado para dev, ~20-30% de speedup
  (medições Microsoft). Mover para Dev Drive: `CARGO_HOME`, código do projeto,
  `target/`. Adicionar essas pastas às exclusões do antivírus (Virus & threat
  protection settings) para ganho adicional.

### Ferramentas úteis (instalação rápida)

```bash
cargo install cargo-watch cargo-nextest cargo-machete cargo-shear cargo-udeps \
             cargo-features-manager cargo-llvm-lines cargo-bloat cargo-hakari \
             cargo-add-dynamic cargo-chef cargo-outdated cargo-audit sccache
```

---

# Rust Skill Reference

This section provides a quick lookup index for Rust development rules.


# Layer 1 — Lookup: Rust Rule Index

Quick-reference rule index for Rust development. Use `cargo skill lookup <prefix>` to filter.

Priority: CRITICAL > HIGH > MEDIUM > LOW > REF

---

## **own-** — Ownership & Borrowing (CRITICAL)

- **own-01** `borrow-over-clone` — Prefer `&T` over `.clone()`; clone only when ownership is required
- **own-02** `slice-over-vec` — Accept `&[T]` not `&Vec<T>`; accept `&str` not `&String`
- **own-03** `cow-conditional` — Use `Cow<'a, T>` when data is sometimes owned, sometimes borrowed
- **own-04** `arc-shared` — Use `Arc<T>` for thread-safe shared ownership across threads
- **own-05** `rc-single-thread` — Use `Rc<T>` for single-threaded shared ownership
- **own-06** `refcell-interior` — Use `RefCell<T>` for interior mutability in single-threaded code
- **own-07** `mutex-interior` — Use `Mutex<T>` for interior mutability in multi-threaded code
- **own-08** `rwlock-readers` — Use `RwLock<T>` when reads significantly outnumber writes
- **own-09** `copy-small` — Derive `Copy` for small, trivially-copyable types
- **own-10** `clone-explicit` — Make `Clone` explicit; never rely on implicit copies
- **own-11** `move-large` — Move large data instead of cloning it
- **own-12** `lifetime-elision` — Rely on lifetime elision; annotate only when compiler requires it

---

## **err-** — Error Handling (CRITICAL)

- **err-01** `thiserror-lib` — Use `thiserror` for library error types
- **err-02** `anyhow-app` — Use `anyhow` for application-level error handling
- **err-03** `result-over-panic` — Return `Result<T, E>` for expected errors; never `panic!` on recoverable conditions
- **err-04** `context-chain` — Add context with `.context()` or `.with_context()` at every error boundary
- **err-05** `no-unwrap-prod` — Never use `.unwrap()` in production code paths
- **err-06** `expect-bugs-only` — Use `.expect("msg")` only for programming errors that should never occur
- **err-07** `question-mark` — Use `?` operator for clean error propagation; avoid nested `match` on `Result`
- **err-08** `from-impl` — Use `#[from]` for automatic `From` conversion between error types
- **err-09** `source-chain` — Use `#[source]` to preserve the underlying error in the chain
- **err-10** `lowercase-msg` — Error messages: lowercase, no trailing punctuation
- **err-11** `doc-errors` — Document all error conditions under `# Errors` in rustdoc
- **err-12** `custom-type` — Define custom error types; never use `Box<dyn Error>` as a return type

---

## **mem-** — Memory Optimization (CRITICAL)

- **mem-01** `with-capacity` — Use `Vec::with_capacity(n)` and `String::with_capacity(n)` when size is known
- **mem-02** `smallvec` — Use `SmallVec<[T; N]>` for collections usually smaller than N elements
- **mem-03** `arrayvec` — Use `ArrayVec<T, N>` for strictly bounded-size collections
- **mem-04** `box-large-variant` — Box large enum variants to keep the enum size small
- **mem-05** `boxed-slice` — Use `Box<[T]>` instead of `Vec<T>` for fixed-length sequences
- **mem-06** `thinvec` — Use `ThinVec` for frequently-empty vectors (1 pointer vs 3)
- **mem-07** `clone-from` — Use `dst.clone_from(&src)` to reuse allocations instead of `dst = src.clone()`
- **mem-08** `reuse-collections` — Reuse collections across iterations with `.clear()` instead of reallocating
- **mem-09** `avoid-format` — Avoid `format!()` when a string literal or `write!()` suffices
- **mem-10** `write-over-format` — Use `write!(buf, ...)` into an existing buffer instead of `format!(...)`
- **mem-11** `arena-allocator` — Use arena allocators (`bumpalo`) for batch allocations with shared lifetime
- **mem-12** `zero-copy` — Use zero-copy patterns with slices and `bytes::Bytes` instead of copying data
- **mem-13** `compact-string` — Use `CompactString` or `SmolStr` for strings usually under 24 bytes
- **mem-14** `smaller-integers` — Use the smallest integer type that fits the domain (`u8`, `u16`, etc.)
- **mem-15** `assert-type-size` — Use `static_assertions::assert_eq_size!` to catch hot-type size regressions

---

## **api-** — API Design (HIGH)

- **api-01** `builder-pattern` — Use Builder pattern for structs with more than 3 optional fields
- **api-02** `builder-must-use` — Add `#[must_use]` to builder types and their final build method
- **api-03** `newtype-safety` — Use newtypes to encode domain distinctions in the type system
- **api-04** `typestate` — Use typestate pattern for compile-time state machine enforcement
- **api-05** `sealed-trait` — Seal traits with a private supertrait to prevent external implementations
- **api-06** `extension-trait` — Use extension traits to add methods to foreign types
- **api-07** `parse-dont-validate` — Parse unvalidated input into validated types at system boundaries
- **api-08** `impl-into` — Accept `impl Into<T>` for ergonomic string and conversion inputs
- **api-09** `impl-asref` — Accept `impl AsRef<Path>` / `impl AsRef<str>` for borrowed inputs
- **api-10** `must-use` — Add `#[must_use]` to all `Result`-returning and pure functions
- **api-11** `non-exhaustive` — Use `#[non_exhaustive]` on public enums/structs to allow future fields
- **api-12** `from-not-into` — Implement `From<T>`, never `Into<T>` directly (auto-derived)
- **api-13** `default-impl` — Implement `Default` for all types with a sensible zero/empty state
- **api-14** `common-traits` — Eagerly derive `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash` where valid
- **api-15** `serde-optional` — Gate `Serialize` / `Deserialize` behind a `serde` feature flag

---

## **async-** — Async/Await (HIGH)

- **async-01** `tokio-runtime` — Use Tokio as the production async runtime
- **async-02** `no-lock-await` — Never hold a `Mutex` or `RwLock` guard across an `.await` point
- **async-03** `spawn-blocking` — Use `tokio::task::spawn_blocking` for CPU-bound or blocking I/O work
- **async-04** `tokio-fs` — Use `tokio::fs` instead of `std::fs` in async contexts
- **async-05** `cancellation-token` — Use `CancellationToken` from `tokio-util` for graceful shutdown
- **async-06** `join-parallel` — Use `tokio::join!` to run independent futures in parallel
- **async-07** `try-join` — Use `tokio::try_join!` for parallel fallible futures; fails fast on first error
- **async-08** `select-racing` — Use `tokio::select!` for racing futures or implementing timeouts
- **async-09** `bounded-channel` — Use bounded channels (`mpsc`) to apply backpressure
- **async-10** `mpsc-queue` — Use `tokio::sync::mpsc` for work queues (multi-producer, single-consumer)
- **async-11** `broadcast-pubsub` — Use `tokio::sync::broadcast` for fan-out pub/sub patterns
- **async-12** `watch-latest` — Use `tokio::sync::watch` for sharing the latest value across tasks
- **async-13** `oneshot-response` — Use `tokio::sync::oneshot` for request/response patterns
- **async-14** `joinset-structured` — Use `JoinSet` for managing a dynamic set of spawned tasks
- **async-15** `clone-before-await` — Clone shared data before the `.await` point; release locks first

---

## **opt-** — Compiler Optimization (HIGH)

- **opt-01** `inline-small` — Use `#[inline]` on small, frequently-called functions in hot paths
- **opt-02** `inline-always-rare` — Use `#[inline(always)]` sparingly; only when profiling confirms benefit
- **opt-03** `inline-never-cold` — Use `#[inline(never)]` on cold/error paths to keep hot code compact
- **opt-04** `cold-unlikely` — Mark cold functions with `#[cold]` to guide branch prediction
- **opt-05** `lto-release` — Enable `lto = "fat"` in release profile for whole-program optimization
- **opt-06** `codegen-units` — Set `codegen-units = 1` in release profile for maximum optimization
- **opt-07** `pgo-profile` — Use Profile-Guided Optimization (PGO) for performance-critical binaries
- **opt-08** `target-cpu` — Set `RUSTFLAGS="-C target-cpu=native"` for local/bench builds
- **opt-09** `bounds-check` — Use iterators and `.get_unchecked()` (with SAFETY comment) to eliminate bounds checks
- **opt-10** `simd-portable` — Use `std::simd` (portable SIMD) for data-parallel operations
- **opt-11** `cache-friendly` — Design hot data structures with SoA (Struct of Arrays) layout for cache locality
- **opt-12** `panic-abort` — Set `panic = "abort"` in release to eliminate unwinding overhead

---

## **type-** — Type Safety (MEDIUM)

- **type-01** `newtype-ids` — Wrap primitive IDs in newtypes: `struct UserId(u64)`
- **type-02** `newtype-validated` — Use newtypes for validated data: `Email`, `NonEmptyString`
- **type-03** `enum-states` — Use enums for mutually exclusive states; exhaustive matching catches bugs
- **type-04** `option-nullable` — Use `Option<T>` for nullable values; never use sentinel values
- **type-05** `result-fallible` — Use `Result<T, E>` for every fallible operation, including constructors
- **type-06** `phantom-marker` — Use `PhantomData<T>` for type-level markers without runtime cost
- **type-07** `never-diverge` — Use `!` (never type) for functions that provably never return
- **type-08** `generic-bounds` — Add trait bounds only at usage sites; keep struct definitions bound-free
- **type-09** `no-stringly` — Never use `String` for structured data; use enums or newtypes
- **type-10** `repr-transparent` — Use `#[repr(transparent)]` for newtypes used in FFI

---

## **perf-** — Performance Patterns (MEDIUM)

- **perf-01** `iter-over-index` — Use iterators over manual index loops; eliminates bounds checks
- **perf-02** `iter-lazy` — Keep iterator chains lazy; call `.collect()` only when a collection is required
- **perf-03** `collect-once` — Never `.collect()` an intermediate iterator; chain adaptors instead
- **perf-04** `entry-api` — Use `HashMap::entry()` for insert-or-update; avoids double lookup
- **perf-05** `drain-reuse` — Use `.drain(..)` to move elements out while reusing the allocation
- **perf-06** `extend-batch` — Use `.extend()` for batch insertions; avoids repeated reallocations
- **perf-07** `chain-avoid` — Avoid `.chain()` in hot loops; prefer manual unrolling or `itertools`
- **perf-08** `collect-into` — Use `.collect_into(&mut vec)` (stable 1.82+) to reuse an existing allocation
- **perf-09** `black-box-bench` — Use `std::hint::black_box()` in benchmarks to prevent dead-code elimination
- **perf-10** `release-profile` — Always benchmark against `--release`; dev builds are not representative
- **perf-11** `profile-first` — Profile with `cargo flamegraph` or `perf` before optimizing any code

---

## **test-** — Testing (MEDIUM)

- **test-01** `cfg-test-module` — Use `#[cfg(test)] mod tests { }` for unit tests in the same file
- **test-02** `use-super` — Use `use super::*;` inside test modules to access private items
- **test-03** `integration-dir` — Put integration tests in the `tests/` directory, not inline
- **test-04** `descriptive-names` — Name tests as full sentences: `fn returns_error_on_empty_input()`
- **test-05** `arrange-act-assert` — Structure every test as Arrange / Act / Assert
- **test-06** `proptest-properties` — Use `proptest` for property-based testing of pure functions
- **test-07** `mockall-mocking` — Use `mockall` for auto-generating trait mocks
- **test-08** `mock-traits` — Design dependencies as traits to make them mockable
- **test-09** `fixture-raii` — Use RAII (`Drop`) for test fixture cleanup; never rely on test order
- **test-10** `tokio-async` — Use `#[tokio::test]` for async test functions
- **test-11** `should-panic` — Use `#[should_panic(expected = "...")]` for panic contract tests
- **test-12** `criterion-bench` — Use `criterion` for statistically rigorous micro-benchmarks
- **test-13** `doctest-examples` — Keep rustdoc `# Examples` blocks as executable doctests

---

## **doc-** — Documentation (MEDIUM)

- **doc-01** `all-public` — Document every public item with `///`; `#![warn(missing_docs)]` in CI
- **doc-02** `module-inner` — Use `//!` for module-level and crate-level documentation
- **doc-03** `examples-section` — Include `# Examples` with runnable code in every public function
- **doc-04** `errors-section` — Include `# Errors` listing all error variants for fallible functions
- **doc-05** `panics-section` — Include `# Panics` documenting all panic conditions
- **doc-06** `safety-section` — Include `# Safety` explaining required invariants for `unsafe fn`
- **doc-07** `question-mark` — Use `?` in doc examples, never `.unwrap()`
- **doc-08** `hidden-setup` — Use `# ` prefix to hide boilerplate setup in doc examples
- **doc-09** `intra-links` — Use intra-doc links `[Vec]`, `[std::io::Error]` instead of raw URLs
- **doc-10** `link-types` — Cross-link related types and functions in documentation
- **doc-11** `cargo-metadata` — Keep `description`, `repository`, `keywords`, `categories` in `Cargo.toml`

---

## **name-** — Naming Conventions (MEDIUM)

- **name-01** `types-camel` — `UpperCamelCase` for types, traits, enums, and enum variants
- **name-02** `funcs-snake` — `snake_case` for functions, methods, modules, and local variables
- **name-03** `consts-screaming` — `SCREAMING_SNAKE_CASE` for constants and statics
- **name-04** `lifetime-short` — Short single-letter lifetimes: `'a`, `'b`; named only for clarity: `'de`, `'src`
- **name-05** `type-param-single` — Single uppercase for type params: `T`, `E`, `K`, `V`, `I`
- **name-06** `as-free` — `as_` prefix: cheap reference conversion (`as_str`, `as_bytes`)
- **name-07** `to-expensive` — `to_` prefix: expensive owned conversion (`to_string`, `to_vec`)
- **name-08** `into-ownership` — `into_` prefix: consumes self and transfers ownership (`into_bytes`)
- **name-09** `no-get-prefix` — No `get_` prefix for simple field accessors; just use the field name
- **name-10** `is-has-bool` — Boolean methods: `is_`, `has_`, `can_` prefixes
- **name-11** `iter-convention` — Iterator methods: `iter()` / `iter_mut()` / `into_iter()`
- **name-12** `acronym-word` — Treat acronyms as words: `Uuid` not `UUID`, `HttpClient` not `HTTPClient`
- **name-13** `crate-no-rs` — Crate names must not have `-rs` suffix; redundant on crates.io

---

## **proj-** — Project Structure (LOW)

- **proj-01** `lib-main-split` — Keep `main.rs` minimal (arg parsing + run); logic lives in `lib.rs`
- **proj-02** `mod-by-feature` — Organize modules by feature, not by type (`auth/` not `models/`)
- **proj-03** `flat-small` — Keep small crates flat; resist premature module hierarchy
- **proj-04** `pub-crate-internal` — Use `pub(crate)` for APIs internal to the crate
- **proj-05** `pub-super-parent` — Use `pub(super)` for APIs internal to a parent module
- **proj-06** `pub-use-reexport` — Use `pub use` to flatten internal paths into a clean public API
- **proj-07** `prelude-module` — Create a `prelude` module for commonly imported items
- **proj-08** `bin-dir` — Put multiple binaries in `src/bin/`; keep each binary thin
- **proj-09** `workspace-large` — Use Cargo workspaces for multi-crate projects
- **proj-10** `workspace-deps` — Use `[workspace.dependencies]` inheritance to deduplicate versions
- **proj-11** `workspace-lints` — Define `[workspace.lints]` once; inherit in all member crates

---

## **lint-** — Clippy & Linting (LOW)

- **lint-01** `deny-correctness` — `#![deny(clippy::correctness)]` — must always pass
- **lint-02** `warn-suspicious` — `#![warn(clippy::suspicious)]` in all crates
- **lint-03** `warn-style` — `#![warn(clippy::style)]` in all crates
- **lint-04** `warn-complexity` — `#![warn(clippy::complexity)]` in all crates
- **lint-05** `warn-perf` — `#![warn(clippy::perf)]` in all crates
- **lint-06** `pedantic-selective` — Enable `clippy::pedantic` selectively for published libraries
- **lint-07** `missing-docs` — `#![warn(missing_docs)]` in library crates
- **lint-08** `unsafe-doc` — `#![warn(clippy::undocumented_unsafe_blocks)]` everywhere
- **lint-09** `cargo-metadata` — `#![warn(clippy::cargo)]` for published crates
- **lint-10** `rustfmt-check` — Run `cargo fmt --check` in CI; enforce on every PR
- **lint-11** `workspace-lints` — Configure all lints at `[workspace.lints]` level

---

## **anti-** — Anti-patterns (REF)

- **anti-01** `unwrap-abuse` — No `.unwrap()` in production; use `?`, `.unwrap_or()`, or proper error handling
- **anti-02** `expect-lazy` — No `.expect()` for recoverable errors; only for invariant violations
- **anti-03** `clone-excessive` — No `.clone()` when borrowing works; profile clone frequency in hot paths
- **anti-04** `lock-across-await` — Never hold a `Mutex`/`RwLock` guard across an `.await` point
- **anti-05** `string-for-str` — Never accept `&String` in function signatures; accept `&str`
- **anti-06** `vec-for-slice` — Never accept `&Vec<T>` in function signatures; accept `&[T]`
- **anti-07** `index-over-iter` — Never index manually when an iterator adapter exists
- **anti-08** `panic-expected` — Never `panic!` on expected/recoverable errors in library code
- **anti-09** `empty-catch` — Never silently discard errors with `let _ = result;`
- **anti-10** `over-abstraction` — Never add generic type parameters without a concrete use case
- **anti-11** `premature-optimize` — Never optimize without profiler evidence
- **anti-12** `type-erasure` — Never use `Box<dyn Trait>` when `impl Trait` in position works
- **anti-13** `format-hot-path` — Never call `format!()` in hot loops; pre-allocate or use `write!()`
- **anti-14** `collect-intermediate` — Never `.collect()` into a `Vec` only to immediately iterate it
- **anti-15** `stringly-typed` — Never use `String` or `&str` for structured data; use enums

---

## Recommended Cargo.toml profiles

```toml
[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
panic = "abort"
strip = true

[profile.bench]
inherits = "release"
debug = true
strip = false

[profile.dev]
opt-level = 0
debug = true

[profile.dev.package."*"]
opt-level = 3
```
