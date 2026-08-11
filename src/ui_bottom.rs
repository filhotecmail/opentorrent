//! Área inferior da TUI (US-041/US-042): Card elevado de entrada + barra
//! externa de status/atalhos.
//!
//! Este módulo implementa o **novo** layout proposto pelas US-041/US-042 de
//! forma isolada: recebe apenas o estado já resolvido pelo
//! [`crate::session_ui::Tui`] (texto do prompt, cursor, notice, métricas de
//! progresso) e desenha no [`Frame`]. Estrutura alinhada ao card de entrada do
//! opencode (`component/prompt/index.tsx`): acento vertical `▌` na margem
//! esquerda de todas as linhas do card, borda inferior com `▀` (mais o canto
//! `╹`) e rodapé externo dedicado com atividade + `esc interromper` à esquerda
//! e métricas + lembrete de menu à direita. A UI anterior permanece intacta em
//! `session_ui.rs` (funções `render_home_prompt` + `render_footer`); para
//! voltar a ela basta trocar a `BottomStyle` em `session_ui.rs` (campo
//! `bottom_style`).

use crossterm::{cursor, queue, terminal};
use size_format::SizeFormatterBinary as SF;

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
/// legado (prompt no fim do Body + footer) e o novo Card elevado + barra de
/// status sem alterar o restante do render — basta trocar esta variante no
/// chamador.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BottomStyle {
    /// Layout legado (pré-US-041): `render_home_prompt` + `render_footer`.
    Legacy,
    /// Novo layout elevado (US-041/US-042): Card de entrada (input + badges +
    /// borda inferior) com acento vertical `▌` e barra externa de status.
    Elevated,
}

/// Geometria resolvida da área inferior (US-041/US-042): linhas absolutas na
/// tela do Card, da borda inferior e da barra de status, além do espaço (em
/// linhas) reservado abaixo do Body.
#[derive(Clone, Copy, Debug)]
pub struct BottomGeometry {
    /// Linha do Card com o campo de entrada (digitação).
    pub input_row: u16,
    /// Linha do Card com os badges de contexto.
    pub badges_row: u16,
    /// Linha da borda inferior do Card (`▀`, US-042).
    pub edge_row: u16,
    /// Linha da barra externa de status/atalhos.
    pub status_row: u16,
    /// Altura (linhas) que o layout reserva abaixo do Body (`input` + `badges`
    /// + `edge` dentro do Card + `status` externa).
    pub reserved: u16,
}

/// Dados já resolvidos pelo chamador para desenhar o Card e a barra de status.
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
    /// Bytes já baixados em todas as torrents (US-042, métrica do rodapé).
    pub progress_bytes: u64,
    /// Bytes totais de todas as torrents (US-042, métrica do rodapé).
    pub total_bytes: u64,
}

pub struct ActivityIndicator {
    phase: u64,
    start: std::time::Instant,
}

impl ActivityIndicator {
    pub fn new() -> Self {
        Self {
            phase: 0,
            start: std::time::Instant::now(),
        }
    }

    /// Avança a fase a cada `interval` (30 FPS) e retorna a barra animada.
    pub fn tick(&mut self) -> String {
        let phase = (self.start.elapsed().as_millis() / 33) as u64;
        self.phase = phase;
        render_activity(phase)
    }
}

impl Default for ActivityIndicator {
    fn default() -> Self {
        Self::new()
    }
}

/// Barra animada de atividade (US-041): `… ▇█` com o bloco se movendo.
fn render_activity(phase: u64) -> String {
    let blocks = ["▁", "▂", "▃", "▄", "▆", "▇", "█"];
    let idx = (phase as usize) % blocks.len();
    let mut s = String::from("… ");
    for (i, b) in blocks.iter().enumerate() {
        if i == idx {
            s.push_str(b);
        } else {
            s.push('·');
        }
    }
    s
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
        return; // confirmação não exibe cursor
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

/// Barra externa de status/atalhos (US-041/US-042): indicador de atividade +
/// `esc interromper` à esquerda e métricas de progresso + `Digite / para o
/// menu de opções` à direita. Os atalhos genéricos (US-042) ficam de fora.
pub fn render_status_bar(
    frame: &mut Frame,
    cols: u16,
    row: u16,
    data: &BottomData,
    activity: &str,
) {
    if row >= frame.rows {
        return;
    }
    frame.fill(row, 0, cols, ' ', None, Some(THEME.footer_bg));

    // Indicador de atividade + atalho de cancelamento à esquerda.
    let mut col = 1u16;
    frame.put_styled(
        row,
        col,
        activity,
        Some(THEME.accent),
        Some(THEME.footer_bg),
    );
    col += activity.chars().count() as u16 + 1;
    frame.put_styled(row, col, "esc", Some(THEME.text), Some(THEME.footer_bg));
    col += 4;
    frame.put_styled(
        row,
        col,
        "interromperZQ",
        Some(THEME.muted),
        Some(THEME.footer_bg),
    );
    col += "interromper".chars().count() as u16 + 2;

    // Métricas + lembrete do menu à direita, alinhados à direita.
    let progress = progress_label(data.progress_bytes, data.total_bytes);
    let menu_hint = "Digite / para o menu de opções";
    let right = format!("{progress}   {menu_hint}");
    let right_col = cols.saturating_sub(right.chars().count() as u16);
    frame.put_styled(
        row,
        right_col,
        &progress,
        Some(THEME.text),
        Some(THEME.footer_bg),
    );
    frame.put_styled(
        row,
        cols.saturating_sub(menu_hint.chars().count() as u16),
        menu_hint,
        Some(THEME.muted),
        Some(THEME.footer_bg),
    );

    // Notice à esquerda (após o atalho) quando houver espaço.
    if let Some(notice) = data.notice {
        let color = if notice.starts_with("erro") {
            THEME.error
        } else {
            THEME.warning
        };
        let shown =
            crate::session_ui::Tui::truncate(notice, right_col.saturating_sub(col) as usize)
                .to_string();
        if !shown.is_empty() {
            frame.put_styled(row, col, &shown, Some(color), Some(THEME.footer_bg));
        }
    }
}

/// Rótulo de progresso do rodapé (US-042): `155.7K (78%)`.
fn progress_label(progress_bytes: u64, total_bytes: u64) -> String {
    // `SizeFormatterBinary` emite sufixo IEC (`Ki`/`Mi`); removemos o `i` para
    // casar com o mockup (`155.7K`).
    let size = SF::new(progress_bytes)
        .to_string()
        .trim_end_matches('i')
        .to_string();
    let pct = if total_bytes > 0 {
        (progress_bytes as f64 / total_bytes as f64 * 100.0).round() as u64
    } else {
        0
    };
    format!("{size} ({pct}%)")
}

/// Calcula as linhas absolutas da área inferior conforme o estilo e o total de
/// linhas do terminal. `rows.min(4)` evita underflow em terminais minúsculos.
pub fn bottom_geometry(style: BottomStyle, rows: u16) -> BottomGeometry {
    match style {
        BottomStyle::Legacy => BottomGeometry {
            input_row: rows.saturating_sub(2),
            badges_row: rows.saturating_sub(2),
            edge_row: rows.saturating_sub(2),
            status_row: rows.saturating_sub(1),
            reserved: 1,
        },
        // Us-042: Card com 3 linhas (input + badges + borda inferior) + rodapé
        // externo com 1 linha, isolados no render como `Length(3)` + `Length(1)`.
        BottomStyle::Elevated => {
            let input_row = rows.saturating_sub(4);
            BottomGeometry {
                input_row,
                badges_row: rows.saturating_sub(3),
                edge_row: rows.saturating_sub(2),
                status_row: rows.saturating_sub(1),
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
            progress_bytes: 0,
            total_bytes: 0,
        }
    }

    #[test]
    fn geometry_elevated_reserves_four_rows() {
        let g = bottom_geometry(BottomStyle::Elevated, 24);
        assert_eq!(g.reserved, 4);
        assert_eq!(g.input_row, 20);
        assert_eq!(g.badges_row, 21);
        assert_eq!(g.edge_row, 22);
        assert_eq!(g.status_row, 23);
    }

    #[test]
    fn geometry_legacy_reserves_one_row() {
        let g = bottom_geometry(BottomStyle::Legacy, 24);
        assert_eq!(g.reserved, 1);
        assert_eq!(g.input_row, 22);
        assert_eq!(g.status_row, 23);
    }

    #[test]
    fn activity_contains_dots_and_block() {
        let s = render_activity(0);
        assert!(s.starts_with("… "));
        assert!(s.contains('▁'));
    }

    #[test]
    fn activity_rotates_by_phase() {
        let a = render_activity(0);
        let b = render_activity(1);
        assert_ne!(a, b);
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
    fn confirm_label_does_not_set_cursor() {
        let mut frame = Frame::new(60, 3);
        let mut data = make_data("> ", "", 0);
        data.confirm_label = Some("excluir 'x'? [Y] ou [N] ".to_string());
        render_input_card(&mut frame, 60, 1, &data, true);
        assert_eq!(frame.cursor, None);
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

    #[test]
    fn status_bar_shows_progress_and_normal_mode_does_not_show_hints() {
        let mut frame = Frame::new(120, 3);
        let mut data = make_data("> ", "", 0);
        data.progress_bytes = 159_437;
        data.total_bytes = 205_000;
        render_status_bar(&mut frame, 120, 2, &data, "… ▇");
        // A linha inteira tem fundo do footer: nenhuma célula em branco.
        assert_eq!(frame.cell(2, 0).bg, Some(THEME.footer_bg));
        let row_text: String = (0..120).map(|c| frame.cell(2, c).ch()).collect();
        assert!(row_text.contains("155.7K (78%)"));
        assert!(row_text.contains("esc interromper"));
        assert!(row_text.contains("Digite / para o menu de opções"));
        assert!(!row_text.contains("p pausar"));
    }

    #[test]
    fn progress_label_rounds_percent() {
        assert_eq!(progress_label(159_437, 205_000), "155.7K (78%)");
        assert_eq!(progress_label(0, 0), "0 (0%)");
        assert_eq!(progress_label(10_000, 10_000), "9.7K (100%)");
    }
}
