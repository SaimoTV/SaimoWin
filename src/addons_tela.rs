//! Telas de configuração de Addons Stremio + Debrid (com QR Code na porta 8999),
//! descoberta via catálogos / TMDB e seletor de fontes (streams) com Debrid.

use crate::addons::AddonStream;
use crate::debrid::{ConfigDebrid, AVISO_SEM_CHAVE};
use crate::tela::{aceso, botao, botao_redondo, forte, moldura_de_foco, normal, Icone, acento, ELEVADO, SECUNDARIO, TEXTO};
use crate::{addons, connect, debrid, qr, App};
use eframe::egui::{self, Align2, Color32, Pos2, Sense, Stroke};

/// Item de catálogo vindo de um addon Stremio ou da descoberta TMDB.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ObraAddon {
    pub titulo: String,
    pub detalhe: String,
    pub serie: bool,
    pub capa: Option<String>,
    pub fundo: Option<String>,
    pub sinopse: String,
    pub tmdb_id: Option<u32>,
    pub imdb_id: Option<String>,
    pub addon_id: Option<String>,
    pub tipo_stremio: String,
}

const ARQUIVO_OBRAS_HIST: &str = "saimo_addons_obras.json";

pub fn carregar_obras_salvas() -> std::collections::HashMap<String, ObraAddon> {
    let caminho = crate::catalogo::pasta().join(ARQUIVO_OBRAS_HIST);
    std::fs::read_to_string(caminho)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn salvar_obras_salvas(mapa: &std::collections::HashMap<String, ObraAddon>) {
    let caminho = crate::catalogo::pasta().join(ARQUIVO_OBRAS_HIST);
    if let Ok(texto) = serde_json::to_string(mapa) {
        let _ = std::fs::write(caminho, texto);
    }
}

/// Fila de descoberta montada em segundo plano (TMDB + catálogos dos addons).
#[derive(Clone, Debug)]
pub struct FilaDescoberta {
    pub titulo: String,
    pub tipo_stremio: String,
    pub itens: Vec<ObraAddon>,
}

/// Estado do diálogo "Addons e Debrid".
pub struct DialogoAddons {
    pub url_nova: String,
    pub adicionando: bool,
    pub status: Option<(bool, String)>,
    pub cfg_debrid: ConfigDebrid,
    pub mostrar_chaves: bool,
    qr_url: String,
    qr_textura: Option<egui::TextureHandle>,
}

impl DialogoAddons {
    pub fn novo() -> Self {
        Self {
            url_nova: String::new(),
            adicionando: false,
            status: None,
            cfg_debrid: debrid::config(),
            mostrar_chaves: false,
            qr_url: String::new(),
            qr_textura: None,
        }
    }

    fn textura_qr(&mut self, ctx: &egui::Context, url: &str) -> Option<egui::TextureHandle> {
        if self.qr_url == url {
            if let Some(ref t) = self.qr_textura {
                return Some(t.clone());
            }
        }
        let matriz = qr::gerar(url).ok()?;
        let img = matriz.para_imagem(6);
        let tex = ctx.load_texture("qr-addons", img, egui::TextureOptions::NEAREST);
        self.qr_url = url.to_string();
        self.qr_textura = Some(tex.clone());
        Some(tex)
    }
}

/// Estado do seletor de fontes de addon (ao clicar Assistir ou num episódio).
pub struct SeletorStreams {
    pub titulo_exibicao: String,
    pub serie_nome: String,
    pub serie: bool,
    pub temporada: Option<u32>,
    pub episodio: Option<u32>,
    pub tmdb_id: Option<u32>,
    pub imdb_id: Option<String>,
    pub extra_id: Option<String>,
    pub lista_episodios: Vec<crate::vod::Episodio>,
    pub carregando: bool,
    pub resolvendo: bool,
    pub streams: Vec<AddonStream>,
    pub erro: Option<String>,
    pub aviso_sem_debrid: bool,
}

pub fn url_lan_addons() -> String {
    match connect::ip_local() {
        Some(ip) => format!("http://{ip}:{}/addons", connect::porta()),
        None => format!("http://127.0.0.1:{}/addons", connect::porta()),
    }
}

/// Carrega filas iniciais de descoberta TMDB + `/catalog` dos addons ativos.
pub fn montar_descoberta() -> Vec<FilaDescoberta> {
    let mut filas = Vec::new();
    let ativos = addons::ativos();
    if ativos.is_empty() {
        return filas;
    }

    let de_tmdb = |titulo: &str, endpoint: &str, tipo_stremio: &str| -> Option<FilaDescoberta> {
        let itens: Vec<ObraAddon> = addons::descobrir_tmdb(endpoint)
            .into_iter()
            .take(24)
            .map(|t| ObraAddon {
                titulo: t.titulo,
                detalhe: if t.ano.is_empty() {
                    if t.nota > 0.0 { format!("★ {:.1}", t.nota) } else { String::new() }
                } else if t.nota > 0.0 {
                    format!("{} · ★ {:.1}", t.ano, t.nota)
                } else {
                    t.ano
                },
                serie: t.serie,
                capa: t.poster,
                fundo: None,
                sinopse: String::new(),
                tmdb_id: Some(t.id),
                imdb_id: None,
                addon_id: None,
                tipo_stremio: if t.serie { "series".into() } else { "movie".into() },
            })
            .collect();
        if itens.is_empty() {
            None
        } else {
            Some(FilaDescoberta {
                titulo: titulo.into(),
                tipo_stremio: tipo_stremio.into(),
                itens,
            })
        }
    };

    if let Some(f) = de_tmdb("Filmes em alta (TMDB)", "trending/movie/week", "movie") {
        filas.push(f);
    }
    if let Some(f) = de_tmdb("Séries em alta (TMDB)", "trending/tv/week", "series") {
        filas.push(f);
    }
    if let Some(f) = de_tmdb("Filmes populares (TMDB)", "movie/popular", "movie") {
        filas.push(f);
    }
    if let Some(f) = de_tmdb("Séries populares (TMDB)", "tv/popular", "series") {
        filas.push(f);
    }

    // Catálogos nativos declarados pelos addons instalados.
    for addon in ativos.iter().take(4) {
        for cat in addon.manifest.catalogs.iter().take(2) {
            let precisa_busca = cat.extra.iter().any(|e| e.is_required);
            if precisa_busca {
                continue;
            }
            let metas = addons::buscar_catalogo(addon, cat, None);
            if metas.is_empty() {
                continue;
            }
            let itens: Vec<ObraAddon> = metas
                .into_iter()
                .take(24)
                .map(obra_de_meta_preview)
                .collect();
            let nome_cat = cat.name.clone().unwrap_or_else(|| cat.id.clone());
            filas.push(FilaDescoberta {
                titulo: format!("{} · {}", addon.manifest.name, nome_cat),
                tipo_stremio: cat.tipo.clone(),
                itens,
            });
        }
    }
    filas
}

pub fn buscar_obras(termo: &str) -> Vec<ObraAddon> {
    let mut saida = Vec::new();
    let mut vistos = std::collections::HashSet::new();
    let q = addons::codificar_segmento(termo.trim());
    for t in addons::descobrir_tmdb(&format!("search/multi?query={q}")) {
        let chave = (t.titulo.to_ascii_lowercase(), t.serie);
        if vistos.insert(chave) {
            saida.push(ObraAddon {
                titulo: t.titulo,
                detalhe: t.ano,
                serie: t.serie,
                capa: t.poster,
                fundo: None,
                sinopse: String::new(),
                tmdb_id: Some(t.id),
                imdb_id: None,
                addon_id: None,
                tipo_stremio: if t.serie { "series".into() } else { "movie".into() },
            });
        }
    }
    for addon in addons::ativos().iter().take(4) {
        for cat in &addon.manifest.catalogs {
            if !cat.extra.iter().any(|e| e.name == "search") {
                continue;
            }
            for meta in addons::buscar_catalogo(addon, cat, Some(termo)) {
                let obra = obra_de_meta_preview(meta);
                let chave = (obra.titulo.to_ascii_lowercase(), obra.serie);
                if vistos.insert(chave) {
                    saida.push(obra);
                }
            }
        }
    }
    saida
}

fn obra_de_meta_preview(m: addons::MetaPreview) -> ObraAddon {
    let serie = m.tipo == "series" || m.tipo == "anime";
    let (tmdb_id, imdb_id) = if m.id.starts_with("tt") {
        (None, Some(m.id.clone()))
    } else if let Some(rest) = m.id.strip_prefix("tmdb:") {
        (rest.parse::<u32>().ok(), None)
    } else {
        (None, None)
    };
    let ano = m
        .release_info
        .as_ref()
        .and_then(|v| v.as_str().map(str::to_string).or_else(|| v.as_u64().map(|n| n.to_string())))
        .unwrap_or_default();
    ObraAddon {
        titulo: m.name,
        detalhe: ano,
        serie,
        capa: m.poster,
        fundo: m.background,
        sinopse: m.description.unwrap_or_default(),
        tmdb_id,
        imdb_id,
        addon_id: Some(m.id),
        tipo_stremio: if m.tipo.is_empty() {
            if serie { "series".into() } else { "movie".into() }
        } else {
            m.tipo
        },
    }
}

/// Desenha o modal "Addons e Debrid" com QR Code LAN + gerenciamento completo.
pub fn dialogo(app: &mut App, ctx: &egui::Context) {
    if app.addons_dialogo.is_none() {
        return;
    }
    crate::tela::janelas::veu(ctx);

    let url_lan = url_lan_addons();
    let mut fechar = false;
    let mut adicionar_url: Option<String> = None;
    let mut alternar: Option<(String, bool)> = None;
    let mut mover: Option<(String, isize)> = None;
    let mut remover: Option<String> = None;
    let mut salvar_debrid: Option<ConfigDebrid> = None;

    let tela = ctx.content_rect();
    let largura = (tela.width() - 60.0).min(880.0);
    let altura = (tela.height() - 60.0).min(640.0);

    egui::Area::new(egui::Id::new("addons_dialogo"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            crate::tela::janelas::modal().show(ui, |ui| {
                ui.set_width((largura - 56.0).max(320.0));
                ui.set_max_height((altura - 56.0).max(320.0));
                let Some(d) = app.addons_dialogo.as_mut() else { return };
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Addons Stremio e Debrid").font(forte(24.0)).color(TEXTO));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if botao_redondo(ui, Icone::Fechar, 38.0, "Fechar (Esc)", false).clicked() {
                            fechar = true;
                        }
                    });
                });
                ui.label(
                    egui::RichText::new("Nenhum addon vem pré-instalado. Adicione manifests HTTP (/manifest.json) ou configure pelo celular na mesma rede.")
                        .font(normal(13.5))
                        .color(SECUNDARIO),
                );
                ui.add_space(12.0);

                egui::ScrollArea::vertical().max_height(altura - 88.0).show(ui, |ui| {
                    // QR Code LAN + input URL lado a lado
                    ui.horizontal_top(|ui| {
                        let tex = d.textura_qr(ctx, &url_lan);
                        ui.vertical(|ui| {
                            ui.set_width(164.0);
                            if let Some(t) = tex {
                                ui.add(egui::Image::new(&t).fit_to_exact_size(egui::vec2(148.0, 148.0)));
                            }
                            ui.add_space(4.0);
                            ui.label(egui::RichText::new("Escaneie no celular").font(forte(12.5)).color(acento()));
                            ui.label(egui::RichText::new(&url_lan).font(normal(11.5)).color(SECUNDARIO));
                        });
                        ui.add_space(14.0);
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("Adicionar addon por URL").font(forte(16.0)).color(TEXTO));
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                let edit = ui.add_sized(
                                    [ui.available_width() - 150.0, 36.0],
                                    egui::TextEdit::singleline(&mut d.url_nova)
                                        .hint_text("https://... ou stremio://.../manifest.json"),
                                );
                                let enter = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                                let clicou = ui
                                    .add_enabled(!d.adicionando, egui::Button::new(if d.adicionando { "Validando…" } else { "Adicionar" }))
                                    .clicked();
                                if (enter || clicou) && !d.url_nova.trim().is_empty() && !d.adicionando {
                                    adicionar_url = Some(d.url_nova.trim().to_string());
                                }
                            });
                            if let Some((ok, ref msg)) = d.status {
                                ui.add_space(6.0);
                                let cor = if ok { Color32::from_rgb(74, 222, 128) } else { Color32::from_rgb(248, 113, 113) };
                                ui.label(egui::RichText::new(msg).font(normal(13.0)).color(cor));
                            }
                            ui.add_space(10.0);
                            ui.label(egui::RichText::new("Addons instalados").font(forte(16.0)).color(TEXTO));
                            ui.add_space(6.0);
                            let instalados = addons::todos();
                            if instalados.is_empty() {
                                ui.label(
                                    egui::RichText::new("Nenhum addon instalado ainda.")
                                        .font(normal(13.5))
                                        .color(SECUNDARIO),
                                );
                            } else {
                                let total = instalados.len();
                                for (idx, a) in instalados.into_iter().enumerate() {
                                    egui::Frame::NONE
                                        .fill(ELEVADO)
                                        .corner_radius(10.0)
                                        .inner_margin(egui::Margin::symmetric(12, 8))
                                        .show(ui, |ui| {
                                            ui.horizontal(|ui| {
                                                let mut ativo = a.enabled;
                                                if ui.checkbox(&mut ativo, "").changed() {
                                                    alternar = Some((a.transport_url.clone(), ativo));
                                                }
                                                ui.vertical(|ui| {
                                                    ui.label(
                                                        egui::RichText::new(format!("{}  v{}", a.manifest.name, a.manifest.version))
                                                            .font(forte(14.5))
                                                            .color(if a.enabled { TEXTO } else { SECUNDARIO }),
                                                    );
                                                    let rec = a.manifest.nomes_recursos().join(", ");
                                                    ui.label(egui::RichText::new(format!("{rec} · {}", a.transport_url)).font(normal(11.5)).color(SECUNDARIO));
                                                });
                                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                                    if ui.button("Remover").clicked() {
                                                        remover = Some(a.transport_url.clone());
                                                    }
                                                    if idx + 1 < total && ui.button("▼").clicked() {
                                                        mover = Some((a.transport_url.clone(), 1));
                                                    }
                                                    if idx > 0 && ui.button("▲").clicked() {
                                                        mover = Some((a.transport_url.clone(), -1));
                                                    }
                                                });
                                            });
                                        });
                                    ui.add_space(4.0);
                                }
                            }
                        });
                    });

                    ui.add_space(16.0);
                    ui.separator();
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Provedores Debrid (Torrents em nuvem)").font(forte(16.5)).color(TEXTO));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.checkbox(&mut d.mostrar_chaves, "Mostrar chaves");
                        });
                    });
                    ui.label(
                        egui::RichText::new("Opcional. Sem Debrid, torrents tocam pelo tocador embutido (baixa enquanto assiste); com Debrid, abrem mais rápido.")
                            .font(normal(12.5))
                            .color(SECUNDARIO),
                    );
                    ui.add_space(8.0);

                    let masked = !d.mostrar_chaves;
                    let campo_prov = |ui: &mut egui::Ui, nome: &str, cfg: &mut debrid::ProvedorConfig| {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut cfg.enabled, "");
                            ui.add_sized([120.0, 24.0], egui::Label::new(egui::RichText::new(nome).font(forte(13.5)).color(TEXTO)));
                            ui.add_sized(
                                [ui.available_width().max(200.0), 30.0],
                                egui::TextEdit::singleline(&mut cfg.api_key)
                                    .password(masked)
                                    .hint_text(format!("Chave / Token da API {nome}")),
                            );
                        });
                        ui.add_space(4.0);
                    };
                    campo_prov(ui, "TorBox", &mut d.cfg_debrid.torbox);
                    campo_prov(ui, "Real-Debrid", &mut d.cfg_debrid.real_debrid);
                    campo_prov(ui, "AllDebrid", &mut d.cfg_debrid.all_debrid);
                    campo_prov(ui, "Premiumize", &mut d.cfg_debrid.premiumize);

                    ui.add_space(6.0);
                    ui.checkbox(
                        &mut d.cfg_debrid.sincronizar_connect,
                        "Sincronizar chaves Debrid com o celular via Saimo Connect (opcional)",
                    );
                    ui.add_space(8.0);
                    if botao(ui, Some(Icone::Certo), "Salvar chaves Debrid", true).clicked() {
                        salvar_debrid = Some(d.cfg_debrid.clone());
                    }
                });
            });
        });

    if let Some(url) = adicionar_url {
        if let Some(d) = app.addons_dialogo.as_mut() {
            d.adicionando = true;
            d.status = Some((true, "Consultando /manifest.json…".into()));
        }
        let emissor = app.emissor.clone();
        std::thread::spawn(move || {
            let res = addons::adicionar_url(&url);
            let _ = emissor.send(crate::Recado::AddonAdicionado(res));
        });
    }
    if let Some((u, ativo)) = alternar {
        addons::alternar(&u, ativo);
        app.atualizar_descoberta_addons();
        app.mandar_dados();
    }
    if let Some((u, delta)) = mover {
        addons::mover(&u, delta);
        app.atualizar_descoberta_addons();
        app.mandar_dados();
    }
    if let Some(u) = remover {
        addons::remover(&u);
        app.atualizar_descoberta_addons();
        app.mandar_dados();
    }
    if let Some(cfg) = salvar_debrid {
        let msg = match debrid::salvar_config(cfg) {
            Ok(()) => {
                app.mandar_dados();
                (true, "Chaves Debrid salvas com sucesso.".into())
            }
            Err(e) => (false, e),
        };
        if let Some(d) = app.addons_dialogo.as_mut() {
            d.status = Some(msg);
        }
    }
    if fechar {
        app.addons_dialogo = None;
    }
}

/// Modal para escolher um stream de addon e resolver via Debrid sob demanda.
pub fn seletor_streams(app: &mut App, ctx: &egui::Context) {
    if app.seletor_addon.is_none() {
        return;
    }
    crate::tela::janelas::veu(ctx);

    let mut fechar = false;
    let mut escolher_stream: Option<AddonStream> = None;
    let mut abrir_config_debrid = false;

    let tela = ctx.content_rect();
    let largura = (tela.width() - 80.0).min(760.0);
    let altura = (tela.height() - 80.0).min(560.0);

    egui::Area::new(egui::Id::new("seletor_addon_streams"))
        .order(egui::Order::Foreground)
        .anchor(Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            crate::tela::janelas::modal().show(ui, |ui| {
                ui.set_width((largura - 56.0).max(320.0));
                ui.set_max_height((altura - 56.0).max(280.0));
                let Some(sel) = app.seletor_addon.as_ref() else { return };
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Fontes disponíveis (Addons)").font(forte(22.0)).color(TEXTO));
                        ui.label(egui::RichText::new(&sel.titulo_exibicao).font(normal(14.0)).color(acento()));
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if botao_redondo(ui, Icone::Fechar, 36.0, "Fechar (Esc)", false).clicked() {
                            fechar = true;
                        }
                    });
                });
                ui.add_space(10.0);

                if let Some(ref err) = sel.erro {
                    egui::Frame::NONE
                        .fill(Color32::from_rgb(69, 10, 10))
                        .corner_radius(10.0)
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.label(egui::RichText::new(err).font(normal(13.5)).color(Color32::from_rgb(254, 202, 202)));
                            if sel.aviso_sem_debrid {
                                ui.add_space(8.0);
                                if botao(ui, Some(Icone::Ajustes), "Configurar Debrid agora", true).clicked() {
                                    abrir_config_debrid = true;
                                }
                            }
                        });
                    ui.add_space(10.0);
                }

                if sel.carregando {
                    ui.add_space(24.0);
                    ui.label(egui::RichText::new("Consultando addons habilitados…").font(normal(16.0)).color(SECUNDARIO));
                    return;
                }
                if sel.resolvendo {
                    ui.add_space(10.0);
                    ui.label(egui::RichText::new("Resolvendo link no Debrid…").font(forte(15.0)).color(acento()));
                    ui.add_space(8.0);
                }
                if sel.streams.is_empty() {
                    ui.add_space(20.0);
                    ui.label(
                        egui::RichText::new("Nenhum stream encontrado nos addons ativos para este título.")
                            .font(normal(15.0))
                            .color(SECUNDARIO),
                    );
                    return;
                }

                egui::ScrollArea::vertical().max_height(altura - 130.0).show(ui, |ui| {
                    for (idx, stream) in sel.streams.iter().enumerate() {
                        let largura_card = ui.available_width();
                        let titulo = stream.texto_principal();
                        let detalhe = stream.detalhe_completo();
                        let (rect, resp) = ui.allocate_exact_size(egui::vec2(largura_card, 64.0), Sense::click());
                        let t = aceso(ui, egui::Id::new(("stream-opt", idx)), resp.hovered());
                        let pintor = ui.painter();
                        pintor.rect_filled(rect, 10.0, ELEVADO);
                        pintor.rect_stroke(
                            rect,
                            10.0,
                            Stroke::new(1.5, if t > 0.1 { acento() } else { Color32::from_white_alpha(24) }),
                            egui::StrokeKind::Inside,
                        );
                        moldura_de_foco(pintor, rect, t, 10.0);
                        pintor.text(
                            Pos2::new(rect.left() + 14.0, rect.top() + 14.0),
                            Align2::LEFT_TOP,
                            &titulo,
                            forte(14.5),
                            TEXTO,
                        );
                        pintor.text(
                            Pos2::new(rect.left() + 14.0, rect.top() + 36.0),
                            Align2::LEFT_TOP,
                            &detalhe,
                            normal(12.5),
                            SECUNDARIO,
                        );
                        if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() && !sel.resolvendo {
                            escolher_stream = Some(stream.clone());
                        }
                        ui.add_space(6.0);
                    }
                });
            });
        });

    if let Some(stream) = escolher_stream {
        if stream.e_torrent() && !debrid::config().tem_ativo() {
            if let Some(sel) = app.seletor_addon.as_mut() {
                sel.erro = Some(AVISO_SEM_CHAVE.into());
                sel.aviso_sem_debrid = true;
            }
        } else {
            app.resolver_e_tocar_addon_stream(stream);
        }
    }
    if abrir_config_debrid {
        app.seletor_addon = None;
        app.addons_dialogo = Some(DialogoAddons::novo());
    }
    if fechar {
        app.seletor_addon = None;
    }
}
