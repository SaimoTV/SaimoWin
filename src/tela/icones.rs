//! Os ícones Material da TV Box, desenhados a partir do mesmo `pathData`.
//!
//! Os ícones da TV Box são vetores Android (`res/drawable/ic_*.xml`), que são
//! caminhos SVG numa caixa de 24 por 24. Aqui o caminho é lido, as curvas e os
//! arcos viram segmentos, e o desenho é rasterizado com anti-serrilhado num
//! tamanho de textura que cabe o ícone na tela sem borrar. A textura é branca
//! com a cobertura no alfa: a cor vem na hora de pintar.
//!
//! Sem biblioteca de SVG: são quarenta ícones de preenchimento simples, e um
//! rasterizador de cem linhas resolve sem trazer dependência pesada.

use eframe::egui::{self, Color32, Pos2, Rect};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Icone {
    Buscar,
    Inicio,
    AoVivo,
    Eventos,
    Filmes,
    Estrela,
    EstrelaVazia,
    Radio,
    Ajustes,
    Mais,
    Tocar,
    Pausar,
    Voltar10,
    Avancar10,
    Proximo,
    Anterior,
    Episodios,
    Legendas,
    Lista,
    Guia,
    MultiTela,
    Fechar,
    Voltar,
    Certo,
    Coracao,
    Brilho,
    Historico,
    Hd,
    Trocar,
    TelaCheia,
    Som,
    Mudo,
    Preencher,
    Velocidade,
    Teclado,
    Baixar,
    Ordenar,
    Janela,
    Parar,
    Direita,
    Esquerda,
    /// O "Cast" da TV Box: o Saimo Connect.
    Cast,
    Link,
}

/// O `pathData` de cada ícone, na caixa de 24 x 24 da Material.
fn caminhos(icone: Icone) -> &'static [&'static str] {
    match icone {
        Icone::Buscar => &["M15.5,14h-0.79l-0.28,-0.27C15.41,12.59 16,11.11 16,9.5 16,5.91 13.09,3 9.5,3S3,5.91 3,9.5 5.91,16 9.5,16c1.61,0 3.09,-0.59 4.23,-1.57l0.27,0.28v0.79l5,4.99L20.49,19l-4.99,-5zM9.5,14C7.01,14 5,11.99 5,9.5S7.01,5 9.5,5 14,7.01 14,9.5 11.99,14 9.5,14z"],
        Icone::Link => &["M3.9,12c0,-1.71 1.39,-3.1 3.1,-3.1h4V7H7c-2.76,0 -5,2.24 -5,5s2.24,5 5,5h4v-1.9H7c-1.71,0 -3.1,-1.39 -3.1,-3.1zM8,13h8v-2H8v2zM17,7h-4v1.9h4c1.71,0 3.1,1.39 3.1,3.1s-1.39,3.1 -3.1,3.1h-4V17h4c2.76,0 5,-2.24 5,-5s-2.24,-5 -5,-5z"],
        Icone::Inicio => &["M10,20v-6h4v6h5v-8h3L12,3 2,12h3v8z"],
        Icone::AoVivo => &["M21,6h-7.59l3.29,-3.29L16,2l-4,4 -4,-4 -0.71,0.71L10.59,6H3c-1.1,0 -2,0.89 -2,2v12c0,1.1 0.9,2 2,2h18c1.1,0 2,-0.9 2,-2V8c0,-1.11 -0.9,-2 -2,-2zM21,20H3V8h18v12zM9,10v8l7,-4z"],
        Icone::Eventos => &["M12,2A10,10 0 1,0 22,12A10,10 0 0,0 12,2Zm5.77,5.55l-2.45,1.75L15,12l0.32,2.7L17.77,16.45A8,8 0 0,1 6.23,16.45L8.68,14.7L9,12l-0.32,-2.7L6.23,7.55A8,8 0 0,1 17.77,7.55ZM12,14.5a2.5,2.5 0 1,1 2.5,-2.5A2.5,2.5 0 0,1 12,14.5Z"],
        Icone::Filmes => &["M18,4l2,4h-3l-2,-4h-2l2,4h-3l-2,-4H8l2,4H7L5,4H4c-1.1,0 -1.99,0.9 -1.99,2L2,18c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2V4h-4z"],
        Icone::Estrela => &["M12,17.27L18.18,21l-1.64,-7.03L22,9.24l-7.19,-0.61L12,2 9.19,8.63 2,9.24l5.46,4.73L5.82,21z"],
        Icone::EstrelaVazia => &["M22,9.24l-7.19,-0.62L12,2L9.19,8.63L2,9.24l5.46,4.73L5.82,21L12,17.27L18.18,21l-1.63,-7.03L22,9.24zM12,15.4l-3.76,2.27 1,-4.28 -3.32,-2.88 4.38,-0.38L12,6.1l1.71,4.04 4.38,0.38 -3.32,2.88 1,4.28L12,15.4z"],
        Icone::Radio => &[
            "M2,20v-8c0,-2.2 1.8,-4 4,-4h12c2.2,0 4,1.8 4,4v8H2zM4,10c-1.1,0 -2,0.9 -2,2v6h3v-8H4zM20,10h-1v8h3v-6C22,10.9 21.1,10 20,10zM8,12c-1.1,0 -2,0.9 -2,2s0.9,2 2,2s2,-0.9 2,-2S9.1,12 8,12z",
            "M8,4.2L20.5,1.1L21,3L8.5,6.1Z",
        ],
        Icone::Ajustes => &["M19.14,12.94c0.04,-0.3 0.06,-0.61 0.06,-0.94c0,-0.32 -0.02,-0.64 -0.07,-0.94l2.03,-1.58c0.18,-0.14 0.23,-0.41 0.12,-0.61l-1.92,-3.32c-0.12,-0.22 -0.37,-0.29 -0.59,-0.22l-2.39,0.96c-0.5,-0.38 -1.03,-0.7 -1.62,-0.94L14.4,2.81c-0.04,-0.24 -0.24,-0.41 -0.48,-0.41h-3.84c-0.24,0 -0.43,0.17 -0.47,0.41L9.25,5.35C8.66,5.59 8.12,5.92 7.63,6.29L5.24,5.33c-0.22,-0.08 -0.47,0 -0.59,0.22L2.74,8.87C2.62,9.08 2.66,9.34 2.86,9.48l2.03,1.58C4.84,11.36 4.8,11.69 4.8,12s0.02,0.64 0.07,0.94l-2.03,1.58c-0.18,0.14 -0.23,0.41 -0.12,0.61l1.92,3.32c0.12,0.22 0.37,0.29 0.59,0.22l2.39,-0.96c0.5,0.38 1.03,0.7 1.62,0.94l0.36,2.54c0.05,0.24 0.24,0.41 0.48,0.41h3.84c0.24,0 0.44,-0.17 0.47,-0.41l0.36,-2.54c0.59,-0.24 1.13,-0.56 1.62,-0.94l2.39,0.96c0.22,0.08 0.47,0 0.59,-0.22l1.92,-3.32c0.12,-0.22 0.07,-0.47 -0.12,-0.61L19.14,12.94zM12,15.6c-1.98,0 -3.6,-1.62 -3.6,-3.6s1.62,-3.6 3.6,-3.6s3.6,1.62 3.6,3.6S13.98,15.6 12,15.6z"],
        Icone::Mais => &["M6,10c-1.1,0 -2,0.9 -2,2s0.9,2 2,2 2,-0.9 2,-2 -0.9,-2 -2,-2zM18,10c-1.1,0 -2,0.9 -2,2s0.9,2 2,2 2,-0.9 2,-2 -0.9,-2 -2,-2zM12,10c-1.1,0 -2,0.9 -2,2s0.9,2 2,2 2,-0.9 2,-2 -0.9,-2 -2,-2z"],
        Icone::Tocar => &["M8,5v14l11,-7z"],
        Icone::Pausar => &["M6,19h4L10,5L6,5v14zM14,5v14h4L18,5h-4z"],
        Icone::Voltar10 => &["M11.99,5V1l-5,5 5,5V7c3.31,0 6,2.69 6,6s-2.69,6 -6,6 -6,-2.69 -6,-6h-2c0,4.42 3.58,8 8,8s8,-3.58 8,-8 -3.58,-8 -8,-8zM10.89,16h-0.85v-3.26l-1.01,0.31v-0.69l1.77,-0.63h0.09L10.89,16zM15.17,14.24c0,0.32 -0.03,0.6 -0.1,0.82s-0.17,0.42 -0.29,0.57 -0.28,0.26 -0.45,0.33 -0.37,0.1 -0.59,0.1 -0.41,-0.03 -0.59,-0.1 -0.33,-0.18 -0.46,-0.33 -0.23,-0.34 -0.3,-0.57 -0.11,-0.5 -0.11,-0.82v-0.74c0,-0.32 0.03,-0.6 0.1,-0.82s0.17,-0.42 0.29,-0.57 0.28,-0.26 0.45,-0.33 0.37,-0.1 0.59,-0.1 0.41,0.03 0.59,0.1 0.33,0.18 0.46,0.33 0.23,0.34 0.3,0.57 0.11,0.5 0.11,0.82L15.17,14.24zM14.32,13.38c0,-0.19 -0.01,-0.35 -0.04,-0.48s-0.07,-0.23 -0.12,-0.31 -0.11,-0.14 -0.19,-0.17 -0.16,-0.05 -0.25,-0.05 -0.18,0.02 -0.25,0.05 -0.14,0.09 -0.19,0.17 -0.09,0.18 -0.12,0.31 -0.04,0.29 -0.04,0.48v0.97c0,0.19 0.01,0.35 0.04,0.48s0.07,0.24 0.12,0.32 0.11,0.14 0.19,0.17 0.16,0.05 0.25,0.05 0.18,-0.02 0.25,-0.05 0.14,-0.09 0.19,-0.17 0.09,-0.19 0.11,-0.32 0.04,-0.29 0.04,-0.48v-0.97z"],
        Icone::Avancar10 => &["M18,13c0,3.31 -2.69,6 -6,6s-6,-2.69 -6,-6 2.69,-6 6,-6v4l5,-5 -5,-5v4c-4.42,0 -8,3.58 -8,8s3.58,8 8,8 8,-3.58 8,-8h-2zM10.86,15.94v-4.27h-0.09L9,12.3v0.69l1.01,-0.31v3.26h0.85zM12.25,13.44v0.74c0,1.9 1.31,1.82 1.44,1.82 0.14,0 1.44,0.09 1.44,-1.82v-0.74c0,-1.9 -1.31,-1.82 -1.44,-1.82 -0.14,0 -1.44,-0.09 -1.44,1.82zM14.29,13.32v0.97c0,0.77 -0.21,1.03 -0.59,1.03s-0.6,-0.26 -0.6,-1.03v-0.97c0,-0.75 0.22,-1.01 0.59,-1.01 0.38,-0.01 0.6,0.26 0.6,1.01z"],
        Icone::Proximo => &["M6,18l8.5,-6L6,6v12zM16,6v12h2V6h-2z"],
        Icone::Anterior => &["M6,6h2v12H6zM9.5,12l8.5,6V6z"],
        Icone::Episodios => &["M4,6H2v14c0,1.1 0.9,2 2,2h14v-2H4V6zM20,2H8c-1.1,0 -2,0.9 -2,2v12c0,1.1 0.9,2 2,2h12c1.1,0 2,-0.9 2,-2V4c0,-1.1 -0.9,-2 -2,-2zM12,14.5v-9l6,4.5 -6,4.5z"],
        Icone::Legendas => &["M20,4H4c-1.1,0 -2,0.9 -2,2v12c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2V6c0,-1.1 -0.9,-2 -2,-2zM4,12h4v2H4v-2zM14,18H4v-2h10v2zM20,18h-4v-2h4v2zM20,14H10v-2h10v2z"],
        Icone::Lista => &["M3,13h2v-2H3v2zM3,17h2v-2H3v2zM3,9h2V7H3v2zM7,13h14v-2H7v2zM7,17h14v-2H7v2zM7,7v2h14V7H7z"],
        Icone::Guia => &["M17,12h-5v5h5v-5zM16,1v2L8,3L8,1L6,1v2L5,3c-1.11,0 -1.99,0.9 -1.99,2L3,19c0,1.1 0.89,2 2,2h14c1.1,0 2,-0.9 2,-2L21,5c0,-1.1 -0.9,-2 -2,-2h-1L18,1h-2zM19,19L5,19L5,8h14v11z"],
        Icone::MultiTela => &["M3,5v14h8V5H3zM9,17H5V7h4V17zM13,5v14h8V5H13zM19,17h-4V7h4V17z"],
        Icone::Fechar => &["M19,6.41L17.59,5 12,10.59 6.41,5 5,6.41 10.59,12 5,17.59 6.41,19 12,13.41 17.59,19 19,17.59 13.41,12z"],
        Icone::Voltar => &["M20,11H7.83l5.59,-5.59L12,4l-8,8 8,8 1.41,-1.41L7.83,13H20v-2z"],
        Icone::Certo => &["M9,16.17L4.83,12l-1.42,1.41L9,19 21,7l-1.41,-1.41z"],
        Icone::Coracao => &["M12,21.35l-1.45,-1.32C5.4,15.36 2,12.28 2,8.5 2,5.42 4.42,3 7.5,3c1.74,0 3.41,0.81 4.5,2.09C13.09,3.81 14.76,3 16.5,3 19.58,3 22,5.42 22,8.5c0,3.78 -3.4,6.86 -8.55,11.54L12,21.35z"],
        Icone::Brilho => &["M19,9l1.25,-2.75L23,5l-2.75,-1.25L19,1l-1.25,2.75L15,5l2.75,1.25L19,9zM11.5,9.5L9,4L6.5,9.5L1,12l5.5,2.5L9,20l2.5,-5.5L17,12L11.5,9.5zM19,15l-1.25,2.75L15,19l2.75,1.25L19,23l1.25,-2.75L23,19l-2.75,-1.25L19,15z"],
        Icone::Historico => &["M13,3c-4.97,0 -9,4.03 -9,9L1,12l3.89,3.89 0.07,0.14L9,12L6,12c0,-3.87 3.13,-7 7,-7s7,3.13 7,7 -3.13,7 -7,7c-1.93,0 -3.68,-0.79 -4.94,-2.06l-1.42,1.42C8.27,19.99 10.51,21 13,21c4.97,0 9,-4.03 9,-9s-4.03,-9 -9,-9zM12,8v5l4.28,2.54 0.72,-1.21 -3.5,-2.08L13.5,8L12,8z"],
        Icone::Hd => &["M19,4H5c-1.11,0 -2,0.9 -2,2v12c0,1.1 0.89,2 2,2h14c1.1,0 2,-0.9 2,-2V6c0,-1.1 -0.9,-2 -2,-2zM11,15H9.5v-2h-2v2H6V9h1.5v2.5h2V9H11v6zM18,14c0,0.55 -0.45,1 -1,1h-0.75v1.5h-1.5V15H14c-0.55,0 -1,-0.45 -1,-1v-4c0,-0.55 0.45,-1 1,-1h3c0.55,0 1,0.45 1,1v4zM14.5,13.5h2v-3h-2v3z"],
        Icone::Trocar => &["M6.99,11L3,15l3.99,4v-3H14v-2H6.99v-3zM21,9l-3.99,-4v3H10v2h7.01v3L21,9z"],
        Icone::TelaCheia => &["M7,14H5v5h5v-2H7v-3zM5,10h2V7h3V5H5v5zM17,17h-3v2h5v-5h-2v3zM14,5v2h3v3h2V5h-5z"],
        Icone::Som => &["M3,9v6h4l5,5V4L7,9H3zM16.5,12c0,-1.77 -1.02,-3.29 -2.5,-4.03v8.05c1.48,-0.73 2.5,-2.25 2.5,-4.02zM14,3.23v2.06c2.89,0.86 5,3.54 5,6.71s-2.11,5.85 -5,6.71v2.06c4.01,-0.91 7,-4.49 7,-8.77s-2.99,-7.86 -7,-8.77z"],
        Icone::Mudo => &["M16.5,12c0,-1.77 -1.02,-3.29 -2.5,-4.03v2.21l2.45,2.45c0.03,-0.2 0.05,-0.41 0.05,-0.63zM19,12c0,0.94 -0.2,1.82 -0.54,2.64l1.51,1.51C20.63,14.91 21,13.5 21,12c0,-4.28 -2.99,-7.86 -7,-8.77v2.06c2.89,0.86 5,3.54 5,6.71zM4.27,3L3,4.27 7.73,9H3v6h4l5,5v-6.73l4.25,4.25c-0.67,0.52 -1.42,0.93 -2.25,1.18v2.06c1.38,-0.31 2.63,-0.95 3.69,-1.81L19.73,21 21,19.73l-9,-9L4.27,3zM12,4L9.91,6.09 12,8.18V4z"],
        Icone::Preencher => &["M19,12h-2v3h-3v2h5v-5zM7,9h3V7H5v5h2V9zM21,3H3c-1.1,0 -2,0.9 -2,2v14c0,1.1 0.9,2 2,2h18c1.1,0 2,-0.9 2,-2V5c0,-1.1 -0.9,-2 -2,-2zM21,19.01H3V4.99h18v14.02z"],
        Icone::Velocidade => &["M20.38,8.57l-1.23,1.85a8,8 0 0,1 -0.22,7.58H5.07A8,8 0 0,1 15.58,6.85l1.85,-1.23A10,10 0 0,0 3.35,19a2,2 0 0,0 1.72,1h13.85a2,2 0 0,0 1.74,-1 10,10 0 0,0 -0.27,-10.44zM10.59,15.41a2,2 0 0,0 2.83,0l5.66,-8.49 -8.49,5.66a2,2 0 0,0 0,2.83z"],
        Icone::Teclado => &["M20,5H4c-1.1,0 -1.99,0.9 -1.99,2L2,17c0,1.1 0.9,2 2,2h16c1.1,0 2,-0.9 2,-2V7c0,-1.1 -0.9,-2 -2,-2zM11,8h2v2h-2V8zM11,11h2v2h-2v-2zM8,8h2v2H8V8zM8,11h2v2H8v-2zM7,13H5v-2h2v2zM7,10H5V8h2v2zM16,17H8v-2h8v2zM16,13h-2v-2h2v2zM16,10h-2V8h2v2zM19,13h-2v-2h2v2zM19,10h-2V8h2v2z"],
        Icone::Baixar => &["M19,9h-4V3H9v6H5l7,7 7,-7zM5,18v2h14v-2H5z"],
        Icone::Ordenar => &["M3,18h6v-2H3v2zM3,6v2h18V6H3zM3,13h12v-2H3v2z"],
        Icone::Janela => &["M19,7h-8v6h8V7zM21,3H3c-1.1,0 -2,0.9 -2,2v14c0,1.1 0.9,2 2,2h18c1.1,0 1.98,-0.9 1.98,-2L23,5c0,-1.1 -0.9,-2 -2,-2zM21,19.01H3V4.98h18v14.03z"],
        Icone::Parar => &["M6,6h12v12H6z"],
        Icone::Direita => &["M10,6L8.59,7.41 13.17,12l-4.58,4.59L10,18l6,-6z"],
        Icone::Esquerda => &["M15.41,7.41L14,6l-6,6 6,6 1.41,-1.41L10.83,12z"],
        Icone::Cast => &["M1,18v3h3c0,-1.66 -1.34,-3 -3,-3zM1,14v2c2.76,0 5,2.24 5,5h2c0,-3.87 -3.13,-7 -7,-7zM1,10v2c4.97,0 9,4.03 9,9h2c0,-6.08 -4.92,-11 -11,-11zM21,3L3,3c-1.1,0 -2,0.9 -2,2v3h2L3,5h18v14h-7v2h7c1.1,0 2,-0.9 2,-2L23,5c0,-1.1 -0.9,-2 -2,-2z"],
    }
}

/// Pinta o ícone centrado em `centro`, com `lado` pontos e a cor pedida.
pub fn pintar(pintor: &egui::Painter, icone: Icone, centro: Pos2, lado: f32, cor: Color32) {
    let ctx = pintor.ctx();
    let pixels = (lado * ctx.pixels_per_point()).ceil().max(8.0);
    // Poucos tamanhos de textura: o ícone de 18 e o de 20 pontos dividem a de 32.
    let textura_lado = [24u32, 32, 48, 64, 96, 128, 192]
        .into_iter()
        .find(|t| *t as f32 >= pixels)
        .unwrap_or(256);
    let Some(textura) = textura(ctx, icone, textura_lado) else { return };
    let alvo = Rect::from_center_size(centro, egui::vec2(lado, lado));
    pintor.image(textura.id(), alvo, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), cor);
}

fn textura(ctx: &egui::Context, icone: Icone, lado: u32) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("icone-material", icone, lado));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    let cobertura = rasterizar(caminhos(icone), lado as usize);
    let pixels: Vec<Color32> = cobertura
        .into_iter()
        .map(|a| Color32::from_white_alpha((a.clamp(0.0, 1.0) * 255.0).round() as u8))
        .collect();
    let imagem = egui::ColorImage::new([lado as usize, lado as usize], pixels);
    let t = ctx.load_texture(format!("icone-{icone:?}-{lado}"), imagem, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    Some(t)
}

// MARK: - Leitura do caminho

/// Um caminho SVG vira uma lista de polígonos fechados (na caixa de 24).
pub fn poligonos(dados: &str) -> Vec<Vec<(f32, f32)>> {
    let fichas = fichas(dados);
    let mut saida: Vec<Vec<(f32, f32)>> = Vec::new();
    let mut atual: Vec<(f32, f32)> = Vec::new();
    let (mut x, mut y) = (0.0f32, 0.0f32);
    let (mut inicio_x, mut inicio_y) = (0.0f32, 0.0f32);
    // O último ponto de controle, para os comandos S e T.
    let mut controle: Option<(f32, f32)> = None;
    let mut i = 0;
    let mut comando = 'M';
    while i < fichas.len() {
        if let Ficha::Comando(c) = fichas[i] {
            comando = c;
            i += 1;
            if c == 'Z' || c == 'z' {
                if atual.len() > 2 {
                    saida.push(std::mem::take(&mut atual));
                } else {
                    atual.clear();
                }
                x = inicio_x;
                y = inicio_y;
                controle = None;
                continue;
            }
        }
        let relativo = comando.is_ascii_lowercase();
        let (bx, by) = if relativo { (x, y) } else { (0.0, 0.0) };
        let mut ler = |n: usize| -> Option<Vec<f32>> {
            let mut v = Vec::with_capacity(n);
            for _ in 0..n {
                match fichas.get(i) {
                    Some(Ficha::Numero(valor)) => {
                        v.push(*valor);
                        i += 1;
                    }
                    _ => return None,
                }
            }
            Some(v)
        };
        match comando.to_ascii_uppercase() {
            'M' => {
                let Some(v) = ler(2) else { break };
                if atual.len() > 2 {
                    saida.push(std::mem::take(&mut atual));
                } else {
                    atual.clear();
                }
                x = bx + v[0];
                y = by + v[1];
                inicio_x = x;
                inicio_y = y;
                atual.push((x, y));
                // Pares depois do M são linhas.
                comando = if relativo { 'l' } else { 'L' };
                controle = None;
            }
            'L' => {
                let Some(v) = ler(2) else { break };
                x = bx + v[0];
                y = by + v[1];
                atual.push((x, y));
                controle = None;
            }
            'H' => {
                let Some(v) = ler(1) else { break };
                x = if relativo { x + v[0] } else { v[0] };
                atual.push((x, y));
                controle = None;
            }
            'V' => {
                let Some(v) = ler(1) else { break };
                y = if relativo { y + v[0] } else { v[0] };
                atual.push((x, y));
                controle = None;
            }
            'C' => {
                let Some(v) = ler(6) else { break };
                let p1 = (bx + v[0], by + v[1]);
                let p2 = (bx + v[2], by + v[3]);
                let p3 = (bx + v[4], by + v[5]);
                cubica(&mut atual, (x, y), p1, p2, p3);
                controle = Some(p2);
                (x, y) = p3;
            }
            'S' => {
                let Some(v) = ler(4) else { break };
                let p1 = controle.map(|(cx, cy)| (2.0 * x - cx, 2.0 * y - cy)).unwrap_or((x, y));
                let p2 = (bx + v[0], by + v[1]);
                let p3 = (bx + v[2], by + v[3]);
                cubica(&mut atual, (x, y), p1, p2, p3);
                controle = Some(p2);
                (x, y) = p3;
            }
            'Q' => {
                let Some(v) = ler(4) else { break };
                let p1 = (bx + v[0], by + v[1]);
                let p2 = (bx + v[2], by + v[3]);
                let c1 = (x + 2.0 / 3.0 * (p1.0 - x), y + 2.0 / 3.0 * (p1.1 - y));
                let c2 = (p2.0 + 2.0 / 3.0 * (p1.0 - p2.0), p2.1 + 2.0 / 3.0 * (p1.1 - p2.1));
                cubica(&mut atual, (x, y), c1, c2, p2);
                controle = Some(p1);
                (x, y) = p2;
            }
            'A' => {
                let Some(v) = ler(7) else { break };
                let fim = (bx + v[5], by + v[6]);
                arco(&mut atual, (x, y), v[0], v[1], v[2], v[3] != 0.0, v[4] != 0.0, fim);
                (x, y) = fim;
                controle = None;
            }
            _ => {
                i += 1;
            }
        }
    }
    if atual.len() > 2 {
        saida.push(atual);
    }
    saida
}

#[derive(Clone, Copy, Debug)]
enum Ficha {
    Comando(char),
    Numero(f32),
}

fn fichas(dados: &str) -> Vec<Ficha> {
    let mut saida = Vec::new();
    let bytes: Vec<char> = dados.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_alphabetic() && c != 'e' && c != 'E' {
            saida.push(Ficha::Comando(c));
            i += 1;
        } else if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' {
            let inicio = i;
            let mut ponto = c == '.';
            i += 1;
            while i < bytes.len() {
                let d = bytes[i];
                if d.is_ascii_digit() {
                    i += 1;
                } else if d == '.' && !ponto {
                    ponto = true;
                    i += 1;
                } else if (d == 'e' || d == 'E') && i + 1 < bytes.len() {
                    i += 1;
                    if bytes[i] == '-' || bytes[i] == '+' {
                        i += 1;
                    }
                } else {
                    break;
                }
            }
            let texto: String = bytes[inicio..i].iter().collect();
            if let Ok(v) = texto.parse::<f32>() {
                saida.push(Ficha::Numero(v));
            }
        } else {
            i += 1;
        }
    }
    saida
}

fn cubica(pontos: &mut Vec<(f32, f32)>, p0: (f32, f32), p1: (f32, f32), p2: (f32, f32), p3: (f32, f32)) {
    const PASSOS: usize = 12;
    for k in 1..=PASSOS {
        let t = k as f32 / PASSOS as f32;
        let u = 1.0 - t;
        let a = u * u * u;
        let b = 3.0 * u * u * t;
        let c = 3.0 * u * t * t;
        let d = t * t * t;
        pontos.push((
            a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0,
            a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1,
        ));
    }
}

/// O arco elíptico do SVG, pela conversão do ponto final para o centro.
#[allow(clippy::too_many_arguments)]
fn arco(pontos: &mut Vec<(f32, f32)>, de: (f32, f32), rx: f32, ry: f32, rotacao: f32, grande: bool, horario: bool, ate: (f32, f32)) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    if rx < 1e-6 || ry < 1e-6 {
        pontos.push(ate);
        return;
    }
    let fi = rotacao.to_radians();
    let (cf, sf) = (fi.cos(), fi.sin());
    let dx = (de.0 - ate.0) / 2.0;
    let dy = (de.1 - ate.1) / 2.0;
    let x1 = cf * dx + sf * dy;
    let y1 = -sf * dx + cf * dy;
    let lambda = (x1 * x1) / (rx * rx) + (y1 * y1) / (ry * ry);
    if lambda > 1.0 {
        let s = lambda.sqrt();
        rx *= s;
        ry *= s;
    }
    let num = rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1;
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut coef = if den > 0.0 { (num / den).max(0.0).sqrt() } else { 0.0 };
    if grande == horario {
        coef = -coef;
    }
    let cx1 = coef * rx * y1 / ry;
    let cy1 = -coef * ry * x1 / rx;
    let cx = cf * cx1 - sf * cy1 + (de.0 + ate.0) / 2.0;
    let cy = sf * cx1 + cf * cy1 + (de.1 + ate.1) / 2.0;
    let angulo = |ux: f32, uy: f32, vx: f32, vy: f32| (ux * vy - uy * vx).atan2(ux * vx + uy * vy);
    let t1 = angulo(1.0, 0.0, (x1 - cx1) / rx, (y1 - cy1) / ry);
    let mut dt = angulo((x1 - cx1) / rx, (y1 - cy1) / ry, (-x1 - cx1) / rx, (-y1 - cy1) / ry);
    if !horario && dt > 0.0 {
        dt -= std::f32::consts::TAU;
    } else if horario && dt < 0.0 {
        dt += std::f32::consts::TAU;
    }
    let passos = ((dt.abs() / (std::f32::consts::PI / 16.0)).ceil() as usize).max(2);
    for k in 1..=passos {
        let t = t1 + dt * k as f32 / passos as f32;
        let (ct, st) = (t.cos(), t.sin());
        pontos.push((cx + cf * rx * ct - sf * ry * st, cy + sf * rx * ct + cf * ry * st));
    }
}

// MARK: - Rasterização

/// A cobertura (0 a 1) de cada pixel de um quadrado de `lado` pixels, pela
/// regra do "diferente de zero" que os vetores Android usam por padrão.
pub fn rasterizar(caminhos: &[&str], lado: usize) -> Vec<f32> {
    let escala = lado as f32 / 24.0;
    let mut arestas: Vec<(f32, f32, f32, f32)> = Vec::new();
    for dados in caminhos {
        for poligono in poligonos(dados) {
            for k in 0..poligono.len() {
                let a = poligono[k];
                let b = poligono[(k + 1) % poligono.len()];
                arestas.push((a.0 * escala, a.1 * escala, b.0 * escala, b.1 * escala));
            }
        }
    }
    const SUB: usize = 5;
    let mut cobertura = vec![0.0f32; lado * lado];
    let mut cruzamentos: Vec<(f32, i32)> = Vec::new();
    for linha in 0..lado {
        for s in 0..SUB {
            let y = linha as f32 + (s as f32 + 0.5) / SUB as f32;
            cruzamentos.clear();
            for &(x0, y0, x1, y1) in &arestas {
                if (y0 <= y && y1 > y) || (y1 <= y && y0 > y) {
                    let t = (y - y0) / (y1 - y0);
                    cruzamentos.push((x0 + t * (x1 - x0), if y1 > y0 { 1 } else { -1 }));
                }
            }
            cruzamentos.sort_by(|a, b| a.0.total_cmp(&b.0));
            let mut voltas = 0;
            for k in 0..cruzamentos.len() {
                let antes = voltas;
                voltas += cruzamentos[k].1;
                if antes == 0 && voltas != 0 {
                    // Começa um trecho cheio; acaba no cruzamento que zera.
                    let inicio = cruzamentos[k].0;
                    let mut fim = inicio;
                    let mut v = voltas;
                    for c in cruzamentos.iter().skip(k + 1) {
                        v += c.1;
                        if v == 0 {
                            fim = c.0;
                            break;
                        }
                    }
                    preencher(&mut cobertura[linha * lado..(linha + 1) * lado], inicio, fim, 1.0 / SUB as f32);
                }
            }
        }
    }
    cobertura
}

/// Soma `peso` na linha entre `de` e `ate`, com as pontas fracionadas.
fn preencher(linha: &mut [f32], de: f32, ate: f32, peso: f32) {
    let largura = linha.len() as f32;
    let (de, ate) = (de.clamp(0.0, largura), ate.clamp(0.0, largura));
    if ate <= de {
        return;
    }
    let primeiro = de.floor() as usize;
    let ultimo = (ate.ceil() as usize).min(linha.len());
    for (px, valor) in linha.iter_mut().enumerate().take(ultimo).skip(primeiro) {
        let a = (px as f32).max(de);
        let b = (px as f32 + 1.0).min(ate);
        if b > a {
            *valor += (b - a) * peso;
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn quadrado_cheio_cobre_o_miolo() {
        // "Parar" é um quadrado de 6 a 18: o meio cheio, o canto vazio.
        let c = rasterizar(caminhos(Icone::Parar), 24);
        assert!((c[12 * 24 + 12] - 1.0).abs() < 1e-3);
        assert!(c[0] < 1e-3);
    }

    #[test]
    fn furo_do_caminho_fica_vazio() {
        // A lupa tem o vidro vazado: o centro do círculo não é pintado.
        let c = rasterizar(caminhos(Icone::Buscar), 48);
        let centro = (9.5f32 * 2.0) as usize;
        assert!(c[centro * 48 + centro] < 0.05);
    }

    #[test]
    fn todo_icone_desenha_alguma_coisa() {
        for icone in [Icone::Eventos, Icone::Velocidade, Icone::Radio, Icone::Ajustes, Icone::Coracao, Icone::Cast] {
            let soma: f32 = rasterizar(caminhos(icone), 32).iter().sum();
            assert!(soma > 20.0, "{icone:?}");
        }
    }
}
