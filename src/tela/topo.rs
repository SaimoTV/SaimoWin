//! O menu do topo da TV Box (`MenuGlobal.kt`), o mesmo em todas as telas.
//!
//! O ícone do app, as abas com ícone e nome — a da tela atual num fundo claro
//! translúcido, a do mouse em branco com o texto escuro, como o foco da TV —,
//! a engrenagem dos Ajustes e o relógio. Sobre o vídeo, o menu ganha a sombra
//! de cima da TV (`bg_top_scrim`) no lugar do fundo.

use super::{aceso, branco, forte, icones, Icone, ACENTO, FUNDO, TEXTO};
use crate::{epg, Aba, App};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Vec2};
use std::time::Duration;

pub const ALTURA: f32 = 68.0;

/// As abas do menu, na ordem da TV Box. O +18 só existe com a lista liberada.
fn abas(app: &App) -> Vec<(Aba, &'static str, Icone)> {
    let mut abas = vec![
        (Aba::Buscar, "Buscar", Icone::Buscar),
        (Aba::Inicio, "Início", Icone::Inicio),
        (Aba::Canais, "Ao vivo", Icone::AoVivo),
        (Aba::Eventos, "Eventos", Icone::Eventos),
        (Aba::OnDemand, "On Demand", Icone::Filmes),
        (Aba::Favoritos, "Favoritos", Icone::Estrela),
        (Aba::Radios, "Rádios", Icone::Radio),
    ];
    if app.liberado {
        abas.push((Aba::Extras, "+18", Icone::Mais));
    }
    abas
}

/// A aba que fica marcada: as páginas de um tipo moram dentro do On Demand.
fn aba_marcada(aba: Aba) -> Aba {
    if aba.de_tipo() { Aba::OnDemand } else { aba }
}

/// Uma aba: pílula clara na tela atual, branca com texto escuro no mouse.
fn aba(ui: &mut egui::Ui, rotulo: &str, icone: Icone, atual: bool) -> egui::Response {
    let fonte = forte(15.5);
    let galeria = ui.painter().layout_no_wrap(rotulo.to_string(), fonte.clone(), TEXTO);
    let largura = if rotulo.is_empty() { 42.0 } else { galeria.size().x + 30.0 + 26.0 };
    let (rect, resposta) = ui.allocate_exact_size(egui::vec2(largura, 40.0), Sense::click());
    let t = aceso(ui, resposta.id, resposta.hovered());
    let pintor = ui.painter();
    if atual {
        pintor.rect_filled(rect, 20.0, branco(31));
    }
    if t > 0.01 {
        pintor.rect_filled(rect.expand(t), 20.0, Color32::WHITE.gamma_multiply(t));
    }
    let tinta = TEXTO.lerp_to_gamma(FUNDO, t);
    if rotulo.is_empty() {
        icones::pintar(pintor, icone, rect.center(), 22.0, tinta);
    } else {
        icones::pintar(pintor, icone, Pos2::new(rect.left() + 24.0, rect.center().y), 20.0, tinta);
        pintor.text(Pos2::new(rect.left() + 40.0, rect.center().y), Align2::LEFT_CENTER, rotulo, fonte, tinta);
    }
    resposta.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// O ícone do app: o "S" do Saimo num ladrilho ciano arredondado.
fn marca(pintor: &egui::Painter, centro: Pos2) {
    let rect = Rect::from_center_size(centro, Vec2::splat(32.0));
    pintor.rect_filled(rect, 9.0, ACENTO);
    pintor.text(rect.center(), Align2::CENTER_CENTER, "S", forte(20.0), FUNDO);
}

/// Desenha o menu. `sobre_video` troca o fundo pela sombra de cima.
pub fn menu(app: &mut App, ui_pai: &mut egui::Ui, sobre_video: bool) {
    let ctx = ui_pai.ctx().clone();
    egui::Panel::top("menu-topo")
        .exact_size(ALTURA)
        .show_separator_line(false)
        .frame(egui::Frame::NONE)
        .show(ui_pai, |ui| {
            let area = ui.max_rect();
            if sobre_video {
                let sombra = Rect::from_min_size(area.min, egui::vec2(area.width(), area.height() + 50.0));
                super::degrade3(
                    &ctx.layer_painter(egui::LayerId::background()),
                    sombra,
                    Color32::from_black_alpha(242),
                    Color32::from_black_alpha(166),
                    0.5,
                    Color32::TRANSPARENT,
                    false,
                );
            }
            let margem = 28.0;
            // A direita primeiro: relógio, rádio no ar e a engrenagem.
            let hora = epg::hora_local(epg::agora());
            let galeria = ui.painter().layout_no_wrap(hora, forte(19.0), TEXTO);
            let x_hora = area.right() - margem - galeria.size().x;
            ui.painter().galley(Pos2::new(x_hora, area.center().y - galeria.size().y / 2.0), galeria, TEXTO);
            let mut direita = x_hora - 14.0;

            let engrenagem = Rect::from_min_size(Pos2::new(direita - 42.0, area.center().y - 20.0), egui::vec2(42.0, 40.0));
            direita = engrenagem.left() - 8.0;
            let mut escolhida = None;
            ui.scope_builder(egui::UiBuilder::new().max_rect(engrenagem), |ui| {
                if aba(ui, "", Icone::Ajustes, app.aba == Aba::Ajustes).on_hover_text("Ajustes").clicked() {
                    escolhida = Some(Aba::Ajustes);
                }
            });

            if let Some(nome) = app.radio.as_ref().map(|r| r.nome.clone()) {
                direita = radio_no_ar(app, ui, &nome, direita, area.center().y);
            }

            // A esquerda: o ícone e as abas, que rolam para o lado se a
            // janela for estreita, como na TV.
            marca(ui.painter(), Pos2::new(area.left() + margem + 16.0, area.center().y));
            let inicio = area.left() + margem + 44.0;
            let faixa = Rect::from_min_max(Pos2::new(inicio, area.top()), Pos2::new(direita.max(inicio + 40.0), area.bottom()));
            let marcada = aba_marcada(app.aba);
            ui.scope_builder(egui::UiBuilder::new().max_rect(faixa), |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("abas-do-topo")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.set_min_height(faixa.height());
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().item_spacing.x = 2.0;
                            for (destino, rotulo, icone) in abas(app) {
                                if aba(ui, rotulo, icone, marcada == destino).clicked() {
                                    escolhida = Some(destino);
                                }
                            }
                        });
                    });
            });
            if let Some(destino) = escolhida {
                ir(app, destino);
            }
        });
    // O relógio anda sozinho.
    ctx.request_repaint_after(Duration::from_secs(20));
}

/// Vai a uma aba do menu, como o `ir` da TV Box: tudo a um clique.
pub fn ir(app: &mut App, destino: Aba) {
    if destino == Aba::Canais {
        app.lista_aberta = true;
    }
    if app.aba == destino {
        // Clicar de novo na aba atual volta ao começo dela.
        app.fechar_ficha();
        app.serie_aberta = None;
        app.explorar = false;
        app.foco_fila = 0;
        app.foco_vod = 0;
        return;
    }
    app.fechar_ficha();
    app.abrir_secao(destino);
    if destino == Aba::Buscar {
        app.pedir_foco_na_busca = true;
    }
}

/// A rádio no ar, perto do relógio: o equalizador, o nome e o botão de parar.
fn radio_no_ar(app: &mut App, ui: &mut egui::Ui, nome: &str, direita: f32, meio: f32) -> f32 {
    let fonte = forte(13.5);
    let mut job = egui::text::LayoutJob::simple_singleline(nome.to_string(), fonte, TEXTO);
    job.wrap = egui::text::TextWrapping::truncate_at_width(150.0);
    let galeria = ui.painter().layout_job(job);
    let largura = galeria.size().x + 36.0 + 34.0;
    let rect = Rect::from_min_size(Pos2::new(direita - largura, meio - 18.0), egui::vec2(largura, 36.0));
    let resposta = ui.interact(rect, egui::Id::new("radio-no-ar"), Sense::click());
    let t = aceso(ui, resposta.id, resposta.hovered());
    ui.painter().rect_filled(rect, 18.0, branco(31 + (30.0 * t) as u8));
    ui.painter().rect_stroke(rect, 18.0, egui::Stroke::new(1.0, ACENTO.gamma_multiply(0.5)), egui::StrokeKind::Inside);
    equalizador(ui, Rect::from_center_size(Pos2::new(rect.left() + 18.0, meio), egui::vec2(16.0, 16.0)), 4);
    ui.painter().galley(Pos2::new(rect.left() + 32.0, meio - galeria.size().y / 2.0), galeria, TEXTO);
    let parar = Pos2::new(rect.right() - 18.0, meio);
    icones::pintar(ui.painter(), Icone::Parar, parar, 18.0, TEXTO);
    let resposta = resposta.on_hover_text("Clique para parar a rádio");
    if resposta.clicked() {
        app.parar_radio();
    }
    rect.left() - 10.0
}

/// As barras de um equalizador andando, como o da rádio da TV Box.
pub fn equalizador(ui: &egui::Ui, area: Rect, barras: usize) {
    let tempo = ui.input(|i| i.time) as f32;
    let largura = area.width() / (barras as f32 * 1.6);
    for k in 0..barras {
        let fase = k as f32 * 1.7;
        let altura = 0.3 + 0.7 * (0.5 + 0.5 * (tempo * (3.1 + k as f32 * 0.9) + fase).sin()).abs();
        let x = area.left() + k as f32 * largura * 1.6;
        let barra = Rect::from_min_max(Pos2::new(x, area.bottom() - area.height() * altura), Pos2::new(x + largura, area.bottom()));
        ui.painter().rect_filled(barra, largura / 2.0, ACENTO);
    }
    ui.ctx().request_repaint_after(Duration::from_millis(50));
}
