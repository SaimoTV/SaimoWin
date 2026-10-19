//! O desenho, refeito para ser o da TV Box no computador.
//!
//! O mesmo menu do topo em todas as telas (Buscar, Início, Ao vivo, Eventos,
//! On Demand, Favoritos, Rádios, a engrenagem e o relógio); as páginas do
//! acervo com o destaque em cima — o título em foco com a imagem larga dele ao
//! fundo — e as fileiras embaixo; o ao vivo com o vídeo inteiro e a lista de
//! canais por cima; a ficha e a página da série como na TV. As cores, o
//! contorno ciano do foco e o branco do botão escolhido são os da TV Box.
//!
//! O que muda é a mão: no lugar do controle remoto, mouse e teclado. Passar o
//! mouse num cartão é o "foco" da TV (troca o destaque), clicar é o OK, e as
//! setas continuam andando pelas fileiras. A janela pode ter qualquer tamanho:
//! o desenho é feito para 1280 pontos de largura e a escala acompanha a janela.

pub mod icones;
mod aovivo;
mod ficha;
mod grade;
mod janelas;
mod paginas;
mod topo;
mod video;

use crate::{Aba, App};
use eframe::egui::{self, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Vec2};
pub use icones::Icone;
pub use paginas::{
    abrir_em_foco_na_fileira, em_fileiras, favoritar_em_foco_na_fileira, ficha_em_foco_na_fileira, seguir_destaque,
    tamanho_das_filas, Heroi,
};
pub use video::{abertura, telas_da_multi_tela, veu_da_abertura};

// MARK: - Cores da TV Box (res/values/colors.xml)

/// `surface`: o fundo de tudo.
pub const FUNDO: Color32 = Color32::from_rgb(11, 14, 20);
/// `accent`: o único destaque — foco, andamento, a aba escolhida.
pub const ACENTO: Color32 = Color32::from_rgb(79, 227, 208);
pub const TEXTO: Color32 = Color32::WHITE;
/// `text_secondary`.
pub const SECUNDARIO: Color32 = Color32::from_rgb(168, 180, 192);
/// `text_faint`: branco a 40%.
pub const APAGADO: Color32 = Color32::from_rgba_premultiplied(102, 102, 102, 102);
/// `live`: o vermelho do "AO VIVO".
pub const VIVO: Color32 = Color32::from_rgb(255, 77, 79);
pub const AMARELO: Color32 = Color32::from_rgb(255, 196, 61);
/// `surface_raised`: branco a 8%, o fundo das capas e dos ladrilhos.
pub const ELEVADO: Color32 = Color32::from_rgba_premultiplied(20, 20, 20, 20);
/// `surface_card`: o vidro escuro dos cartões sobre o vídeo.
pub const VIDRO: Color32 = Color32::from_rgba_premultiplied(9, 12, 17, 217);
/// `bg_painel_lateral`: a lista de canais e os painéis por cima do vídeo.
pub const PAINEL: Color32 = Color32::from_rgba_premultiplied(10, 13, 19, 242);

/// Branco com a opacidade pedida (0 a 255), já pré-multiplicado.
pub fn branco(alfa: u8) -> Color32 {
    Color32::from_white_alpha(alfa)
}

/// A cor do fundo com a opacidade pedida.
pub fn fundo_com(alfa: u8) -> Color32 {
    FUNDO.gamma_multiply(alfa as f32 / 255.0)
}

pub fn forte(tamanho: f32) -> FontId {
    FontId::new(tamanho, egui::FontFamily::Name("forte".into()))
}

pub fn normal(tamanho: f32) -> FontId {
    FontId::proportional(tamanho)
}

/// Visual base e, no Windows, a fonte do sistema (Segoe UI e a Semibold no
/// lugar do sans-serif-medium da TV Box).
pub fn aplicar_estilo(ctx: &egui::Context) {
    let mut fontes = egui::FontDefinitions::default();
    let candidatas = [
        ("sistema", "C:\\Windows\\Fonts\\segoeui.ttf"),
        ("sistema-forte", "C:\\Windows\\Fonts\\seguisb.ttf"),
    ];
    let mut achou = false;
    for (nome, caminho) in candidatas {
        if let Ok(bytes) = std::fs::read(caminho) {
            fontes.font_data.insert(nome.into(), std::sync::Arc::new(egui::FontData::from_owned(bytes)));
            achou = true;
        }
    }
    if achou {
        let familia = fontes.families.entry(egui::FontFamily::Proportional).or_default();
        familia.insert(0, "sistema".into());
        if fontes.font_data.contains_key("sistema-forte") {
            fontes
                .families
                .insert(egui::FontFamily::Name("forte".into()), vec!["sistema-forte".into(), "sistema".into()]);
        }
    }
    if !fontes.families.contains_key(&egui::FontFamily::Name("forte".into())) {
        let padrao = fontes.families[&egui::FontFamily::Proportional].clone();
        fontes.families.insert(egui::FontFamily::Name("forte".into()), padrao);
    }
    ctx.set_fonts(fontes);

    let mut estilo = (*ctx.global_style()).clone();
    let mut visual = egui::Visuals::dark();
    visual.panel_fill = FUNDO;
    visual.window_fill = FUNDO;
    visual.window_stroke = Stroke::new(1.0, branco(20));
    visual.window_corner_radius = 24.0.into();
    visual.menu_corner_radius = 16.0.into();
    visual.extreme_bg_color = branco(31);
    visual.faint_bg_color = branco(20);
    visual.selection.bg_fill = ACENTO.gamma_multiply(0.45);
    visual.selection.stroke = Stroke::new(1.0, TEXTO);
    visual.hyperlink_color = ACENTO;
    visual.override_text_color = Some(TEXTO);
    visual.popup_shadow = egui::epaint::Shadow { offset: [0, 8], blur: 28, spread: 0, color: Color32::from_black_alpha(140) };
    for w in [
        &mut visual.widgets.inactive,
        &mut visual.widgets.hovered,
        &mut visual.widgets.active,
        &mut visual.widgets.open,
        &mut visual.widgets.noninteractive,
    ] {
        w.corner_radius = 10.0.into();
    }
    visual.widgets.inactive.weak_bg_fill = branco(38);
    visual.widgets.inactive.bg_fill = branco(38);
    visual.widgets.hovered.weak_bg_fill = branco(64);
    visual.widgets.hovered.bg_fill = branco(64);
    visual.widgets.active.weak_bg_fill = ACENTO.gamma_multiply(0.5);
    visual.widgets.hovered.bg_stroke = Stroke::new(1.0, ACENTO.gamma_multiply(0.6));
    visual.slider_trailing_fill = true;
    estilo.visuals = visual;
    estilo.spacing.item_spacing = egui::vec2(8.0, 6.0);
    estilo.spacing.button_padding = egui::vec2(12.0, 6.0);
    estilo.spacing.slider_width = 110.0;
    estilo.spacing.menu_margin = egui::Margin::same(10);
    estilo.animation_time = 0.16;
    ctx.set_global_style(estilo);
    // Ctrl + e Ctrl − são do volume aqui, não do tamanho da letra.
    ctx.options_mut(|o| o.zoom_with_keyboard = false);
}

/// A escala acompanha a janela: o desenho é pensado para 1280 pontos de
/// largura, e uma janela maior (ou a tela cheia de uma TV) aumenta tudo junto,
/// como a TV Box em 1080p. Em degraus de 5%, para não refazer as letras a
/// cada pixel de redimensionamento.
fn ajustar_escala(ctx: &egui::Context) {
    let nativo = ctx.native_pixels_per_point().unwrap_or(1.0).max(0.5);
    let fisico = ctx.content_rect().width() * ctx.pixels_per_point();
    let largura = fisico / nativo;
    let alvo = ((largura / 1280.0).clamp(0.9, 1.6) * 20.0).round() / 20.0;
    if (ctx.zoom_factor() - alvo).abs() > 0.01 {
        ctx.set_zoom_factor(alvo);
    }
}

/// Estado vazio de uma lista sem endereço: o texto e o atalho para Ajustes.
pub fn vazio_com_atalho(app: &mut App, ui: &mut egui::Ui, centro: Pos2, aviso: &str) {
    ui.painter().text(centro, Align2::CENTER_CENTER, aviso, normal(17.0), SECUNDARIO);
    let area = Rect::from_center_size(centro + egui::vec2(0.0, 50.0), egui::vec2(220.0, 46.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(area).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        if botao(ui, Some(Icone::Ajustes), "Abrir Ajustes", true).clicked() {
            app.abrir_secao(Aba::Ajustes);
        }
    });
}

pub fn desenhar(app: &mut App, ui: &mut egui::Ui) {
    let ctx = &ui.ctx().clone();
    if !app.pip {
        ajustar_escala(ctx);
    }
    video::video(app, ctx);

    if app.aba == Aba::Canais {
        aovivo::tela(app, ui);
    } else {
        pintar_fundo_do_app(ui, ctx.content_rect());
        topo::menu(app, ui, false);
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            if app.ficha_aberta.is_some() {
                ficha::ficha_tela(app, ui);
            } else if let Some(serie) = app.serie_aberta.clone() {
                ficha::serie(app, ui, &serie);
            } else if app.aba == Aba::Ajustes {
                grade::ajustes(app, ui);
            } else if paginas::em_fileiras(app) {
                paginas::pagina(app, ui);
            } else {
                grade::grade_tela(app, ui);
            }
        });
    }
    janelas::avisos(app, ctx);
    if app.ajuda_aberta {
        janelas::atalhos(app, ctx);
    }
    janelas::atualizacao(app, ctx);
    janelas::aprovacao(app, ctx);
    if app.dialogo_listas.is_some() {
        janelas::listas(app, ctx);
    }
}

// MARK: - Fundo do app

/// A imagem de fundo do app (a mesma da TV Box), carregada uma vez.
fn fundo(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("fundo-do-app");
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    let imagem = image::load_from_memory(include_bytes!("../../assets/fundo.jpg")).ok()?.to_rgba8();
    let (w, h) = imagem.dimensions();
    let cor = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], imagem.as_raw());
    let textura = ctx.load_texture("fundo-do-app", cor, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, textura.clone()));
    Some(textura)
}

/// O fundo das telas do acervo: a aurora da TV Box, um pouco escurecida.
pub fn pintar_fundo_do_app(ui: &egui::Ui, area: Rect) {
    let pintor = ui.ctx().layer_painter(egui::LayerId::background());
    pintor.rect_filled(area, 0.0, FUNDO);
    if let Some(textura) = fundo(ui.ctx()) {
        let uv = uv_cobrindo(textura.size_vec2(), area.size(), 0.5);
        pintor.image(textura.id(), area, uv, Color32::WHITE);
        pintor.rect_filled(area, 0.0, fundo_com(70));
    }
}

// MARK: - Peças de desenho

/// O trecho da imagem que cobre o alvo sem esticar, como o `centerCrop`.
fn uv_cobrindo(imagem: Vec2, alvo: Vec2, foco_y: f32) -> Rect {
    let razao_img = imagem.x / imagem.y.max(1.0);
    let razao_alvo = alvo.x / alvo.y.max(1.0);
    if razao_img > razao_alvo {
        let largura = razao_alvo / razao_img;
        let x0 = (1.0 - largura) / 2.0;
        Rect::from_min_max(Pos2::new(x0, 0.0), Pos2::new(x0 + largura, 1.0))
    } else {
        let altura = razao_img / razao_alvo;
        let y0 = (1.0 - altura) * foco_y;
        Rect::from_min_max(Pos2::new(0.0, y0), Pos2::new(1.0, y0 + altura))
    }
}

/// A imagem cobrindo o retângulo, com os cantos arredondados e a opacidade pedida.
pub fn pintar_cobrindo(ui: &egui::Ui, textura: &egui::TextureHandle, alvo: Rect, cantos: f32, foco_y: f32, opacidade: f32) {
    let uv = uv_cobrindo(textura.size_vec2(), alvo.size(), foco_y);
    egui::Image::new(textura)
        .uv(uv)
        .fit_to_exact_size(alvo.size())
        .corner_radius(cantos)
        .tint(Color32::WHITE.gamma_multiply(opacidade))
        .paint_at(ui, alvo);
}

/// A imagem inteira dentro de `area`, sem esticar: escudo e logo não são quadrados.
pub fn pintar_contido(ui: &egui::Ui, textura: &egui::TextureHandle, area: Rect) {
    let tamanho = textura.size_vec2();
    if tamanho.x <= 0.0 || tamanho.y <= 0.0 {
        return;
    }
    let escala = (area.width() / tamanho.x).min(area.height() / tamanho.y);
    let alvo = Rect::from_center_size(area.center(), tamanho * escala);
    egui::Image::new(textura).fit_to_exact_size(alvo.size()).paint_at(ui, alvo);
}

/// Degradê de três paradas, como os `gradient` com `centerColor` da TV Box.
pub fn degrade3(
    pintor: &egui::Painter,
    alvo: Rect,
    inicio: Color32,
    meio: Color32,
    onde: f32,
    fim: Color32,
    horizontal: bool,
) {
    let mut malha = egui::Mesh::default();
    let pontos: [(f32, Color32); 3] = [(0.0, inicio), (onde, meio), (1.0, fim)];
    for (t, cor) in pontos {
        if horizontal {
            let x = alvo.left() + alvo.width() * t;
            malha.colored_vertex(Pos2::new(x, alvo.top()), cor);
            malha.colored_vertex(Pos2::new(x, alvo.bottom()), cor);
        } else {
            let y = alvo.top() + alvo.height() * t;
            malha.colored_vertex(Pos2::new(alvo.left(), y), cor);
            malha.colored_vertex(Pos2::new(alvo.right(), y), cor);
        }
    }
    for k in 0..2u32 {
        let a = k * 2;
        malha.add_triangle(a, a + 1, a + 2);
        malha.add_triangle(a + 1, a + 2, a + 3);
    }
    pintor.add(egui::Shape::mesh(malha));
}

/// Texto que acaba em reticências depois de `linhas` linhas; devolve a altura.
#[allow(clippy::too_many_arguments)]
pub fn texto(
    pintor: &egui::Painter,
    texto: &str,
    fonte: FontId,
    cor: Color32,
    canto: Pos2,
    largura: f32,
    linhas: usize,
    centro: bool,
) -> f32 {
    let mut job = egui::text::LayoutJob::simple(texto.to_string(), fonte, cor, largura.max(10.0));
    job.wrap.max_rows = linhas;
    job.wrap.break_anywhere = false;
    job.wrap.overflow_character = Some('…');
    if centro {
        job.halign = egui::Align::Center;
    }
    let galeria = pintor.layout_job(job);
    let altura = galeria.size().y;
    pintor.galley(canto, galeria, cor);
    altura
}

/// Uma linha de texto centrada na vertical em `esquerda_meio`.
pub fn texto_numa_linha(pintor: &egui::Painter, conteudo: &str, fonte: FontId, cor: Color32, esquerda_meio: Pos2, largura: f32) {
    let mut job = egui::text::LayoutJob::simple(conteudo.to_string(), fonte, cor, largura.max(10.0));
    job.wrap.max_rows = 1;
    job.wrap.overflow_character = Some('…');
    let galeria = pintor.layout_job(job);
    pintor.galley(Pos2::new(esquerda_meio.x, esquerda_meio.y - galeria.size().y / 2.0), galeria, cor);
}

/// O quanto um elemento está "aceso" (passando o mouse ou em foco), de 0 a 1,
/// com a transição suave do egui.
pub fn aceso(ui: &egui::Ui, id: egui::Id, ligado: bool) -> f32 {
    ui.ctx().animate_bool_with_time(id, ligado, 0.14)
}

/// O contorno de foco da TV Box (`row_focus`): véu claro e borda ciano.
pub fn moldura_de_foco(pintor: &egui::Painter, rect: Rect, t: f32, cantos: f32) {
    if t <= 0.01 {
        return;
    }
    pintor.rect_filled(rect, cantos, branco((38.0 * t) as u8));
    pintor.rect_stroke(rect, cantos, Stroke::new(3.0, ACENTO.gamma_multiply(t)), egui::StrokeKind::Inside);
}

/// O fundo do botão da TV Box: branco a 15%, branco cheio no foco.
fn cor_de_botao(t: f32, principal: bool) -> (Color32, Color32) {
    let base = if principal { branco(235) } else { branco(38) };
    let fundo = base.lerp_to_gamma(Color32::WHITE, t);
    let tinta = if principal { FUNDO } else { TEXTO.lerp_to_gamma(FUNDO, t) };
    (fundo, tinta)
}

/// Botão com ícone e texto, como o "Assistir" da ficha: claro no foco.
pub fn botao(ui: &mut egui::Ui, icone: Option<Icone>, rotulo: &str, principal: bool) -> egui::Response {
    let fonte = forte(15.0);
    let galeria = ui.painter().layout_no_wrap(rotulo.to_string(), fonte.clone(), TEXTO);
    let extra = if icone.is_some() { 26.0 } else { 0.0 };
    let tamanho = egui::vec2(galeria.size().x + 40.0 + extra, 44.0);
    let (rect, resposta) = ui.allocate_exact_size(tamanho, Sense::click());
    let t = aceso(ui, resposta.id, resposta.hovered() || resposta.has_focus());
    let (fundo, tinta) = cor_de_botao(t, principal);
    let rect = rect.expand(1.5 * t);
    ui.painter().rect_filled(rect, 10.0, fundo);
    let mut x = rect.center().x - (galeria.size().x + extra) / 2.0;
    if let Some(icone) = icone {
        icones::pintar(ui.painter(), icone, Pos2::new(x + 10.0, rect.center().y), 20.0, tinta);
        x += extra;
    }
    ui.painter().text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, rotulo, fonte, tinta);
    resposta.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Botão redondo só de ícone, como os do player da TV Box (`botao_player`).
pub fn botao_redondo(ui: &mut egui::Ui, icone: Icone, lado: f32, dica: &str, ativo: bool) -> egui::Response {
    let (rect, resposta) = ui.allocate_exact_size(Vec2::splat(lado), Sense::click());
    let t = aceso(ui, resposta.id, resposta.hovered());
    let (fundo, tinta) = cor_de_botao(t, false);
    ui.painter().circle_filled(rect.center(), lado / 2.0, fundo);
    if ativo && t < 0.5 {
        ui.painter().circle_stroke(rect.center(), lado / 2.0 - 1.0, Stroke::new(2.0, ACENTO));
    }
    icones::pintar(ui.painter(), icone, rect.center(), lado * 0.5, tinta);
    resposta.on_hover_text(dica).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Pílula com ícone e texto, a dos botões do player e dos filtros.
pub fn pilula(ui: &mut egui::Ui, icone: Option<Icone>, rotulo: &str, ativa: bool, altura: f32) -> egui::Response {
    let fonte = if ativa { forte(14.0) } else { normal(14.0) };
    let galeria = ui.painter().layout_no_wrap(rotulo.to_string(), fonte.clone(), TEXTO);
    let extra = if icone.is_some() { 24.0 } else { 0.0 };
    let tamanho = egui::vec2(galeria.size().x + 32.0 + extra, altura);
    let (rect, resposta) = ui.allocate_exact_size(tamanho, Sense::click());
    let t = aceso(ui, resposta.id, resposta.hovered());
    let (fundo, tinta) = cor_de_botao(t, false);
    ui.painter().rect_filled(rect, altura / 2.0, fundo);
    if ativa && t < 0.5 {
        ui.painter().rect_stroke(rect, altura / 2.0, Stroke::new(2.0, ACENTO), egui::StrokeKind::Inside);
    }
    let mut x = rect.left() + 16.0;
    if let Some(icone) = icone {
        icones::pintar(ui.painter(), icone, Pos2::new(x + 9.0, rect.center().y), 18.0, tinta);
        x += extra;
    }
    ui.painter().text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, rotulo, fonte, tinta);
    resposta.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// O campo de busca da TV Box (`bg_campo`): claro, com o contorno ciano no foco.
pub fn campo_de_busca(ui: &mut egui::Ui, conteudo: &mut String, dica: &str, id: &str, largura: f32, altura: f32) -> egui::Response {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(largura, altura), Sense::hover());
    let id_campo = egui::Id::new(id);
    let focado = ui.memory(|m| m.has_focus(id_campo));
    let t = aceso(ui, id_campo.with("foco"), focado);
    ui.painter().rect_filled(rect, 12.0, branco(31 + (7.0 * t) as u8));
    if t > 0.01 {
        ui.painter().rect_stroke(rect, 12.0, Stroke::new(2.0, ACENTO.gamma_multiply(t)), egui::StrokeKind::Inside);
    }
    icones::pintar(ui.painter(), Icone::Buscar, Pos2::new(rect.left() + 20.0, rect.center().y), 20.0, SECUNDARIO);
    let tamanho_letra = (altura * 0.36).clamp(13.0, 20.0);
    let campo = Rect::from_min_max(Pos2::new(rect.left() + 38.0, rect.top()), Pos2::new(rect.right() - 12.0, rect.bottom()));
    let mut resposta = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(campo).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        resposta = Some(
            ui.add(
                egui::TextEdit::singleline(conteudo)
                    .id(id_campo)
                    .font(normal(tamanho_letra))
                    .hint_text(egui::RichText::new(dica).font(normal(tamanho_letra)).color(APAGADO))
                    .frame(egui::Frame::NONE)
                    .desired_width(f32::INFINITY),
            ),
        );
    });
    resposta.expect("o campo é sempre desenhado")
}

/// Selo pequeno com contorno (`bg_selo`): nota, ano, duração.
pub fn selo(ui: &mut egui::Ui, icone: Option<Icone>, rotulo: &str, cor: Color32) {
    let fonte = forte(13.5);
    let galeria = ui.painter().layout_no_wrap(rotulo.to_string(), fonte.clone(), cor);
    let extra = if icone.is_some() { 18.0 } else { 0.0 };
    let (rect, _) = ui.allocate_exact_size(egui::vec2(galeria.size().x + 20.0 + extra, 28.0), Sense::hover());
    ui.painter().rect_filled(rect, 6.0, branco(20));
    ui.painter().rect_stroke(rect, 6.0, Stroke::new(1.0, branco(64)), egui::StrokeKind::Inside);
    let mut x = rect.left() + 10.0;
    if let Some(icone) = icone {
        icones::pintar(ui.painter(), icone, Pos2::new(x + 7.0, rect.center().y), 14.0, cor);
        x += extra;
    }
    ui.painter().text(Pos2::new(x, rect.center().y), Align2::LEFT_CENTER, rotulo, fonte, cor);
}

/// O selo vermelho "AO VIVO" (`bg_chip`), ou o neutro com borda ciano.
pub fn selo_vivo(pintor: &egui::Painter, canto: Pos2, rotulo: &str, vivo: bool, tamanho: f32) -> Rect {
    let fonte = forte(tamanho);
    let galeria = pintor.layout_no_wrap(rotulo.to_string(), fonte, TEXTO);
    let rect = Rect::from_min_size(canto, galeria.size() + egui::vec2(tamanho * 1.4, tamanho * 0.6));
    if vivo {
        pintor.rect_filled(rect, rect.height() / 2.0, VIVO);
    } else {
        pintor.rect_filled(rect, rect.height() / 2.0, fundo_com(204));
        pintor.rect_stroke(rect, rect.height() / 2.0, Stroke::new(1.0, ACENTO.gamma_multiply(0.4)), egui::StrokeKind::Inside);
    }
    pintor.galley(rect.center() - galeria.size() / 2.0, galeria, TEXTO);
    rect
}

/// A barrinha de andamento da TV Box (`progress_track`).
pub fn andamento(pintor: &egui::Painter, trilho: Rect, fracao: f32) {
    let raio = trilho.height() / 2.0;
    pintor.rect_filled(trilho, raio, branco(51));
    let feito = Rect::from_min_size(trilho.min, egui::vec2(trilho.width() * fracao.clamp(0.0, 1.0), trilho.height()));
    if feito.width() > 0.5 {
        pintor.rect_filled(feito, raio, ACENTO);
    }
}

/// O rótulo de seção em maiúsculas e ciano ("REPRODUÇÃO", "TV ABERTA").
pub fn rotulo_de_secao(ui: &mut egui::Ui, rotulo: &str) {
    ui.label(egui::RichText::new(rotulo.to_uppercase()).font(forte(13.0)).color(ACENTO).extra_letter_spacing(1.5));
}

/// A inicial do título, no lugar da capa que ainda não chegou.
pub fn inicial(nome: &str) -> String {
    nome.chars().find(|c| c.is_alphanumeric()).unwrap_or(' ').to_uppercase().to_string()
}

pub fn relogio(segundos: f64) -> String {
    let total = segundos.max(0.0) as i64;
    let (h, m, s) = (total / 3600, (total % 3600) / 60, total % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m:02}:{s:02}") }
}

/// O servidor de onde vem um endereço ("cdn.exemplo.com"), sem porta nem "www.".
pub fn servidor(url: &str) -> String {
    let resto = url.split("://").nth(1).unwrap_or(url);
    let host = resto.split(['/', ':', '?']).next().unwrap_or("");
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn capa_alta_corta_em_cima_e_embaixo() {
        let uv = uv_cobrindo(egui::vec2(100.0, 300.0), egui::vec2(100.0, 100.0), 0.5);
        assert!((uv.width() - 1.0).abs() < 1e-5);
        assert!((uv.height() - 1.0 / 3.0).abs() < 1e-5);
        assert!((uv.top() - 1.0 / 3.0).abs() < 1e-5);
    }

    #[test]
    fn servidor_sem_www_nem_porta() {
        assert_eq!(servidor("https://www.exemplo.com:8080/a.m3u8"), "exemplo.com");
        assert_eq!(relogio(3725.0), "1:02:05");
        assert_eq!(inicial("#Alive"), "A");
    }
}
