//! O que abre por cima de tudo: o aviso curto (o "Toast" da TV), a lista de
//! atalhos e a oferta de versão nova, no modal escuro da TV Box (`bg_modal`).

use super::{botao, branco, forte, normal, acento, FUNDO, SECUNDARIO, TEXTO, VIDRO};
use crate::{atualizacao, listas, App};
use eframe::egui::{self, Align2, Stroke, Vec2};
use std::time::Duration;

pub fn modal() -> egui::Frame {
    egui::Frame::NONE
        .fill(FUNDO)
        .stroke(Stroke::new(1.0, branco(20)))
        .corner_radius(24.0)
        .inner_margin(egui::Margin::same(28))
        .shadow(egui::epaint::Shadow { offset: [0, 12], blur: 40, spread: 0, color: egui::Color32::from_black_alpha(170) })
}

/// O véu que escurece o app atrás do modal.
pub fn veu(ctx: &egui::Context) {
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
                            ui.label(egui::RichText::new(*tecla).font(forte(14.0)).color(acento()));
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
                    super::icones::pintar(ui.painter(), super::Icone::Cast, lugar.center(), 26.0, acento());
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
                    super::icones::pintar(ui.painter(), super::Icone::Link, lugar.center(), 26.0, acento());
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
                        ui.add(egui::Spinner::new().color(acento()));
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

/// Ajustes > Conta Saimo. Sem conta: o QR (e o código curto) para entrar pelo
/// celular, ou e-mail e senha. Com conta: e-mail, sincronizar agora e sair.
/// Criar ou trocar a senha fica no celular ou no site (lá chega o código do e-mail).
pub fn conta(app: &mut App, ctx: &egui::Context) {
    if !app.conta.aberta || app.boas_vindas {
        return;
    }
    veu(ctx);
    ctx.request_repaint_after(Duration::from_millis(500));
    let mut fechar = false;
    let mut sincronizar = false;
    egui::Area::new(egui::Id::new("conta-saimo"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            modal().show(ui, |ui| {
                ui.set_max_width(760.0);
                super::rotulo_de_secao(ui, "Conta Saimo");
                ui.add_space(6.0);
                let c = &mut app.conta;
                if c.logado() {
                    ui.label(egui::RichText::new(c.email().unwrap_or("Conta Saimo").to_string()).font(forte(24.0)).color(TEXTO));
                    ui.add_space(4.0);
                    let estado = if c.sincronizando { "sincronizando…".to_string() } else { c.erro.clone().unwrap_or_else(|| c.ha_quanto()) };
                    ui.label(egui::RichText::new(estado).font(normal(15.0)).color(SECUNDARIO));
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Perfis, onde parou, favoritos, listas e addons. Sair mantém os dados neste computador.")
                            .font(normal(14.0))
                            .color(SECUNDARIO),
                    );
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        if botao(ui, None, "Sincronizar agora", true).clicked() {
                            sincronizar = true;
                        }
                        ui.add_space(8.0);
                        if botao(ui, None, "Sair da conta", false).clicked() {
                            c.sair();
                        }
                        ui.add_space(8.0);
                        if botao(ui, Some(super::Icone::Fechar), "Fechar", false).clicked() {
                            fechar = true;
                        }
                    });
                } else if c.com_senha {
                    ui.label(egui::RichText::new("Entrar com e-mail e senha").font(forte(24.0)).color(TEXTO));
                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new("Entre com o e-mail e a senha da sua conta, ou crie uma conta agora (senha com 8 ou mais).")
                            .font(normal(14.0))
                            .color(SECUNDARIO),
                    );
                    ui.add_space(12.0);
                    ui.add(egui::TextEdit::singleline(&mut c.email_form).hint_text("Seu e-mail").desired_width(420.0));
                    ui.add_space(8.0);
                    let senha = ui.add(egui::TextEdit::singleline(&mut c.senha_form).password(true).hint_text("Senha").desired_width(420.0));
                    let enter = senha.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    if !c.aviso_form.is_empty() {
                        ui.add_space(6.0);
                        ui.label(egui::RichText::new(c.aviso_form.clone()).font(normal(14.0)).color(egui::Color32::from_rgb(248, 113, 113)));
                    }
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if botao(ui, None, if c.entrando { "Um momento…" } else { "Entrar" }, true).clicked() || enter {
                            c.entrar_com_senha(false);
                        }
                        ui.add_space(8.0);
                        if botao(ui, None, "Criar conta", false).clicked() {
                            c.entrar_com_senha(true);
                        }
                        ui.add_space(8.0);
                        if botao(ui, None, "Voltar ao QR", false).clicked() {
                            c.com_senha = false;
                            c.aviso_form.clear();
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        let url = c.pedido.as_ref().map(|p| p.url.clone()).unwrap_or_default();
                        if !url.is_empty() && c.qr_url != url {
                            c.qr_textura = crate::qr::gerar(&url).ok().map(|m| ctx.load_texture("qr-conta", m.para_imagem(6), egui::TextureOptions::NEAREST));
                            c.qr_url = url.clone();
                        }
                        let (lugar, _) = ui.allocate_exact_size(Vec2::splat(240.0), egui::Sense::hover());
                        ui.painter().rect_filled(lugar, 14.0, egui::Color32::WHITE);
                        if let (Some(t), false) = (&c.qr_textura, url.is_empty()) {
                            ui.painter().image(t.id(), lugar.shrink(12.0), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                        }
                        ui.add_space(24.0);
                        ui.vertical(|ui| {
                            ui.set_max_width(440.0);
                            ui.label(egui::RichText::new("Entrar na Conta Saimo").font(forte(24.0)).color(TEXTO));
                            ui.add_space(6.0);
                            ui.label(
                                egui::RichText::new("No celular com o Saimo e a sua conta, leia o QR com a câmera. Ou abra Ajustes > Conta Saimo > Entrar numa TV e digite o código:")
                                    .font(normal(14.0))
                                    .color(SECUNDARIO),
                            );
                            ui.add_space(10.0);
                            let codigo = c.pedido.as_ref().map(|p| p.codigo.clone()).unwrap_or_else(|| "…".into());
                            ui.label(egui::RichText::new(codigo).font(forte(40.0)).color(acento()));
                            ui.add_space(6.0);
                            ui.label(egui::RichText::new(c.situacao.clone()).font(normal(14.0)).color(SECUNDARIO));
                            ui.add_space(14.0);
                            ui.horizontal(|ui| {
                                if botao(ui, None, "Entrar com e-mail e senha", true).clicked() {
                                    c.com_senha = true;
                                }
                                ui.add_space(8.0);
                                if botao(ui, Some(super::Icone::Fechar), "Fechar", false).clicked() {
                                    fechar = true;
                                }
                            });
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new("Sem conta, tudo continua só neste computador.").font(normal(13.0)).color(super::APAGADO));
                        });
                    });
                }
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        fechar = true;
    }
    if sincronizar {
        app.sincronizar_conta_agora();
    }
    if fechar {
        app.conta.fechar();
    }
}

fn arquivo_boas_vindas() -> std::path::PathBuf {
    crate::catalogo::pasta().join("boas-vindas.txt")
}

/// Sem conta e sem ter escolhido "entrar sem login": mostra as boas-vindas.
pub fn precisa_boas_vindas(app_conta_logada: bool) -> bool {
    !app_conta_logada && !arquivo_boas_vindas().exists()
}

/// Primeira tela para quem ainda não entrou numa conta: à esquerda entrar ou
/// criar a conta, à direita seguir sem login. A conta é opcional.
pub fn boas_vindas(app: &mut App, ctx: &egui::Context) {
    if !app.boas_vindas {
        return;
    }
    if app.conta.logado() {
        app.boas_vindas = false;
        app.conta.fechar();
        return;
    }
    veu(ctx);
    let mut sem_login = false;
    // O QR já aparece aqui: a conta pede o código enquanto a tela estiver aberta.
    if !app.conta.aberta {
        app.conta.abrir();
    }
    ctx.request_repaint_after(Duration::from_millis(500));
    egui::Area::new(egui::Id::new("boas-vindas"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                modal().stroke(Stroke::new(2.0, acento())).show(ui, |ui| {
                    ui.set_width(440.0);
                    ui.label(egui::RichText::new("Entrar ou criar conta").font(forte(28.0)).color(TEXTO));
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Por que criar uma conta?").font(forte(16.0)).color(TEXTO));
                    ui.add_space(4.0);
                    for t in [
                        "• Perfis, favoritos, listas e addons guardados e em qualquer aparelho.",
                        "• Continue de onde parou no celular, na TV e no computador.",
                        "• Trocou de aparelho ou reinstalou? Nada se perde.",
                    ] {
                        ui.label(egui::RichText::new(t).font(normal(15.0)).color(SECUNDARIO));
                    }
                    ui.add_space(14.0);
                    entrar_embutido(&mut app.conta, ctx, ui);
                });
                ui.add_space(20.0);
                modal().show(ui, |ui| {
                    ui.set_width(440.0);
                    ui.label(egui::RichText::new("Entrar sem login").font(forte(28.0)).color(TEXTO));
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("A conta não é obrigatória. Dá para usar o Saimo TV inteiro sem entrar: tudo fica guardado só neste computador.")
                            .font(normal(16.0))
                            .color(SECUNDARIO),
                    );
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("Crie a conta quando quiser, em Ajustes > Conta Saimo.").font(normal(16.0)).color(SECUNDARIO));
                    ui.add_space(16.0);
                    if botao(ui, None, "Entrar sem login", false).clicked() {
                        sem_login = true;
                    }
                });
            });
        });
    if sem_login {
        let _ = std::fs::write(arquivo_boas_vindas(), "1");
        app.boas_vindas = false;
        app.conta.fechar();
    }
}

/// O QR (e o código) para entrar pelo celular, ou o e-mail e a senha, dentro das
/// boas-vindas. "Entrar com e-mail e senha" troca o QR pelo formulário e
/// "Voltar ao QR" volta.
fn entrar_embutido(c: &mut crate::conta::Conta, ctx: &egui::Context, ui: &mut egui::Ui) {
    if c.com_senha {
        ui.add(egui::TextEdit::singleline(&mut c.email_form).hint_text("Seu e-mail").desired_width(400.0));
        ui.add_space(8.0);
        let senha = ui.add(egui::TextEdit::singleline(&mut c.senha_form).password(true).hint_text("Senha").desired_width(400.0));
        let enter = senha.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if !c.aviso_form.is_empty() {
            ui.add_space(6.0);
            ui.label(egui::RichText::new(c.aviso_form.clone()).font(normal(14.0)).color(egui::Color32::from_rgb(248, 113, 113)));
        }
        ui.add_space(12.0);
        ui.horizontal(|ui| {
            if botao(ui, None, if c.entrando { "Um momento…" } else { "Entrar" }, true).clicked() || enter {
                c.entrar_com_senha(false);
            }
            ui.add_space(8.0);
            if botao(ui, None, "Criar conta", false).clicked() {
                c.entrar_com_senha(true);
            }
        });
        ui.add_space(8.0);
        if botao(ui, None, "Voltar ao QR code", false).clicked() {
            c.com_senha = false;
            c.aviso_form.clear();
        }
        return;
    }
    ui.horizontal(|ui| {
        let url = c.pedido.as_ref().map(|p| p.url.clone()).unwrap_or_default();
        if !url.is_empty() && c.qr_url != url {
            c.qr_textura = crate::qr::gerar(&url).ok().map(|m| ctx.load_texture("qr-conta", m.para_imagem(6), egui::TextureOptions::NEAREST));
            c.qr_url = url.clone();
        }
        let (lugar, _) = ui.allocate_exact_size(Vec2::splat(190.0), egui::Sense::hover());
        ui.painter().rect_filled(lugar, 14.0, egui::Color32::WHITE);
        if let (Some(t), false) = (&c.qr_textura, url.is_empty()) {
            ui.painter().image(t.id(), lugar.shrink(10.0), egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
        }
        ui.add_space(16.0);
        ui.vertical(|ui| {
            ui.set_max_width(200.0);
            ui.label(egui::RichText::new("Leia o QR no celular com o Saimo, ou digite o código em Ajustes > Conta Saimo > Entrar numa TV:").font(normal(13.0)).color(SECUNDARIO));
            ui.add_space(6.0);
            let codigo = c.pedido.as_ref().map(|p| p.codigo.clone()).unwrap_or_else(|| "…".into());
            ui.label(egui::RichText::new(codigo).font(forte(30.0)).color(acento()));
            ui.label(egui::RichText::new(c.situacao.clone()).font(normal(13.0)).color(SECUNDARIO));
        });
    });
    ui.add_space(12.0);
    if botao(ui, None, "Entrar com e-mail e senha", true).clicked() {
        c.com_senha = true;
    }
}
