//! O lado do app no Saimo Connect: o que o celular manda fazer e o estado
//! que volta para ele.
//!
//! Os comandos são os da TV Box (`Connect.Listener`), traduzidos para o que o
//! app já faz pelo teclado e pelo mouse — trocar de canal, multi tela, abrir
//! filme, volume. O controle remoto anda pelas mesmas teclas do teclado
//! ([`App::tecla`]), então faz no computador exatamente o que a pessoa faria
//! sentada na frente dele, inclusive o código da lista restrita, que só vale
//! digitado como em qualquer outro lugar.

use crate::connect::{self, Pedido, TelaNoAr};
use crate::{catalogo, epg, vod, Aba, App};
use eframe::egui;
use serde_json::{json, Value};
use std::time::{Duration, Instant};

/// De quanto em quanto tempo o estado é refeito para ver se mudou.
const REFAZER_ESTADO: Duration = Duration::from_millis(500);
/// Quanto um filme pedido pelo celular espera a letra do acervo chegar.
const ESPERA_DO_ACERVO: Duration = Duration::from_secs(25);

/// Um filme ou episódio pedido pelo celular, esperando o acervo chegar.
pub struct VodDoCelular {
    titulo: String,
    serie: bool,
    letra: String,
    /// O endereço que o celular mandou: vale quando o acervo daqui não acha.
    url: Option<String>,
    nome_na_tela: String,
    temporada: Option<u32>,
    episodio: Option<u32>,
    fonte: Option<usize>,
    inicio: Option<f64>,
    desde: Instant,
}

#[derive(Default)]
pub struct Remoto {
    vod: Option<VodDoCelular>,
    /// O ponto (em segundos) para onde ir quando o título abrir.
    pulo: Option<(String, f64)>,
}

impl App {
    /// A cada quadro: os pedidos do celular, as telas para o aviso de
    /// posição e o estado, quando mudou.
    pub(crate) fn cuidar_do_connect(&mut self, ctx: &egui::Context) {
        let pedidos: Vec<Pedido> = self.connect.try_iter().collect();
        let mut mudou = false;
        for pedido in pedidos {
            match pedido {
                Pedido::Aprovar { cliente, nome } => {
                    if !self.aprovacoes.iter().any(|(c, _, _)| *c == cliente) {
                        self.aprovacoes.push_back((cliente, nome, Instant::now()));
                    }
                    // A pergunta não pode passar despercebida com a janela
                    // minimizada: o ícone pisca na barra de tarefas.
                    ctx.send_viewport_cmd(egui::ViewportCommand::RequestUserAttention(
                        egui::UserAttentionType::Informational,
                    ));
                }
                Pedido::Desistiu { cliente } => self.aprovacoes.retain(|(c, _, _)| *c != cliente),
                Pedido::Conectou(nome) => {
                    self.aviso = Some((format!("Saimo Connect: {nome} conectado"), Instant::now()));
                    mudou = true;
                }
                Pedido::Comando(tipo, msg) => {
                    self.comando_do_celular(ctx, &tipo, &msg);
                    mudou = true;
                }
                Pedido::SomNoCelular => {
                    self.ajustar_som_das_telas();
                    let ouvem = connect::quem_ouve();
                    if !ouvem.is_empty() {
                        let multi = self.outra_tela.is_some();
                        let nomes: Vec<String> = ouvem
                            .iter()
                            .map(|(nome, src)| {
                                let qual = if !multi {
                                    "o som"
                                } else {
                                    match src.as_str() {
                                        "0" => "a tela da esquerda",
                                        "1" => "a tela da direita",
                                        _ => "as duas telas",
                                    }
                                };
                                format!("{nome} ouvindo {qual} no celular")
                            })
                            .collect();
                        self.aviso = Some((nomes.join(", "), Instant::now()));
                    }
                    mudou = true;
                }
            }
        }
        // A pergunta some sozinha depois do tempo que o servidor espera.
        self.aprovacoes.retain(|(_, _, desde)| desde.elapsed() < connect::ESPERA_DA_APROVACAO);
        if !self.aprovacoes.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        self.seguir_vod_do_celular();
        if !connect::ativo() {
            return;
        }
        connect::publicar_telas(self.telas_no_ar());
        let busca_aberta = self.aba == Aba::Buscar && self.ficha_aberta.is_none() && self.serie_aberta.is_none();
        connect::pedir_texto(busca_aberta.then_some(("Buscar filmes e séries", self.busca_vod.as_str())));
        // Refeito mesmo sem celular ligado: quem entra já recebe o de agora.
        if mudou || self.connect_publicado.elapsed() >= REFAZER_ESTADO {
            self.connect_publicado = Instant::now();
            connect::publicar_estado(self.estado_para_o_celular(), mudou);
        }
        if connect::tem_celular() {
            // O estado é refeito mesmo com a janela parada.
            ctx.request_repaint_after(REFAZER_ESTADO);
        }
    }

    /// As telas, da esquerda para a direita: (índice, mpv, o que toca nela).
    fn telas_da_tv(&self) -> Vec<(u32, std::sync::Arc<crate::mpv::Mpv>, Option<&crate::Tocando>)> {
        let Some(principal) = self.mpv.clone() else { return Vec::new() };
        match self.outra_tela.as_ref() {
            None => vec![(0, principal, self.tocando.as_ref())],
            Some(outra) => {
                let foco = (principal, self.tocando.as_ref());
                let fora = (outra.mpv.clone(), outra.tocando.as_ref());
                let (esquerda, direita) = if self.foco_a_direita { (fora, foco) } else { (foco, fora) };
                vec![(0, esquerda.0, esquerda.1), (1, direita.0, direita.1)]
            }
        }
    }

    fn telas_no_ar(&self) -> Vec<TelaNoAr> {
        if self.tocando_vod.as_ref().is_some_and(|t| t.confirmado) {
            return self.mpv.clone().map(|mpv| vec![TelaNoAr { idx: 0, mpv, vod: true }]).unwrap_or_default();
        }
        if self.radio.is_some() {
            return self.mpv.clone().map(|mpv| vec![TelaNoAr { idx: 0, mpv, vod: false }]).unwrap_or_default();
        }
        self.telas_da_tv()
            .into_iter()
            .filter(|(_, _, tocando)| tocando.is_some())
            .map(|(idx, mpv, _)| TelaNoAr { idx, mpv, vod: false })
            .collect()
    }

    /// O canal não aparece no celular se está escondido aqui (lista restrita
    /// fechada): a lista visível já não o tem, e o estado também não.
    fn canal_a_mostrar(&self, indice: usize) -> Option<&catalogo::Canal> {
        self.canais.get(indice).filter(|c| self.liberado || c.secao() != "Adulto")
    }

    /// O estado no formato da TV Box (`obterEstado`).
    fn estado_para_o_celular(&self) -> Value {
        let tela_do_app = match self.aba {
            _ if self.guia_aberto && self.aba == Aba::Canais => "guide",
            Aba::Canais => "live",
            Aba::Buscar => "search",
            Aba::Ajustes => "settings",
            _ => "home",
        };
        let mut estado = json!({
            "t": "state",
            "screen": tela_do_app,
            "multi": self.outra_tela.is_some(),
            "active": if self.outra_tela.is_some() && self.foco_a_direita { 1 } else { 0 },
            "volume": (self.volume as f64 / 100.0).clamp(0.0, 1.0),
            "muted": self.mudo,
        });

        if let Some(t) = self.tocando_vod.as_ref() {
            let url = t.urls.get(t.fonte).cloned().unwrap_or_default();
            let (titulo, subtitulo, chave) = match t.episodio.as_ref() {
                Some(e) => (e.serie.clone(), format!("Temporada {} · Episódio {}", e.temporada, e.numero), e.serie.clone()),
                None => (t.titulo.clone(), String::new(), t.titulo.clone()),
            };
            estado["mode"] = json!("vod");
            estado["playing"] = json!(t.confirmado && !self.pausado);
            estado["vod"] = json!({
                "title": titulo,
                "subtitle": subtitulo,
                "pos": (t.visto.0 * 1000.0) as i64,
                "dur": (t.visto.1 * 1000.0) as i64,
                "key": chave,
                "url": url,
            });
            estado["screens"] = json!([{
                "idx": 0,
                "channel": titulo,
                "num": 0,
                "logo": "",
                "program": subtitulo,
                "sourceIndex": t.fonte,
                "source": {"url": url, "referer": null, "userAgent": null, "keyId": null, "key": null},
                "liveOffset": 0,
            }]);
            return estado;
        }

        if let Some(r) = self.radio.as_ref() {
            estado["mode"] = json!("radio");
            estado["playing"] = json!(r.confirmado);
            estado["screens"] = json!([{
                "idx": 0,
                "channel": r.nome,
                "num": 0,
                "logo": r.logo.clone().unwrap_or_default(),
                "program": "Rádio ao vivo",
                "sourceIndex": 0,
                "source": {"url": r.url, "referer": null, "userAgent": null, "keyId": null, "key": null},
                "liveOffset": self.atraso_da_tela(0),
            }]);
            return estado;
        }

        let mut telas = Vec::new();
        let mut tocando = false;
        for (idx, mpv, no_ar) in self.telas_da_tv() {
            let Some(t) = no_ar else { continue };
            let Some(canal) = self.canal_a_mostrar(t.canal) else { continue };
            let Some(fonte) = canal.fontes.get(t.fonte).or_else(|| canal.fontes.first()) else { continue };
            tocando |= t.confirmado;
            let programa = epg::agora_e_depois(&canal.nome).map(|(p, _)| p.titulo).unwrap_or_default();
            let atraso = connect::instante(&TelaNoAr { idx, mpv, vod: false }).and_then(|(_, a)| a).unwrap_or(12_000);
            telas.push(json!({
                "idx": idx,
                "channel": canal.nome,
                "num": t.canal + 1,
                "logo": canal.logo.clone().unwrap_or_default(),
                "program": programa,
                "sourceIndex": t.fonte,
                "source": {
                    "url": fonte.url,
                    "referer": fonte.referer,
                    "userAgent": fonte.agente,
                    // ClearKey não toca no mpv: essas fontes nem entram na lista.
                    "keyId": null,
                    "key": null,
                },
                "liveOffset": atraso,
            }));
        }
        estado["mode"] = json!(if telas.is_empty() { "idle" } else { "live" });
        estado["playing"] = json!(tocando);
        estado["screens"] = Value::Array(telas);
        estado
    }

    fn atraso_da_tela(&self, idx: u32) -> i64 {
        self.telas_no_ar()
            .into_iter()
            .find(|t| t.idx == idx)
            .and_then(|t| connect::instante(&t))
            .and_then(|(_, a)| a)
            .unwrap_or(12_000)
    }

    /// O canal pelo nome que o celular mandou (a lista é a mesma publicada).
    fn canal_por_nome(&self, nome: &str) -> Option<usize> {
        let chave = catalogo::chave_de_ordem(nome.trim());
        self.canais
            .iter()
            .position(|c| c.nome == nome)
            .or_else(|| self.canais.iter().position(|c| catalogo::chave_de_ordem(&c.nome) == chave))
    }

    /// Põe em foco a tela da direita (ou da esquerda), na multi tela.
    fn focar_lado(&mut self, direita: bool) {
        if self.outra_tela.is_some() && self.foco_a_direita != direita {
            self.trocar_tela_em_foco();
        }
    }

    /// Para filme e rádio antes de um canal entrar no lugar.
    fn largar_vod_e_radio(&mut self) {
        if self.tocando_vod.is_some() {
            self.guardar_progresso(true);
            self.tocando_vod = None;
            self.episodios_no_player = false;
            crate::telemetria::parou();
        }
        if self.radio.is_some() {
            self.parar_radio();
        }
    }

    fn ir_para_o_ao_vivo(&mut self) {
        // O canal parado ao abrir o acervo não volta por cima do pedido.
        self.canal_parado = None;
        if self.aba != Aba::Canais {
            self.abrir_secao(Aba::Canais);
        }
        self.largar_vod_e_radio();
    }

    fn comando_do_celular(&mut self, ctx: &egui::Context, tipo: &str, msg: &Value) {
        let numero = |campo: &str| msg.get(campo).and_then(Value::as_f64);
        match tipo {
            "tune" => {
                let Some(nome) = msg.get("channel").and_then(Value::as_str) else { return };
                let Some(indice) = self.canal_por_nome(nome) else {
                    self.aviso = Some((format!("{nome} não está na lista agora"), Instant::now()));
                    return;
                };
                self.ir_para_o_ao_vivo();
                if let Some(lado) = numero("screen") {
                    self.focar_lado(lado as i64 == 1);
                }
                self.foco = indice;
                self.tocar(indice, 0, true);
            }
            "multi" => {
                let ligar = msg.get("on").and_then(Value::as_bool).unwrap_or(false);
                if ligar && self.outra_tela.is_none() {
                    self.ir_para_o_ao_vivo();
                    self.entrar_na_multi_tela();
                } else if !ligar {
                    self.sair_da_multi_tela();
                }
            }
            "activeScreen" => self.focar_lado(numero("idx").unwrap_or(0.0) as i64 == 1),
            "tvAudio" => self.ajustar_som_das_telas(),
            "playVod" | "playMedia" => self.abrir_vod_do_celular(msg),
            "playPause" => {
                if self.tocando_vod.is_some() {
                    self.alternar_pausa();
                }
            }
            "seek" => {
                let Some(t) = self.tocando_vod.as_ref() else { return };
                let duracao = t.visto.1;
                if let Some(delta) = numero("delta") {
                    self.avancar(delta / 1000.0);
                } else if let Some(ms) = numero("ms").filter(|ms| *ms >= 0.0) {
                    let alvo = ms / 1000.0;
                    self.ir_para(if duracao > 1.0 { alvo.min(duracao - 1.0) } else { alvo });
                    self.mostrar_controles();
                }
            }
            "fonte" => {
                let delta = numero("delta").unwrap_or(1.0) as i64;
                if let Some(t) = self.tocando.as_ref().filter(|_| self.tocando_vod.is_none()) {
                    let (canal, fonte) = (t.canal, t.fonte);
                    let total = self.canais.get(canal).map(|c| c.fontes.len()).unwrap_or(0) as i64;
                    if total > 1 {
                        self.tocar(canal, (fonte as i64 + delta).rem_euclid(total) as usize, false);
                    }
                } else if let Some(t) = self.tocando_vod.as_ref() {
                    let total = t.urls.len() as i64;
                    if total > 1 {
                        let proxima = (t.fonte as i64 + delta).rem_euclid(total) as usize;
                        self.escolher_fonte(proxima);
                    }
                }
            }
            "key" => self.tecla_do_celular(ctx, msg.get("code").and_then(Value::as_str).unwrap_or("")),
            "volume" => {
                if let Some(delta) = numero("delta") {
                    self.mudar_volume(if delta < 0.0 { -5 } else { 5 });
                } else if let Some(valor) = numero("set") {
                    self.definir_volume((valor.clamp(0.0, 1.0) * 100.0).round() as i32);
                } else if let Some(calar) = msg.get("mute").and_then(Value::as_bool) {
                    if calar != self.mudo {
                        self.alternar_mudo();
                    }
                }
            }
            "text" => {
                let valor = msg.get("value").and_then(Value::as_str).unwrap_or("").to_string();
                if self.aba == Aba::Canais {
                    self.busca = valor;
                    self.lista_aberta = true;
                } else {
                    if self.aba != Aba::Buscar {
                        self.abrir_secao(Aba::Buscar);
                    }
                    self.busca_vod = valor;
                }
            }
            _ => {}
        }
    }

    /// As teclas do controle do celular, pelas mesmas teclas do teclado.
    fn tecla_do_celular(&mut self, ctx: &egui::Context, codigo: &str) {
        let nada = egui::Modifiers::NONE;
        let tecla = match codigo {
            "UP" => egui::Key::ArrowUp,
            "DOWN" => egui::Key::ArrowDown,
            "LEFT" => egui::Key::ArrowLeft,
            "RIGHT" => egui::Key::ArrowRight,
            "OK" => egui::Key::Enter,
            "BACK" => egui::Key::Escape,
            "HOME" => {
                self.abrir_secao(Aba::Inicio);
                return;
            }
            "MENU" => {
                if self.aba == Aba::Canais {
                    self.lista_aberta = !self.lista_aberta;
                    self.mostrar_controles();
                } else {
                    self.tecla(ctx, egui::Key::I, nada);
                }
                return;
            }
            "CH_UP" | "CH_DOWN" => {
                if self.tocando_vod.is_none() && self.radio.is_none() {
                    self.ir_para_o_ao_vivo();
                    self.pular_canal(if codigo == "CH_UP" { -1 } else { 1 });
                }
                return;
            }
            "DEL" => {
                self.digitado.pop();
                return;
            }
            "CLEAR" => {
                self.digitado.clear();
                return;
            }
            d if d.len() == 1 && d.chars().all(|c| c.is_ascii_digit()) => {
                if self.aba == Aba::Canais {
                    self.digito(d.chars().next().unwrap());
                    // O número entra sozinho 1,2 s depois, como digitado aqui.
                    ctx.request_repaint_after(Duration::from_millis(1300));
                }
                return;
            }
            _ => return,
        };
        self.tecla(ctx, tecla, nada);
        self.mostrar_controles();
    }

    // MARK: - Filme e série pedidos pelo celular

    fn abrir_vod_do_celular(&mut self, msg: &Value) {
        let texto = |campo: &str| msg.get(campo).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty()).map(str::to_string);
        let numero = |campo: &str| msg.get(campo).and_then(Value::as_f64);
        // "saimo|s|<letra>|<título>|<ano>": a chave do acervo publicado.
        let chave = texto("key").unwrap_or_default();
        let partes: Vec<String> = if chave.starts_with("saimo|") {
            chave.split('|').map(connect::hls::decodificar).collect()
        } else {
            Vec::new()
        };
        let serie_da_chave = partes.get(1).map(|t| t == "s");
        let titulo = partes
            .get(3)
            .cloned()
            .filter(|t| !t.is_empty())
            .or_else(|| texto("title"))
            .or_else(|| (!chave.is_empty() && !chave.starts_with("saimo|")).then(|| chave.clone()))
            .unwrap_or_else(|| "Vídeo".into());
        let ano = partes.get(4).cloned().or_else(|| texto("year")).unwrap_or_default();
        let temporada = numero("temporada").map(|v| v as u32).filter(|v| *v > 0);
        let episodio = numero("episodio").map(|v| v as u32).filter(|v| *v > 0);
        let serie = msg.get("isSeries").and_then(Value::as_bool).or(serie_da_chave).unwrap_or(temporada.is_some());

        // O título no acervo daqui: igual, depois sem acento e caixa, depois
        // com o ano colado ("Nome (2024)").
        let alvo = catalogo::chave_de_ordem(&titulo);
        let com_ano = catalogo::chave_de_ordem(&format!("{titulo} ({ano})"));
        let achado = self
            .acervo
            .iter()
            .find(|a| a.serie == serie && a.titulo == titulo)
            .or_else(|| self.acervo.iter().find(|a| a.serie == serie && catalogo::chave_de_ordem(&a.titulo) == alvo))
            .or_else(|| {
                (!ano.is_empty()).then(|| self.acervo.iter().find(|a| a.serie == serie && catalogo::chave_de_ordem(&a.titulo) == com_ano)).flatten()
            })
            .cloned();
        let letra = achado
            .as_ref()
            .map(|a| a.letra.clone())
            .or_else(|| partes.get(2).cloned().filter(|l| !l.is_empty()))
            .or_else(|| texto("letter"))
            .unwrap_or_default();
        let no_acervo = achado.is_some();
        let titulo = achado.map(|a| a.titulo).unwrap_or(titulo);
        let nome_na_tela = texto("title").unwrap_or_else(|| titulo.clone());

        if serie {
            // Animes e doramas moram nas coleções, não nas letras.
            self.pedir_colecoes();
        }
        if !letra.is_empty() && self.letra != letra {
            self.abrir_letra(letra.clone());
        }
        self.remoto.vod = Some(VodDoCelular {
            titulo,
            serie,
            letra,
            url: texto("url").filter(|u| u.starts_with("http")),
            nome_na_tela,
            temporada,
            episodio,
            fonte: numero("fonte").map(|f| f.max(0.0) as usize),
            inicio: numero("startTime").filter(|s| *s > 0.0),
            desde: Instant::now(),
        });
        // Filme que o índice do acervo (já chegado) não conhece: não adianta
        // esperar a letra.
        if !serie && !no_acervo && !self.acervo.is_empty() {
            self.vod_do_celular_sem_acervo();
            return;
        }
        self.seguir_vod_do_celular();
    }

    /// Abre o filme ou episódio pedido assim que a letra (ou a coleção, ou os
    /// episódios da série) chegou; e, aberto, vai ao ponto pedido.
    fn seguir_vod_do_celular(&mut self) {
        if let Some((titulo, segundos)) = self.remoto.pulo.clone() {
            match self.tocando_vod.as_ref() {
                Some(t) if t.titulo == titulo && t.confirmado => {
                    self.remoto.pulo = None;
                    self.ir_para(segundos);
                }
                Some(t) if t.titulo == titulo => {}
                _ => self.remoto.pulo = None,
            }
        }
        let Some(pedido) = self.remoto.vod.as_ref() else { return };
        let venceu = pedido.desde.elapsed() > ESPERA_DO_ACERVO;
        let (titulo, serie) = (pedido.titulo.clone(), pedido.serie);
        let por_fonte = |mut urls: Vec<String>, fonte: Option<usize>| {
            if let Some(f) = fonte.filter(|f| *f > 0 && *f < urls.len()) {
                urls.rotate_left(f);
            }
            urls
        };

        if !serie {
            let filme = self.filmes.iter().find(|f| f.titulo == titulo).cloned();
            if let Some(f) = filme {
                let pedido = self.remoto.vod.take().unwrap();
                if let Some((_, urls)) = f.versoes.first().cloned() {
                    self.tocar_filme(f.titulo.clone(), por_fonte(urls, pedido.fonte));
                    self.remoto.pulo = pedido.inicio.map(|s| (f.titulo, s));
                }
                return;
            }
        } else {
            // Coleção (anime, dorama): os episódios já vêm junto.
            let da_colecao = self
                .colecoes
                .values()
                .flat_map(|l| l.iter())
                .find(|(s, _)| s.titulo == titulo)
                .map(|(s, eps)| (s.clone(), eps.clone()));
            let (serie_achada, episodios) = match da_colecao {
                Some((s, eps)) => (Some(s), Some(eps)),
                None => {
                    let s = self
                        .series
                        .iter()
                        .chain(self.series_da_letra.iter().flatten())
                        .find(|s| s.titulo == titulo)
                        .cloned();
                    let eps = (self.serie_aberta.as_ref().is_some_and(|a| a.titulo == titulo) && !self.episodios.is_empty())
                        .then(|| self.episodios.clone());
                    (s, eps)
                }
            };
            if let Some(s) = serie_achada {
                let Some(episodios) = episodios else {
                    // Os episódios vêm da letra da série: pede e espera.
                    if self.serie_aberta.as_ref().is_none_or(|a| a.titulo != titulo) {
                        self.abrir_serie(s);
                    }
                    if venceu {
                        self.vod_do_celular_sem_acervo();
                    }
                    return;
                };
                let pedido = self.remoto.vod.take().unwrap();
                let (Some(temporada), Some(numero)) = (pedido.temporada, pedido.episodio) else {
                    // Sem episódio escolhido: abre a página da série.
                    self.abrir_secao(Aba::Series);
                    self.foco_vod = 0;
                    self.abrir_serie(s);
                    return;
                };
                let candidatos: Vec<&vod::Episodio> =
                    episodios.iter().filter(|e| e.temporada == temporada && e.numero == numero).collect();
                let Some(ep) = candidatos.first().map(|e| (*e).clone()) else {
                    self.aviso = Some((format!("{titulo}: T{temporada} E{numero} não está no acervo"), Instant::now()));
                    return;
                };
                let mut ep = ep;
                ep.urls = por_fonte(ep.urls, pedido.fonte);
                let nome = format!("{} · T{} E{}", s.titulo, ep.temporada, ep.numero);
                self.tocar_episodio(&s.titulo, ep, episodios.clone());
                self.remoto.pulo = pedido.inicio.map(|segundos| (nome, segundos));
                return;
            }
        }
        if venceu || (self.remoto.vod.as_ref().is_some_and(|p| p.letra.is_empty()) && !self.carregando_vod) {
            self.vod_do_celular_sem_acervo();
        }
    }

    /// O acervo daqui não tem o título: toca o endereço que o celular mandou,
    /// como a TV Box faz.
    fn vod_do_celular_sem_acervo(&mut self) {
        let Some(pedido) = self.remoto.vod.take() else { return };
        match pedido.url {
            Some(url) => {
                self.fonte_preferida = None;
                self.tocar_vod(pedido.nome_na_tela.clone(), vec![url], 0, true);
                self.remoto.pulo = pedido.inicio.map(|s| (pedido.nome_na_tela, s));
            }
            None => {
                self.aviso = Some((format!("{} não foi encontrado no acervo", pedido.nome_na_tela), Instant::now()));
            }
        }
    }
}
