//! Build script da US-021: injeta metadados do projeto (nome, repositório e
//! objetivo) na seção somente leitura `.note.opentorrent` do binário ELF.
//!
//! Os valores são extraídos das configurações do projeto (Cargo.toml) via as
//! variáveis `CARGO_PKG_*` disponibilizadas pelo cargo ao build script. O
//! resultado é um arquivo binário no `OUT_DIR` que o crate embute na seção
//! com `#[link_section]` — legível com `strings`/`readelf`, sem afetar a
//! execução nem a assinatura digital (que assina o binário já compilado).

use std::{env, fs, path::PathBuf};

fn main() {
    // Mudanças no Cargo.toml (nome/repositório/objetivo) reexecutam o script.
    println!("cargo:rerun-if-changed=Cargo.toml");

    let name = env::var("CARGO_PKG_NAME").unwrap_or_else(|_| "opentorrent".into());
    let version = env::var("CARGO_PKG_VERSION").unwrap_or_default();
    let repo = env::var("CARGO_PKG_REPOSITORY").unwrap_or_default();
    let description = env::var("CARGO_PKG_DESCRIPTION").unwrap_or_default();

    // Descrição em texto puro (UTF-8), legível diretamente via `strings`.
    let desc_text =
        format!("OpenTorrent v{version}\nRepositorio: {repo}\nObjetivo: {description}\n");

    // Formato GNU ELF note (reconhecido por `readelf -n`):
    //   namesz (u32 LE) | descsz (u32 LE) | type (u32 LE) | name | desc
    let note_name = b"opentorrent\0"; // 12 bytes, já alinhado a 4
    let mut note: Vec<u8> = Vec::with_capacity(12 + note_name.len() + desc_text.len() + 4);
    note.extend_from_slice(&(note_name.len() as u32).to_le_bytes());
    note.extend_from_slice(&(desc_text.len() as u32).to_le_bytes());
    note.extend_from_slice(&0u32.to_le_bytes()); // tipo customizado da nota
    note.extend_from_slice(note_name);
    note.extend_from_slice(desc_text.as_bytes());
    // Padding final para manter o alinhamento de 4 bytes do formato.
    while !note.len().is_multiple_of(4) {
        note.push(0);
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR definido pelo cargo"));
    let note_path = out_dir.join("opentorrent-note.bin");
    fs::write(&note_path, &note).expect("falha ao gravar a nota de metadados");

    // Constante de tamanho para o crate declarar o static `[u8; N]` com o
    // tamanho exato da nota (o `_` não é permitido em static).
    let size_path = out_dir.join("opentorrent_note_size.rs");
    fs::write(
        &size_path,
        format!("pub const OPENTORRENT_NOTE_LEN: usize = {};\n", note.len()),
    )
    .expect("falha ao gravar a constante de tamanho da nota");

    println!(
        "metadados: seção .note.opentorrent com {} bytes ({name} v{version})",
        note.len()
    );
}
