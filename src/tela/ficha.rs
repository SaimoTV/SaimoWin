//! A ficha do título, a página da série e a página do ator, como a
//! `FichaActivity` da TV Box: a imagem larga ao fundo escurecendo pela
//! esquerda e por baixo, a capa, "SÉRIE" ou "FILME" em ciano, o nome grande,
//! os selos (nota, ano, duração, temporadas, classificação), os gêneros, os
//! botões e a sinopse. A série tem as temporadas em pílulas e os episódios em
//! cartões com a imagem da cena, o andamento e a marca de visto.

use super::{
    aceso, andamento, botao, botao_redondo, branco, degrade3, forte, fundo_com, icones, inicial, moldura_de_foco, normal,
    pilula, pintar_cobrindo, selo, texto, Icone, ACENTO, AMARELO, ELEVADO, SECUNDARIO, TEXTO,
};
use crate::{ficha, progresso, vod, App, Destino, ItemNaTela};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke};

const MARGEM: f32 = 56.0;

/// O fundo da ficha: a imagem larga e os degradês (`bg_ficha_lado`, `bg_ficha_baixo`).
fn fundo(app: &mut App, ui: &egui::Ui, area: Rect, url: Option<String>) {
    let pintor = ui.painter();
    pintor.rect_filled(area, 0.0, fundo_com(150));
    if let Some(textura) = url.and_then(|u| app.imagem(&u, 1280)) {
        let t = ui.ctx().animate_bool_with_time(egui::Id::new(("fundo-da-ficha", textura.id())), true, 0.35);
        pintar_cobrindo(ui, &textura, area, 0.0, 0.25, t);
    }
    degrade3(pintor, area, fundo_com(250), fundo_com(200), 0.5, fundo_com(40), true);
    let baixo = Rect::from_min_max(Pos2::new(area.left(), area.top() + area.height() * 0.35), area.max);
    degrade3(pintor, baixo, fundo_com(0), fundo_com(200), 0.5, fundo_com(250), false);
}

/// As cores da classificação indicativa brasileira, as mesmas do celular.
fn cor_da_classificacao(nota: &str) -> Color32 {
    match nota.to_uppercase().as_str() {
        "L" => Color32::from_rgb(16, 185, 129),
        "10" => Color32::from_rgb(59, 130, 246),
        "12" => Color32::from_rgb(245, 158, 11),
        "14" => Color32::from_rgb(249, 115, 22),
        "16" | "18" => Color32::from_rgb(239, 68, 68),
        _ => SECUNDARIO,
    }
}

/// A capa grande, ou a inicial no lugar dela.
fn capa(app: &mut App, ui: &mut egui::Ui, titulo: &str, serie: bool, ficha: Option<&ficha::Ficha>, tamanho: egui::Vec2) {
    let (quadro, _) = ui.allocate_exact_size(tamanho, Sense::hover());
    ui.painter().rect_filled(quadro, 14.0, ELEVADO);
    let endereco = ficha.and_then(|f| f.capa.clone()).or_else(|| app.endereco_da_capa(titulo, serie));
    match endereco.and_then(|u| app.imagem(&u, 342)) {
        Some(textura) => pintar_cobrindo(ui, &textura, quadro, 14.0, 0.5, 1.0),
        None => {
            ui.painter().text(quadro.center(), Align2::CENTER_CENTER, inicial(titulo), forte(64.0), ACENTO);
        }
    }
}

/// Os selos da ficha: nota, ano, duração, temporadas e classificação.
fn selos(ui: &mut egui::Ui, f: &ficha::Ficha, serie: bool) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        if f.nota > 0.0 {
            selo(ui, Some(Icone::Estrela), &format!("{:.1}", f.nota).replace('.', ","), AMARELO);
        }
        if !f.ano.is_empty() {
            selo(ui, None, &f.ano, TEXTO);
        }
        if let Some(m) = f.duracao {
            let texto = if serie {
                format!("{m} min/ep")
            } else if m >= 60 {
                format!("{}h {:02}min", m / 60, m % 60)
            } else {
                format!("{m} min")
            };
            selo(ui, None, &texto, TEXTO);
        }
        if let Some(n) = f.temporadas {
            selo(ui, None, &if n == 1 { "1 temporada".into() } else { format!("{n} temporadas") }, TEXTO);
        }
        if let Some(c) = f.classificacao.as_ref() {
            selo(ui, None, c, cor_da_classificacao(c));
        }
    });
}

enum AcaoDaFicha {
    Fechar,
    Assistir,
    Favoritar,
    Ator(u32, String, Option<String>),
    Titulo(vod::Achado),
}

pub fn ficha_tela(app: &mut App, ui: &mut egui::Ui) {
    let area = ui.max_rect();
    if app.ficha_ator.is_some() {
        ator_tela(app, ui, area);
        return;
    }
    let Some((titulo, serie)) = app.ficha_aberta.clone() else { return };
    let ficha = app.ficha.clone();
    fundo(app, ui, area, ficha.as_ref().and_then(|f| f.fundo.clone()));

    let mut acao: Option<AcaoDaFicha> = None;
    let conteudo = area.shrink2(egui::vec2(MARGEM, 16.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(conteudo), |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if botao_redondo(ui, Icone::Voltar, 44.0, "Voltar (Esc)", false).clicked() {
                acao = Some(AcaoDaFicha::Fechar);
            }
            ui.add_space(10.0);
            ui.horizontal_top(|ui| {
                capa(app, ui, &titulo, serie, ficha.as_ref(), egui::vec2(230.0, 345.0));
                ui.add_space(34.0);
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width().min(780.0));
                    ui.label(egui::RichText::new(if serie { "SÉRIE" } else { "FILME" }).font(forte(14.0)).color(ACENTO).extra_letter_spacing(1.5));
                    ui.label(egui::RichText::new(crate::generos::sem_ano(&titulo)).font(forte(40.0)).color(TEXTO));
                    let Some(f) = ficha.as_ref() else {
                        ui.add_space(10.0);
                        let aviso = if app.ficha_carregando { "Buscando a ficha…" } else { "Sem ficha para este título." };
                        ui.label(egui::RichText::new(aviso).font(normal(16.0)).color(SECUNDARIO));
                        ui.add_space(16.0);
                        if botao(ui, Some(Icone::Tocar), if serie { "Ver episódios" } else { "Assistir" }, true).clicked() {
                            acao = Some(AcaoDaFicha::Assistir);
                        }
                        return;
                    };
                    if !f.frase.is_empty() {
                        ui.label(egui::RichText::new(&f.frase).font(normal(16.0)).italics().color(branco(180)));
                    }
                    ui.add_space(8.0);
                    selos(ui, f, serie);
                    if !f.generos.is_empty() {
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new(f.generos.join("  ·  ")).font(normal(15.0)).color(SECUNDARIO));
                    }
                    ui.add_space(18.0);
                    ui.horizontal(|ui| {
                        let andamento = !serie && progresso::onde_parou(&titulo).is_some();
                        let rotulo = if serie {
                            "Ver episódios"
                        } else if andamento {
                            "Continuar"
                        } else {
                            "Assistir"
                        };
                        if botao(ui, Some(Icone::Tocar), rotulo, true).clicked() {
                            acao = Some(AcaoDaFicha::Assistir);
                        }
                        ui.add_space(10.0);
                        let marcado = app.favoritos_vod.contains(&titulo);
                        let (icone, rotulo) = if marcado { (Icone::Estrela, "Nos favoritos") } else { (Icone::EstrelaVazia, "Favoritar") };
                        if botao(ui, Some(icone), rotulo, false).clicked() {
                            acao = Some(AcaoDaFicha::Favoritar);
                        }
                    });
                    if !f.sinopse.is_empty() {
                        ui.add_space(18.0);
                        ui.label(egui::RichText::new(&f.sinopse).font(normal(16.5)).color(branco(230)));
                    }
                    let creditos = [
                        (if serie { "Criação" } else { "Direção" }, f.assinatura.as_str()),
                        ("Roteiro", f.roteiro.as_str()),
                        ("Produção", f.produtora.as_str()),
                    ];
                    ui.add_space(10.0);
                    for (rotulo, valor) in creditos.iter().filter(|(_, v)| !v.is_empty()) {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(format!("{rotulo}:")).font(forte(14.0)).color(TEXTO));
                            ui.label(egui::RichText::new(*valor).font(normal(14.0)).color(SECUNDARIO));
                        });
                    }
                });
            });

            if let Some(f) = ficha.as_ref().filter(|f| !f.elenco.is_empty()) {
                ui.add_space(28.0);
                ui.label(egui::RichText::new("Elenco").font(forte(22.0)).color(TEXTO));
                ui.add_space(10.0);
                egui::ScrollArea::horizontal().id_salt("elenco").show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 14.0;
                        for pessoa in &f.elenco {
                            let foto = pessoa.foto.as_ref().and_then(|u| app.imagem(u, 185));
                            if cartao_de_pessoa(ui, &pessoa.nome, &pessoa.papel, foto).clicked() {
                                acao = Some(AcaoDaFicha::Ator(pessoa.id, pessoa.nome.clone(), pessoa.foto.clone()));
                            }
                        }
                    });
                });
            }
            ui.add_space(30.0);
        });
    });

    match acao {
        Some(AcaoDaFicha::Fechar) => app.fechar_ficha(),
        Some(AcaoDaFicha::Assistir) => app.assistir_da_ficha(),
        Some(AcaoDaFicha::Favoritar) => app.favoritar_vod(titulo),
        Some(AcaoDaFicha::Ator(id, nome, foto)) => app.abrir_ator(id, nome, foto),
        Some(AcaoDaFicha::Titulo(_)) | None => {}
    }
}

fn pintar_redondo(ui: &egui::Ui, circulo: Rect, foto: Option<egui::TextureHandle>, nome: &str) {
    ui.painter().circle_filled(circulo.center(), circulo.width() / 2.0, branco(26));
    match foto {
        Some(textura) => pintar_cobrindo(ui, &textura, circulo, circulo.width() / 2.0, 0.15, 1.0),
        None => {
            ui.painter().text(circulo.center(), Align2::CENTER_CENTER, inicial(nome), forte(circulo.width() * 0.36), SECUNDARIO);
        }
    }
}

/// A foto redonda do elenco, com o contorno ciano no foco (`foco_ator`).
fn cartao_de_pessoa(ui: &mut egui::Ui, nome: &str, papel: &str, foto: Option<egui::TextureHandle>) -> egui::Response {
    let (rect, resposta) = ui.allocate_exact_size(egui::vec2(124.0, 168.0), Sense::click());
    let t = aceso(ui, resposta.id, resposta.hovered());
    moldura_de_foco(ui.painter(), rect, t, 16.0);
    let lado = 96.0;
    let circulo = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 60.0), egui::vec2(lado, lado));
    pintar_redondo(ui, circulo, foto, nome);
    let pintor = ui.painter();
    texto(pintor, nome, forte(14.0), TEXTO, Pos2::new(rect.left() + 6.0, rect.top() + 116.0), rect.width() - 12.0, 1, true);
    texto(pintor, papel, normal(12.5), SECUNDARIO, Pos2::new(rect.left() + 6.0, rect.top() + 136.0), rect.width() - 12.0, 1, true);
    resposta.on_hover_text(format!("Ver o que {nome} tem no acervo")).on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn cartao_de_trabalho(ui: &mut egui::Ui, nome: &str, papel: &str, capa: Option<egui::TextureHandle>, largura: f32) -> egui::Response {
    let altura_capa = largura * 1.5;
    let (rect, resposta) = ui.allocate_exact_size(egui::vec2(largura + 12.0, altura_capa + 60.0), Sense::click());
    let t = aceso(ui, resposta.id, resposta.hovered());
    moldura_de_foco(ui.painter(), rect, t, 14.0);
    let quadro = Rect::from_min_size(rect.min + egui::vec2(6.0, 6.0), egui::vec2(largura, altura_capa));
    ui.painter().rect_filled(quadro, 12.0, ELEVADO);
    match capa {
        Some(textura) => pintar_cobrindo(ui, &textura, quadro, 12.0, 0.5, 1.0),
        None => {
            ui.painter().text(quadro.center(), Align2::CENTER_CENTER, inicial(nome), forte(34.0), ACENTO);
        }
    }
    texto(ui.painter(), nome, normal(14.0), TEXTO, Pos2::new(quadro.left(), quadro.bottom() + 6.0), largura, 1, false);
    texto(ui.painter(), papel, normal(12.5), SECUNDARIO, Pos2::new(quadro.left(), quadro.bottom() + 25.0), largura, 1, false);
    resposta.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Quem é a pessoa e as capas de tudo que ela tem no acervo.
fn ator_tela(app: &mut App, ui: &mut egui::Ui, area: Rect) {
    let Some((_, nome)) = app.ficha_ator.clone() else { return };
    ui.painter().rect_filled(area, 0.0, fundo_com(170));
    let perfil = app.perfil.clone();
    let foto_url = perfil.as_ref().and_then(|p| p.foto.clone()).or_else(|| app.ficha_ator_foto.clone());
    let trabalhos = app.filmografia.clone();
    let mut acao: Option<AcaoDaFicha> = None;

    ui.scope_builder(egui::UiBuilder::new().max_rect(area.shrink2(egui::vec2(MARGEM, 16.0))), |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            if botao_redondo(ui, Icone::Voltar, 44.0, "Voltar à ficha (Esc)", false).clicked() {
                acao = Some(AcaoDaFicha::Fechar);
            }
            ui.add_space(14.0);
            ui.horizontal(|ui| {
                let lado = if perfil.as_ref().and_then(|p| p.foto.as_ref()).is_some() { 632 } else { 185 };
                let foto = foto_url
                    .as_ref()
                    .and_then(|u| app.imagem(u, lado))
                    .or_else(|| app.ficha_ator_foto.clone().and_then(|u| app.imagem(&u, 185)));
                let (circulo, _) = ui.allocate_exact_size(egui::vec2(150.0, 150.0), Sense::hover());
                pintar_redondo(ui, circulo, foto, &nome);
                ui.painter().circle_stroke(circulo.center(), 76.0, Stroke::new(3.0, ACENTO));
                ui.add_space(26.0);
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width().min(860.0));
                    let exibido = perfil.as_ref().map(|p| p.nome.clone()).filter(|n| !n.is_empty()).unwrap_or(nome.clone());
                    ui.label(egui::RichText::new(exibido).font(forte(36.0)).color(TEXTO));
                    if let Some(p) = perfil.as_ref() {
                        if !p.dados.is_empty() {
                            ui.label(egui::RichText::new(&p.dados).font(normal(16.0)).color(ACENTO));
                        }
                        if !p.biografia.is_empty() {
                            ui.add_space(6.0);
                            let curta: String = p.biografia.chars().take(420).collect();
                            let curta = if p.biografia.chars().count() > 420 { format!("{curta}…") } else { curta };
                            ui.label(egui::RichText::new(curta).font(normal(15.0)).color(branco(190)));
                        }
                    }
                });
            });
            ui.add_space(24.0);
            let contagem = if app.filmografia_carregando {
                "Procurando no acervo…".to_string()
            } else if trabalhos.len() == 1 {
                "1 título no acervo".to_string()
            } else {
                format!("{} títulos no acervo", trabalhos.len())
            };
            ui.label(egui::RichText::new(contagem).font(forte(22.0)).color(TEXTO));
            if !app.filmografia_carregando && trabalhos.is_empty() {
                ui.label(egui::RichText::new("Aparece aqui só o que dá para assistir neste app.").font(normal(15.0)).color(SECUNDARIO));
            }
            ui.add_space(12.0);
            let largura = 140.0;
            let espaco = 10.0;
            let colunas = (((ui.available_width() + espaco) / (largura + 12.0 + espaco)).floor() as usize).max(1);
            for linha in trabalhos.chunks(colunas) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = espaco;
                    for trabalho in linha {
                        let capa = trabalho.capa.as_ref().and_then(|u| app.imagem(u, 185));
                        let nome = if trabalho.achado.ano.is_empty() {
                            trabalho.achado.titulo.clone()
                        } else {
                            format!("{} ({})", trabalho.achado.titulo, trabalho.achado.ano)
                        };
                        let papel = if trabalho.papel.is_empty() {
                            if trabalho.achado.serie { "Série".to_string() } else { "Filme".to_string() }
                        } else {
                            trabalho.papel.clone()
                        };
                        if cartao_de_trabalho(ui, &nome, &papel, capa, largura).clicked() {
                            acao = Some(AcaoDaFicha::Titulo(trabalho.achado.clone()));
                        }
                    }
                });
                ui.add_space(espaco);
            }
        });
    });

    match acao {
        Some(AcaoDaFicha::Fechar) => {
            app.ficha_ator = None;
            app.filmografia.clear();
        }
        Some(AcaoDaFicha::Titulo(achado)) => {
            let item = ItemNaTela {
                titulo: achado.titulo.clone(),
                detalhe: if achado.serie { "Série".into() } else { "Filme".into() },
                serie: achado.serie,
                letra: achado.letra.clone(),
            };
            app.abrir_ficha(achado.titulo.clone(), achado.serie, Destino::Item(item));
        }
        _ => {}
    }
}

// MARK: - Página da série

/// Os episódios agrupados por número: cada um com as versões que tem
/// (dublado, legendado) e a posição dela em `app.episodios`.
fn agrupar(episodios: &[vod::Episodio], temporada: u32) -> Vec<(u32, Vec<(usize, String)>)> {
    let mut grupos: Vec<(u32, Vec<(usize, String)>)> = Vec::new();
    for (i, e) in episodios.iter().enumerate().filter(|(_, e)| e.temporada == temporada) {
        match grupos.iter_mut().find(|(n, _)| *n == e.numero) {
            Some((_, versoes)) => versoes.push((i, e.versao.clone())),
            None => grupos.push((e.numero, vec![(i, e.versao.clone())])),
        }
    }
    grupos.sort_by_key(|(n, _)| *n);
    grupos
}

/// "dub" vira "Dublado", "leg" vira "Legendado".
fn nome_da_versao(versao: &str) -> String {
    let v = versao.to_lowercase();
    if v.starts_with("dub") {
        "Dublado".into()
    } else if v.starts_with("leg") {
        "Legendado".into()
    } else if v.is_empty() {
        "Original".into()
    } else {
        versao.to_uppercase()
    }
}

pub fn serie(app: &mut App, ui: &mut egui::Ui, serie: &vod::Serie) {
    let area = ui.max_rect();
    let dados = ficha::conhecida(&serie.titulo, true);
    fundo(app, ui, area, dados.as_ref().and_then(|f| f.fundo.clone()));

    let temporadas: Vec<u32> = {
        let mut t: Vec<u32> = app.episodios.iter().map(|e| e.temporada).collect();
        t.sort_unstable();
        t.dedup();
        t
    };
    // Onde a pessoa parou na série: é a temporada que abre e o botão principal.
    let parou = progresso::onde_parou_na_serie(&serie.titulo);
    let (parou_t, parou_e) = parou
        .as_ref()
        .and_then(|(rotulo, _)| {
            let resto = rotulo.rsplit(" · ").next()?;
            let (t, e) = resto.trim_start_matches('T').split_once(" E")?;
            Some((t.parse::<u32>().ok()?, e.parse::<u32>().ok()?))
        })
        .unwrap_or((0, 0));
    if app.temporada_aberta == 0 || !temporadas.contains(&app.temporada_aberta) {
        app.temporada_aberta = if temporadas.contains(&parou_t) { parou_t } else { temporadas.first().copied().unwrap_or(1) };
    }
    // O teclado anda pelos episódios: a temporada acompanha o foco.
    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowUp) || i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowRight)) {
        if let Some(e) = app.episodios.get(app.foco_vod) {
            app.temporada_aberta = e.temporada;
        }
    }
    let temporada = app.temporada_aberta;
    let tmdb = app.pedir_temporada(&serie.titulo, temporada).unwrap_or_default();

    let mut tocar: Option<usize> = None;
    let mut fechar = false;
    let mut favoritar = false;
    let mut nova_temporada = None;
    let conteudo = area.shrink2(egui::vec2(MARGEM, 16.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(conteudo), |ui| {
        egui::ScrollArea::vertical().id_salt("pagina-da-serie").auto_shrink([false, false]).show(ui, |ui| {
            if botao_redondo(ui, Icone::Voltar, 44.0, "Voltar (Esc)", false).clicked() {
                fechar = true;
            }
            ui.add_space(10.0);
            ui.horizontal_top(|ui| {
                capa(app, ui, &serie.titulo, true, dados.as_ref(), egui::vec2(200.0, 300.0));
                ui.add_space(32.0);
                ui.vertical(|ui| {
                    ui.set_max_width(ui.available_width().min(820.0));
                    ui.label(egui::RichText::new("SÉRIE").font(forte(14.0)).color(ACENTO).extra_letter_spacing(1.5));
                    ui.label(egui::RichText::new(crate::generos::sem_ano(&serie.titulo)).font(forte(40.0)).color(TEXTO));
                    ui.add_space(6.0);
                    match dados.as_ref() {
                        Some(f) => {
                            selos(ui, f, true);
                            if !f.generos.is_empty() {
                                ui.add_space(8.0);
                                ui.label(egui::RichText::new(f.generos.join("  ·  ")).font(normal(15.0)).color(SECUNDARIO));
                            }
                        }
                        None => {
                            let ano = if serie.ano.is_empty() { String::new() } else { format!("{} · ", serie.ano) };
                            ui.label(egui::RichText::new(format!("{ano}{} episódios", serie.episodios)).font(normal(15.0)).color(SECUNDARIO));
                        }
                    }
                    ui.add_space(16.0);
                    ui.horizontal(|ui| {
                        // Continuar de onde parou; sem nada pela metade, o primeiro.
                        let alvo = app
                            .episodios
                            .iter()
                            .position(|e| e.temporada == parou_t && e.numero == parou_e)
                            .or_else(|| (!app.episodios.is_empty()).then_some(0));
                        if let Some(i) = alvo {
                            let e = &app.episodios[i];
                            let rotulo = if parou.is_some() {
                                format!("Continuar T{} E{}", e.temporada, e.numero)
                            } else {
                                format!("Assistir T{} E{}", e.temporada, e.numero)
                            };
                            if botao(ui, Some(Icone::Tocar), &rotulo, true).clicked() {
                                tocar = Some(i);
                            }
                            ui.add_space(10.0);
                        }
                        let marcado = app.favoritos_vod.contains(&serie.titulo);
                        let (icone, rotulo) = if marcado { (Icone::Estrela, "Nos favoritos") } else { (Icone::EstrelaVazia, "Favoritar") };
                        if botao(ui, Some(icone), rotulo, false).clicked() {
                            favoritar = true;
                        }
                    });
                    if let Some(f) = dados.as_ref().filter(|f| !f.sinopse.is_empty()) {
                        ui.add_space(16.0);
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 96.0), Sense::hover());
                        texto(ui.painter(), &f.sinopse, normal(16.0), branco(225), rect.min, rect.width(), 4, false);
                    }
                });
            });

            ui.add_space(26.0);
            ui.label(egui::RichText::new("Episódios").font(forte(24.0)).color(TEXTO));
            ui.add_space(10.0);
            if app.carregando_vod && app.episodios.is_empty() {
                ui.label(egui::RichText::new("Buscando episódios…").font(normal(16.0)).color(SECUNDARIO));
                return;
            }
            egui::ScrollArea::horizontal().id_salt("temporadas").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    for t in &temporadas {
                        if pilula(ui, None, &format!("Temporada {t}"), *t == temporada, 40.0).clicked() {
                            nova_temporada = Some(*t);
                        }
                    }
                });
            });
            ui.add_space(14.0);

            let grupos = agrupar(&app.episodios, temporada);
            let largura = 300.0;
            let espaco = 12.0;
            let colunas = (((ui.available_width() + espaco) / (largura + 12.0 + espaco)).floor() as usize).max(1);
            for linha in grupos.chunks(colunas) {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = espaco;
                    for (numero, versoes) in linha {
                        let info = tmdb.iter().find(|e| e.numero == *numero).cloned().unwrap_or_default();
                        let chave = format!("{} · T{temporada} E{numero}", serie.titulo);
                        let marca = progresso::onde_parou(&chave);
                        let visto = (temporada, *numero) < (parou_t, parou_e);
                        let focado = versoes.iter().any(|(i, _)| *i == app.foco_vod);
                        if let Some(i) = cartao_de_episodio(app, ui, *numero, &info, versoes, marca, visto, focado, largura) {
                            tocar = Some(i);
                        }
                    }
                });
                ui.add_space(espaco);
            }
            ui.add_space(30.0);
        });
    });

    if let Some(t) = nova_temporada {
        app.temporada_aberta = t;
        if let Some(i) = app.episodios.iter().position(|e| e.temporada == t) {
            app.foco_vod = i;
        }
    }
    if favoritar {
        app.favoritar_vod(serie.titulo.clone());
    }
    if fechar {
        app.serie_aberta = None;
        return;
    }
    if let Some(i) = tocar {
        app.foco_vod = i;
        app.abrir_do_acervo();
    }
}

/// O cartão do episódio (`item_episodio.xml`): a imagem da cena com o
/// andamento, "1. Nome", a duração, a versão e a sinopse. Devolve o episódio
/// (a posição em `app.episodios`) quando clicado.
#[allow(clippy::too_many_arguments)]
fn cartao_de_episodio(
    app: &mut App,
    ui: &mut egui::Ui,
    numero: u32,
    info: &ficha::EpisodioTmdb,
    versoes: &[(usize, String)],
    marca: Option<progresso::Marca>,
    visto: bool,
    focado: bool,
    largura: f32,
) -> Option<usize> {
    let altura_imagem = largura * 9.0 / 16.0;
    let (rect, resposta) = ui.allocate_exact_size(egui::vec2(largura + 12.0, altura_imagem + 104.0), Sense::click());
    let t = aceso(ui, resposta.id, resposta.hovered() || focado);
    let pintor = ui.painter().clone();
    moldura_de_foco(&pintor, rect, t, 14.0);
    let quadro = Rect::from_min_size(rect.min + egui::vec2(6.0, 6.0), egui::vec2(largura, altura_imagem));
    pintor.rect_filled(quadro, 12.0, ELEVADO);
    match info.imagem.as_ref().and_then(|u| app.imagem(u, 342)) {
        Some(t) => pintar_cobrindo(ui, &t, quadro, 12.0, 0.5, 1.0),
        None => icones::pintar(&pintor, Icone::Tocar, quadro.center(), 40.0, branco(90)),
    }
    if t > 0.01 {
        pintor.circle_filled(quadro.center(), 28.0, Color32::WHITE.gamma_multiply(0.9 * t));
        icones::pintar(&pintor, Icone::Tocar, quadro.center(), 30.0, super::FUNDO.gamma_multiply(t));
    }
    if visto {
        let centro = quadro.right_top() + egui::vec2(-20.0, 20.0);
        pintor.circle_filled(centro, 14.0, ACENTO);
        icones::pintar(&pintor, Icone::Certo, centro, 18.0, super::FUNDO);
    }
    if let Some(m) = marca.as_ref() {
        andamento(&pintor, Rect::from_min_size(Pos2::new(quadro.left() + 10.0, quadro.bottom() - 12.0), egui::vec2(quadro.width() - 20.0, 4.0)), progresso::fracao(m));
    }
    let nome = if info.nome.is_empty() { format!("{numero}. Episódio {numero}") } else { format!("{numero}. {}", info.nome) };
    let x = quadro.left();
    let mut y = quadro.bottom() + 8.0;
    texto(&pintor, &nome, forte(15.5), TEXTO, Pos2::new(x, y), largura, 1, false);
    y += 22.0;
    let mut partes: Vec<String> = Vec::new();
    if let Some(d) = info.duracao {
        partes.push(format!("{d} min"));
    }
    if let Some(m) = marca.as_ref() {
        partes.push(progresso::falta(m));
    } else if versoes.len() == 1 {
        partes.push(nome_da_versao(&versoes[0].1));
    }
    if !info.sinopse.is_empty() {
        partes.push(info.sinopse.clone());
    }
    texto(&pintor, &partes.join(" · "), normal(13.0), SECUNDARIO, Pos2::new(x, y), largura, 2, false);

    let mut escolhido = None;
    // Mais de uma versão: uma pílula para cada, embaixo; o cartão toca a primeira.
    if versoes.len() > 1 {
        let mut px = x;
        for (i, versao) in versoes {
            let rotulo = nome_da_versao(versao);
            let galeria = pintor.layout_no_wrap(rotulo.clone(), forte(12.0), TEXTO);
            let alvo = Rect::from_min_size(Pos2::new(px, rect.bottom() - 30.0), egui::vec2(galeria.size().x + 20.0, 24.0));
            let r = ui.interact(alvo, resposta.id.with(i), Sense::click());
            let claro = aceso(ui, r.id, r.hovered());
            pintor.rect_filled(alvo, 12.0, branco(38).lerp_to_gamma(Color32::WHITE, claro));
            pintor.text(alvo.center(), Align2::CENTER_CENTER, rotulo, forte(12.0), TEXTO.lerp_to_gamma(super::FUNDO, claro));
            if r.on_hover_text("Assistir nesta versão").clicked() {
                escolhido = Some(*i);
            }
            px = alvo.right() + 6.0;
        }
    }
    if escolhido.is_none() && resposta.clicked() {
        escolhido = versoes.first().map(|(i, _)| *i);
    }
    if resposta.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
        if let Some((i, _)) = versoes.first() {
            app.foco_vod = *i;
        }
    }
    resposta.on_hover_cursor(egui::CursorIcon::PointingHand);
    escolhido
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ep(temporada: u32, numero: u32, versao: &str) -> vod::Episodio {
        vod::Episodio { temporada, numero, versao: versao.into(), urls: Vec::new() }
    }

    #[test]
    fn versoes_do_mesmo_episodio_ficam_juntas() {
        let lista = vec![ep(1, 2, "leg"), ep(1, 1, "dub"), ep(1, 1, "leg"), ep(2, 1, "dub")];
        let grupos = agrupar(&lista, 1);
        assert_eq!(grupos.len(), 2);
        assert_eq!(grupos[0].0, 1);
        assert_eq!(grupos[0].1.len(), 2);
        assert_eq!(nome_da_versao("dub"), "Dublado");
        assert_eq!(nome_da_versao("LEG"), "Legendado");
    }
}
