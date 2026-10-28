//! A janela "Minhas listas" (Ajustes): adicionar o link de uma lista .m3u,
//! atualizar, ativar/desativar e remover. O link digitado nunca é mostrado de
//! volta; cada lista aparece pelo nome.

use crate::minhas_listas::{self as ml, Erro};
use crate::tela::{botao, branco, forte, normal, rotulo_de_secao, Icone, ACENTO, APAGADO, FUNDO, SECUNDARIO, TEXTO, VIVO};
use crate::App;
use eframe::egui::{self, Align2, Stroke};
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct Dialogo {
    pub link: String,
    pub nome: String,
    pub erro: Option<&'static str>,
    /// A lista que espera o "Sim, remover".
    pub confirmar: Option<String>,
    pub focar: bool,
}

impl Dialogo {
    pub fn novo() -> Self {
        Dialogo { focar: true, ..Default::default() }
    }
}

enum Acao {
    Adicionar,
    Arquivo,
    Atualizar(String),
    Alternar(String),
    Remover(String),
    Fechar,
}

fn campo(ui: &mut egui::Ui, texto: &mut String, id: &str, dica: &str) -> egui::Response {
    ui.add(
        egui::TextEdit::singleline(texto)
            .id(egui::Id::new(id))
            .font(normal(16.0))
            .text_color(TEXTO)
            .hint_text(egui::RichText::new(dica).font(normal(16.0)).color(APAGADO))
            .margin(egui::Margin::symmetric(12, 10))
            .desired_width(f32::INFINITY),
    )
}

pub fn dialogo(app: &mut App, ctx: &egui::Context) {
    let Some(mut d) = app.minhas_dialogo.clone() else { return };
    let pintor = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("veu-minhas-listas")));
    pintor.rect_filled(ctx.content_rect(), 0.0, egui::Color32::from_black_alpha(140));
    let listas = app.minhas_meta.clone();
    let ocupado = !app.minhas_ocupadas.is_empty();
    let mut acao: Option<Acao> = None;
    let mut mudou = false;
    egui::Area::new(egui::Id::new("minhas-listas"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(FUNDO)
                .stroke(Stroke::new(1.0, branco(20)))
                .corner_radius(24.0)
                .inner_margin(egui::Margin::same(28))
                .shadow(egui::epaint::Shadow { offset: [0, 12], blur: 40, spread: 0, color: egui::Color32::from_black_alpha(170) })
                .show(ui, |ui| {
                    ui.set_width(600.0);
                    rotulo_de_secao(ui, "Listas");
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Minhas listas").font(forte(26.0)).color(TEXTO));
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Cole o link de uma lista .m3u ou escolha o arquivo. Ela fica só neste aparelho.")
                            .font(normal(15.0))
                            .color(SECUNDARIO),
                    );
                    ui.add_space(14.0);
                    if !listas.is_empty() {
                        egui::ScrollArea::vertical().max_height(250.0).id_salt("minhas-listas-rolagem").show(ui, |ui| {
                            for l in &listas {
                                ui.horizontal(|ui| {
                                    ui.vertical(|ui| {
                                        ui.set_width(250.0);
                                        ui.label(egui::RichText::new(&l.nome).font(forte(16.0)).color(if l.ativa { TEXTO } else { SECUNDARIO }));
                                        let mut sub = format!("{} canais · {}", l.canais, ml::ha_quanto_tempo(l.atualizada_em));
                                        if !l.ativa {
                                            sub.push_str(" · desativada");
                                        }
                                        ui.label(egui::RichText::new(sub).font(normal(13.0)).color(SECUNDARIO));
                                    });
                                    if app.minhas_ocupadas.contains(&l.id) {
                                        ui.add(egui::Spinner::new().color(ACENTO));
                                    } else if d.confirmar.as_deref() == Some(l.id.as_str()) {
                                        if botao(ui, Some(Icone::Fechar), "Sim, remover", true).clicked() {
                                            acao = Some(Acao::Remover(l.id.clone()));
                                        }
                                        if botao(ui, None, "Não", false).clicked() {
                                            d.confirmar = None;
                                        }
                                    } else {
                                        if botao(ui, Some(Icone::Trocar), "Atualizar", false).clicked() {
                                            acao = Some(Acao::Atualizar(l.id.clone()));
                                        }
                                        if botao(ui, None, if l.ativa { "Desativar" } else { "Ativar" }, false).clicked() {
                                            acao = Some(Acao::Alternar(l.id.clone()));
                                        }
                                        if botao(ui, None, "Remover", false).clicked() {
                                            d.confirmar = Some(l.id.clone());
                                        }
                                    }
                                });
                                ui.add_space(8.0);
                            }
                        });
                        ui.add_space(10.0);
                    }
                    ui.label(egui::RichText::new("Link da lista (.m3u)").font(forte(14.0)).color(SECUNDARIO));
                    ui.add_space(4.0);
                    let r = campo(ui, &mut d.link, "minhas-link", "http://… ou https://…");
                    if d.focar {
                        r.request_focus();
                        d.focar = false;
                    }
                    mudou |= r.changed();
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        acao = Some(Acao::Adicionar);
                    }
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Nome (opcional)").font(forte(14.0)).color(SECUNDARIO));
                    ui.add_space(4.0);
                    let r = campo(ui, &mut d.nome, "minhas-nome", "Lista 1");
                    mudou |= r.changed();
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        acao = Some(Acao::Adicionar);
                    }
                    if let Some(e) = d.erro {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(e).font(normal(14.0)).color(VIVO));
                    }
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if ocupado && !app.minhas_ocupadas.is_empty() && app.minhas_adicionando {
                            ui.add(egui::Spinner::new().color(ACENTO));
                            ui.add_space(8.0);
                            ui.label(egui::RichText::new("Lendo a lista…").font(normal(14.0)).color(SECUNDARIO));
                        } else if botao(ui, Some(Icone::Certo), "Adicionar lista", true).clicked() {
                            acao = Some(Acao::Adicionar);
                        }
                        ui.add_space(8.0);
                        if !ocupado && botao(ui, None, "Escolher arquivo", false).clicked() {
                            acao = Some(Acao::Arquivo);
                        }
                        ui.add_space(8.0);
                        if botao(ui, None, "Fechar", false).clicked() {
                            acao = Some(Acao::Fechar);
                        }
                    });
                });
        });
    if ocupado {
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    if mudou {
        d.erro = None;
    }
    match acao {
        Some(Acao::Fechar) => {
            app.minhas_dialogo = None;
            return;
        }
        Some(Acao::Adicionar) => {
            let (link, nome) = (d.link.trim().to_string(), d.nome.clone());
            if link.is_empty() {
                d.erro = Some("Cole o link da lista");
            } else {
                match app.minhas_adicionar(&link, &nome) {
                    Ok(()) => d.erro = None,
                    Err(e) => d.erro = Some(e.mensagem()),
                }
            }
        }
        Some(Acao::Arquivo) => match app.minhas_adicionar_arquivo(&d.nome) {
            Ok(()) => d.erro = None,
            Err(e) => d.erro = Some(e.mensagem()),
        },
        Some(Acao::Atualizar(id)) => {
            d.erro = None;
            app.minhas_atualizar(&id, false);
        }
        Some(Acao::Alternar(id)) => app.minhas_alternar(&id),
        Some(Acao::Remover(id)) => {
            d.confirmar = None;
            app.minhas_remover(&id);
        }
        None => {}
    }
    app.minhas_dialogo = Some(d);
}

/// O erro que volta da thread de fundo, para a janela mostrar.
pub fn mostrar_erro(app: &mut App, e: Erro) {
    if let Some(d) = app.minhas_dialogo.as_mut() {
        d.erro = Some(e.mensagem());
    }
}
