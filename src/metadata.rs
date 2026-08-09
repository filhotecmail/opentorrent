//! Metadados do projeto embutidos no binário (US-021).
//!
//! O `build.rs` gera uma nota ELF (formato GNU) com o nome do projeto, a URL
//! do repositório e o objetivo/descrição; este módulo a injeta na seção
//! somente leitura `.note.opentorrent` do executável, de onde pode ser lida
//! com ferramentas nativas do Linux (`strings`, `readelf`). A injeção é
//! automática em todo `cargo build` e não altera o comportamento do binário.

/// Constante com o tamanho da nota, gerada pelo `build.rs` (necessária para
/// declarar o `static` com o tamanho exato do array embutido).
mod generated {
    include!(concat!(env!("OUT_DIR"), "/opentorrent_note_size.rs"));
}

/// Nota ELF com os metadados do projeto, gerada pelo `build.rs` e colocada na
/// seção `.note.opentorrent`. `#[used]` impede que o linker a remova (dead
/// stripping) mesmo com LTO/`strip` do perfil release.
#[used]
// Edição 2024: atributos de link são unsafe e exigem o wrapper `unsafe(...)`.
#[unsafe(link_section = ".note.opentorrent")]
static OPENTORRENT_NOTE: [u8; generated::OPENTORRENT_NOTE_LEN] =
    *include_bytes!(concat!(env!("OUT_DIR"), "/opentorrent-note.bin"));

/// Bytes da nota embutida (para testes e inspeção programática).
#[cfg(test)]
pub(crate) fn project_note() -> &'static [u8] {
    &OPENTORRENT_NOTE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_is_embedded_and_readable() {
        let note = project_note();
        assert!(!note.is_empty());
        let text = String::from_utf8_lossy(note);
        assert!(text.contains("opentorrent"));
        assert!(text.contains("Repositorio:"));
        assert!(text.contains("Objetivo:"));
        assert!(text.contains(env!("CARGO_PKG_NAME")));
    }

    #[test]
    fn note_has_valid_elf_note_header() {
        let note = project_note();
        assert!(note.len() >= 12);
        let namesz = u32::from_le_bytes([note[0], note[1], note[2], note[3]]) as usize;
        let descsz = u32::from_le_bytes([note[4], note[5], note[6], note[7]]) as usize;
        assert!(namesz > 0);
        assert!(descsz > 0);
        // O nome começa após o cabeçalho de 12 bytes e termina com NUL.
        let name = String::from_utf8_lossy(&note[12..12 + namesz]);
        assert_eq!(name.trim_end_matches('\0'), "opentorrent");
        // A descrição ocupa o restante (descontado o padding final).
        assert!(note.len() >= 12 + namesz + descsz);
    }
}
