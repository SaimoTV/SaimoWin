//! Apoie o Saimo TV: por que doar, o Pix direto (QR code para doar pelo
//! celular e o botão que copia o código "copia e cola").

use super::{botao, forte, normal, ACENTO, SECUNDARIO, TEXTO};
use eframe::egui::{self, Color32};
use std::time::{Duration, Instant};

/// Pix "copia e cola" (BR Code estático): o valor vai direto para o desenvolvedor.
const CODIGO_PIX: &str = "00020126580014br.gov.bcb.pix0136950460d4-51c5-49ba-bf71-b720b60cde435204000053039865802BR5925GABRIEL SAIMO FERREIRA ES60085500019962070503***6304F442";

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
                    ui.label(egui::RichText::new("Ajude a manter o Saimo TV ").font(forte(38.0)).color(TEXTO));
                    ui.label(egui::RichText::new("100% gratuito").font(forte(38.0)).color(ACENTO));
                });
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Sem anúncios e sem mensalidade.").font(normal(18.0)).color(TEXTO));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("O Saimo TV é um player: ele toca as listas e os links que você mesmo adiciona. O app não traz nenhum canal, filme ou série dentro; o que você assiste vem das suas próprias listas.").font(normal(15.0)).color(TEXTO));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("O desenvolvimento e as atualizações do app e do site custam dinheiro todo mês. Quem puder ajudar com qualquer valor faz uma diferença enorme para o Saimo TV continuar gratuito e sem anúncios.").font(normal(15.0)).color(SECUNDARIO));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Agora o Pix é direto: o valor vai direto para o desenvolvedor, Gabriel Saimo Ferreira, sem intermediário. Aponte a câmera do celular para o QR code ou copie o código Pix e cole no app do seu banco.").font(normal(15.0)).color(TEXTO));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    vantagem(ui, "Zero anúncios", "Nada interrompendo o que você usa");
                    vantagem(ui, "Atualizações constantes", "Melhorias e correções para o app e o site");
                    vantagem(ui, "Sem mensalidade", "O app é gratuito para todo mundo");
                });
                ui.add_space(18.0);
                let id_copia = egui::Id::new("pix-copiado");
                let copiado = ctx.data(|d| d.get_temp::<Instant>(id_copia))
                    .is_some_and(|t| t.elapsed() < Duration::from_secs(2));
                let rotulo = if copiado { "Código copiado" } else { "Copiar Pix copia e cola" };
                if botao(ui, None, rotulo, true).clicked() {
                    ui.ctx().copy_text(CODIGO_PIX.to_string());
                    ctx.data_mut(|d| d.insert_temp(id_copia, Instant::now()));
                }
                if copiado {
                    ctx.request_repaint_after(Duration::from_millis(250));
                }
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
                        ui.label(egui::RichText::new("Pix direto").font(forte(15.0)).color(Color32::from_rgb(14, 116, 144)));
                        ui.label(egui::RichText::new("Gabriel Saimo Ferreira").font(normal(12.0)).color(Color32::from_rgb(91, 100, 117)));
                    });
                });
        });
    });
}
