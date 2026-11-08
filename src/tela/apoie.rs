//! Apoie o Saimo TV: por que doar, o QR code para doar pelo celular, o link
//! escrito e o botão que abre a página de doação no navegador.

use super::{botao, forte, normal, ACENTO, SECUNDARIO, TEXTO};
use crate::atualizacao;
use eframe::egui::{self, Color32};

const LINK: &str = "https://deflow.exchange/pt/qr/saimo-tv";

/// O QR code, carregado uma vez.
fn qr(ctx: &egui::Context) -> Option<egui::TextureHandle> {
    let id = egui::Id::new("doacao-qr");
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    let imagem = image::load_from_memory(include_bytes!("../../assets/doacao-qr.png")).ok()?.to_rgba8();
    let (w, h) = imagem.dimensions();
    let cor = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], imagem.as_raw());
    let textura = ctx.load_texture("doacao-qr", cor, egui::TextureOptions::NEAREST);
    ctx.data_mut(|d| d.insert_temp(id, textura.clone()));
    Some(textura)
}

fn vantagem(ui: &mut egui::Ui, titulo: &str, texto: &str) {
    egui::Frame::NONE
        .fill(Color32::from_white_alpha(14))
        .corner_radius(12.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(200.0);
            ui.label(egui::RichText::new(titulo).font(forte(15.0)).color(TEXTO));
            ui.label(egui::RichText::new(texto).font(normal(12.5)).color(SECUNDARIO));
        });
}

pub fn tela(ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(50.0);
        ui.horizontal_top(|ui| {
            ui.add_space(60.0);
            ui.vertical(|ui| {
                ui.set_max_width(680.0);
                ui.label(egui::RichText::new("Comunidade Saimo TV").font(forte(14.0)).color(ACENTO));
                ui.add_space(10.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label(egui::RichText::new("Ajude a manter tudo ").font(forte(38.0)).color(TEXTO));
                    ui.label(egui::RichText::new("100% gratuito").font(forte(38.0)).color(ACENTO));
                });
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Sem anúncios, sem mensalidade e livre para todo mundo.").font(normal(18.0)).color(TEXTO));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Manter servidores rápidos, a transmissão e as atualizações de todo dia custa dinheiro todo mês. Quem puder ajudar com qualquer valor faz uma diferença enorme para o Saimo TV continuar no ar para todos.").font(normal(15.0)).color(SECUNDARIO));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    vantagem(ui, "Zero anúncios", "Nada interrompendo o que você assiste");
                    vantagem(ui, "Servidores rápidos", "Transmissão estável e em alta qualidade");
                    vantagem(ui, "Sem mensalidade", "Canais, filmes e séries para todo mundo");
                });
                ui.add_space(18.0);
                ui.horizontal(|ui| {
                    if botao(ui, Some(super::Icone::Coracao), "Fazer doação (Pix ou cripto)", true).clicked() {
                        atualizacao::abrir_no_navegador(LINK);
                    }
                    if botao(ui, None, "Copiar link", false).clicked() {
                        ui.ctx().copy_text(LINK.to_string());
                    }
                });
            });
            ui.add_space(50.0);
            egui::Frame::NONE
                .fill(Color32::WHITE)
                .corner_radius(22.0)
                .inner_margin(24.0)
                .show(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.label(egui::RichText::new("Aponte a câmera do celular").font(forte(17.0)).color(Color32::from_rgb(11, 15, 26)));
                        ui.add_space(10.0);
                        if let Some(t) = qr(&ctx) {
                            ui.image((t.id(), egui::vec2(240.0, 240.0)));
                        }
                        ui.add_space(10.0);
                        ui.label(egui::RichText::new("deflow.exchange/pt/qr/saimo-tv").font(forte(15.0)).color(Color32::from_rgb(14, 116, 144)));
                        ui.label(egui::RichText::new("Pagamento seguro pela Deflow").font(normal(12.0)).color(Color32::from_rgb(91, 100, 117)));
                    });
                });
        });
    });
}
