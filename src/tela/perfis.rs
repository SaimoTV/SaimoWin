//! "Quem está assistindo?": escolher, gerenciar e editar perfis (até 10, com
//! avatar), desenhado por cima de tudo. Abre sozinho na partida com 2 ou mais
//! perfis, pelo avatar do topo e pelos Ajustes. Perfil Kids: sair dele,
//! gerenciar estando nele e editar um Kids pedem o código de 4 dígitos.

use super::{botao, forte, normal, ACENTO, SECUNDARIO, TEXTO};
use crate::perfis::{self, Perfil};
use crate::App;
use eframe::egui::{self, Color32, Sense, Vec2};

#[derive(Clone)]
pub enum Modo {
    Escolher,
    Gerenciar,
    Editar(Option<Perfil>),
}

/// O que fazer depois do código certo.
#[derive(Clone)]
enum Depois {
    Entrar(String),
    Gerenciar,
    Editar(Option<Perfil>),
    Ajustes,
}

#[derive(Clone)]
pub struct Estado {
    pub obrigatoria: bool,
    pub modo: Modo,
    destrancado: bool,
    pedido: Option<(Perfil, String, Depois)>,
    codigo: String,
    erro: bool,
    // edição
    nome: String,
    avatar: String,
    kids: bool,
    cod1: String,
    cod2: String,
    aviso: String,
    confirmar_apagar: bool,
}

impl Estado {
    pub fn novo(obrigatoria: bool, gerenciar: bool) -> Self {
        Estado {
            obrigatoria,
            modo: if gerenciar { Modo::Gerenciar } else { Modo::Escolher },
            destrancado: false,
            pedido: None,
            codigo: String::new(),
            erro: false,
            nome: String::new(),
            avatar: perfis::AVATARES[0].into(),
            kids: false,
            cod1: String::new(),
            cod2: String::new(),
            aviso: String::new(),
            confirmar_apagar: false,
        }
    }

    /// Pede o código do Kids `p` (se for o caso) antes de seguir.
    fn com_codigo(&mut self, p: &Perfil, motivo: &str, depois: Depois) -> Option<Depois> {
        if !perfis::pede_codigo(p) || (self.destrancado && p.id == perfis::id_ativo()) {
            return Some(depois);
        }
        self.pedido = Some((p.clone(), motivo.into(), depois));
        self.codigo.clear();
        self.erro = false;
        None
    }

    fn abrir_edicao(&mut self, p: Option<Perfil>) {
        self.nome = p.as_ref().map(|p| p.nome.clone()).unwrap_or_default();
        let usados: Vec<String> = perfis::todos().into_iter().map(|p| p.avatar).collect();
        self.avatar = p.as_ref().map(|p| p.avatar.clone()).unwrap_or_else(|| {
            perfis::AVATARES.iter().find(|a| !usados.iter().any(|u| u == *a)).unwrap_or(&perfis::AVATARES[0]).to_string()
        });
        self.kids = p.as_ref().is_some_and(|p| p.kids);
        self.cod1.clear();
        self.cod2.clear();
        self.aviso.clear();
        self.confirmar_apagar = false;
        self.modo = Modo::Editar(p);
    }
}

/// Textura de um avatar, carregada uma vez.
pub fn textura(ctx: &egui::Context, avatar: &str) -> Option<egui::TextureHandle> {
    let id = egui::Id::new(("avatar", avatar.to_string()));
    if let Some(t) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return Some(t);
    }
    let imagem = image::load_from_memory(perfis::png(avatar)?).ok()?.to_rgba8();
    let (w, h) = imagem.dimensions();
    let cor = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], imagem.as_raw());
    let t = ctx.load_texture(format!("avatar-{avatar}"), cor, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    Some(t)
}

/// O avatar desenhado (ou a inicial, se for desconhecido). Devolve a resposta do clique.
pub fn avatar(ui: &mut egui::Ui, nome: &str, avatar: &str, lado: f32, destaque: Option<Color32>) -> egui::Response {
    let (rect, resposta) = ui.allocate_exact_size(Vec2::splat(lado), Sense::click());
    let raio = lado * 0.24;
    if let Some(t) = textura(ui.ctx(), avatar) {
        egui::Image::new(&t).corner_radius(raio).paint_at(ui, rect);
    } else {
        ui.painter().rect_filled(rect, raio, Color32::from_rgb(59, 63, 140));
        let inicial = nome.trim().chars().next().unwrap_or('?').to_uppercase().to_string();
        ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, inicial, forte(lado * 0.45), TEXTO);
    }
    let cor = if resposta.hovered() { Some(TEXTO) } else { destaque };
    if let Some(c) = cor {
        ui.painter().rect_stroke(rect.expand(3.0), raio + 3.0, egui::Stroke::new(3.0, c), egui::StrokeKind::Outside);
    }
    resposta
}

/// Desenha a tela de perfis, se aberta.
pub fn desenhar(app: &mut App, ctx: &egui::Context) {
    let Some(mut e) = app.perfis_tela.take() else { return };
    let mut fechar = false;
    let mut acao: Option<Depois> = None;
    egui::Area::new(egui::Id::new("perfis"))
        .fixed_pos(egui::Pos2::ZERO)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let tela = ctx.content_rect();
            ui.painter().rect_filled(tela, 0.0, Color32::from_rgb(11, 15, 42));
            ui.scope_builder(egui::UiBuilder::new().max_rect(tela.shrink(40.0)), |ui| {
                ui.vertical_centered(|ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.vertical_centered(|ui| {
                            if let Some((p, motivo, depois)) = e.pedido.clone() {
                                pedir_codigo(ui, &mut e, &p, &motivo, depois, &mut acao);
                            } else {
                                match e.modo.clone() {
                                    Modo::Escolher | Modo::Gerenciar => grade(ui, &mut e, &mut acao, &mut fechar),
                                    Modo::Editar(p) => editar(ui, &mut e, p),
                                }
                            }
                        });
                    });
                });
            });
        });
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        if e.pedido.is_some() {
            e.pedido = None;
        } else {
            match e.modo {
                Modo::Editar(_) => e.modo = Modo::Gerenciar,
                Modo::Gerenciar => e.modo = Modo::Escolher,
                Modo::Escolher => fechar = !e.obrigatoria,
            }
        }
    }
    match acao {
        Some(Depois::Entrar(id)) => {
            perfis::marcar_escolhido();
            if id == perfis::id_ativo() {
                fechar = true;
            } else {
                perfis::entrar(&id);
            }
        }
        Some(Depois::Gerenciar) => e.modo = Modo::Gerenciar,
        Some(Depois::Editar(p)) => e.abrir_edicao(p),
        Some(Depois::Ajustes) => {
            app.ajustes_liberados = true;
            app.aba = crate::Aba::Ajustes;
            fechar = true;
        }
        None => {}
    }
    if !fechar {
        app.perfis_tela = Some(e);
    }
    ctx.request_repaint();
}

fn grade(ui: &mut egui::Ui, e: &mut Estado, acao: &mut Option<Depois>, fechar: &mut bool) {
    let gerenciar = matches!(e.modo, Modo::Gerenciar);
    let lista = perfis::todos();
    let atual = perfis::ativo();
    ui.add_space(30.0);
    ui.label(egui::RichText::new(if gerenciar { "Gerenciar perfis" } else { "Quem está assistindo?" }).font(forte(40.0)).color(TEXTO));
    if gerenciar {
        ui.label(egui::RichText::new("Clique num perfil para mudar o nome, o avatar ou o Kids.").font(normal(15.0)).color(SECUNDARIO));
    }
    ui.add_space(28.0);
    let largura = (lista.len() + 1).min(6) as f32 * 150.0;
    ui.allocate_ui(Vec2::new(largura, 0.0), |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(26.0, 22.0);
            for p in &lista {
                ui.allocate_ui_with_layout(Vec2::new(124.0, 160.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                    let destaque = (p.id == atual.id && !gerenciar).then_some(ACENTO);
                    let r = avatar(ui, &p.nome, &p.avatar, 120.0, destaque);
                    if p.kids {
                        let pos = r.rect.right_bottom() + Vec2::new(-24.0, -8.0);
                        let caixa = egui::Rect::from_center_size(pos, Vec2::new(42.0, 18.0));
                        ui.painter().rect_filled(caixa, 9.0, Color32::from_rgb(255, 180, 61));
                        ui.painter().text(caixa.center(), egui::Align2::CENTER_CENTER, "KIDS", forte(11.0), Color32::from_rgb(42, 20, 0));
                    }
                    if gerenciar {
                        ui.painter().rect_filled(r.rect, 120.0 * 0.24, Color32::from_black_alpha(120));
                        ui.painter().text(r.rect.center(), egui::Align2::CENTER_CENTER, "editar", forte(16.0), TEXTO);
                    }
                    ui.label(egui::RichText::new(&p.nome).font(if p.id == atual.id { forte(16.0) } else { normal(16.0) }).color(TEXTO));
                    if r.clicked() {
                        *acao = if gerenciar {
                            e.com_codigo(p, &format!("Para editar o perfil {}, digite o código dele.", p.nome), Depois::Editar(Some(p.clone())))
                        } else if p.id == atual.id {
                            Some(Depois::Entrar(p.id.clone()))
                        } else {
                            e.com_codigo(&atual, &format!("Para sair do perfil {}, digite o código.", atual.nome), Depois::Entrar(p.id.clone()))
                        };
                    }
                });
            }
            if lista.len() < perfis::MAXIMO && (gerenciar || !atual.kids) {
                ui.allocate_ui_with_layout(Vec2::new(124.0, 160.0), egui::Layout::top_down(egui::Align::Center), |ui| {
                    let (rect, r) = ui.allocate_exact_size(Vec2::splat(120.0), Sense::click());
                    let cor = if r.hovered() { TEXTO } else { Color32::from_white_alpha(100) };
                    ui.painter().rect_stroke(rect, 28.0, egui::Stroke::new(2.0, cor), egui::StrokeKind::Inside);
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, "+", forte(48.0), cor);
                    ui.label(egui::RichText::new("Adicionar perfil").font(normal(15.0)).color(SECUNDARIO));
                    if r.clicked() {
                        *acao = if gerenciar {
                            Some(Depois::Editar(None))
                        } else {
                            e.com_codigo(&atual, "Para criar um perfil, digite o código deste perfil Kids.", Depois::Editar(None))
                        };
                    }
                });
            }
        });
    });
    ui.add_space(26.0);
    ui.horizontal(|ui| {
        let largura = if gerenciar { 120.0 } else { 330.0 };
        ui.add_space(((ui.available_width() - largura) / 2.0).max(0.0));
        if gerenciar {
            if botao(ui, None, "Pronto", true).clicked() {
                e.modo = Modo::Escolher;
            }
        } else {
            if botao(ui, None, "Gerenciar perfis", false).clicked() {
                *acao = e.com_codigo(&atual, "Para gerenciar os perfis, digite o código deste perfil Kids.", Depois::Gerenciar);
            }
            if !e.obrigatoria && botao(ui, None, "Fechar", false).clicked() {
                *fechar = true;
            }
        }
    });
    if !gerenciar {
        ui.add_space(14.0);
        let mut perguntar = perfis::perguntar_ao_abrir();
        if ui.checkbox(&mut perguntar, egui::RichText::new("Perguntar quem está assistindo ao abrir").color(SECUNDARIO)).changed() {
            perfis::definir_perguntar(perguntar);
        }
    }
    ui.add_space(18.0);
    ui.label(
        egui::RichText::new(format!(
            "Cada perfil tem os seus favoritos, o que está assistindo e os recentes. As listas e os ajustes valem para todos. Até {} perfis. O perfil Kids só mostra filmes e séries classificados até 12 anos e nada adulto.",
            perfis::MAXIMO
        ))
        .font(normal(12.5))
        .color(Color32::from_white_alpha(110)),
    );
}

fn so4(t: &mut String) {
    *t = t.chars().filter(char::is_ascii_digit).take(4).collect();
}

fn pedir_codigo(ui: &mut egui::Ui, e: &mut Estado, p: &Perfil, motivo: &str, depois: Depois, acao: &mut Option<Depois>) {
    ui.add_space(60.0);
    avatar(ui, &p.nome, &p.avatar, 90.0, None);
    ui.add_space(10.0);
    ui.label(egui::RichText::new(format!("Código do perfil {}", p.nome)).font(forte(30.0)).color(TEXTO));
    ui.label(egui::RichText::new(motivo).font(normal(15.0)).color(SECUNDARIO));
    ui.add_space(16.0);
    let campo = ui.add(egui::TextEdit::singleline(&mut e.codigo).password(true).hint_text("••••").desired_width(160.0).font(forte(26.0)).horizontal_align(egui::Align::Center));
    campo.request_focus();
    so4(&mut e.codigo);
    if e.codigo.len() == 4 {
        if perfis::conferir_codigo(p, &e.codigo) {
            if p.id == perfis::id_ativo() {
                e.destrancado = true;
            }
            e.pedido = None;
            *acao = Some(depois);
        } else {
            e.erro = true;
            e.codigo.clear();
        }
    }
    ui.label(egui::RichText::new(if e.erro { "Código errado. Tente de novo." } else { " " }).color(Color32::from_rgb(255, 179, 179)));
    if botao(ui, None, "Cancelar", false).clicked() {
        e.pedido = None;
    }
}

fn editar(ui: &mut egui::Ui, e: &mut Estado, perfil: Option<Perfil>) {
    ui.add_space(20.0);
    ui.label(egui::RichText::new(if perfil.is_some() { "Editar perfil" } else { "Novo perfil" }).font(forte(34.0)).color(TEXTO));
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - 520.0) / 2.0).max(0.0));
        avatar(ui, if e.nome.is_empty() { "?" } else { &e.nome }, &e.avatar.clone(), 100.0, None);
        ui.add_space(18.0);
        ui.add(egui::TextEdit::singleline(&mut e.nome).hint_text("Nome do perfil").desired_width(380.0).font(forte(20.0)).char_limit(perfis::MAX_NOME));
    });
    ui.add_space(14.0);
    ui.label(egui::RichText::new("Escolha um avatar").color(SECUNDARIO));
    ui.allocate_ui(Vec2::new(7.0 * 74.0, 0.0), |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
            for a in perfis::AVATARES {
                let escolhido = e.avatar == a;
                if avatar(ui, a, a, 62.0, escolhido.then_some(ACENTO)).clicked() {
                    e.avatar = a.into();
                }
            }
        });
    });
    let principal = perfil.as_ref().is_some_and(|p| p.id == perfis::PRINCIPAL);
    let ja_tinha = perfil.as_ref().is_some_and(|p| perfis::pede_codigo(p));
    if !principal {
        ui.add_space(14.0);
        ui.checkbox(&mut e.kids, egui::RichText::new("Perfil Kids: só filmes e séries até 12 anos, nada adulto. Sair dele pede um código de 4 números.").color(TEXTO));
        if e.kids {
            ui.horizontal(|ui| {
                ui.add_space(((ui.available_width() - 340.0) / 2.0).max(0.0));
                ui.add(egui::TextEdit::singleline(&mut e.cod1).password(true).hint_text(if ja_tinha { "Novo código" } else { "Código" }).desired_width(160.0));
                ui.add(egui::TextEdit::singleline(&mut e.cod2).password(true).hint_text("Repita o código").desired_width(160.0));
                so4(&mut e.cod1);
                so4(&mut e.cod2);
            });
        }
    }
    ui.label(egui::RichText::new(if e.aviso.is_empty() { " " } else { &e.aviso }).color(Color32::from_rgb(255, 179, 179)));
    ui.horizontal(|ui| {
        ui.add_space(((ui.available_width() - 420.0) / 2.0).max(0.0));
        if botao(ui, None, "Salvar", true).clicked() {
            if e.nome.trim().is_empty() {
                e.aviso = "Escreva um nome para o perfil.".into();
            } else {
                let mut kids_codigo: Option<String> = None;
                let mut ok = true;
                if e.kids {
                    if !e.cod1.is_empty() || !e.cod2.is_empty() || !ja_tinha {
                        if e.cod1.len() != 4 {
                            e.aviso = "O código do Kids tem 4 números.".into();
                            ok = false;
                        } else if e.cod1 != e.cod2 {
                            e.aviso = "Os dois códigos não são iguais.".into();
                            ok = false;
                        } else {
                            kids_codigo = Some(e.cod1.clone());
                        }
                    }
                } else if perfil.as_ref().is_some_and(|p| p.kids) {
                    kids_codigo = Some(String::new());
                }
                if ok {
                    match &perfil {
                        Some(p) => perfis::editar(&p.id, &e.nome, &e.avatar, kids_codigo.as_deref()),
                        None => {
                            perfis::criar(&e.nome, &e.avatar, kids_codigo.as_deref());
                        }
                    }
                    e.modo = Modo::Gerenciar;
                }
            }
        }
        if botao(ui, None, "Cancelar", false).clicked() {
            e.modo = Modo::Gerenciar;
        }
        if let Some(p) = perfil.as_ref().filter(|p| p.id != perfis::PRINCIPAL) {
            let rotulo = if e.confirmar_apagar { "Apagar mesmo" } else { "Apagar perfil" };
            if botao(ui, None, rotulo, false).clicked() {
                if e.confirmar_apagar {
                    perfis::apagar(&p.id);
                    e.modo = Modo::Gerenciar;
                } else {
                    e.confirmar_apagar = true;
                }
            }
        }
    });
}

/// Pede o código do Kids para abrir os Ajustes (a tela de perfis cuida do código).
pub fn pedir_ajustes(app: &mut App) {
    let p = perfis::ativo();
    let mut e = Estado::novo(false, false);
    e.pedido = Some((p, "Os Ajustes ficam trancados no perfil Kids. Digite o código para abrir.".into(), Depois::Ajustes));
    app.perfis_tela = Some(e);
}
