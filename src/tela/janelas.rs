//! O que abre por cima de tudo: o aviso curto (o "Toast" da TV), a lista de
//! atalhos e a oferta de versão nova, no modal escuro da TV Box (`bg_modal`).

use super::{botao, branco, forte, normal, ACENTO, FUNDO, SECUNDARIO, TEXTO, VIDRO};
use crate::{atualizacao, listas, App};
use eframe::egui::{self, Align2, Stroke, Vec2};
use std::time::Duration;

fn modal() -> egui::Frame {
    egui::Frame::NONE
        .fill(FUNDO)
        .stroke(Stroke::new(1.0, branco(20)))
        .corner_radius(24.0)
        .inner_margin(egui::Margin::same(28))
        .shadow(egui::epaint::Shadow { offset: [0, 12], blur: 40, spread: 0, color: egui::Color32::from_black_alpha(170) })
}

/// O véu que escurece o app atrás do modal.
fn veu(ctx: &egui::Context) {
    let pintor = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::new("veu-do-modal")));
    pintor.rect_filled(ctx.content_rect(), 0.0, egui::Color32::from_black_alpha(140));
}

pub fn avisos(app: &mut App, ctx: &egui::Context) {
    let Some((texto, quando)) = app.aviso.clone() else { return };
    let passou = quando.elapsed();
    if passou > Duration::from_millis(3200) {
        app.aviso = None;
        return;
    }
    // Entra e sai devagar, como o Toast.
    let t = (passou.as_secs_f32() / 0.2).min(1.0).min((3.2 - passou.as_secs_f32()) / 0.3).clamp(0.0, 1.0);
    ctx.request_repaint_after(Duration::from_millis(30));
    egui::Area::new(egui::Id::new("aviso"))
        .order(egui::Order::Tooltip)
        .anchor(Align2::CENTER_BOTTOM, egui::vec2(0.0, -40.0 - 12.0 * (1.0 - t)))
        .interactable(false)
        .show(ctx, |ui| {
            ui.multiply_opacity(t);
            egui::Frame::NONE
                .fill(VIDRO)
                .stroke(Stroke::new(1.0, branco(26)))
                .corner_radius(22.0)
                .inner_margin(egui::Margin::symmetric(22, 11))
                .show(ui, |ui| {
                    ui.label(egui::RichText::new(texto).font(forte(15.0)).color(TEXTO));
                });
        });
}

/// Todos os comandos numa tela só.
pub fn atalhos(app: &mut App, ctx: &egui::Context) {
    let linhas: [(&str, &str); 22] = [
        ("Espaço", "tocar ou pausar"),
        ("Setas", "andar pelas fileiras e pela lista"),
        ("Enter", "abrir ou assistir o escolhido"),
        ("Page Up / Page Down", "canal anterior / próximo"),
        ("Seta direita", "trocar a fonte do canal"),
        ("Setas no filme", "voltar / avançar 10 s"),
        ("0 a 9", "ir direto ao número do canal"),
        ("G", "guia de programação"),
        ("L", "mostrar ou esconder a lista de canais"),
        ("E", "episódios, no player da série"),
        ("Tab", "a próxima aba do menu"),
        ("S", "favoritar (canal, filme ou série)"),
        ("I", "ficha do título: sinopse, elenco e mais"),
        ("Ctrl + F", "buscar canal"),
        ("F  ou  F11", "tela cheia"),
        ("M", "sem som"),
        ("+  −", "volume"),
        ("D", "multi tela: dois canais lado a lado"),
        ("T", "na multi tela, trocar a tela em foco"),
        ("P", "janelinha por cima de tudo"),
        ("H  ou  F1", "esta tela"),
        ("Esc", "voltar / fechar / sair da tela cheia"),
    ];
    veu(ctx);
    let mut fechar = false;
    egui::Area::new(egui::Id::new("atalhos"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            modal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Atalhos do teclado").font(forte(26.0)).color(TEXTO));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if super::botao_redondo(ui, super::Icone::Fechar, 40.0, "Fechar (Esc)", false).clicked() {
                            fechar = true;
                        }
                    });
                });
                ui.add_space(14.0);
                egui::Grid::new("atalhos").num_columns(4).spacing([18.0, 10.0]).show(ui, |ui| {
                    for (k, (tecla, acao)) in linhas.iter().enumerate() {
                        egui::Frame::NONE.fill(branco(26)).corner_radius(8.0).inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| {
                            ui.label(egui::RichText::new(*tecla).font(forte(14.0)).color(ACENTO));
                        });
                        ui.label(egui::RichText::new(*acao).font(normal(14.5)).color(SECUNDARIO));
                        if k % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
                ui.add_space(14.0);
                ui.label(egui::RichText::new("Com o mouse: clique abre, botão direito favorita, e todo botão mostra o atalho ao passar o mouse.").font(normal(13.5)).color(super::APAGADO));
            });
        });
    if fechar {
        app.ajuda_aberta = false;
    }
}

pub fn atualizacao(app: &mut App, ctx: &egui::Context) {
    let Some(versao) = app.nova_versao.clone() else { return };
    veu(ctx);
    let mut escolha = None;
    egui::Area::new(egui::Id::new("atualizacao"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            modal().show(ui, |ui| {
                ui.set_max_width(520.0);
                super::rotulo_de_secao(ui, "Atualização");
                ui.label(egui::RichText::new(format!("Versão {} disponível", versao.numero)).font(forte(26.0)).color(TEXTO));
                ui.label(egui::RichText::new(format!("Você está na {}", crate::VERSAO)).font(normal(15.0)).color(SECUNDARIO));
                if !versao.notas.is_empty() {
                    ui.add_space(10.0);
                    egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                        ui.label(egui::RichText::new(versao.notas.chars().take(900).collect::<String>()).font(normal(14.0)));
                    });
                }
                ui.add_space(10.0);
                ui.label(egui::RichText::new("O download abre no navegador. Depois é só fechar o Saimo TV e instalar por cima.").font(normal(13.0)).color(super::APAGADO));
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if botao(ui, Some(super::Icone::Baixar), "Baixar", true).clicked() {
                        escolha = Some(true);
                    }
                    ui.add_space(8.0);
                    if botao(ui, None, "Depois", false).clicked() {
                        escolha = Some(false);
                    }
                });
            });
        });
    if let Some(baixar) = escolha {
        if baixar {
            atualizacao::abrir_no_navegador(&versao.link);
        }
        atualizacao::adiar(&versao);
        app.nova_versao = None;
    }
}

/// Saimo Connect: um celular novo quer mandar neste computador. Só entra com
/// alguém clicando em "Permitir" aqui; sem resposta em um minuto, fica de fora.
pub fn aprovacao(app: &mut App, ctx: &egui::Context) {
    let Some((cliente, nome, desde)) = app.aprovacoes.front().cloned() else { return };
    veu(ctx);
    let resta = crate::connect::ESPERA_DA_APROVACAO.saturating_sub(desde.elapsed()).as_secs();
    let mut escolha = None;
    egui::Area::new(egui::Id::new("aprovacao-connect"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            modal().show(ui, |ui| {
                ui.set_max_width(520.0);
                ui.horizontal(|ui| {
                    let (lugar, _) = ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::hover());
                    super::icones::pintar(ui.painter(), super::Icone::Cast, lugar.center(), 26.0, ACENTO);
                    super::rotulo_de_secao(ui, "Saimo Connect");
                });
                ui.add_space(6.0);
                ui.label(egui::RichText::new(format!("{nome} quer se conectar a este computador")).font(forte(24.0)).color(TEXTO));
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Permitindo, o celular troca de canal, abre filmes e séries, muda o volume e pode tocar o som daqui. Ele fica lembrado; os Ajustes esquecem todos.")
                        .font(normal(15.0))
                        .color(SECUNDARIO),
                );
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if botao(ui, Some(super::Icone::Certo), "Permitir", true).clicked() {
                        escolha = Some(true);
                    }
                    ui.add_space(8.0);
                    if botao(ui, Some(super::Icone::Fechar), "Recusar", false).clicked() {
                        escolha = Some(false);
                    }
                    ui.add_space(12.0);
                    ui.label(egui::RichText::new(format!("recusa sozinho em {resta} s")).font(normal(13.0)).color(super::APAGADO));
                });
            });
        });
    if let Some(permitido) = escolha {
        crate::connect::responder(cliente, permitido);
        app.aprovacoes.pop_front();
    }
}

/// "Endereço das listas": a edição dos Ajustes (nunca abre sozinha).
pub fn listas(app: &mut App, ctx: &egui::Context) {
    let Some(mut d) = app.dialogo_listas.clone() else { return };
    veu(ctx);
    let mut salvar = false;
    let mut sem_teste = false;
    let mut fechar = false;
    let testando = d.fase == listas::Fase::Testando;
    let mut mudou = false;
    egui::Area::new(egui::Id::new("enderecos-das-listas"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            modal().show(ui, |ui| {
                ui.set_width(560.0);
                ui.horizontal(|ui| {
                    let (lugar, _) = ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::hover());
                    super::icones::pintar(ui.painter(), super::Icone::Link, lugar.center(), 26.0, ACENTO);
                    super::rotulo_de_secao(ui, "Listas");
                });
                ui.add_space(6.0);
                ui.label(egui::RichText::new("Endereço das listas").font(forte(26.0)).color(TEXTO));
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Os endereços já vêm preenchidos. Troque se quiser usar outra lista; em branco, a lista fica desligada.")
                        .font(normal(15.0))
                        .color(SECUNDARIO),
                );
                ui.add_space(16.0);
                for (lista, rotulo, dica) in [
                    (listas::Lista::Canais, "Canais", "endereço da lista de canais"),
                    (listas::Lista::Filmes, "Filmes e séries", "endereço da lista de filmes e séries"),
                ] {
                    ui.label(egui::RichText::new(rotulo).font(forte(14.0)).color(SECUNDARIO));
                    ui.add_space(4.0);
                    let texto = if lista == listas::Lista::Canais { &mut d.canais } else { &mut d.filmes };
                    let id = egui::Id::new(("campo-lista", rotulo));
                    let resposta = ui.add_enabled(
                        !testando,
                        egui::TextEdit::singleline(texto)
                            .id(id)
                            .font(normal(16.0))
                            .text_color(TEXTO)
                            .hint_text(egui::RichText::new(dica).font(normal(16.0)).color(super::APAGADO))
                            .margin(egui::Margin::symmetric(12, 10))
                            .desired_width(f32::INFINITY),
                    );
                    if d.focar && lista == listas::Lista::Canais {
                        resposta.request_focus();
                        d.focar = false;
                    }
                    if resposta.changed() {
                        mudou = true;
                    }
                    if resposta.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) && !testando {
                        salvar = true;
                    }
                    if d.fase == listas::Fase::Falhou && d.falhas.contains(&lista) {
                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Não consegui abrir este endereço. Confira o link.").font(normal(13.5)).color(super::VIVO));
                    }
                    ui.add_space(12.0);
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    if testando {
                        ui.add(egui::Spinner::new().color(ACENTO));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Conferindo os endereços…").font(normal(14.0)).color(SECUNDARIO));
                    } else if d.fase == listas::Fase::Falhou {
                        if botao(ui, Some(super::Icone::Trocar), "Tentar de novo", true).clicked() {
                            salvar = true;
                        }
                        if !d.invalido {
                            ui.add_space(8.0);
                            if botao(ui, Some(super::Icone::Certo), "Salvar mesmo assim", false).clicked() {
                                salvar = true;
                                sem_teste = true;
                            }
                        }
                        ui.add_space(8.0);
                        if botao(ui, None, "Cancelar", false).clicked() {
                            fechar = true;
                        }
                    } else {
                        if botao(ui, Some(super::Icone::Certo), "Salvar", true).clicked() {
                            salvar = true;
                        }
                        ui.add_space(8.0);
                        if botao(ui, None, "Cancelar", false).clicked() {
                            fechar = true;
                        }
                    }
                });
                if !testando {
                    ui.add_space(10.0);
                    if botao(ui, Some(super::Icone::Trocar), "Restaurar padrão", false).clicked() {
                        d.canais = listas::PADRAO_CANAIS.to_string();
                        d.filmes = listas::PADRAO_FILMES.to_string();
                        d.fase = listas::Fase::Editando;
                        d.falhas.clear();
                        d.invalido = false;
                    }
                    ui.add_space(6.0);
                    ui.label(egui::RichText::new("Deixar um campo em branco e salvar desliga e apaga aquela lista deste computador.").font(normal(13.0)).color(super::APAGADO));
                }
            });
        });
    if testando {
        ctx.request_repaint_after(Duration::from_millis(100));
    }
    if mudou && d.fase == listas::Fase::Falhou {
        d.fase = listas::Fase::Editando;
        d.falhas.clear();
        d.invalido = false;
    }
    if fechar {
        app.dialogo_listas = None;
    } else {
        app.dialogo_listas = Some(d);
        if salvar {
            app.salvar_listas(sem_teste);
        }
    }
}
