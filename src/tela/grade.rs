//! A grade de A a Z (`GradeActivity` da TV Box), os Favoritos e os Ajustes.
//!
//! A grade é o "Todos de A a Z" de cada tipo: as capas em quantas colunas
//! couberem, com a busca, a ordem (mais novos ou de A a Z) e os gêneros em
//! pílulas. Favoritos e o +18 usam a mesma grade.

use super::paginas::{self, cartao_de_titulo, desenhar_cartao, medidas, Tipo, ESPACO, MARGEM};
use super::{aceso, botao_redondo, forte, normal, pilula, rotulo_de_secao, Icone, ACENTO, FUNDO, SECUNDARIO, TEXTO};
use crate::{legendas, Aba, App, Destino};
use eframe::egui::{self, Align2, Pos2, Rect, Sense};

/// O nome da grade que está aberta.
fn titulo(app: &App) -> String {
    match app.aba {
        Aba::Favoritos => "Favoritos".into(),
        Aba::Extras => "+18".into(),
        aba => {
            let (nome, ..) = paginas::categoria(aba);
            if app.genero.is_empty() { format!("{nome} de A a Z") } else { format!("{nome}: {}", app.genero) }
        }
    }
}

pub fn grade_tela(app: &mut App, ui: &mut egui::Ui) {
    let area = ui.max_rect();
    let conteudo = area.shrink2(egui::vec2(MARGEM + 6.0, 0.0));
    let mut topo = conteudo.top() + 14.0;

    // Cabeçalho: voltar à página do tipo, o nome, a busca.
    let cabecalho = Rect::from_min_size(Pos2::new(conteudo.left(), topo), egui::vec2(conteudo.width(), 52.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(cabecalho).layout(egui::Layout::left_to_right(egui::Align::Center)), |ui| {
        if app.aba.de_tipo() && botao_redondo(ui, Icone::Voltar, 44.0, "Voltar (Esc)", false).clicked() {
            app.explorar = false;
            app.foco_vod = 0;
        }
        ui.add_space(8.0);
        ui.label(egui::RichText::new(titulo(app)).font(forte(32.0)).color(TEXTO));
        ui.add_space(10.0);
        let total = app.itens_na_tela().len();
        if total > 0 {
            ui.label(egui::RichText::new(format!("{total} títulos")).font(normal(15.0)).color(SECUNDARIO));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let largura = (ui.available_width() * 0.45).clamp(200.0, 380.0);
            let dica = match app.aba {
                Aba::Series | Aba::Animes | Aba::Doramas => "Buscar série",
                Aba::Favoritos => "Buscar nos favoritos",
                _ => "Buscar filme",
            };
            if super::campo_de_busca(ui, &mut app.busca_vod, dica, "busca-acervo", largura, 44.0).changed() {
                app.foco_vod = 0;
            }
        });
    });
    topo = cabecalho.bottom() + 12.0;

    // Ordem e gêneros, como a régua da grade da TV Box.
    let filtros = Rect::from_min_size(Pos2::new(conteudo.left(), topo), egui::vec2(conteudo.width(), 38.0));
    let mut escolhido: Option<String> = None;
    let mut letra: Option<String> = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(filtros), |ui| {
        egui::ScrollArea::horizontal()
            .id_salt("filtros-da-grade")
            .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    // As letras sobraram só no +18: o resto lista o acervo
                    // inteiro a partir do índice de busca.
                    if app.aba == Aba::Extras {
                        for (l, quantos) in app.gavetas.iter().map(|g| (g.letra.clone(), g.filmes)).collect::<Vec<_>>() {
                            if pilula(ui, None, &l, l == app.letra, 34.0).on_hover_text(format!("{quantos} títulos")).clicked() {
                                letra = Some(l);
                            }
                        }
                        ui.add_space(12.0);
                    } else if app.aba != Aba::Favoritos {
                        if pilula(ui, Some(Icone::Historico), "Mais novos", !app.ordem_az, 34.0).clicked() {
                            app.ordem_az = false;
                            app.foco_vod = 0;
                        }
                        if pilula(ui, Some(Icone::Ordenar), "A a Z", app.ordem_az, 34.0).clicked() {
                            app.ordem_az = true;
                            app.foco_vod = 0;
                        }
                        ui.add_space(12.0);
                    }
                    // Só depois que o arquivo de gêneros chega: oferecer um
                    // filtro que devolve vazio é pior que não oferecer.
                    if !app.generos.todos.is_empty() {
                        if pilula(ui, None, "Todos os gêneros", app.genero.is_empty(), 34.0).clicked() {
                            escolhido = Some(String::new());
                        }
                        for nome in app.generos.todos.clone() {
                            if pilula(ui, None, &nome, app.genero == nome, 34.0).clicked() {
                                escolhido = Some(if app.genero == nome { String::new() } else { nome });
                            }
                        }
                    }
                });
            });
    });
    if let Some(l) = letra {
        app.busca_vod.clear();
        app.abrir_letra(l);
    }
    if let Some(novo) = escolhido {
        app.genero = novo;
        app.foco_vod = 0;
    }
    topo = filtros.bottom() + 10.0;

    let lista = Rect::from_min_max(Pos2::new(area.left(), topo), area.max);
    ui.scope_builder(egui::UiBuilder::new().max_rect(lista), |ui| grade(app, ui));
}

/// As capas, só as linhas à vista.
fn grade(app: &mut App, ui: &mut egui::Ui) {
    let itens = app.itens_na_tela();
    let total = itens.len();
    let area = ui.max_rect();
    if app.carregando_vod && total == 0 {
        ui.painter().text(area.center() - egui::vec2(0.0, 60.0), Align2::CENTER_CENTER, "Carregando…", normal(17.0), SECUNDARIO);
        return;
    }
    if total == 0 {
        let aviso = match app.aba {
            Aba::Favoritos => "Nenhum favorito ainda. Marque com S, com o botão direito ou na ficha.",
            Aba::Extras => "Nenhum título extra nesta letra.",
            _ if !app.busca_vod.trim().is_empty() => "Nenhum título com esse nome.",
            _ if !app.genero.is_empty() => "Nenhum título desse gênero.",
            _ => "Acervo a caminho…",
        };
        ui.painter().text(area.center() - egui::vec2(0.0, 60.0), Align2::CENTER_CENTER, aviso, normal(17.0), SECUNDARIO);
        return;
    }
    let (moldura, _) = medidas(Tipo::Capa);
    let util = area.width() - 2.0 * MARGEM;
    let colunas = (((util + ESPACO) / (moldura.x + ESPACO)).floor() as usize).max(1);
    app.colunas_vod = colunas;
    // A sobra fica dividida entre as colunas: a grade ocupa a largura toda.
    let passo_x = if colunas > 1 { (util - moldura.x) / (colunas - 1) as f32 } else { 0.0 };
    let linhas = total.div_ceil(colunas);
    let foco = app.foco_vod.min(total - 1);
    let andou = ui.input(|i| {
        i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowUp)
            || i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowRight)
    }) && !ui.memory(|m| m.focused().is_some());

    let mut acao: Option<(usize, bool)> = None;
    egui::ScrollArea::vertical()
        .id_salt(("grade", app.aba as u8))
        .auto_shrink([false, false])
        .show_rows(ui, moldura.y + ESPACO, linhas + 1, |ui, faixa| {
            for linha in faixa {
                let (linha_rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), moldura.y + ESPACO), Sense::hover());
                for coluna in 0..colunas {
                    let indice = linha * colunas + coluna;
                    if indice >= total {
                        break;
                    }
                    let item = itens[indice].clone();
                    let rect = Rect::from_min_size(
                        Pos2::new(linha_rect.left() + MARGEM + coluna as f32 * passo_x, linha_rect.top()),
                        moldura,
                    );
                    let cartao = cartao_de_titulo(app, item, None);
                    let resposta = desenhar_cartao(app, ui, rect, &cartao, Tipo::Capa, indice == foco, egui::Id::new(("grade-cartao", indice)));
                    if indice == foco && andou {
                        ui.scroll_to_rect(rect, None);
                    }
                    if resposta.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                        app.foco_vod = indice;
                    }
                    if resposta.clicked() {
                        acao = Some((indice, false));
                    }
                    if resposta.secondary_clicked() {
                        acao = Some((indice, true));
                    }
                }
            }
        });

    if let Some((indice, favoritar)) = acao {
        app.foco_vod = indice;
        let item = itens[indice].clone();
        if favoritar {
            app.favoritar_vod(item.titulo);
        } else {
            app.abrir_ficha(item.titulo.clone(), item.serie, Destino::Item(item));
        }
    }
}

// MARK: - Ajustes

/// Uma linha dos Ajustes (`item_painel`): o nome e o valor embaixo, e o bloco
/// branco no foco.
fn linha(ui: &mut egui::Ui, nome: &str, valor: &str, icone: Icone) -> egui::Response {
    let largura = ui.available_width();
    let (rect, resposta) = ui.allocate_exact_size(egui::vec2(largura, 72.0), Sense::click());
    let t = aceso(ui, resposta.id, resposta.hovered());
    let pintor = ui.painter();
    if t > 0.01 {
        pintor.rect_filled(rect, 12.0, egui::Color32::WHITE.gamma_multiply(t));
    }
    let tinta = TEXTO.lerp_to_gamma(FUNDO, t);
    let sub = SECUNDARIO.lerp_to_gamma(FUNDO.gamma_multiply(0.75), t);
    super::icones::pintar(pintor, icone, Pos2::new(rect.left() + 30.0, rect.center().y), 24.0, if t > 0.5 { FUNDO } else { ACENTO });
    pintor.text(Pos2::new(rect.left() + 60.0, rect.top() + 14.0), Align2::LEFT_TOP, nome, forte(18.0), tinta);
    pintor.text(Pos2::new(rect.left() + 60.0, rect.top() + 40.0), Align2::LEFT_TOP, valor, normal(15.0), sub);
    resposta.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn nome_do_idioma(codigo: &str) -> &'static str {
    match codigo {
        "pob" => "Português (Brasil)",
        "por" => "Português (Portugal)",
        "eng" => "Inglês",
        "spa" => "Espanhol",
        _ => "Desligadas",
    }
}

pub fn ajustes(app: &mut App, ui: &mut egui::Ui) {
    let area = ui.max_rect().shrink2(egui::vec2(MARGEM + 14.0, 30.0));
    let esquerda = Rect::from_min_size(area.min, egui::vec2((area.width() * 0.36).min(420.0), area.height()));
    let direita = Rect::from_min_max(Pos2::new(esquerda.right() + 30.0, area.top()), area.max);

    ui.scope_builder(egui::UiBuilder::new().max_rect(esquerda), |ui| {
        ui.add_space(40.0);
        ui.label(egui::RichText::new("Ajustes").font(forte(48.0)).color(TEXTO));
        ui.add_space(14.0);
        ui.label(egui::RichText::new(format!("Saimo TV {}", crate::VERSAO)).font(normal(18.0)).color(SECUNDARIO));
        ui.label(egui::RichText::new("para Windows").font(normal(18.0)).color(SECUNDARIO));
        ui.add_space(26.0);
        ui.label(
            egui::RichText::new("Seus favoritos, o que ficou pela metade e a última rádio ficam guardados neste computador.")
                .font(normal(15.0))
                .color(SECUNDARIO),
        );
        ui.add_space(6.0);
        ui.label(egui::RichText::new(crate::catalogo::pasta().display().to_string()).font(normal(13.0)).color(super::APAGADO));
    });

    let mut acao: Option<u8> = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(direita), |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.set_max_width(direita.width().min(760.0));
            ui.add_space(34.0);
            rotulo_de_secao(ui, "Reprodução");
            ui.add_space(4.0);
            if linha(ui, "Preencher a tela", if app.preencher { "Ligado: corta as bordas, sem faixas pretas" } else { "Desligado: o vídeo inteiro, com faixas se preciso" }, Icone::Preencher).clicked() {
                acao = Some(0);
            }
            if linha(ui, "Super Resolução (Upscaling)", &app.rotulo_upscaling(), Icone::TelaCheia).clicked() {
                acao = Some(7);
            }
            if linha(ui, "Velocidade dos filmes e séries", &format!("{}×", app.velocidade), Icone::Velocidade).clicked() {
                acao = Some(1);
            }
            if linha(ui, "Som", &if app.mudo { "Sem som".to_string() } else { format!("Volume {}%", app.volume) }, if app.mudo { Icone::Mudo } else { Icone::Som }).clicked() {
                acao = Some(2);
            }
            ui.add_space(18.0);
            rotulo_de_secao(ui, "Legendas");
            ui.add_space(4.0);
            let idioma = legendas::idioma_guardado();
            if linha(ui, "Idioma preferido", nome_do_idioma(&idioma), Icone::Legendas).on_hover_text("Liga sozinha a primeira legenda deste idioma").clicked() {
                acao = Some(3);
            }
            ui.add_space(18.0);
            rotulo_de_secao(ui, "Saimo Connect");
            ui.add_space(4.0);
            let onde = match (crate::connect::ativo(), crate::connect::ip_local()) {
                (true, Some(ip)) => format!("No ar em {ip}:{}: o app Saimo do celular acha este computador na mesma rede", crate::connect::porta()),
                (true, None) => "No ar, mas sem rede local: ligue o computador ao Wi-Fi ou ao cabo".to_string(),
                (false, _) => format!("Fora do ar: a porta {} está ocupada por outro programa", crate::connect::porta()),
            };
            linha(ui, "Controle pelo celular", &onde, Icone::Cast).on_hover_text(
                "Se o Windows perguntar sobre o Firewall, permita em redes privadas: sem isso o celular não acha o computador",
            );
            let autorizados = crate::connect::quantos_autorizados();
            let resumo = match autorizados {
                0 => "Nenhum celular autorizado".to_string(),
                1 => "1 celular autorizado · clique para esquecer".to_string(),
                n => format!("{n} celulares autorizados · clique para esquecer"),
            };
            if linha(ui, "Esquecer todos os celulares", &resumo, Icone::Fechar).clicked() {
                acao = Some(8);
            }
            ui.add_space(18.0);
            rotulo_de_secao(ui, "Aplicativo");
            ui.add_space(4.0);
            if linha(ui, "Procurar atualização", &format!("Você está na versão {}", crate::VERSAO), Icone::Baixar).clicked() {
                acao = Some(4);
            }
            if linha(ui, "Atalhos do teclado", "H ou F1 mostra todos", Icone::Teclado).clicked() {
                acao = Some(5);
            }
            if linha(ui, "Tela cheia", "F ou F11 · Esc sai", Icone::TelaCheia).clicked() {
                acao = Some(6);
            }
            ui.add_space(40.0);
        });
    });
    match acao {
        Some(0) => app.alternar_preencher(),
        Some(7) => app.alternar_upscaling(),
        Some(1) => {
            let ordem = [1.0, 1.25, 1.5, 2.0, 0.75];
            let atual = ordem.iter().position(|v| *v == app.velocidade).unwrap_or(0);
            app.definir_velocidade(ordem[(atual + 1) % ordem.len()]);
        }
        Some(2) => app.alternar_mudo(),
        Some(3) => {
            let ordem = ["", "pob", "por", "eng", "spa"];
            let atual = ordem.iter().position(|c| *c == legendas::idioma_guardado()).unwrap_or(0);
            legendas::guardar_idioma(ordem[(atual + 1) % ordem.len()]);
        }
        Some(4) => app.procurar_atualizacao_agora(),
        Some(5) => app.ajuda_aberta = true,
        Some(6) => app.alternar_tela_cheia(ui.ctx()),
        Some(8) => {
            crate::connect::esquecer_todos();
            app.aviso = Some(("Celulares esquecidos. Na próxima conexão o computador vai perguntar de novo.".into(), std::time::Instant::now()));
        }
        _ => {}
    }
}
