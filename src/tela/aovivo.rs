//! O ao vivo e o player, como a `MainActivity` e a `PlayerActivity` da TV Box.
//!
//! O vídeo ocupa a janela inteira. Por cima, quando a pessoa mexe: o menu do
//! topo sobre a sombra de cima, a lista de canais à esquerda (com as seções em
//! pílulas, o número, a logo, o que passa agora e o andamento) e, embaixo,
//! sobre a sombra de baixo, o canal com o programa e os botões redondos. O guia
//! cobre a tela com os canais de um lado e a programação do outro.
//!
//! No filme e na série, o player da TV Box: a logo do título e a linha do
//! tempo embaixo, os botões em pílula, e o painel de episódios subindo de
//! baixo com as temporadas e os cartões com a imagem da cena.

use super::{
    aceso, andamento, botao, botao_redondo, branco, degrade3, forte, fundo_com, icones, inicial, moldura_de_foco, normal,
    pilula, pintar_contido, pintar_cobrindo, selo_vivo, texto, texto_numa_linha, topo, Icone, ACENTO, APAGADO, FUNDO, PAINEL,
    SECUNDARIO, TEXTO, VIDRO, VIVO,
};
use crate::{epg, progresso, Aba, App};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke, Vec2};
use std::time::Duration;

const LARGURA_DA_LISTA: f32 = 400.0;
const ALTURA_DA_LINHA: f32 = 68.0;

pub fn tela(app: &mut App, ui: &mut egui::Ui) {
    let nada_no_ar = app.tocando.is_none() && app.tocando_vod.is_none() && app.outra_tela.is_none();
    let por_cima = app.controles_visiveis() || app.lista_aberta || nada_no_ar || app.guia_aberto;
    // O fundo do guia e o da rádio vêm antes do menu: pintados depois, na
    // mesma camada do fundo, cobririam o menu.
    let guia_na_tela = app.guia_aberto && app.tocando_vod.is_none();
    let so_radio = app.radio.is_some() && app.tocando.is_none() && app.tocando_vod.is_none() && app.mpv.is_some();
    if guia_na_tela || so_radio {
        super::pintar_fundo_do_app(ui, ui.ctx().content_rect());
    }
    if !app.pip && por_cima && app.tocando_vod.is_none() {
        topo::menu(app, ui, true);
    }
    // Sem nada à vista sobre o vídeo, o ponteiro some também.
    if !por_cima && !app.pip && (app.tocando.is_some() || app.tocando_vod.is_some()) {
        ui.ctx().set_cursor_icon(egui::CursorIcon::None);
    }
    if app.guia_aberto && app.tocando_vod.is_none() {
        guia(app, ui);
        return;
    }
    if app.lista_aberta && app.tocando_vod.is_none() && !app.pip {
        lista(app, ui);
    }
    palco(app, ui);
}

// MARK: - Lista de canais

/// Uma linha da lista: título de seção ou canal.
enum Item {
    Secao(String),
    Canal(usize),
}

fn itens_da_lista(app: &App) -> Vec<Item> {
    let visiveis = app.visiveis();
    let mut itens = Vec::with_capacity(visiveis.len() + 12);
    let mut secao_atual = String::new();
    for indice in visiveis {
        let canal = &app.canais[indice];
        let secao = if app.favoritos.contains(&canal.nome) { "Favoritos".to_string() } else { canal.secao().to_string() };
        if secao != secao_atual {
            itens.push(Item::Secao(secao.clone()));
            secao_atual = secao;
        }
        itens.push(Item::Canal(indice));
    }
    itens
}

fn lista(app: &mut App, ui_pai: &mut egui::Ui) {
    egui::Panel::left("lista-de-canais")
        .exact_size(LARGURA_DA_LISTA)
        .resizable(false)
        .show_separator_line(false)
        .frame(
            egui::Frame::NONE
                .fill(PAINEL)
                .corner_radius(egui::CornerRadius { ne: 24, se: 24, nw: 0, sw: 0 })
                .inner_margin(egui::Margin { left: 16, right: 14, top: 14, bottom: 10 }),
        )
        .show(ui_pai, |ui| {
            let itens = itens_da_lista(app);
            let total = app.visiveis().len();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Canais").font(forte(28.0)).color(TEXTO));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(total.to_string()).font(normal(18.0)).color(SECUNDARIO));
                    if let Some(t) = app.tocando.as_ref() {
                        ui.label(egui::RichText::new((t.canal + 1).to_string()).font(forte(18.0)).color(ACENTO));
                    }
                });
            });
            ui.add_space(6.0);
            let largura = ui.available_width();
            let busca = super::campo_de_busca(ui, &mut app.busca, "Buscar canal", "busca-canais", largura, 42.0);
            if busca.changed() {
                if let Some(primeiro) = app.visiveis().first() {
                    app.foco = *primeiro;
                }
            }

            // As seções em pílulas: um clique leva a lista até ela. A da
            // linha em foco fica branca, como na TV.
            let secao_do_foco = app.canais.get(app.foco).map(|c| {
                if app.favoritos.contains(&c.nome) { "Favoritos".to_string() } else { c.secao().to_string() }
            });
            let secoes: Vec<(String, usize)> = itens
                .iter()
                .enumerate()
                .filter_map(|(i, item)| match item {
                    Item::Secao(nome) => Some((nome.clone(), i)),
                    Item::Canal(_) => None,
                })
                .collect();
            ui.add_space(10.0);
            let mut pular_para: Option<f32> = None;
            egui::ScrollArea::horizontal()
                .id_salt("secoes-canais")
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        for (nome, posicao) in &secoes {
                            let ativa = secao_do_foco.as_deref() == Some(nome.as_str());
                            let resposta = secao(ui, nome, ativa);
                            if ativa && ui.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowUp)) {
                                resposta.scroll_to_me(Some(egui::Align::Center));
                            }
                            if resposta.clicked() {
                                pular_para = Some(*posicao as f32 * ALTURA_DA_LINHA);
                            }
                        }
                    });
                });
            ui.add_space(8.0);

            let tocando = app.tocando.as_ref().map(|t| t.canal);
            let foco = app.foco;
            let mut clicado = None;
            let mut favoritar = None;
            let rolar_para_foco = ui.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowUp));
            let altura_rodape = 34.0;
            let mut rolagem = egui::ScrollArea::vertical()
                .id_salt("lista-de-canais")
                .auto_shrink([false, false])
                .max_height(ui.available_height() - altura_rodape);
            if let Some(y) = pular_para {
                rolagem = rolagem.vertical_scroll_offset(y);
            }
            rolagem.show_rows(ui, ALTURA_DA_LINHA, itens.len(), |ui, faixa| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for item in &itens[faixa] {
                    match item {
                        Item::Secao(nome) => {
                            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ALTURA_DA_LINHA), Sense::hover());
                            ui.painter().text(
                                Pos2::new(rect.left() + 86.0, rect.bottom() - 12.0),
                                Align2::LEFT_BOTTOM,
                                nome.to_uppercase(),
                                forte(13.0),
                                ACENTO,
                            );
                        }
                        Item::Canal(indice) => {
                            let (resposta, estrela) = linha_do_canal(app, ui, *indice, Some(*indice) == tocando, *indice == foco);
                            if estrela || resposta.secondary_clicked() {
                                favoritar = app.canais.get(*indice).map(|c| c.nome.clone());
                            } else if resposta.clicked() {
                                clicado = Some(*indice);
                            }
                            if *indice == foco && rolar_para_foco {
                                resposta.scroll_to_me(Some(egui::Align::Center));
                            }
                        }
                    }
                }
            });
            if let Some(canal) = favoritar {
                app.favoritar(canal);
            }
            if let Some(indice) = clicado {
                app.foco = indice;
                app.tocar(indice, 0, true);
            }
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new("Enter assiste · S favorita · G guia · L esconde a lista")
                    .font(normal(12.5))
                    .color(APAGADO),
            );
        });
}

/// A pílula de uma seção da lista.
fn secao(ui: &mut egui::Ui, nome: &str, ativa: bool) -> egui::Response {
    let fonte = forte(14.0);
    let galeria = ui.painter().layout_no_wrap(nome.to_string(), fonte.clone(), TEXTO);
    let (rect, resposta) = ui.allocate_exact_size(egui::vec2(galeria.size().x + 28.0, 36.0), Sense::click());
    let t = aceso(ui, resposta.id, resposta.hovered() || ativa);
    ui.painter().rect_filled(rect, 18.0, branco(38).lerp_to_gamma(Color32::WHITE, t));
    ui.painter().text(rect.center(), Align2::CENTER_CENTER, nome, fonte, TEXTO.lerp_to_gamma(FUNDO, t));
    resposta.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A linha do canal (`item_channel.xml`): número, logo, nome, o que passa e o
/// andamento. Devolve também se o clique foi na estrela.
fn linha_do_canal(app: &mut App, ui: &mut egui::Ui, indice: usize, tocando: bool, focado: bool) -> (egui::Response, bool) {
    let (nome, logo) = {
        let c = &app.canais[indice];
        (c.nome.clone(), c.logo.clone())
    };
    let favorito = app.favoritos.contains(&nome);
    let textura = logo.as_ref().and_then(|url| app.imagem(url, 128));
    let no_ar = epg::agora_e_depois(&nome).map(|(p, _)| p);

    let (rect, resposta) = ui.allocate_exact_size(egui::vec2(ui.available_width(), ALTURA_DA_LINHA), Sense::click());
    let caixa = rect.shrink2(egui::vec2(0.0, 3.0));
    let pintor = ui.painter().clone();
    if tocando {
        pintor.rect_filled(caixa, 14.0, branco(20));
        pintor.rect_stroke(caixa, 14.0, Stroke::new(2.0, ACENTO.gamma_multiply(0.35)), egui::StrokeKind::Inside);
    }
    let t = aceso(ui, resposta.id, focado || resposta.hovered());
    moldura_de_foco(&pintor, caixa, t, 14.0);

    // O número do canal, como no controle: é o que se digita para ir direto.
    pintor.text(
        Pos2::new(caixa.left() + 30.0, caixa.center().y),
        Align2::RIGHT_CENTER,
        (indice + 1).to_string(),
        normal(15.0),
        if tocando { ACENTO } else { APAGADO },
    );
    let lado = 48.0;
    let ladrilho = Rect::from_min_size(Pos2::new(caixa.left() + 40.0, caixa.center().y - lado / 2.0), Vec2::splat(lado));
    pintor.rect_filled(ladrilho, 12.0, branco(31));
    match textura {
        Some(t) => pintar_contido(ui, &t, ladrilho.shrink(5.0)),
        None => {
            pintor.text(ladrilho.center(), Align2::CENTER_CENTER, inicial(&nome), forte(18.0), ACENTO);
        }
    }
    let x = ladrilho.right() + 14.0;
    let direita = caixa.right() - 34.0;
    let largura = (direita - x).max(20.0);
    match no_ar.as_ref() {
        Some(programa) => {
            texto_numa_linha(&pintor, &nome, forte(16.5), TEXTO, Pos2::new(x, caixa.top() + 17.0), largura);
            texto_numa_linha(&pintor, &programa.titulo, normal(13.5), SECUNDARIO, Pos2::new(x, caixa.top() + 37.0), largura);
            andamento(&pintor, Rect::from_min_size(Pos2::new(x, caixa.bottom() - 10.0), egui::vec2(largura, 3.0)), programa.andamento(epg::agora()));
        }
        None => texto_numa_linha(&pintor, &nome, forte(16.5), TEXTO, Pos2::new(x, caixa.center().y), largura),
    }

    // A estrela: sempre nos favoritos, e ao passar o mouse nos outros.
    let mut na_estrela = false;
    if favorito || resposta.hovered() {
        let centro = Pos2::new(caixa.right() - 16.0, caixa.center().y);
        let sobre = ui.rect_contains_pointer(Rect::from_center_size(centro, Vec2::splat(26.0)));
        let (icone, cor) = if favorito { (Icone::Estrela, super::AMARELO) } else { (Icone::EstrelaVazia, if sobre { TEXTO } else { SECUNDARIO }) };
        icones::pintar(&pintor, icone, centro, 20.0, cor);
        na_estrela = sobre && resposta.clicked();
    }
    let dica = if favorito { "Clique assiste · a estrela ou S tira dos favoritos" } else { "Clique assiste · a estrela, S ou o botão direito favorita" };
    (resposta.on_hover_text(dica).on_hover_cursor(egui::CursorIcon::PointingHand), na_estrela)
}

// MARK: - Guia

/// O guia da TV Box (`activity_guide.xml`): canais de um lado, a
/// programação do escolhido do outro.
fn guia(app: &mut App, ui_pai: &mut egui::Ui) {
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui_pai, |ui| {
        let area = ui.max_rect();
        let pintor = ui.painter().clone();
        let margem = 50.0;
        pintor.text(Pos2::new(area.left() + margem, area.top() + 14.0), Align2::LEFT_TOP, "Programação", forte(34.0), TEXTO);
        let resumo = format!("{} de {} canais com guia", epg::canais_com_guia(), app.canais.len());
        let fim = Rect::from_min_size(Pos2::new(area.right() - margem - 44.0, area.top() + 14.0), egui::vec2(44.0, 44.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(fim), |ui| {
            if botao_redondo(ui, Icone::Fechar, 44.0, "Fechar o guia (G ou Esc)", false).clicked() {
                app.guia_aberto = false;
            }
        });
        let galeria = pintor.layout_no_wrap(resumo, normal(16.0), SECUNDARIO);
        let x = fim.left() - 20.0 - galeria.size().x;
        pintor.galley(Pos2::new(x, fim.center().y - galeria.size().y / 2.0), galeria, SECUNDARIO);
        pintor.text(Pos2::new(x - 18.0, fim.center().y), Align2::RIGHT_CENTER, epg::hora_local(epg::agora()), forte(22.0), ACENTO);

        let corpo = Rect::from_min_max(Pos2::new(area.left() + margem - 6.0, area.top() + 80.0), Pos2::new(area.right() - margem, area.bottom() - 10.0));
        let divisa = corpo.left() + (corpo.width() * 0.42).clamp(320.0, 520.0);
        let esquerda = Rect::from_min_max(corpo.min, Pos2::new(divisa - 18.0, corpo.bottom()));
        let direita = Rect::from_min_max(Pos2::new(divisa + 18.0, corpo.top()), corpo.max);
        pintor.line_segment([Pos2::new(divisa, corpo.top() + 10.0), Pos2::new(divisa, corpo.bottom() - 10.0)], Stroke::new(1.0, branco(26)));

        // Os canais, com o foco do teclado; um clique escolhe, dois assistem.
        let itens: Vec<usize> = app.visiveis();
        let foco = app.foco;
        let tocando = app.tocando.as_ref().map(|t| t.canal);
        let mut escolhido = None;
        let mut assistir = None;
        let rolar = ui.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowUp));
        ui.scope_builder(egui::UiBuilder::new().max_rect(esquerda), |ui| {
            egui::ScrollArea::vertical().id_salt("guia-canais").auto_shrink([false, false]).show_rows(ui, ALTURA_DA_LINHA, itens.len(), |ui, faixa| {
                ui.spacing_mut().item_spacing.y = 0.0;
                for indice in &itens[faixa] {
                    let (resposta, _) = linha_do_canal(app, ui, *indice, Some(*indice) == tocando, *indice == foco);
                    if *indice == foco && rolar {
                        resposta.scroll_to_me(Some(egui::Align::Center));
                    }
                    if resposta.double_clicked() {
                        assistir = Some(*indice);
                    } else if resposta.clicked() {
                        escolhido = Some(*indice);
                    }
                }
            });
        });
        if let Some(i) = escolhido {
            app.foco = i;
        }
        if let Some(i) = assistir {
            app.foco = i;
            app.guia_aberto = false;
            app.tocar(i, 0, true);
        }

        let Some(nome) = app.canais.get(app.foco).map(|c| c.nome.clone()) else { return };
        let programas = epg::grade(&nome);
        let instante = epg::agora();
        let mut tocar = false;
        ui.scope_builder(egui::UiBuilder::new().max_rect(direita), |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&nome).font(forte(22.0)).color(TEXTO));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if botao(ui, Some(Icone::Tocar), "Assistir", true).clicked() {
                        tocar = true;
                    }
                });
            });
            ui.add_space(10.0);
            if programas.is_empty() {
                ui.add_space(20.0);
                let aviso = if epg::canais_com_guia() == 0 { "Montando o guia…" } else { "Este canal não tem programação publicada." };
                ui.label(egui::RichText::new(aviso).font(normal(16.0)).color(SECUNDARIO));
                return;
            }
            egui::ScrollArea::vertical().id_salt(("guia-programas", &nome)).auto_shrink([false, false]).show(ui, |ui| {
                for programa in programas.iter().filter(|p| p.fim > instante) {
                    let vivo = programa.no_ar(instante);
                    let altura = if vivo && !programa.descricao.is_empty() { 132.0 } else { 74.0 };
                    let (rect, resposta) = ui.allocate_exact_size(egui::vec2(ui.available_width(), altura), Sense::click());
                    let t = aceso(ui, resposta.id, resposta.hovered());
                    if vivo {
                        ui.painter().rect_filled(rect, 14.0, branco(20));
                    }
                    moldura_de_foco(ui.painter(), rect, t, 14.0);
                    let p = ui.painter();
                    p.text(Pos2::new(rect.left() + 16.0, rect.top() + 20.0), Align2::LEFT_CENTER, epg::hora_local(programa.inicio), forte(19.0), if vivo { ACENTO } else { TEXTO });
                    let x = rect.left() + 100.0;
                    let mut fim_titulo = rect.right() - 16.0;
                    if vivo {
                        let selo = selo_vivo(p, Pos2::new(rect.right() - 110.0, rect.top() + 9.0), "AO VIVO", true, 12.5);
                        fim_titulo = selo.left() - 10.0;
                    }
                    texto_numa_linha(p, &programa.titulo, if vivo { forte(18.0) } else { normal(18.0) }, TEXTO, Pos2::new(x, rect.top() + 20.0), fim_titulo - x);
                    let detalhe = [programa.categoria.as_str(), programa.horario().as_str()].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join(" · ");
                    texto_numa_linha(p, &detalhe, normal(13.5), SECUNDARIO, Pos2::new(x, rect.top() + 45.0), rect.right() - 16.0 - x);
                    if vivo {
                        andamento(p, Rect::from_min_size(Pos2::new(x, rect.top() + 60.0), egui::vec2((rect.right() - x - 16.0).min(260.0), 4.0)), programa.andamento(instante));
                        if !programa.descricao.is_empty() {
                            texto(p, &programa.descricao, normal(13.5), branco(200), Pos2::new(x, rect.top() + 72.0), rect.right() - 16.0 - x, 3, false);
                        }
                    }
                    if resposta.on_hover_text("Clique assiste o canal").clicked() {
                        tocar = true;
                    }
                    ui.add_space(4.0);
                }
            });
        });
        if tocar {
            let i = app.foco;
            app.guia_aberto = false;
            app.tocar(i, 0, true);
        }
    });
}

// MARK: - Palco

fn palco(app: &mut App, ui_pai: &mut egui::Ui) {
    egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui_pai, |ui| {
        let area = ui.max_rect();
        let janela = ui.ctx().content_rect();
        let demo = std::env::var("SAIMO_DEMO").is_ok();
        if demo && app.tocando.is_none() && app.tocando_vod.is_none() && !app.canais.is_empty() {
            app.tocando = Some(crate::Tocando { canal: 1, fonte: 0, desde: std::time::Instant::now(), confirmado: true });
        }
        if app.mpv.is_none() && !demo {
            sem_mpv(app, ui, area);
            return;
        }
        if let Some(erro) = app.mpv.as_ref().and_then(|mpv| mpv.erro_de_video()) {
            ui.painter().text(area.center() - egui::vec2(0.0, 14.0), Align2::CENTER_CENTER, "Não foi possível iniciar o vídeo", forte(22.0), TEXTO);
            ui.painter().text(area.center() + egui::vec2(0.0, 18.0), Align2::CENTER_CENTER, erro, normal(14.0), SECUNDARIO);
            return;
        }
        if app.outra_tela.is_some() {
            multi_tela(app, ui);
        }
        if app.tocando.is_none() && app.tocando_vod.is_none() {
            if app.radio.is_some() {
                radio(app, ui, area);
            } else if app.outra_tela.is_none() {
                estado_vazio(app, ui, area);
            }
            return;
        }
        // Fora da regra dos controles: o botão de pular só serve na hora.
        pulos_na_tela(app, ui, janela);
        if app.pip {
            return;
        }
        if app.tocando_vod.is_some() {
            if app.episodios_no_player {
                painel_de_episodios(app, ui, janela);
                return;
            }
            if app.controles_visiveis() {
                player(app, ui, janela);
            }
            return;
        }
        if app.controles_visiveis() {
            barra_do_ao_vivo(app, ui, Rect::from_min_max(Pos2::new(area.left(), area.top()), janela.max));
        }
    });
}

fn sem_mpv(app: &App, ui: &mut egui::Ui, area: Rect) {
    // Aberto de dentro do ZIP é o caso comum, e tem saída simples: dizer isso
    // é mais útil que anunciar que falta um arquivo que a pessoa nem conhece.
    let do_zip = crate::aberto_de_dentro_do_zip();
    let (titulo, recado) = if do_zip {
        (
            "Descompacte a pasta antes de abrir",
            "O Windows copiou só o programa para uma pasta temporária e deixou a libmpv-2.dll para trás. Clique com o \
             botão direito no arquivo SaimoTV-Windows.zip, escolha \"Extrair tudo\" e abra o Saimo TV.exe de dentro da \
             pasta extraída — ou baixe o instalador, que resolve isto de uma vez.",
        )
    } else {
        ("Falta o mpv", "O arquivo libmpv-2.dll precisa estar na mesma pasta do Saimo TV.exe.")
    };
    let pintor = ui.painter();
    pintor.text(area.center() - egui::vec2(0.0, 60.0), Align2::CENTER_CENTER, titulo, forte(24.0), TEXTO);
    texto(pintor, recado, normal(15.0), SECUNDARIO, area.center() - egui::vec2(300.0, 30.0), 600.0, 6, true);
    let pasta = crate::pasta_do_programa();
    let detalhe = if do_zip && !pasta.is_empty() { format!("abriu daqui: {pasta}") } else { app.erro_do_mpv.clone().unwrap_or_default() };
    if !detalhe.is_empty() {
        texto(pintor, &detalhe, normal(12.5), APAGADO, area.center() + egui::vec2(-300.0, 70.0), 600.0, 3, true);
    }
}

fn estado_vazio(app: &mut App, ui: &mut egui::Ui, area: Rect) {
    let centro = area.center();
    icones::pintar(ui.painter(), Icone::AoVivo, centro - egui::vec2(0.0, 90.0), 64.0, APAGADO);
    ui.painter().text(centro - egui::vec2(0.0, 30.0), Align2::CENTER_CENTER, "Escolha um canal na lista", forte(24.0), TEXTO);
    ui.painter().text(
        centro,
        Align2::CENTER_CENTER,
        "Setas andam · Enter assiste · números vão direto ao canal · H mostra todos os atalhos",
        normal(15.0),
        SECUNDARIO,
    );
    let botoes = Rect::from_center_size(centro + egui::vec2(0.0, 56.0), egui::vec2(470.0, 48.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(botoes).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        if botao(ui, Some(Icone::Lista), "Canais", true).on_hover_text("Mostrar a lista (L)").clicked() {
            app.lista_aberta = true;
        }
        ui.add_space(8.0);
        if botao(ui, Some(Icone::Guia), "Guia", false).on_hover_text("Programação completa (G)").clicked() {
            app.guia_aberto = true;
        }
        ui.add_space(8.0);
        if botao(ui, Some(Icone::Filmes), "On Demand", false).clicked() {
            app.abrir_secao(Aba::OnDemand);
        }
    });
}

/// A rádio tocando sem vídeo (`painel_radio.xml`): a logo grande sobre ela
/// mesma desfocada, o nome, o estilo e o equalizador.
fn radio(app: &mut App, ui: &mut egui::Ui, area: Rect) {
    let Some((nome, logo, confirmado)) = app.radio.as_ref().map(|r| (r.nome.clone(), r.logo.clone(), r.confirmado)) else { return };
    let janela = ui.ctx().content_rect();
    let textura = logo.and_then(|u| app.imagem(&u, 256));
    let pintor = ui.painter().clone();
    if let Some(t) = textura.as_ref() {
        // Sem desfoque no egui: a logo enorme e bem apagada faz a mancha de cor.
        pintar_cobrindo(ui, t, janela, 0.0, 0.5, 0.12);
        pintor.rect_filled(janela, 0.0, fundo_com(150));
    }
    let lado = 220.0;
    let quadro = Rect::from_center_size(area.center() - egui::vec2(0.0, 60.0), Vec2::splat(lado));
    pintor.rect_filled(quadro, 36.0, branco(31));
    pintor.rect_stroke(quadro, 36.0, Stroke::new(1.0, branco(51)), egui::StrokeKind::Inside);
    match textura {
        Some(t) => pintar_contido(ui, &t, quadro.shrink(28.0)),
        None => {
            pintor.text(quadro.center(), Align2::CENTER_CENTER, inicial(&nome), forte(90.0), ACENTO);
        }
    }
    let pulso = (0.625 + 0.375 * (ui.input(|i| i.time) as f32 * std::f32::consts::TAU / 1.8).cos()).clamp(0.0, 1.0);
    let y = quadro.bottom() + 34.0;
    pintor.circle_filled(Pos2::new(area.center().x - 70.0, y), 6.0, VIVO.gamma_multiply(pulso));
    pintor.text(Pos2::new(area.center().x - 56.0, y), Align2::LEFT_CENTER, if confirmado { "NO AR" } else { "SINTONIZANDO" }, forte(14.0), TEXTO);
    texto(&pintor, &nome, forte(32.0), TEXTO, Pos2::new(area.center().x - 300.0, y + 22.0), 600.0, 1, true);
    let estilo = crate::radios::estilo_de(&nome).unwrap_or("Rádio ao vivo");
    pintor.text(Pos2::new(area.center().x, y + 84.0), Align2::CENTER_CENTER, estilo, normal(17.0), SECUNDARIO);
    topo::equalizador(ui, Rect::from_center_size(Pos2::new(area.center().x, y + 128.0), egui::vec2(90.0, 40.0)), 9);
    let botoes = Rect::from_center_size(Pos2::new(area.center().x, y + 190.0), egui::vec2(300.0, 48.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(botoes).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        if botao(ui, Some(Icone::Parar), "Parar", true).clicked() {
            app.parar_radio();
        }
        ui.add_space(8.0);
        if botao(ui, Some(Icone::Radio), "Outras rádios", false).clicked() {
            app.abrir_secao(Aba::Radios);
        }
    });
}

/// O que a multi tela põe sobre os vídeos: a moldura da tela em foco, o nome
/// do canal de cada uma, e o botão de fechar a segunda.
fn multi_tela(app: &mut App, ui: &mut egui::Ui) {
    let janela = ui.ctx().content_rect();
    let (em_foco, fora) = super::telas_da_multi_tela(janela, app.foco_a_direita);
    let nome_de = |tocando: Option<&crate::Tocando>| tocando.and_then(|t| app.canais.get(t.canal)).map(|c| c.nome.clone());
    let nome_em_foco = nome_de(app.tocando.as_ref()).unwrap_or_else(|| "Escolha um canal na lista".into());
    let nome_fora = nome_de(app.outra_tela.as_ref().and_then(|o| o.tocando.as_ref())).unwrap_or_default();
    let pintor = ui.painter().clone();
    pintor.rect_stroke(em_foco.shrink(1.5), 0.0, Stroke::new(3.0, ACENTO), egui::StrokeKind::Inside);
    let etiqueta = |parte: Rect, conteudo: &str, em_foco: bool| {
        if conteudo.is_empty() {
            return;
        }
        let galeria = pintor.layout_no_wrap(conteudo.to_string(), forte(15.0), TEXTO);
        let extra = if em_foco { 26.0 } else { 0.0 };
        let caixa = Rect::from_min_size(
            Pos2::new(parte.left() + 16.0, parte.bottom() - 16.0 - galeria.size().y - 16.0),
            galeria.size() + egui::vec2(28.0 + extra, 16.0),
        );
        pintor.rect_filled(caixa, caixa.height() / 2.0, VIDRO);
        let mut x = caixa.left() + 14.0;
        if em_foco {
            icones::pintar(&pintor, Icone::Som, Pos2::new(x + 9.0, caixa.center().y), 18.0, ACENTO);
            x += extra;
        }
        pintor.galley(Pos2::new(x, caixa.top() + 8.0), galeria, TEXTO);
    };
    etiqueta(em_foco, &nome_em_foco, true);
    etiqueta(fora, &nome_fora, false);
    let clique = ui.interact(fora, egui::Id::new("multi-tela-outra"), Sense::click());
    if clique.on_hover_text("Clique para ouvir e controlar esta tela (T)").clicked() {
        app.trocar_tela_em_foco();
    }
    let lugar = Rect::from_min_size(Pos2::new(janela.right() - 260.0, janela.top() + topo::ALTURA + 12.0), egui::vec2(240.0, 44.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(lugar).layout(egui::Layout::right_to_left(egui::Align::Center)), |ui| {
        if botao(ui, Some(Icone::Fechar), "Fechar segunda tela", false).on_hover_text("Volta a uma tela só, com a que está em foco (D)").clicked() {
            app.sair_da_multi_tela();
        }
    });
}

/// "Pular abertura" e o cartão do próximo episódio, no canto de baixo.
fn pulos_na_tela(app: &mut App, ui: &mut egui::Ui, janela: Rect) {
    let acima = if app.controles_visiveis() && app.tocando_vod.is_some() { 190.0 } else { 48.0 };
    let base = Pos2::new(janela.right() - 48.0, janela.bottom() - acima);
    if let Some(trecho) = app.pulo_na_tela() {
        let caixa = Rect::from_min_max(base - egui::vec2(300.0, 52.0), base);
        ui.scope_builder(egui::UiBuilder::new().max_rect(caixa).layout(egui::Layout::right_to_left(egui::Align::Max)), |ui| {
            if botao(ui, Some(Icone::Proximo), &format!("{} · Enter", trecho.tipo.rotulo()), true).clicked() {
                app.pular_trecho();
            }
        });
        ui.ctx().request_repaint_after(Duration::from_millis(250));
        return;
    }
    let Some((seguinte, resta)) = app.proximo_na_tela() else { return };
    let serie = app.tocando_vod.as_ref().and_then(|t| t.episodio.as_ref()).map(|e| e.serie.clone()).unwrap_or_default();
    let info = app
        .pedir_temporada(&serie, seguinte.temporada)
        .and_then(|l| l.into_iter().find(|e| e.numero == seguinte.numero));
    let caixa = Rect::from_min_max(base - egui::vec2(540.0, 150.0), base);
    let pintor = ui.painter().clone();
    pintor.rect_filled(caixa, 24.0, FUNDO.gamma_multiply(0.96));
    pintor.rect_stroke(caixa, 24.0, Stroke::new(1.0, branco(20)), egui::StrokeKind::Inside);
    let imagem = Rect::from_min_size(caixa.min + egui::vec2(18.0, 18.0), egui::vec2(176.0, 99.0));
    pintor.rect_filled(imagem, 12.0, branco(20));
    if let Some(t) = info.as_ref().and_then(|e| e.imagem.clone()).and_then(|u| app.imagem(&u, 342)) {
        pintar_cobrindo(ui, &t, imagem, 12.0, 0.5, 1.0);
    } else {
        icones::pintar(&pintor, Icone::Proximo, imagem.center(), 36.0, branco(120));
    }
    let x = imagem.right() + 16.0;
    let largura = caixa.right() - 18.0 - x;
    pintor.text(Pos2::new(x, caixa.top() + 18.0), Align2::LEFT_TOP, format!("Próximo episódio em {resta} s"), normal(14.0), ACENTO);
    let nome = info.as_ref().map(|e| e.nome.clone()).filter(|n| !n.is_empty());
    let titulo = match nome {
        Some(n) => format!("T{} E{} · {n}", seguinte.temporada, seguinte.numero),
        None => format!("Temporada {} · Episódio {}", seguinte.temporada, seguinte.numero),
    };
    texto(&pintor, &titulo, forte(18.0), TEXTO, Pos2::new(x, caixa.top() + 40.0), largura, 2, false);
    let botoes = Rect::from_min_max(Pos2::new(caixa.left() + 18.0, caixa.bottom() - 18.0 - 44.0), caixa.max - egui::vec2(18.0, 18.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(botoes).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        if botao(ui, Some(Icone::Tocar), "Assistir agora · Enter", true).clicked() {
            app.tocar_proximo();
        }
        ui.add_space(8.0);
        if botao(ui, None, "Continuar vendo · Esc", false).clicked() {
            app.dispensar_proximo();
        }
    });
    ui.ctx().request_repaint_after(Duration::from_millis(250));
}

// MARK: - Barra do ao vivo

fn barra_do_ao_vivo(app: &mut App, ui: &mut egui::Ui, area: Rect) {
    let Some((indice, nome, logo)) = app.tocando.as_ref().and_then(|t| app.canais.get(t.canal).map(|c| (t.canal, c.nome.clone(), c.logo.clone()))) else { return };
    let janela = ui.ctx().content_rect();
    let esquerda = if app.lista_aberta { area.left() + 20.0 } else { janela.left() + 40.0 };
    let altura = 236.0;
    let base = Rect::from_min_max(Pos2::new(area.left(), janela.bottom() - altura), janela.max);
    degrade3(ui.painter(), base, Color32::TRANSPARENT, Color32::from_black_alpha(179), 0.45, Color32::from_black_alpha(242), false);

    let pintor = ui.painter().clone();
    let lado = 72.0;
    let topo_bloco = janela.bottom() - altura + 40.0;
    let ladrilho = Rect::from_min_size(Pos2::new(esquerda, topo_bloco), Vec2::splat(lado));
    pintor.rect_filled(ladrilho, 16.0, branco(31));
    match logo.and_then(|u| app.imagem(&u, 128)) {
        Some(t) => pintar_contido(ui, &t, ladrilho.shrink(8.0)),
        None => {
            pintor.text(ladrilho.center(), Align2::CENTER_CENTER, inicial(&nome), forte(28.0), ACENTO);
        }
    }
    let x = ladrilho.right() + 18.0;
    let direita = janela.right() - 40.0;
    // Os selos à direita: resolução, a fonte e o "AO VIVO".
    let mut fim = direita;
    let largura_do_selo = pintor.layout_no_wrap("AO VIVO".into(), forte(13.0), TEXTO).size().x + 13.0 * 1.4;
    let vivo = selo_vivo(&pintor, Pos2::new(direita - largura_do_selo, topo_bloco + 2.0), "AO VIVO", true, 13.0);
    fim = fim.min(vivo.left() - 8.0);
    if let Some(info) = info_da_fonte(app) {
        let galeria = pintor.layout_no_wrap(info.clone(), forte(13.0), TEXTO);
        let caixa = Rect::from_min_size(Pos2::new(fim - galeria.size().x - 24.0, vivo.top()), egui::vec2(galeria.size().x + 24.0, vivo.height()));
        pintor.rect_filled(caixa, caixa.height() / 2.0, branco(38));
        pintor.galley(caixa.center() - galeria.size() / 2.0, galeria, TEXTO);
        fim = caixa.left() - 8.0;
    }
    pintor.text(Pos2::new(x, topo_bloco + 14.0), Align2::LEFT_CENTER, (indice + 1).to_string(), forte(24.0), ACENTO);
    let x_nome = x + 14.0 + 16.0 * (indice + 1).to_string().len() as f32;
    texto_numa_linha(&pintor, &nome, forte(26.0), TEXTO, Pos2::new(x_nome, topo_bloco + 14.0), fim - x_nome - 10.0);
    let instante = epg::agora();
    match epg::agora_e_depois(&nome) {
        Some((atual, depois)) => {
            let faltam = (atual.fim - instante).max(0) / 60;
            texto_numa_linha(&pintor, &atual.titulo, forte(18.0), TEXTO, Pos2::new(x, topo_bloco + 46.0), direita - x);
            let mut linha = format!("{} · faltam {faltam} min", atual.horario());
            if let Some(p) = depois {
                linha.push_str(&format!("   ·   A seguir: {} ({})", p.titulo, epg::hora_local(p.inicio)));
            }
            texto_numa_linha(&pintor, &linha, normal(14.5), SECUNDARIO, Pos2::new(x, topo_bloco + 70.0), direita - x);
            andamento(&pintor, Rect::from_min_size(Pos2::new(x, topo_bloco + 86.0), egui::vec2((direita - x).min(520.0), 4.0)), atual.andamento(instante));
        }
        None => {
            pintor.text(Pos2::new(x, topo_bloco + 50.0), Align2::LEFT_CENTER, "Sem programação publicada", normal(15.0), SECUNDARIO);
        }
    }

    let fila = Rect::from_min_max(Pos2::new(esquerda, janela.bottom() - 66.0), Pos2::new(direita, janela.bottom() - 18.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(fila).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if botao_redondo(ui, Icone::Anterior, 46.0, "Canal anterior (Page Up)", false).clicked() {
            app.pular_canal(-1);
        }
        if botao_redondo(ui, Icone::Proximo, 46.0, "Próximo canal (Page Down)", false).clicked() {
            app.pular_canal(1);
        }
        if botao_redondo(ui, Icone::Lista, 46.0, "Mostrar ou esconder a lista (L)", app.lista_aberta).clicked() {
            app.lista_aberta = !app.lista_aberta;
        }
        if botao_redondo(ui, Icone::Guia, 46.0, "Guia de programação (G)", false).clicked() {
            app.guia_aberto = true;
        }
        let dica = if app.outra_tela.is_some() { "Fechar a segunda tela (D)" } else { "Multi tela: dois canais lado a lado (D)" };
        if botao_redondo(ui, Icone::MultiTela, 46.0, dica, app.outra_tela.is_some()).clicked() {
            if app.outra_tela.is_some() {
                app.sair_da_multi_tela();
            } else {
                app.entrar_na_multi_tela();
            }
        }
        menu_de_fontes(app, ui);
        menu_de_faixas(app, ui);
        comuns(app, ui);
    });
}

/// Resolução e fonte: "1080p · fonte 2/3 · cdn.exemplo.com".
fn info_da_fonte(app: &App) -> Option<String> {
    let (atual, total, url, pronto, nome) = if let Some(t) = app.tocando.as_ref() {
        let canal = app.canais.get(t.canal)?;
        let fonte = canal.fontes.get(t.fonte);
        (t.fonte, canal.fontes.len(), fonte.map(|f| f.url.clone()).unwrap_or_default(), t.confirmado, fonte.and_then(|f| f.nome.clone()))
    } else {
        let f = app.tocando_vod.as_ref()?;
        (f.fonte, f.urls.len(), f.urls.get(f.fonte).cloned().unwrap_or_default(), f.confirmado, None)
    };
    let mut partes = Vec::new();
    if let Some(altura) = app.mpv.as_ref().and_then(|m| m.ler("video-params/h")) {
        partes.push(format!("{altura}p"));
    }
    // Canal 24h: a temporada no ar no lugar do número da fonte.
    let qual = nome.unwrap_or_else(|| format!("fonte {}/{}", atual + 1, total));
    partes.push(if pronto { qual } else { format!("carregando {qual}") });
    let host = super::servidor(&url);
    if !host.is_empty() {
        partes.push(host);
    }
    Some(partes.join(" · "))
}

/// Os botões que o ao vivo e o player dividem: preencher, som, tela cheia, janelinha.
fn comuns(app: &mut App, ui: &mut egui::Ui) {
    let dica = if app.preencher { "Ajustar à tela" } else { "Preencher a tela (corta as bordas)" };
    if botao_redondo(ui, Icone::Preencher, 46.0, dica, app.preencher).clicked() {
        app.alternar_preencher();
    }
    let icone = if app.mudo || app.volume == 0 { Icone::Mudo } else { Icone::Som };
    if botao_redondo(ui, icone, 46.0, "Sem som (M) · volume com + e −", app.mudo).clicked() {
        app.alternar_mudo();
    }
    if ui.available_width() > 230.0 {
        let mut volume = app.volume as f32;
        let deslizou = ui.add(egui::Slider::new(&mut volume, 0.0..=130.0).show_value(false));
        if deslizou.changed() {
            app.definir_volume(volume as i32);
        }
        deslizou.on_hover_text(format!("Volume {}%", app.volume));
    }
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if botao_redondo(ui, Icone::TelaCheia, 46.0, "Tela cheia (F)", false).clicked() {
            app.alternar_tela_cheia(ui.ctx());
        }
        if botao_redondo(ui, Icone::Janela, 46.0, "Janelinha por cima de tudo (P)", app.pip).clicked() {
            app.alternar_janelinha(ui.ctx());
        }
        if botao_redondo(ui, Icone::Teclado, 46.0, "Atalhos do teclado (H)", false).clicked() {
            app.ajuda_aberta = true;
        }
    });
}

// MARK: - Player de filmes e séries

fn player(app: &mut App, ui: &mut egui::Ui, janela: Rect) {
    let Some(filme) = app.tocando_vod.as_ref() else { return };
    let titulo = filme.titulo.clone();
    let episodio = filme.episodio.clone();
    let (nome, detalhe) = match titulo.split_once(" · ") {
        Some((serie, ep)) => (serie.to_string(), ep.to_string()),
        None => (titulo.clone(), String::new()),
    };
    let serie = episodio.is_some();
    let pintor = ui.painter().clone();

    // Em cima: a sombra, o voltar, o episódio, a resolução e o relógio.
    let cima = Rect::from_min_size(janela.min, egui::vec2(janela.width(), 150.0));
    degrade3(&pintor, cima, Color32::from_black_alpha(242), Color32::from_black_alpha(166), 0.5, Color32::TRANSPARENT, false);
    let linha = Rect::from_min_size(janela.min + egui::vec2(40.0, 26.0), egui::vec2(janela.width() - 80.0, 48.0));
    let mut voltar = false;
    ui.scope_builder(egui::UiBuilder::new().max_rect(linha).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        if botao_redondo(ui, Icone::Voltar, 46.0, if serie { "Voltar à série" } else { "Voltar ao acervo" }, false).clicked() {
            voltar = true;
        }
        ui.add_space(10.0);
        ui.vertical(|ui| {
            ui.label(egui::RichText::new(crate::generos::sem_ano(&nome)).font(forte(24.0)).color(TEXTO));
            if !detalhe.is_empty() {
                let info = episodio.as_ref().and_then(|e| app_info(app, e));
                let sub = match info {
                    Some(n) => format!("{detalhe} · {n}"),
                    None => detalhe.clone(),
                };
                ui.label(egui::RichText::new(sub).font(normal(16.0)).color(SECUNDARIO));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(epg::hora_local(epg::agora())).font(forte(22.0)).color(TEXTO));
            ui.add_space(12.0);
            if let Some(info) = info_da_fonte(app) {
                super::selo(ui, Some(Icone::Hd), &info, TEXTO);
            }
        });
    });

    // Embaixo: a logo do título, a linha do tempo e os botões.
    let baixo = Rect::from_min_max(Pos2::new(janela.left(), janela.bottom() - 250.0), janela.max);
    degrade3(&pintor, baixo, Color32::TRANSPARENT, Color32::from_black_alpha(179), 0.4, Color32::from_black_alpha(242), false);
    let esquerda = janela.left() + 40.0;
    let direita = janela.right() - 40.0;
    let logo_url = crate::ficha::logo(&nome, serie);
    let logo = logo_url.and_then(|u| app.imagem(&u, 342));
    let caixa_logo = Rect::from_min_size(Pos2::new(esquerda, janela.bottom() - 196.0), egui::vec2(260.0, 64.0));
    match logo {
        Some(t) => {
            let tamanho = t.size_vec2();
            let escala = (caixa_logo.width() / tamanho.x).min(caixa_logo.height() / tamanho.y);
            let alvo = Rect::from_min_size(Pos2::new(caixa_logo.left(), caixa_logo.bottom() - tamanho.y * escala), tamanho * escala);
            egui::Image::new(&t).fit_to_exact_size(alvo.size()).paint_at(ui, alvo);
        }
        None => {
            texto(&pintor, &crate::generos::sem_ano(&nome), forte(30.0), TEXTO, Pos2::new(caixa_logo.left(), caixa_logo.bottom() - 38.0), 600.0, 1, false);
        }
    }

    let (posicao, duracao) = app.mpv.as_ref().and_then(|m| m.posicao()).unwrap_or((0.0, 0.0));
    let tempo = Rect::from_min_max(Pos2::new(esquerda, janela.bottom() - 120.0), Pos2::new(direita, janela.bottom() - 84.0));
    linha_do_tempo(app, ui, tempo, posicao, duracao);

    let fila = Rect::from_min_max(Pos2::new(esquerda, janela.bottom() - 72.0), Pos2::new(direita, janela.bottom() - 20.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(fila).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (icone, dica) = if app.pausado { (Icone::Tocar, "Tocar (Espaço)") } else { (Icone::Pausar, "Pausar (Espaço)") };
        if botao_redondo(ui, icone, 52.0, dica, false).clicked() {
            app.alternar_pausa();
        }
        if botao_redondo(ui, Icone::Voltar10, 46.0, "Voltar 10 s (seta esquerda)", false).clicked() {
            app.avancar(-10.0);
        }
        if botao_redondo(ui, Icone::Avancar10, 46.0, "Avançar 10 s (seta direita)", false).clicked() {
            app.avancar(10.0);
        }
        if let Some(ep) = episodio.as_ref() {
            if ep.seguinte().is_some() && pilula(ui, Some(Icone::Proximo), "Próximo", false, 46.0).on_hover_text("Próximo episódio").clicked() {
                app.tocar_proximo();
            }
            if pilula(ui, Some(Icone::Episodios), "Episódios", false, 46.0).on_hover_text("Os episódios da série (E)").clicked() {
                app.episodios_no_player = true;
            }
        }
        menu_de_faixas(app, ui);
        menu_de_fontes(app, ui);
        menu_de_velocidade(app, ui);
        comuns(app, ui);
    });
    if voltar {
        app.voltar_do_player();
    }
}

/// O nome do episódio que está tocando, pelo TMDB.
fn app_info(app: &mut App, ep: &crate::EpisodioTocando) -> Option<String> {
    let lista = app.pedir_temporada(&ep.serie, ep.temporada)?;
    lista.into_iter().find(|e| e.numero == ep.numero).map(|e| e.nome).filter(|n| !n.is_empty())
}

/// A linha do tempo da TV Box (`barra_tempo`): o tempo de cada lado, a
/// barra que engrossa no mouse e mostra para onde vai, e o clique que pula.
fn linha_do_tempo(app: &mut App, ui: &mut egui::Ui, rect: Rect, posicao: f64, duracao: f64) {
    let pintor = ui.painter().clone();
    pintor.text(Pos2::new(rect.left(), rect.center().y), Align2::LEFT_CENTER, relogio(posicao), forte(16.0), TEXTO);
    pintor.text(Pos2::new(rect.right(), rect.center().y), Align2::RIGHT_CENTER, relogio(duracao), forte(16.0), SECUNDARIO);
    let barra = Rect::from_min_max(Pos2::new(rect.left() + 80.0, rect.top()), Pos2::new(rect.right() - 80.0, rect.bottom()));
    let resposta = ui.interact(barra, egui::Id::new("linha-do-tempo"), Sense::click_and_drag());
    let t = aceso(ui, resposta.id, resposta.hovered() || resposta.dragged());
    let espessura = 6.0 + 4.0 * t;
    let trilho = Rect::from_center_size(barra.center(), egui::vec2(barra.width(), espessura));
    pintor.rect_filled(trilho, espessura / 2.0, branco(51));
    if duracao <= 0.0 {
        return;
    }
    // O que o mpv já tem guardado à frente, em branco translúcido.
    let guardado = app.mpv.as_ref().and_then(|m| m.ler("demuxer-cache-time")).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let fracao_guardada = (guardado / duracao).clamp(0.0, 1.0) as f32;
    pintor.rect_filled(Rect::from_min_size(trilho.min, egui::vec2(trilho.width() * fracao_guardada, espessura)), espessura / 2.0, branco(102));
    let mut fracao = (posicao / duracao).clamp(0.0, 1.0) as f32;
    let ponteiro = resposta.interact_pointer_pos().or_else(|| resposta.hover_pos());
    if resposta.dragged() {
        if let Some(p) = ponteiro {
            fracao = ((p.x - trilho.left()) / trilho.width()).clamp(0.0, 1.0);
        }
    }
    pintor.rect_filled(Rect::from_min_size(trilho.min, egui::vec2(trilho.width() * fracao, espessura)), espessura / 2.0, ACENTO);
    pintor.circle_filled(Pos2::new(trilho.left() + trilho.width() * fracao, trilho.center().y), 7.0 + 3.0 * t, TEXTO);
    if let Some(p) = resposta.hover_pos() {
        let alvo = ((p.x - trilho.left()) / trilho.width()).clamp(0.0, 1.0) as f64 * duracao;
        let bolha = pintor.layout_no_wrap(relogio(alvo), forte(15.0), TEXTO);
        let caixa = Rect::from_center_size(Pos2::new(p.x, trilho.top() - 24.0), bolha.size() + egui::vec2(20.0, 10.0));
        pintor.rect_filled(caixa, 12.0, VIDRO);
        pintor.galley(caixa.center() - bolha.size() / 2.0, bolha, TEXTO);
    }
    if resposta.clicked() || resposta.drag_stopped() {
        if let Some(p) = ponteiro {
            let alvo = ((p.x - trilho.left()) / trilho.width()).clamp(0.0, 1.0) as f64 * duracao;
            app.ir_para(alvo);
            app.mostrar_controles();
        }
    }
    if resposta.dragged() {
        app.mostrar_controles();
    }
}

fn relogio(segundos: f64) -> String {
    super::relogio(segundos)
}

/// O painel de episódios do player (`painel_episodios.xml`): as temporadas em
/// pílulas e os episódios em cartões com a imagem da cena, por cima do vídeo.
fn painel_de_episodios(app: &mut App, ui: &mut egui::Ui, janela: Rect) {
    let Some(ep) = app.tocando_vod.as_ref().and_then(|t| t.episodio.clone()) else {
        app.episodios_no_player = false;
        return;
    };
    let altura = 400.0_f32.min(janela.height() * 0.7);
    let painel = Rect::from_min_max(Pos2::new(janela.left(), janela.bottom() - altura), janela.max);
    degrade3(ui.painter(), painel, fundo_com(0), fundo_com(230), 0.25, fundo_com(250), false);
    let mut temporadas: Vec<u32> = ep.lista.iter().map(|e| e.temporada).collect();
    temporadas.sort_unstable();
    temporadas.dedup();
    let id_temporada = egui::Id::new("temporada-no-player");
    let temporada = ui.ctx().data(|d| d.get_temp::<u32>(id_temporada)).filter(|t| temporadas.contains(t)).unwrap_or(ep.temporada);
    let tmdb = app.pedir_temporada(&ep.serie, temporada).unwrap_or_default();

    let topo = Rect::from_min_size(painel.min + egui::vec2(40.0, 60.0), egui::vec2(painel.width() - 80.0, 44.0));
    let mut fechar = false;
    let mut nova = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(topo).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        ui.label(egui::RichText::new("Episódios").font(forte(24.0)).color(TEXTO));
        ui.add_space(16.0);
        for t in &temporadas {
            if pilula(ui, None, &format!("Temporada {t}"), *t == temporada, 38.0).clicked() {
                nova = Some(*t);
            }
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if botao_redondo(ui, Icone::Fechar, 42.0, "Fechar (Esc)", false).clicked() {
                fechar = true;
            }
        });
    });
    if let Some(t) = nova {
        ui.ctx().data_mut(|d| d.insert_temp(id_temporada, t));
    }

    let mut numeros: Vec<&crate::vod::Episodio> = ep.lista.iter().filter(|e| e.temporada == temporada).collect();
    numeros.sort_by_key(|e| e.numero);
    numeros.dedup_by_key(|e| e.numero);
    let faixa = Rect::from_min_max(Pos2::new(painel.left(), topo.bottom() + 14.0), Pos2::new(painel.right(), painel.bottom() - 16.0));
    let mut tocar: Option<crate::vod::Episodio> = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(faixa), |ui| {
        egui::ScrollArea::horizontal().id_salt(("episodios-no-player", temporada)).show(ui, |ui| {
            ui.horizontal_top(|ui| {
                ui.add_space(34.0);
                ui.spacing_mut().item_spacing.x = 10.0;
                for e in numeros {
                    let largura = 260.0;
                    let (rect, resposta) = ui.allocate_exact_size(egui::vec2(largura + 12.0, largura * 9.0 / 16.0 + 70.0), Sense::click());
                    let atual = e.temporada == ep.temporada && e.numero == ep.numero;
                    let t = aceso(ui, resposta.id, resposta.hovered());
                    let pintor = ui.painter().clone();
                    moldura_de_foco(&pintor, rect, t.max(if atual { 0.6 } else { 0.0 }), 14.0);
                    let quadro = Rect::from_min_size(rect.min + egui::vec2(6.0, 6.0), egui::vec2(largura, largura * 9.0 / 16.0));
                    pintor.rect_filled(quadro, 12.0, branco(20));
                    let info = tmdb.iter().find(|x| x.numero == e.numero);
                    match info.and_then(|x| x.imagem.clone()).and_then(|u| app.imagem(&u, 342)) {
                        Some(textura) => pintar_cobrindo(ui, &textura, quadro, 12.0, 0.5, 1.0),
                        None => icones::pintar(&pintor, Icone::Tocar, quadro.center(), 36.0, branco(90)),
                    }
                    if atual {
                        selo_vivo(&pintor, quadro.min + egui::vec2(10.0, 10.0), "ASSISTINDO", false, 11.5);
                    }
                    let chave = format!("{} · T{} E{}", ep.serie, e.temporada, e.numero);
                    if let Some(m) = progresso::onde_parou(&chave) {
                        andamento(&pintor, Rect::from_min_size(Pos2::new(quadro.left() + 10.0, quadro.bottom() - 12.0), egui::vec2(quadro.width() - 20.0, 4.0)), progresso::fracao(&m));
                    }
                    let nome = info.map(|x| x.nome.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Episódio {}", e.numero));
                    texto(&pintor, &format!("{}. {nome}", e.numero), forte(15.0), TEXTO, Pos2::new(quadro.left(), quadro.bottom() + 8.0), largura, 1, false);
                    if let Some(d) = info.and_then(|x| x.duracao) {
                        pintor.text(Pos2::new(quadro.left(), quadro.bottom() + 32.0), Align2::LEFT_TOP, format!("{d} min"), normal(13.0), SECUNDARIO);
                    }
                    if resposta.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !atual {
                        tocar = Some(e.clone());
                    }
                }
                ui.add_space(34.0);
            });
        });
    });
    if fechar {
        app.episodios_no_player = false;
    }
    if let Some(e) = tocar {
        // A mesma versão que estava tocando, quando o episódio tem.
        let escolhido = ep
            .lista
            .iter()
            .find(|x| x.temporada == e.temporada && x.numero == e.numero && x.versao == ep.versao)
            .cloned()
            .unwrap_or(e);
        app.trocar_de_episodio(escolhido);
    }
}

// MARK: - Menus

fn menu_de_faixas(app: &mut App, ui: &mut egui::Ui) {
    let Some(mpv) = app.mpv.clone() else { return };
    let videos = mpv.faixas("video");
    let audios = mpv.faixas("audio");
    let legendas = mpv.faixas("sub");
    // Legendas do OpenSubtitles do filme ou episódio que está tocando.
    let externas: Vec<crate::legendas::Opcao> = app.tocando_vod.as_ref().map(|t| t.legendas.clone()).unwrap_or_default();
    if videos.is_empty() && audios.is_empty() && legendas.is_empty() && externas.is_empty() {
        return;
    }
    let resposta = botao_redondo(ui, Icone::Legendas, 46.0, "Qualidade, idioma do áudio e legendas", false);
    egui::Popup::from_toggle_button_response(&resposta)
        .align(egui::RectAlign::TOP_START)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(240.0);
            let titulo = |ui: &mut egui::Ui, t: &str| {
                ui.label(egui::RichText::new(t.to_uppercase()).font(forte(12.0)).color(ACENTO).extra_letter_spacing(1.0));
            };
            titulo(ui, "Qualidade");
            if ui.selectable_label(!videos.iter().any(|f| f.selecionada), "Automática").clicked() {
                mpv.escolher_faixa("video", None);
                ui.close();
            }
            for faixa in &videos {
                if ui.selectable_label(faixa.selecionada, &faixa.titulo).clicked() {
                    mpv.escolher_faixa("video", Some(&faixa.id));
                    ui.close();
                }
            }
            if !audios.is_empty() {
                ui.separator();
                titulo(ui, "Idioma do áudio");
                for faixa in &audios {
                    if ui.selectable_label(faixa.selecionada, &faixa.titulo).clicked() {
                        mpv.escolher_faixa("audio", Some(&faixa.id));
                        ui.close();
                    }
                }
            }
            if !legendas.is_empty() {
                ui.separator();
                titulo(ui, "Legendas");
                let ligada = legendas.iter().any(|f| f.selecionada);
                if ui.selectable_label(!ligada, "Desligadas").clicked() {
                    mpv.escolher_faixa("sub", None);
                    ui.close();
                }
                for faixa in &legendas {
                    if ui.selectable_label(faixa.selecionada, &faixa.titulo).clicked() {
                        mpv.escolher_faixa("sub", Some(&faixa.id));
                        ui.close();
                    }
                }
            }
            if !externas.is_empty() {
                ui.separator();
                titulo(ui, "Legendas · OpenSubtitles");
                let (escolhida, atraso, aviso) = app
                    .tocando_vod
                    .as_ref()
                    .map(|t| (t.legenda_escolhida.as_ref().map(|(o, _)| o.id.clone()), t.legenda_atraso, t.legenda_aviso.clone()))
                    .unwrap_or((None, 0.0, None));
                if ui.selectable_label(escolhida.is_none() && !legendas.iter().any(|f| f.selecionada), "Desligadas").clicked() {
                    app.desligar_legendas();
                    ui.close();
                }
                egui::ScrollArea::vertical().max_height(220.0).show(ui, |ui| {
                    for opcao in &externas {
                        if ui.selectable_label(escolhida.as_deref() == Some(opcao.id.as_str()), &opcao.rotulo).clicked() {
                            app.escolher_legenda(opcao.clone());
                            ui.close();
                        }
                    }
                });
                if escolhida.is_some() {
                    ui.horizontal(|ui| {
                        if ui.button("−0,5 s").on_hover_text("Adiantar a legenda").clicked() {
                            app.ajustar_atraso_legenda(-0.5);
                        }
                        let rotulo = if atraso == 0.0 { "Sincronia original".to_string() } else { format!("{atraso:+.1} s").replace('.', ",") };
                        ui.label(egui::RichText::new(rotulo).font(forte(12.0)).color(SECUNDARIO));
                        if ui.button("+0,5 s").on_hover_text("Atrasar a legenda").clicked() {
                            app.ajustar_atraso_legenda(0.5);
                        }
                    });
                }
                if let Some(aviso) = aviso {
                    ui.label(egui::RichText::new(aviso).font(forte(12.0)).color(SECUNDARIO));
                }
            }
        });
}

fn menu_de_fontes(app: &mut App, ui: &mut egui::Ui) {
    let total = if let Some(t) = app.tocando.as_ref() {
        app.canais.get(t.canal).map(|c| c.fontes.len()).unwrap_or(0)
    } else {
        app.tocando_vod.as_ref().map(|f| f.urls.len()).unwrap_or(0)
    };
    if total < 2 {
        return;
    }
    let atual = app.tocando.as_ref().map(|t| t.fonte).or_else(|| app.tocando_vod.as_ref().map(|f| f.fonte)).unwrap_or(0);
    let resposta = botao_redondo(ui, Icone::Trocar, 46.0, "Escolher a fonte (seta direita passa para a próxima)", false);
    egui::Popup::from_toggle_button_response(&resposta)
        .align(egui::RectAlign::TOP_START)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            ui.set_min_width(220.0);
            ui.label(egui::RichText::new("FONTES").font(forte(12.0)).color(ACENTO).extra_letter_spacing(1.0));
            for i in 0..total {
                // Como na TV Box: o nome da fonte (a temporada, nos canais 24h)
                // ou o número, a qualidade quando informada e o servidor.
                let fonte = app.tocando.as_ref().and_then(|t| app.canais.get(t.canal)).and_then(|c| c.fontes.get(i));
                let nome = fonte.and_then(|f| f.nome.clone()).unwrap_or_else(|| format!("Fonte {}", i + 1));
                let mut partes = vec![nome];
                if let Some(q) = fonte.and_then(|f| f.qualidade.as_deref()).filter(|q| *q != "Qualidade não informada") {
                    partes.push(q.to_string());
                }
                if let Some(f) = fonte {
                    partes.push(super::servidor(&f.url));
                }
                if ui.selectable_label(i == atual, partes.join(" · ")).clicked() {
                    app.escolher_fonte(i);
                }
            }
        });
}

fn menu_de_velocidade(app: &mut App, ui: &mut egui::Ui) {
    let resposta = pilula(ui, Some(Icone::Velocidade), &format!("{}×", app.velocidade), app.velocidade != 1.0, 46.0).on_hover_text("Velocidade");
    egui::Popup::from_toggle_button_response(&resposta)
        .align(egui::RectAlign::TOP_START)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClick)
        .show(|ui| {
            ui.set_min_width(110.0);
            for v in [0.5, 0.75, 1.0, 1.25, 1.5, 2.0] {
                if ui.selectable_label(app.velocidade == v, format!("{v}×")).clicked() {
                    app.definir_velocidade(v);
                }
            }
        });
}
