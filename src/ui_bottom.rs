//! Área inferior da TUI (US-041/US-042): Card elevado de entrada.
//!
//! Este módulo implementa o **novo** layout proposto pelas US-041/US-042 de
//! forma isolada: recebe apenas o estado já resolvido pelo
//! [`crate::session_ui::Tui`] (texto do prompt, cursor, notice) e desenha no
//! [`Frame`]. Estrutura alinhada ao card de entrada do opencode
//! (`component/prompt/index.tsx`): acento vertical `▌` na margem esquerda de
//! todas as linhas do card e borda inferior com `▀` (mais o canto `╹`). A UI
//! anterior permanece intacta em `session_ui.rs` (funções `render_home_prompt`
//! e `render_footer`); para voltar a ela basta trocar a `BottomStyle` no campo
//! `bottom_style` do [`crate::session_ui::Tui`].

use crossterm::{cursor, queue, terminal};

use crate::session_ui::{Frame, THEME};

/// Caractere do acento vertical na margem esquerda do Card (US-042): bloco
/// esquerdo, simulando a borda lateral de 4px do mockup HTML.
const ACCENT_BLOCK: &str = "▌";
/// Caractere do canto inferior esquerdo do Card (US-042, igual ao opencode).
const ACCENT_CORNER: &str = "╹";
/// Caractere de meio-bloco superior da borda inferior do Card (US-042,
/// igual ao opencode): cria o efeito de elevação.
const EDGE_BLOCK: char = '▀';

/// Estilo da área inferior da TUI (US-041). Permite alternar entre o layout
/// legado (prompt no fim do Body + footer) e o novo Card elevado sem alterar
/// o restante do render — basta trocar esta variante no chamador.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BottomStyle {
    /// Layout legado (pré-US-041): `render_home_prompt` + `render_footer`.
    Legacy,
    /// Novo layout elevado (US-041/US-042): Card de entrada (input + badges +
    /// borda inferior) com acento vertical `▌`.
    Elevated,
}

/// Geometria resolvida da área inferior (US-041/US-042): linhas absolutas na
/// tela do Card e da borda inferior, além do espaço (em linhas) reservado
/// abaixo do Body.
#[derive(Clone, Copy, Debug)]
pub struct BottomGeometry {
    /// Linha do Card com o campo de entrada (digitação).
    pub input_row: u16,
    /// Linha do Card com os badges de contexto.
    pub badges_row: u16,
    /// Linha da borda inferior do Card (`▀`, US-042).
    pub edge_row: u16,
    /// Altura (linhas) que o layout reserva abaixo do Body (`input` + `badges`
    /// + `edge` do Card).
    pub reserved: u16,
}

/// Dados já resolvidos pelo chamador para desenhar o Card.
pub struct BottomData<'a> {
    /// Texto do rótulo do prompt (`"> "` ou `"insira o link: "`).
    pub prompt: &'a str,
    /// Texto digitado (ou placeholder) exibido após o rótulo.
    pub input: &'a str,
    /// Índice do cursor em caracteres dentro do texto visível (coluna local).
    pub cursor_col: usize,
    /// Texto da pergunta de confirmação de exclusão (US-035), se ativa.
    pub confirm_label: Option<String>,
    /// Modo "insira o link:" (US-034) — muda o presente de badges.
    pub prompt_add_mode: bool,
    /// Menu flutuante aberto (roda badges).
    pub menu_open: bool,
    /// Notice/status transitório, se houver.
    pub notice: Option<&'a str>,
}

/// Linha do campo de entrada dentro do Card elevado (US-041/US-042): rótulo
/// do prompt + texto digitado + cursor, sobre o fundo do Card, com o caractere
/// de acento vertical `▌` à esquerda (azul quando em foco, cinza quando
/// ocioso).
pub fn render_input_card(frame: &mut Frame, cols: u16, row: u16, data: &BottomData, focused: bool) {
    if row >= frame.rows {
        return;
    }
    let left = 0u16;
    let width = cols;
    let accent = if focused {
        THEME.card_accent_active
    } else {
        THEME.card_accent_idle
    };

    // Acento vertical (glyph) na margem esquerda + fundo do Card.
    frame.put_styled(row, left, ACCENT_BLOCK, Some(accent), Some(THEME.card_bg));
    frame.fill(
        row,
        left + 1,
        width.saturating_sub(1),
        ' ',
        None,
        Some(THEME.card_bg),
    );

    let content_left = 3u16;
    if let Some(label) = &data.confirm_label {
        let max_w = (width as usize).saturating_sub(content_left as usize + 2);
        let shown = crate::session_ui::Tui::truncate(label, max_w);
        frame.put_styled(
            row,
            content_left,
            &shown,
            Some(THEME.home_prompt_fg),
            Some(THEME.card_bg),
        );
        // US-042: o cursor permanece sempre na área de digitação do Card,
        // mesmo durante a confirmação (após o rótulo da pergunta).
        frame.set_cursor(row, content_left + shown.chars().count() as u16);
        return;
    }

    frame.put_styled(
        row,
        content_left,
        data.prompt,
        Some(THEME.home_prompt_fg),
        Some(THEME.card_bg),
    );
    let text_col = content_left + data.prompt.chars().count() as u16;
    let max_w = (width as usize).saturating_sub(text_col as usize + 2);
    let shown = crate::session_ui::Tui::truncate(data.input, max_w);
    frame.put_styled(row, text_col, &shown, Some(THEME.text), Some(THEME.card_bg));
    let cursor_col = text_col.saturating_add(data.cursor_col as u16);
    frame.set_cursor(row, cursor_col);
}

/// Linha de badges de contexto do Card (US-041/US-042): badge do modo atual +
/// metadados `OpenTorrent • Versão`, à esquerda, sobre o fundo do Card com o
/// acento vertical `▌` à esquerda e o notice à direita.
pub fn render_badges_row(frame: &mut Frame, cols: u16, row: u16, data: &BottomData, focused: bool) {
    if row >= frame.rows {
        return;
    }
    let accent = if focused {
        THEME.card_accent_active
    } else {
        THEME.card_accent_idle
    };
    frame.put_styled(row, 0, ACCENT_BLOCK, Some(accent), Some(THEME.card_bg));
    frame.fill(
        row,
        1,
        cols.saturating_sub(1),
        ' ',
        None,
        Some(THEME.card_bg),
    );

    let mode = if data.menu_open {
        "COMANDOS"
    } else if data.prompt_add_mode {
        "ADICIONAR"
    } else {
        "INÍCIO"
    };
    let badge = format!(" {mode} ");
    let badge_len = badge.chars().count() as u16;
    frame.put_styled(
        row,
        3,
        &badge,
        Some(THEME.card_bg),
        Some(THEME.card_accent_active),
    );

    // Metadados/build no estilo do mockup (US-041/US-042): `OpenTorrent •
    // Versão: vX.Y.Z`, separados por " • ".
    let version = env!("CARGO_PKG_VERSION");
    let meta = format!("  •  OpenTorrent  •  Versão: v{version}");
    let meta_col = 3 + badge_len + 1;
    frame.put_styled(row, meta_col, &meta, Some(THEME.text), Some(THEME.card_bg));

    // Notice (erro/aviso) à direita quando houver.
    let after = meta_col + meta.chars().count() as u16;
    if let Some(notice) = data.notice {
        let color = if notice.starts_with("erro") {
            THEME.error
        } else {
            THEME.warning
        };
        let max_w = (cols as usize).saturating_sub(after as usize + 2);
        let shown = crate::session_ui::Tui::truncate(notice, max_w);
        frame.put_styled(row, after, &shown, Some(color), Some(THEME.card_bg));
    }
}

/// Linha de respiro do Card (US-042): fundo do Card com o acento `▌` à
/// esquerda, dando altura extra ao card sem conteúdo adicional.
pub fn render_gap_row(frame: &mut Frame, cols: u16, row: u16, focused: bool) {
    if row >= frame.rows {
        return;
    }
    let accent = if focused {
        THEME.card_accent_active
    } else {
        THEME.card_accent_idle
    };
    frame.put_styled(row, 0, ACCENT_BLOCK, Some(accent), Some(THEME.card_bg));
    frame.fill(
        row,
        1,
        cols.saturating_sub(1),
        ' ',
        None,
        Some(THEME.card_bg),
    );
}

/// Borda inferior do Card (US-042): canto `╹` + `▀` na cor do Card sobre o
/// fundo base, reproduzindo o efeito de elevação do opencode.
pub fn render_edge_row(frame: &mut Frame, cols: u16, row: u16) {
    if row >= frame.rows {
        return;
    }
    frame.put_styled(row, 0, ACCENT_CORNER, Some(THEME.card_accent_idle), None);
    frame.fill(
        row,
        1,
        cols.saturating_sub(1),
        EDGE_BLOCK,
        Some(THEME.card_bg),
        None,
    );
}

/// Calcula as linhas absolutas da área inferior conforme o estilo e o total de
/// linhas do terminal. `rows.min(4)` evita underflow em terminais minúsculos.
pub fn bottom_geometry(style: BottomStyle, rows: u16) -> BottomGeometry {
    match style {
        BottomStyle::Legacy => BottomGeometry {
            input_row: rows.saturating_sub(2),
            badges_row: rows.saturating_sub(2),
            edge_row: rows.saturating_sub(2),
            reserved: 1,
        },
        // US-042: Card com 4 linhas (input + badges + linha de respiro + borda
        // inferior) no fim da tela, isolado no render como `Length(4)`. Sem
        // barra externa.
        BottomStyle::Elevated => {
            let input_row = rows.saturating_sub(4);
            BottomGeometry {
                input_row,
                badges_row: rows.saturating_sub(3),
                edge_row: rows.saturating_sub(1),
                reserved: 4,
            }
        }
    }
}

/// Configura o terminal para a área inferior (alternate screen, raw mode).
#[allow(dead_code)]
pub(crate) fn _enter_alternate() -> anyhow::Result<()> {
    terminal::enable_raw_mode()?;
    queue!(std::io::stdout(), cursor::Hide)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_ui::Frame;

    fn make_data<'a>(prompt: &'a str, input: &'a str, cursor_col: usize) -> BottomData<'a> {
        BottomData {
            prompt,
            input,
            cursor_col,
            confirm_label: None,
            prompt_add_mode: false,
            menu_open: false,
            notice: None,
        }
    }

    #[test]
    fn geometry_elevated_reserves_four_rows() {
        let g = bottom_geometry(BottomStyle::Elevated, 24);
        assert_eq!(g.reserved, 4);
        assert_eq!(g.input_row, 20);
        assert_eq!(g.badges_row, 21);
        assert_eq!(g.edge_row, 23);
    }

    #[test]
    fn geometry_legacy_reserves_one_row() {
        let g = bottom_geometry(BottomStyle::Legacy, 24);
        assert_eq!(g.reserved, 1);
        assert_eq!(g.input_row, 22);
        assert_eq!(g.edge_row, 22);
    }

    #[test]
    fn gap_row_draws_card_background_with_accent() {
        let mut frame = Frame::new(80, 3);
        render_gap_row(&mut frame, 80, 1, true);
        assert_eq!(frame.cell(1, 0).ch(), '▌');
        assert_eq!(frame.cell(1, 0).fg(), Some(THEME.card_accent_active));
        assert_eq!(frame.cell(1, 79).bg, Some(THEME.card_bg));
        assert_eq!(frame.cell(1, 1).ch(), ' ');
    }

    #[test]
    fn input_card_sets_cursor_after_text() {
        let mut frame = Frame::new(60, 3);
        let data = make_data("> ", "abc", 3);
        render_input_card(&mut frame, 60, 1, &data, true);
        assert_eq!(frame.cursor, Some((1, 8))); // 3 (col) + 2 ("> ") + 3 ("abc")
    }

    #[test]
    fn input_card_shows_accent_block_on_left_margin() {
        let mut frame = Frame::new(60, 3);
        let data = make_data("> ", "abc", 3);
        render_input_card(&mut frame, 60, 1, &data, true);
        assert_eq!(frame.cell(1, 0).ch(), '▌');
        assert_eq!(frame.cell(1, 0).fg(), Some(THEME.card_accent_active));
        assert_eq!(frame.cell(1, 1).bg, Some(THEME.card_bg));
    }

    #[test]
    fn confirm_label_keeps_cursor_in_input_card() {
        let mut frame = Frame::new(60, 3);
        let mut data = make_data("> ", "", 0);
        data.confirm_label = Some("excluir 'x'? [Y] ou [N] ".to_string());
        render_input_card(&mut frame, 60, 1, &data, true);
        // Cursor sempre na área de digitação (US-042), após o rótulo da pergunta.
        let label = "excluir 'x'? [Y] ou [N] ";
        assert_eq!(frame.cursor, Some((1, 3 + label.chars().count() as u16)));
    }

    #[test]
    fn badge_row_shows_mode_and_accent_block() {
        let mut frame = Frame::new(80, 3);
        let data = make_data("> ", "", 0);
        render_badges_row(&mut frame, 80, 1, &data, true);
        assert_eq!(frame.cell(1, 0).ch(), '▌');
        assert_eq!(frame.cell(1, 0).fg(), Some(THEME.card_accent_active));
        let badge: String = (3..11).map(|c| frame.cell(1, c).ch()).collect();
        assert_eq!(badge, " INÍCIO ");
    }

    #[test]
    fn edge_row_draws_corner_and_half_blocks() {
        let mut frame = Frame::new(80, 3);
        render_edge_row(&mut frame, 80, 2);
        assert_eq!(frame.cell(2, 0).ch(), '╹');
        assert_eq!(frame.cell(2, 1).ch(), '▀');
        assert_eq!(frame.cell(2, 79).ch(), '▀');
    }
}
