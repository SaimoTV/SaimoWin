//! As páginas em fileiras da TV Box: Início, On Demand, a página de cada
//! tipo, Eventos, Rádios e os resultados da busca.
//!
//! Todas têm o mesmo desenho (`activity_escolha.xml`, `Inicio.kt`,
//! `Destaque.kt`): em cima, o destaque — o título em foco, a linha com nota,
//! ano, temporadas e gêneros em ciano, a sinopse em duas linhas e a imagem
//! larga dele ao fundo, escurecida pela esquerda e por baixo; embaixo, as
//! fileiras, cada uma com o nome e os cartões correndo para o lado.
//!
//! Os cartões vêm em quatro formatos, como na TV: a capa (2:3), o largo
//! (16:9, canal e rádio com a logo), a categoria (colagem de capas com a cor
//! dela) e o jogo (escudos, selo e placar).

use super::{
    aceso, andamento, branco, degrade3, forte, fundo_com, icones, inicial, moldura_de_foco, normal, pintar_contido,
    pintar_cobrindo, selo_vivo, texto, Icone, ACENTO, APAGADO, ELEVADO, FUNDO, SECUNDARIO, TEXTO,
};
use crate::{catalogo, destaques, eventos, ficha, progresso, radios, Aba, App, Destino, ItemNaTela};
use eframe::egui::{self, Align2, Color32, Pos2, Rect, Sense, Stroke};
use std::rc::Rc;

// MARK: - O que vai nas fileiras

/// O que o destaque do topo mostra.
#[derive(Clone, PartialEq, Debug)]
pub enum Heroi {
    /// Um título do acervo: a ficha dele dá a linha, a sinopse e o fundo.
    Titulo { titulo: String, serie: bool },
    /// Qualquer outra coisa (canal, jogo, rádio, categoria), com o texto
    /// pronto e, se houver, o fundo emprestado de um título.
    Texto { titulo: String, meta: String, sinopse: String, fundo_de: Option<(String, bool)> },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tipo {
    Capa,
    Largo,
    Secao,
    Evento,
}

#[derive(Clone, Debug)]
pub enum Imagem {
    Nenhuma,
    /// O endereço da capa, já conhecido.
    Url(String),
    /// A capa do título, procurada pelo nome.
    Capa(String, bool),
    /// Logo de canal ou de rádio: inteira, sem cortar, no meio do cartão.
    Logo(String),
}

#[derive(Clone, Debug)]
pub enum Acao {
    Ficha(ItemNaTela),
    /// Um destaque publicado: `filas[f].itens[i]`.
    Destaque(usize, usize),
    /// Um título pela metade: segue direto, sem passar pela ficha.
    Continuar(ItemNaTela),
    Canal(String),
    Radio(usize),
    Pagina(Aba),
    /// A grade de A a Z de um tipo, com um gênero já escolhido (ou todos).
    Explorar(Aba, String),
}

#[derive(Clone, Debug)]
pub struct Cartao {
    pub titulo: String,
    pub sub: String,
    pub imagem: Imagem,
    pub progresso: Option<f32>,
    pub favorito: bool,
    pub acao: Acao,
    pub heroi: Heroi,
    /// O selo do canto: o texto e se é o vermelho do "ao vivo".
    pub selo: Option<(String, bool)>,
    /// Categoria: as capas da colagem, a cor e o ícone dela.
    pub colagem: Vec<String>,
    pub cor: Color32,
    pub icone: Option<Icone>,
    /// Jogo: escudo da casa, de fora e a logo da liga.
    pub escudos: [String; 3],
}

impl Cartao {
    fn novo(titulo: impl Into<String>, imagem: Imagem, acao: Acao, heroi: Heroi) -> Self {
        Cartao {
            titulo: titulo.into(),
            sub: String::new(),
            imagem,
            progresso: None,
            favorito: false,
            acao,
            heroi,
            selo: None,
            colagem: Vec::new(),
            cor: ACENTO,
            icone: None,
            escudos: Default::default(),
        }
    }

    /// O título do acervo por trás do cartão, para favoritar e abrir a ficha.
    fn obra(&self) -> Option<(String, bool)> {
        match &self.heroi {
            Heroi::Titulo { titulo, serie } => Some((titulo.clone(), *serie)),
            Heroi::Texto { .. } => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Fila {
    pub titulo: String,
    pub tipo: Tipo,
    pub cartoes: Vec<Cartao>,
}

/// As páginas desenhadas em fileiras (e não em grade).
pub fn em_fileiras(app: &App) -> bool {
    match app.aba {
        Aba::Inicio | Aba::OnDemand | Aba::Eventos | Aba::Radios | Aba::Buscar => true,
        aba if aba.de_tipo() => !app.explorar,
        _ => false,
    }
}

thread_local! {
    /// As fileiras montadas da última vez e a chave com que foram montadas:
    /// montar passa pelo acervo inteiro (os gêneros, os lançamentos).
    static FILAS: std::cell::RefCell<Option<(u64, Rc<Vec<Fila>>)>> = const { std::cell::RefCell::new(None) };
}

fn chave(app: &App) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (app.aba as u8).hash(&mut h);
    app.versao_dados.hash(&mut h);
    progresso::versao().hash(&mut h);
    app.busca_vod.hash(&mut h);
    app.favoritos_vod.hash(&mut h);
    app.favoritos.hash(&mut h);
    app.liberado.hash(&mut h);
    (app.filas.len(), app.acervo.len(), app.canais.len(), app.radios.len(), app.eventos.len()).hash(&mut h);
    app.colecoes_carregadas().hash(&mut h);
    // O que passa agora no canal e o grupo do jogo mudam com o relógio.
    if matches!(app.aba, Aba::Inicio | Aba::Eventos | Aba::Buscar) {
        (crate::epg::agora() / 60).hash(&mut h);
    }
    h.finish()
}

pub fn filas_da_pagina(app: &App) -> Rc<Vec<Fila>> {
    let chave = chave(app);
    if let Some(filas) = FILAS.with_borrow(|f| f.as_ref().filter(|(c, _)| *c == chave).map(|(_, f)| f.clone())) {
        return filas;
    }
    let filas: Vec<Fila> = match app.aba {
        Aba::Inicio => inicio(app),
        Aba::OnDemand => on_demand(app),
        Aba::Eventos => jogos(app),
        Aba::Radios => filas_de_radios(app),
        Aba::Buscar => busca(app),
        aba if aba.de_tipo() => do_tipo(app, aba),
        _ => Vec::new(),
    }
    .into_iter()
    .filter(|f| !f.cartoes.is_empty())
    .collect();
    let filas = Rc::new(filas);
    FILAS.with_borrow_mut(|f| *f = Some((chave, filas.clone())));
    filas
}

/// Quantos cartões cada fileira tem, para o teclado saber onde pode ir.
pub fn tamanho_das_filas(app: &App) -> Vec<usize> {
    filas_da_pagina(app).iter().map(|f| f.cartoes.len()).collect()
}

fn em_foco(app: &App) -> Option<Cartao> {
    filas_da_pagina(app).get(app.foco_fila)?.cartoes.get(app.foco_vod).cloned()
}

pub fn abrir_em_foco_na_fileira(app: &mut App) {
    if let Some(cartao) = em_foco(app) {
        executar(app, cartao.acao);
    }
}

pub fn favoritar_em_foco_na_fileira(app: &mut App) {
    if let Some((titulo, _)) = em_foco(app).and_then(|c| c.obra()) {
        app.favoritar_vod(titulo);
    }
}

pub fn ficha_em_foco_na_fileira(app: &mut App) {
    let Some(cartao) = em_foco(app) else { return };
    match cartao.acao {
        Acao::Ficha(item) | Acao::Continuar(item) => app.abrir_ficha(item.titulo.clone(), item.serie, Destino::Item(item)),
        Acao::Destaque(f, i) => {
            if let Some((titulo, serie)) = cartao.obra() {
                app.abrir_ficha(titulo, serie, Destino::Fileira(f, i));
            }
        }
        _ => {}
    }
}

/// Faz o que o cartão diz: o OK da TV.
pub(super) fn executar(app: &mut App, acao: Acao) {
    match acao {
        Acao::Ficha(item) => app.abrir_ficha(item.titulo.clone(), item.serie, Destino::Item(item)),
        Acao::Destaque(f, i) => {
            let Some(item) = app.filas.get(f).and_then(|fila| fila.itens.get(i)).cloned() else { return };
            let serie = item.serie();
            app.abrir_ficha(item.titulo, serie, Destino::Fileira(f, i));
        }
        Acao::Continuar(item) => app.abrir_item(item),
        Acao::Canal(nome) => app.abrir_jogo(&nome),
        Acao::Radio(indice) => {
            if let Some(radio) = app.radios.get(indice).cloned() {
                app.tocar_radio(radio);
            }
        }
        Acao::Pagina(aba) => app.abrir_secao(aba),
        Acao::Explorar(aba, genero) => {
            app.abrir_secao(aba);
            app.explorar = true;
            app.genero = genero;
            app.foco_vod = 0;
        }
    }
}

/// O caminho de um destaque publicado até o título: é o que o "Assistir" da
/// ficha de um destaque percorre. Anime e dorama moram nas coleções: abre-se
/// a página deles e a busca leva ao título, como quem chegou pela grade.
pub fn seguir_destaque(app: &mut App, fila: usize, indice: usize) {
    let Some(item) = app.filas.get(fila).and_then(|f| f.itens.get(indice)).cloned() else { return };
    if !item.da_colecao() {
        let alvo = ItemNaTela { titulo: item.titulo.clone(), detalhe: item.ano.clone(), serie: item.serie(), letra: item.letra.clone() };
        if !item.letra.is_empty() {
            app.abrir_item(alvo);
            return;
        }
    }
    let destino = if item.da_colecao() {
        if item.colecao() == "animes" { Aba::Animes } else { Aba::Doramas }
    } else if item.serie() {
        Aba::Series
    } else {
        Aba::Filmes
    };
    app.abrir_secao(destino);
    if !item.letra.is_empty() {
        app.abrir_letra(item.letra.clone());
    }
    app.explorar = true;
    app.genero.clear();
    app.busca_vod = item.titulo;
    app.foco_vod = 0;
}

// MARK: - Montagem das fileiras

fn heroi_titulo(titulo: &str, serie: bool) -> Heroi {
    Heroi::Titulo { titulo: titulo.to_string(), serie }
}

fn cartao_de_destaque(app: &App, f: usize, i: usize, item: &destaques::Item) -> Cartao {
    let imagem = if item.capa.is_empty() { Imagem::Capa(item.titulo.clone(), item.serie()) } else { Imagem::Url(item.capa.clone()) };
    let mut c = Cartao::novo(item.titulo.clone(), imagem, Acao::Destaque(f, i), heroi_titulo(&item.titulo, item.serie()));
    c.favorito = app.favoritos_vod.contains(&item.titulo);
    if !item.serie() {
        c.progresso = progresso::onde_parou(&item.titulo).map(|m| progresso::fracao(&m));
    }
    c
}

pub(super) fn cartao_de_titulo(app: &App, item: ItemNaTela, capa: Option<String>) -> Cartao {
    let imagem = match capa {
        Some(url) => Imagem::Url(url),
        None => Imagem::Capa(item.titulo.clone(), item.serie),
    };
    let heroi = heroi_titulo(&item.titulo, item.serie);
    let favorito = app.favoritos_vod.contains(&item.titulo);
    let progresso = if item.serie { None } else { progresso::onde_parou(&item.titulo).map(|m| progresso::fracao(&m)) };
    let mut c = Cartao::novo(item.titulo.clone(), imagem, Acao::Ficha(item), heroi);
    c.favorito = favorito;
    c.progresso = progresso;
    c
}

/// "Continue assistindo", só com o que existe no acervo comum: o título
/// reservado não pode reaparecer por aqui sem o código.
fn continuar(app: &App, filtro: impl Fn(bool) -> bool) -> Option<Fila> {
    let no_acervo: std::collections::HashMap<&str, &crate::vod::Achado> =
        app.acervo.iter().map(|a| (a.titulo.as_str(), a)).collect();
    let cartoes: Vec<Cartao> = progresso::pendentes()
        .into_iter()
        .filter_map(|(rotulo, marca)| {
            let nome = rotulo.split(" · ").next().unwrap_or(&rotulo).to_string();
            let achado = no_acervo.get(nome.as_str())?;
            if !filtro(achado.serie) {
                return None;
            }
            let item = ItemNaTela { titulo: nome.clone(), detalhe: achado.ano.clone(), serie: achado.serie, letra: achado.letra.clone() };
            let mut c = Cartao::novo(rotulo.clone(), Imagem::Capa(nome.clone(), achado.serie), Acao::Continuar(item), heroi_titulo(&nome, achado.serie));
            c.progresso = Some(progresso::fracao(&marca).max(0.02));
            c.sub = progresso::falta(&marca);
            c.favorito = app.favoritos_vod.contains(&nome);
            Some(c)
        })
        .take(20)
        .collect();
    Some(Fila { titulo: "Continue assistindo".into(), tipo: Tipo::Capa, cartoes })
}

/// Os canais com o que passa agora: o último assistido e os favoritos, ou os
/// da TV aberta para quem ainda não tem favorito.
fn ao_vivo_agora(app: &App) -> Option<Fila> {
    let comuns: Vec<&catalogo::Canal> = app.canais.iter().filter(|c| c.secao() != "Adulto").collect();
    if comuns.is_empty() {
        return None;
    }
    let ultimo = crate::ler_lista("ultimo-canal.txt").into_iter().next();
    let mut escolhidos: Vec<&catalogo::Canal> = Vec::new();
    if let Some(c) = ultimo.and_then(|u| comuns.iter().find(|c| c.nome == u).copied()) {
        escolhidos.push(c);
    }
    let favoritos: Vec<&catalogo::Canal> = comuns.iter().filter(|c| app.favoritos.contains(&c.nome)).copied().collect();
    let resto = if favoritos.is_empty() {
        comuns.iter().filter(|c| c.secao() == "TV Aberta").copied().collect()
    } else {
        favoritos
    };
    for c in resto {
        if !escolhidos.iter().any(|e| e.nome == c.nome) {
            escolhidos.push(c);
        }
    }
    let agora = crate::epg::agora();
    let cartoes = escolhidos.into_iter().take(16).map(|canal| cartao_de_canal(canal, agora)).collect();
    Some(Fila { titulo: "Ao vivo agora".into(), tipo: Tipo::Largo, cartoes })
}

fn cartao_de_canal(canal: &catalogo::Canal, agora: i64) -> Cartao {
    let programa = crate::epg::agora_e_depois(&canal.nome).map(|(p, _)| p);
    let imagem = canal.logo.clone().map(Imagem::Logo).unwrap_or(Imagem::Nenhuma);
    let heroi = Heroi::Texto {
        titulo: canal.nome.clone(),
        meta: programa.as_ref().map(|p| format!("Agora: {}", p.titulo)).unwrap_or_else(|| canal.secao().to_string()),
        sinopse: programa
            .as_ref()
            .map(|p| p.descricao.clone())
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| "Clique ou Enter para assistir ao vivo".into()),
        fundo_de: None,
    };
    let mut c = Cartao::novo(canal.nome.clone(), imagem, Acao::Canal(canal.nome.clone()), heroi);
    c.sub = programa.as_ref().map(|p| p.titulo.clone()).unwrap_or_default();
    c.progresso = programa.map(|p| p.andamento(agora));
    c.selo = Some(("AO VIVO".into(), true));
    c
}

fn favoritos_vod(app: &App) -> Option<Fila> {
    let por_nome: std::collections::HashMap<&str, &crate::vod::Achado> =
        app.acervo.iter().map(|a| (a.titulo.as_str(), a)).collect();
    let cartoes = app
        .favoritos_vod
        .iter()
        .map(|titulo| {
            let achado = por_nome.get(titulo.as_str());
            let item = ItemNaTela {
                titulo: titulo.clone(),
                detalhe: achado.map(|a| a.ano.clone()).unwrap_or_default(),
                serie: achado.map(|a| a.serie).unwrap_or(false),
                letra: achado.map(|a| a.letra.clone()).unwrap_or_default(),
            };
            cartao_de_titulo(app, item, None)
        })
        .collect();
    Some(Fila { titulo: "Favoritos".into(), tipo: Tipo::Capa, cartoes })
}

fn inicio(app: &App) -> Vec<Fila> {
    let mut filas: Vec<Fila> = [continuar(app, |_| true), ao_vivo_agora(app), favoritos_vod(app)].into_iter().flatten().collect();
    for (f, fila) in app.filas.iter().enumerate() {
        let cartoes = fila.itens.iter().enumerate().map(|(i, item)| cartao_de_destaque(app, f, i, item)).collect();
        filas.push(Fila { titulo: fila.titulo.clone(), tipo: Tipo::Capa, cartoes });
    }
    filas
}

/// A cor, o ícone e o texto de cada categoria, como os do On Demand da TV Box.
pub(super) fn categoria(aba: Aba) -> (&'static str, char, Color32, Icone, &'static str) {
    match aba {
        Aba::Series => ("Séries", 's', Color32::from_rgb(122, 162, 255), Icone::Episodios,
            "Temporadas completas, com o próximo episódio emendando sozinho."),
        Aba::Animes => ("Animes", 'a', Color32::from_rgb(255, 180, 84), Icone::Brilho,
            "Os animes do momento e os que marcaram época."),
        Aba::Doramas => ("Doramas", 'd', Color32::from_rgb(255, 126, 182), Icone::Coracao,
            "Romance, drama e suspense da Coreia, do Japão e da China."),
        _ => ("Filmes", 'f', ACENTO, Icone::Filmes, "Lançamentos, clássicos e tudo entre eles, dublados e legendados."),
    }
}

fn numero(n: usize) -> String {
    // 30000 vira "30.000", como no Brasil.
    let texto = n.to_string();
    let mut saida = String::new();
    for (i, c) in texto.chars().enumerate() {
        if i > 0 && (texto.len() - i).is_multiple_of(3) {
            saida.push('.');
        }
        saida.push(c);
    }
    saida
}

/// Os quatro cartões de categoria do On Demand (ou do "Explorar" no fim).
fn categorias(app: &App, explorar: bool) -> Fila {
    let mut cartoes = Vec::new();
    for aba in [Aba::Filmes, Aba::Series, Aba::Animes, Aba::Doramas] {
        let (nome, letra, cor, icone, sobre) = categoria(aba);
        let em_alta: Vec<&destaques::Item> =
            app.filas.iter().flat_map(|f| &f.itens).filter(|i| i.tipo == letra && !i.capa.is_empty()).collect();
        let mut colagem: Vec<String> = Vec::new();
        for item in &em_alta {
            if !colagem.contains(&item.capa) {
                colagem.push(item.capa.clone());
            }
            if colagem.len() == 4 {
                break;
            }
        }
        let quantos = match aba {
            Aba::Filmes => app.acervo.iter().filter(|a| !a.serie).count(),
            Aba::Series => app.acervo.iter().filter(|a| a.serie).count(),
            _ => app.colecao(aba).map(|l| l.len()).unwrap_or(0),
        };
        let sub = if quantos > 0 { format!("{} títulos", numero(quantos)) } else { String::new() };
        let titulo = if explorar { format!("{nome} de A a Z") } else { nome.to_string() };
        let fundo_de = em_alta.first().map(|i| (i.titulo.clone(), i.serie()));
        let heroi = Heroi::Texto { titulo: titulo.clone(), meta: sub.clone(), sinopse: sobre.into(), fundo_de };
        let acao = if explorar { Acao::Explorar(aba, String::new()) } else { Acao::Pagina(aba) };
        let mut c = Cartao::novo(titulo, Imagem::Nenhuma, acao, heroi);
        c.sub = sub;
        c.colagem = colagem;
        c.cor = cor;
        c.icone = Some(icone);
        cartoes.push(c);
    }
    Fila { titulo: if explorar { "Explorar".into() } else { "Categorias".into() }, tipo: Tipo::Secao, cartoes }
}

/// Um título do acervo que tem capa, com o ano para ordenar.
struct Candidato {
    item: ItemNaTela,
    ano: u32,
    capa: String,
}

fn ano_de(titulo: &str, ano: &str) -> u32 {
    let maximo = chrono::Datelike::year(&chrono::Local::now()) as u32 + 1;
    ano.get(..4)
        .and_then(|a| a.parse::<u32>().ok())
        .or_else(|| {
            let t = titulo.trim_end();
            t.ends_with(')').then_some(t.len()).filter(|n| *n >= 6).and_then(|n| t.get(n - 5..n - 1)?.parse().ok())
        })
        .filter(|a| (1900..=maximo).contains(a))
        .unwrap_or(0)
}

/// Só títulos com capa: fileira cheia de quadrados vazios não convida ninguém.
fn candidatos(app: &App, aba: Aba) -> Vec<Candidato> {
    if matches!(aba, Aba::Animes | Aba::Doramas) {
        return app
            .colecao(aba)
            .map(|lista| {
                lista
                    .iter()
                    .filter_map(|(s, _)| {
                        let capa = app.endereco_da_capa(&s.titulo, true)?;
                        Some(Candidato {
                            ano: ano_de(&s.titulo, &s.ano),
                            item: ItemNaTela { titulo: s.titulo.clone(), detalhe: s.ano.clone(), serie: true, letra: app.letra.clone() },
                            capa,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
    }
    let serie = aba == Aba::Series;
    app.acervo
        .iter()
        .filter(|a| a.serie == serie && crate::tem_nome(&a.titulo))
        .filter_map(|a| {
            let capa = app.generos.capa(&a.titulo, a.serie)?.to_string();
            Some(Candidato {
                ano: ano_de(&a.titulo, &a.ano),
                item: ItemNaTela { titulo: a.titulo.clone(), detalhe: a.ano.clone(), serie: a.serie, letra: a.letra.clone() },
                capa,
            })
        })
        .collect()
}

/// Os gêneros mais procurados primeiro, como na TV Box.
const ORDEM_DOS_GENEROS: [&str; 26] = [
    "Ação", "Comédia", "Drama", "Terror", "Thriller", "Suspense", "Ficção científica", "Animação", "Romance", "Aventura",
    "Crime", "Família", "Fantasia", "Mistério", "Documentário", "Guerra", "Faroeste", "História", "Música",
    "Action & Adventure", "Sci-Fi & Fantasy", "Kids", "Reality", "Soap", "Talk", "War & Politics",
];

/// No On Demand, só os gêneros que leem bem como "Filmes de ...".
const GENEROS_DO_ON_DEMAND: [&str; 11] =
    ["Ação", "Comédia", "Terror", "Ficção científica", "Romance", "Suspense", "Animação", "Aventura", "Drama", "Crime", "Fantasia"];

fn por_genero(app: &App, candidatos: &[Candidato], genero: &str, aba: Aba, titulo: String) -> Option<Fila> {
    let mut lista: Vec<&Candidato> = candidatos.iter().filter(|c| app.generos.tem(&c.item.titulo, c.item.serie, genero)).collect();
    if lista.len() < 6 {
        return None;
    }
    lista.sort_by_key(|c| std::cmp::Reverse(c.ano));
    let mut cartoes: Vec<Cartao> = lista.into_iter().take(25).map(|c| cartao_de_titulo(app, c.item.clone(), Some(c.capa.clone()))).collect();
    cartoes.push(ver_todos(aba, genero));
    Some(Fila { titulo, tipo: Tipo::Capa, cartoes })
}

/// O último cartão da fileira de gênero: a grade inteira daquele gênero.
fn ver_todos(aba: Aba, genero: &str) -> Cartao {
    let heroi = Heroi::Texto {
        titulo: format!("{genero}: ver todos"),
        meta: categoria(aba).0.to_string(),
        sinopse: "Todos os títulos deste gênero, do mais novo para o mais antigo, com busca e ordem de A a Z.".into(),
        fundo_de: None,
    };
    let mut c = Cartao::novo("Ver todos", Imagem::Nenhuma, Acao::Explorar(aba, genero.to_string()), heroi);
    c.icone = Some(Icone::Direita);
    c
}

fn on_demand(app: &App) -> Vec<Fila> {
    let mut filas = vec![categorias(app, false)];
    filas.extend(continuar(app, |_| true));
    for (f, fila) in app.filas.iter().enumerate() {
        if fila.itens.len() >= 4 {
            let cartoes = fila.itens.iter().enumerate().map(|(i, item)| cartao_de_destaque(app, f, i, item)).collect();
            filas.push(Fila { titulo: fila.titulo.clone(), tipo: Tipo::Capa, cartoes });
        }
    }
    let filmes = candidatos(app, Aba::Filmes);
    for genero in GENEROS_DO_ON_DEMAND.iter().filter(|g| app.generos.todos.iter().any(|t| t == *g)) {
        let titulo = format!("Filmes de {}", genero.to_lowercase());
        filas.extend(por_genero(app, &filmes, genero, Aba::Filmes, titulo));
    }
    filas.push(categorias(app, true));
    filas
}

fn do_tipo(app: &App, aba: Aba) -> Vec<Fila> {
    let (_, letra, ..) = categoria(aba);
    let mut filas = Vec::new();
    filas.extend(continuar(app, |serie| (aba == Aba::Filmes) != serie));
    for (f, fila) in app.filas.iter().enumerate() {
        let do_tipo: Vec<Cartao> = fila
            .itens
            .iter()
            .enumerate()
            .filter(|(_, item)| item.tipo == letra)
            .map(|(i, item)| cartao_de_destaque(app, f, i, item))
            .collect();
        if do_tipo.len() >= 4 {
            filas.push(Fila { titulo: fila.titulo.clone(), tipo: Tipo::Capa, cartoes: do_tipo });
        }
    }
    let candidatos = candidatos(app, aba);
    let este_ano = chrono::Datelike::year(&chrono::Local::now()) as u32;
    let mut novos: Vec<&Candidato> = candidatos.iter().filter(|c| c.ano + 1 >= este_ano).collect();
    novos.sort_by_key(|c| std::cmp::Reverse(c.ano));
    if novos.len() >= 4 {
        let cartoes = novos.into_iter().take(30).map(|c| cartao_de_titulo(app, c.item.clone(), Some(c.capa.clone()))).collect();
        filas.push(Fila { titulo: "Lançamentos".into(), tipo: Tipo::Capa, cartoes });
    }
    let mut generos = app.generos.todos.clone();
    generos.sort_by_key(|g| ORDEM_DOS_GENEROS.iter().position(|o| o == g).unwrap_or(99));
    for genero in generos {
        filas.extend(por_genero(app, &candidatos, &genero, aba, genero.clone()));
    }
    // O fim da página: o acervo inteiro do tipo, de A a Z.
    let (nome, _, cor, icone, sobre) = categoria(aba);
    let colagem: Vec<String> = candidatos.iter().take(4).map(|c| c.capa.clone()).collect();
    let heroi = Heroi::Texto { titulo: format!("{nome} de A a Z"), meta: format!("{} títulos", numero(candidatos.len().max(1))), sinopse: sobre.into(), fundo_de: None };
    let mut todos = Cartao::novo("Todos de A a Z", Imagem::Nenhuma, Acao::Explorar(aba, String::new()), heroi);
    todos.colagem = colagem;
    todos.cor = cor;
    todos.icone = Some(icone);
    filas.push(Fila { titulo: "Explorar".into(), tipo: Tipo::Secao, cartoes: vec![todos] });
    filas
}

fn jogos(app: &App) -> Vec<Fila> {
    let agora = eventos::agora();
    app.jogos_na_tela()
        .into_iter()
        .map(|(grupo, lista)| {
            let cartoes = lista
                .into_iter()
                .map(|jogo| {
                    let vivo = jogo.ao_vivo(agora);
                    let selo = if vivo {
                        ["AO VIVO", jogo.placar.as_str()].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join("  ")
                    } else {
                        eventos::quando(jogo.inicio, agora)
                    };
                    let titulo = format!("{} x {}", jogo.casa, jogo.fora);
                    let meta = [jogo.liga.as_str(), selo.as_str(), if vivo { jogo.tempo.as_str() } else { "" }]
                        .iter()
                        .filter(|t| !t.is_empty())
                        .copied()
                        .collect::<Vec<_>>()
                        .join("  ·  ");
                    let heroi = Heroi::Texto {
                        titulo: titulo.clone(),
                        meta,
                        sinopse: format!("No canal {} · clique ou Enter assiste", jogo.canal),
                        fundo_de: None,
                    };
                    let logo = jogo.canal_logo.clone().map(Imagem::Logo).unwrap_or(Imagem::Nenhuma);
                    let mut c = Cartao::novo(titulo, logo, Acao::Canal(jogo.canal.clone()), heroi);
                    c.sub = [jogo.canal.as_str(), jogo.liga.as_str()].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join(" · ");
                    c.selo = Some((selo, vivo));
                    c.escudos = [jogo.casa_logo.clone(), jogo.fora_logo.clone(), jogo.liga_logo.clone()];
                    c
                })
                .collect();
            Fila { titulo: eventos::GRUPOS[grupo].to_string(), tipo: Tipo::Evento, cartoes }
        })
        .collect()
}

fn cartao_de_radio(indice: usize, radio: &radios::Radio) -> Cartao {
    let estilo = radios::estilo_de(&radio.nome);
    let heroi = Heroi::Texto {
        titulo: radio.nome.clone(),
        meta: ["Rádio ao vivo", estilo.unwrap_or("")].iter().filter(|t| !t.is_empty()).copied().collect::<Vec<_>>().join("  ·  "),
        sinopse: "Clique ou Enter para ouvir · a rádio continua tocando enquanto você navega".into(),
        fundo_de: None,
    };
    let imagem = radio.logo.clone().map(Imagem::Logo).unwrap_or(Imagem::Nenhuma);
    let mut c = Cartao::novo(radio.nome.clone(), imagem, Acao::Radio(indice), heroi);
    c.sub = estilo.unwrap_or("Rádio").to_string();
    c
}

fn filas_de_radios(app: &App) -> Vec<Fila> {
    let mut filas = Vec::new();
    let recentes: Vec<Cartao> = radios::recentes()
        .iter()
        .filter_map(|nome| app.radios.iter().position(|r| r.nome == *nome))
        .map(|i| cartao_de_radio(i, &app.radios[i]))
        .collect();
    filas.push(Fila { titulo: "Ouvidas por último".into(), tipo: Tipo::Largo, cartoes: recentes });
    for (estilo, _) in radios::ESTILOS {
        let do_estilo: Vec<Cartao> = app
            .radios
            .iter()
            .enumerate()
            .filter(|(_, r)| radios::estilo_de(&r.nome) == Some(estilo))
            .map(|(i, r)| cartao_de_radio(i, r))
            .collect();
        if do_estilo.len() >= 4 {
            filas.push(Fila { titulo: estilo.to_string(), tipo: Tipo::Largo, cartoes: do_estilo });
        }
    }
    let mut letras: std::collections::BTreeMap<char, Vec<Cartao>> = std::collections::BTreeMap::new();
    for (i, r) in app.radios.iter().enumerate() {
        letras.entry(radios::letra_de(&r.nome)).or_default().push(cartao_de_radio(i, r));
    }
    for (letra, cartoes) in letras {
        let titulo = if letra == '#' { "0 a 9".to_string() } else { letra.to_string() };
        filas.push(Fila { titulo, tipo: Tipo::Largo, cartoes });
    }
    filas
}

/// A busca da TV Box: canais, filmes e séries, enquanto se digita.
fn busca(app: &App) -> Vec<Fila> {
    let termo = catalogo::chave_de_ordem(app.busca_vod.trim());
    if termo.chars().count() < 2 {
        return Vec::new();
    }
    let agora = crate::epg::agora();
    let canais: Vec<Cartao> = app
        .canais
        .iter()
        .filter(|c| catalogo::chave_de_ordem(&c.nome).contains(&termo))
        .take(30)
        .map(|c| cartao_de_canal(c, agora))
        .collect();
    let achados = |serie: bool| -> Vec<Cartao> {
        let mut lista: Vec<&crate::vod::Achado> = app
            .acervo
            .iter()
            .filter(|a| a.serie == serie && crate::tem_nome(&a.titulo))
            .filter(|a| catalogo::chave_de_ordem(&a.titulo).contains(&termo))
            .collect();
        // Quem começa com o que foi digitado vem antes de quem só contém.
        lista.sort_by_key(|a| (!catalogo::chave_de_ordem(&a.titulo).starts_with(&termo), std::cmp::Reverse(ano_de(&a.titulo, &a.ano))));
        lista
            .into_iter()
            .take(40)
            .map(|a| {
                let item = ItemNaTela { titulo: a.titulo.clone(), detalhe: a.ano.clone(), serie: a.serie, letra: a.letra.clone() };
                cartao_de_titulo(app, item, None)
            })
            .collect()
    };
    let colecoes: Vec<Cartao> = app
        .filas
        .iter()
        .enumerate()
        .flat_map(|(f, fila)| fila.itens.iter().enumerate().map(move |(i, item)| (f, i, item)))
        .filter(|(_, _, item)| item.da_colecao() && catalogo::chave_de_ordem(&item.titulo).contains(&termo))
        .map(|(f, i, item)| cartao_de_destaque(app, f, i, item))
        .fold(Vec::new(), |mut lista: Vec<Cartao>, c| {
            if !lista.iter().any(|o| o.titulo == c.titulo) {
                lista.push(c);
            }
            lista
        });
    vec![
        Fila { titulo: "Canais".into(), tipo: Tipo::Largo, cartoes: canais },
        Fila { titulo: "Filmes".into(), tipo: Tipo::Capa, cartoes: achados(false) },
        Fila { titulo: "Séries".into(), tipo: Tipo::Capa, cartoes: achados(true) },
        Fila { titulo: "Animes e doramas".into(), tipo: Tipo::Capa, cartoes: colecoes },
    ]
}

// MARK: - Desenho

/// Largura e altura do cartão inteiro (com a moldura de foco) e da imagem.
pub(super) fn medidas(tipo: Tipo) -> (egui::Vec2, egui::Vec2) {
    match tipo {
        Tipo::Capa => (egui::vec2(152.0, 268.0), egui::vec2(140.0, 210.0)),
        Tipo::Largo => (egui::vec2(276.0, 207.0), egui::vec2(264.0, 148.0)),
        Tipo::Secao => (egui::vec2(352.0, 212.0), egui::vec2(340.0, 200.0)),
        Tipo::Evento => (egui::vec2(340.0, 229.0), egui::vec2(328.0, 170.0)),
    }
}

pub(super) const ESPACO: f32 = 10.0;
const TITULO_DA_FILA: f32 = 40.0;
pub(super) const MARGEM: f32 = 50.0;

fn altura_da_fila(tipo: Tipo) -> f32 {
    TITULO_DA_FILA + medidas(tipo).0.y + 12.0
}

/// A página inteira: fundo do destaque, o texto dele e as fileiras.
pub fn pagina(app: &mut App, ui: &mut egui::Ui) {
    let area = ui.max_rect();
    let filas = filas_da_pagina(app);
    let andou = ui.input(|i| {
        i.key_pressed(egui::Key::ArrowDown) || i.key_pressed(egui::Key::ArrowUp)
            || i.key_pressed(egui::Key::ArrowLeft) || i.key_pressed(egui::Key::ArrowRight)
    }) && !ui.memory(|m| m.focused().is_some());
    if !filas.is_empty() {
        app.foco_fila = app.foco_fila.min(filas.len() - 1);
        let na_fila = filas[app.foco_fila].cartoes.len();
        app.foco_vod = app.foco_vod.min(na_fila.saturating_sub(1));
        if andou {
            app.heroi = filas[app.foco_fila].cartoes.get(app.foco_vod).map(|c| c.heroi.clone());
        }
    }

    let buscando = app.aba == Aba::Buscar;
    let altura_do_destaque = if buscando { 128.0 } else { (area.height() * 0.30).clamp(150.0, 230.0) };
    destaque(app, ui, area, altura_do_destaque);

    let lista = Rect::from_min_max(Pos2::new(area.left(), area.top() + altura_do_destaque), area.max);
    if filas.is_empty() {
        let aviso = if buscando {
            if app.busca_vod.trim().chars().count() < 2 {
                "Digite o nome — os resultados aparecem enquanto você digita."
            } else {
                "Nada com esse nome. Confira a grafia ou tente uma parte do nome."
            }
        } else if app.aba == Aba::Eventos && app.eventos_carregados && app.eventos_erro {
            "Não deu para carregar a agenda agora. Tentando de novo em um minuto."
        } else if app.aba == Aba::Eventos && app.eventos_carregados {
            "Nenhum jogo nos canais da lista agora. A agenda se atualiza sozinha."
        } else if app.aba == Aba::Radios && app.radios.is_empty() && !app.buscando_radios() {
            "Sem rádios por enquanto. Confira a internet e abra de novo."
        } else {
            "Carregando…"
        };
        ui.painter().text(lista.center() - egui::vec2(0.0, 40.0), Align2::CENTER_CENTER, aviso, normal(17.0), SECUNDARIO);
        return;
    }
    fileiras(app, ui, lista, &filas, andou);
}

/// O destaque: a imagem larga ao fundo e o texto do título em foco.
fn destaque(app: &mut App, ui: &mut egui::Ui, area: Rect, altura: f32) {
    let heroi = app.heroi.clone();
    let (titulo, meta, estrela, sinopse, fundo_de) = texto_do_destaque(app, heroi.as_ref());
    if let Some((t, s)) = fundo_de.as_ref() {
        app.pedir_ficha_do_destaque(t, *s);
    }
    let fundo = fundo_de.as_ref().and_then(|(t, s)| ficha::conhecida(t, *s)).and_then(|f| f.fundo);
    pintar_fundo_do_destaque(app, ui, area, fundo);

    let x = area.left() + MARGEM + 6.0;
    let largura = (area.width() * 0.55).clamp(420.0, 820.0);
    let pintor = ui.painter().clone();
    if app.aba == Aba::Buscar {
        let campo = Rect::from_min_size(Pos2::new(x, area.top() + 26.0), egui::vec2(largura.min(area.width() - 2.0 * MARGEM), 56.0));
        ui.scope_builder(egui::UiBuilder::new().max_rect(campo), |ui| {
            let resposta = super::campo_de_busca(ui, &mut app.busca_vod, "Buscar filmes, séries, canais…", "busca-acervo", campo.width(), 56.0);
            if app.pedir_foco_na_busca {
                app.pedir_foco_na_busca = false;
                resposta.request_focus();
            }
            if resposta.changed() {
                app.foco_fila = 0;
                app.foco_vod = 0;
                app.heroi = None;
            }
        });
        pintor.text(
            Pos2::new(x, campo.bottom() + 16.0),
            Align2::LEFT_TOP,
            "Com o teclado, é só digitar. Clique num resultado para abrir; Esc volta ao ao vivo.",
            normal(13.5),
            SECUNDARIO,
        );
        return;
    }

    let mut y = area.top() + 18.0;
    y += texto(&pintor, &titulo, forte(38.0), TEXTO, Pos2::new(x, y), largura, 1, false) + 4.0;
    if !meta.is_empty() {
        let mut mx = x;
        if estrela {
            icones::pintar(&pintor, Icone::Estrela, Pos2::new(mx + 10.0, y + 11.0), 20.0, ACENTO);
            mx += 24.0;
        }
        y += texto(&pintor, &meta, normal(17.0), ACENTO, Pos2::new(mx, y), largura, 1, false) + 8.0;
    }
    if !sinopse.is_empty() {
        texto(&pintor, &sinopse, normal(16.0), branco(217), Pos2::new(x, y), largura, 2, false);
    }
    // A rádio que está tocando, à direita do destaque das Rádios.
    if app.aba == Aba::Radios {
        if let Some(radio) = app.radio.as_ref().map(|r| (r.nome.clone(), r.logo.clone(), r.confirmado)) {
            tocando_agora(app, ui, Rect::from_min_max(Pos2::new(area.right() - 360.0, area.top() + 20.0), Pos2::new(area.right() - MARGEM, area.top() + altura - 20.0)), radio);
        }
    }
}

/// O cartão da rádio no ar: logo, nome, equalizador e o botão de parar.
fn tocando_agora(app: &mut App, ui: &mut egui::Ui, area: Rect, (nome, logo, confirmado): (String, Option<String>, bool)) {
    if area.width() < 200.0 || area.height() < 100.0 {
        return;
    }
    let pintor = ui.painter().clone();
    pintor.rect_filled(area, 22.0, super::VIDRO);
    pintor.rect_stroke(area, 22.0, Stroke::new(1.0, branco(31)), egui::StrokeKind::Inside);
    let lado = (area.height() - 36.0).min(110.0);
    let quadro = Rect::from_min_size(area.left_top() + egui::vec2(18.0, (area.height() - lado) / 2.0), egui::vec2(lado, lado));
    pintor.rect_filled(quadro, lado / 3.0, branco(31));
    match logo.and_then(|u| app.imagem(&u, 256)) {
        Some(t) => pintar_contido(ui, &t, quadro.shrink(10.0)),
        None => {
            pintor.text(quadro.center(), Align2::CENTER_CENTER, inicial(&nome), forte(lado * 0.4), ACENTO);
        }
    }
    let x = quadro.right() + 16.0;
    let largura = area.right() - x - 16.0;
    pintor.text(Pos2::new(x, quadro.top() + 4.0), Align2::LEFT_TOP, if confirmado { "TOCANDO AGORA" } else { "SINTONIZANDO…" }, forte(12.0), ACENTO);
    texto(&pintor, &nome, forte(19.0), TEXTO, Pos2::new(x, quadro.top() + 24.0), largura, 2, false);
    super::topo::equalizador(ui, Rect::from_min_size(Pos2::new(x, quadro.bottom() - 34.0), egui::vec2(28.0, 22.0)), 5);
    let botao = Rect::from_min_size(Pos2::new(area.right() - 60.0, quadro.bottom() - 44.0), egui::vec2(44.0, 44.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(botao), |ui| {
        if super::botao_redondo(ui, Icone::Parar, 44.0, "Parar a rádio", false).clicked() {
            app.parar_radio();
        }
    });
}

/// Título, linha de cima, se a linha leva a estrela, sinopse e de quem é o fundo.
#[allow(clippy::type_complexity)]
fn texto_do_destaque(app: &App, heroi: Option<&Heroi>) -> (String, String, bool, String, Option<(String, bool)>) {
    match heroi {
        Some(Heroi::Titulo { titulo, serie }) => {
            let nome = crate::generos::sem_ano(titulo);
            match ficha::conhecida(titulo, *serie) {
                Some(f) => {
                    let temporadas = f.temporadas.map(|n| if n == 1 { "1 temporada".to_string() } else { format!("{n} temporadas") });
                    let generos = f.generos.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
                    let partes: Vec<String> = [
                        (f.nota > 0.0).then(|| format!("{:.1}", f.nota)),
                        (!f.ano.is_empty()).then(|| f.ano.clone()),
                        temporadas,
                        (!generos.is_empty()).then_some(generos),
                    ]
                    .into_iter()
                    .flatten()
                    .collect();
                    (nome, partes.join("  ·  "), f.nota > 0.0, f.sinopse.clone(), Some((titulo.clone(), *serie)))
                }
                None => (nome, if *serie { "Série".into() } else { "Filme".into() }, false, String::new(), Some((titulo.clone(), *serie))),
            }
        }
        Some(Heroi::Texto { titulo, meta, sinopse, fundo_de }) => (titulo.clone(), meta.clone(), false, sinopse.clone(), fundo_de.clone()),
        None => padrao(app),
    }
}

/// O destaque antes de qualquer cartão em foco.
#[allow(clippy::type_complexity)]
fn padrao(app: &App) -> (String, String, bool, String, Option<(String, bool)>) {
    let (titulo, meta, sinopse) = match app.aba {
        Aba::Inicio => {
            let hora = chrono::Timelike::hour(&chrono::Local::now());
            let saudacao = match hora {
                5..=11 => "Bom dia",
                12..=17 => "Boa tarde",
                _ => "Boa noite",
            };
            (saudacao.to_string(), "O que vamos assistir?".to_string(),
                "Clique num título para abrir · botão direito favorita · as setas também andam pelas fileiras".to_string())
        }
        Aba::OnDemand => {
            let total = app.acervo.len() + app.colecao(Aba::Animes).map(|l| l.len()).unwrap_or(0) + app.colecao(Aba::Doramas).map(|l| l.len()).unwrap_or(0);
            ("On Demand".into(), if total > 0 { format!("{} títulos", numero(total)) } else { "Carregando…".into() },
                "Filmes, séries, animes e doramas, dublados e legendados.".into())
        }
        Aba::Eventos => ("Eventos".into(), eventos::resumo(&app.jogos_na_tela()), "Os jogos nos canais da lista, com o placar ao vivo. A agenda se atualiza a cada três minutos.".into()),
        Aba::Radios => {
            let n = app.radios.len();
            ("Rádios".into(), if n > 0 { format!("{n} rádios ao vivo") } else { "Carregando…".into() },
                "Clique numa rádio para ouvir; ela continua tocando enquanto você navega pelo app.".into())
        }
        aba => {
            let (nome, .., sobre) = categoria(aba);
            (nome.to_string(), String::new(), sobre.to_string())
        }
    };
    (titulo, meta, false, sinopse, None)
}

thread_local! {
    /// O fundo do destaque que está à vista e o que estava antes, para a troca
    /// se dissolver em vez de piscar.
    static FUNDOS: std::cell::RefCell<(Option<String>, Option<String>, f64)> = const { std::cell::RefCell::new((None, None, 0.0)) };
}

fn pintar_fundo_do_destaque(app: &mut App, ui: &egui::Ui, area: Rect, fundo: Option<String>) {
    let agora = ui.input(|i| i.time);
    let (atual, anterior, desde) = FUNDOS.with_borrow_mut(|f| {
        if f.0 != fundo {
            f.1 = f.0.take();
            f.0 = fundo.clone();
            f.2 = agora;
        }
        f.clone()
    });
    let t = ((agora - desde) / 0.35).clamp(0.0, 1.0) as f32;
    if t < 1.0 {
        ui.ctx().request_repaint();
    }
    let pintor = ui.painter();
    let mut pintou = false;
    for (url, opacidade) in [(anterior, 1.0 - t), (atual, t)] {
        let Some(url) = url else { continue };
        if opacidade <= 0.01 {
            continue;
        }
        if let Some(textura) = app.imagem(&url, 1280) {
            pintar_cobrindo(ui, &textura, area, 0.0, 0.3, opacidade);
            pintou = true;
        }
    }
    if pintou {
        // `bg_destaque_lado` e `bg_destaque_baixo`.
        degrade3(pintor, area, fundo_com(255), fundo_com(204), 0.45, fundo_com(0), true);
        let baixo = Rect::from_min_max(Pos2::new(area.left(), area.top() + area.height() * 0.15), area.max);
        degrade3(pintor, baixo, fundo_com(0), fundo_com(230), 0.55, fundo_com(255), false);
    }
}

/// As fileiras, só as à vista, e em cada uma só os cartões à vista.
fn fileiras(app: &mut App, ui: &mut egui::Ui, area: Rect, filas: &Rc<Vec<Fila>>, andou: bool) {
    let alturas: Vec<f32> = filas.iter().map(|f| altura_da_fila(f.tipo)).collect();
    let mut topos = Vec::with_capacity(filas.len());
    let mut y = 0.0;
    for a in &alturas {
        topos.push(y);
        y += a;
    }
    let total = y + 120.0;
    let mut escolhido: Option<Acao> = None;
    let mut favoritar: Option<String> = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(area), |ui| {
        egui::ScrollArea::vertical()
            .id_salt(("fileiras", app.aba as u8))
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, vista| {
                let largura = ui.available_width();
                ui.set_min_size(egui::vec2(largura, total));
                let origem = ui.min_rect().min;
                let lugar = |i: usize| Rect::from_min_size(origem + egui::vec2(0.0, topos[i]), egui::vec2(largura, alturas[i]));
                if andou && app.foco_fila < filas.len() {
                    // A fileira em foco sobe para o topo da lista, como na TV.
                    let r = lugar(app.foco_fila);
                    ui.scroll_to_rect(Rect::from_min_size(r.min, egui::vec2(r.width(), r.height().min(vista.height()))), Some(egui::Align::Min));
                }
                for (i, fila) in filas.iter().enumerate() {
                    if topos[i] + alturas[i] < vista.min.y || topos[i] > vista.max.y {
                        continue;
                    }
                    let r = lugar(i);
                    ui.scope_builder(egui::UiBuilder::new().max_rect(r), |ui| {
                        uma_fileira(app, ui, i, fila, andou, &mut escolhido, &mut favoritar);
                    });
                }
            });
    });
    if let Some(titulo) = favoritar {
        app.favoritar_vod(titulo);
        return;
    }
    if let Some(acao) = escolhido {
        executar(app, acao);
    }
}

fn uma_fileira(
    app: &mut App,
    ui: &mut egui::Ui,
    indice_fila: usize,
    fila: &Fila,
    andou: bool,
    escolhido: &mut Option<Acao>,
    favoritar: &mut Option<String>,
) {
    let r = ui.max_rect();
    ui.painter().text(Pos2::new(r.left() + MARGEM + 6.0, r.top() + 12.0), Align2::LEFT_TOP, &fila.titulo, forte(21.0), TEXTO);
    let (moldura, _) = medidas(fila.tipo);
    let passo = moldura.x + ESPACO;
    let faixa = Rect::from_min_max(Pos2::new(r.left(), r.top() + TITULO_DA_FILA), Pos2::new(r.right(), r.top() + TITULO_DA_FILA + moldura.y + 8.0));
    let id = egui::Id::new(("fila", app.aba as u8, indice_fila, &fila.titulo));
    let empurrar: f32 = ui.ctx().data_mut(|d| d.remove_temp::<f32>(id.with("empurrar")).unwrap_or(0.0));
    let foco = app.foco_vod;
    let mut saida = None;
    ui.scope_builder(egui::UiBuilder::new().max_rect(faixa), |ui| {
        saida = Some(
            egui::ScrollArea::horizontal()
                .id_salt(id)
                .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                .show_viewport(ui, |ui, vista| {
                    let conteudo = MARGEM + fila.cartoes.len() as f32 * passo + MARGEM;
                    ui.set_min_size(egui::vec2(conteudo, moldura.y));
                    if empurrar != 0.0 {
                        ui.scroll_with_delta_animation(egui::vec2(-empurrar, 0.0), egui::style::ScrollAnimation::duration(0.35));
                    }
                    let origem = ui.min_rect().min + egui::vec2(MARGEM, 4.0);
                    let lugar = |k: usize| Rect::from_min_size(origem + egui::vec2(k as f32 * passo, 0.0), moldura);
                    if andou && indice_fila == app.foco_fila && foco < fila.cartoes.len() {
                        ui.scroll_to_rect(lugar(foco).expand2(egui::vec2(MARGEM, 0.0)), None);
                    }
                    let primeiro = ((vista.min.x - MARGEM) / passo).floor().max(0.0) as usize;
                    let ultimo = (((vista.max.x - MARGEM) / passo).ceil().max(0.0) as usize + 1).min(fila.cartoes.len());
                    for k in primeiro..ultimo {
                        let cartao = &fila.cartoes[k];
                        let focado = indice_fila == app.foco_fila && k == foco;
                        let resposta = desenhar_cartao(app, ui, lugar(k), cartao, fila.tipo, focado, id.with(k));
                        if resposta.hovered() && ui.input(|i| i.pointer.delta() != egui::Vec2::ZERO) {
                            app.foco_fila = indice_fila;
                            app.foco_vod = k;
                            app.heroi = Some(cartao.heroi.clone());
                        }
                        if resposta.clicked() {
                            app.foco_fila = indice_fila;
                            app.foco_vod = k;
                            *escolhido = Some(cartao.acao.clone());
                        }
                        if resposta.secondary_clicked() {
                            if let Some((titulo, _)) = cartao.obra() {
                                *favoritar = Some(titulo);
                            }
                        }
                    }
                }),
        );
    });
    // As setas das pontas, para quem está no mouse: cada clique anda quase
    // uma tela de cartões.
    let Some(saida) = saida else { return };
    let sobre = ui.rect_contains_pointer(faixa);
    let deslocado = saida.state.offset.x;
    let cabe = saida.content_size.x - saida.inner_rect.width();
    for (esquerda, visivel) in [(true, deslocado > 1.0), (false, deslocado < cabe - 1.0)] {
        let t = aceso(ui, id.with(("seta", esquerda)), sobre && visivel);
        if t <= 0.01 {
            continue;
        }
        let centro = if esquerda {
            Pos2::new(faixa.left() + 26.0, faixa.center().y - 18.0)
        } else {
            Pos2::new(faixa.right() - 26.0, faixa.center().y - 18.0)
        };
        let alvo = Rect::from_center_size(centro, egui::vec2(44.0, 44.0));
        let resposta = ui.interact(alvo, id.with(("botao-seta", esquerda)), Sense::click());
        let claro = aceso(ui, resposta.id, resposta.hovered());
        ui.painter().circle_filled(centro, 22.0, fundo_com((220.0 * t) as u8).lerp_to_gamma(Color32::WHITE.gamma_multiply(t), claro));
        ui.painter().circle_stroke(centro, 22.0, Stroke::new(1.0, branco((50.0 * t) as u8)));
        let tinta = TEXTO.lerp_to_gamma(FUNDO, claro).gamma_multiply(t);
        let icone = if esquerda { Icone::Esquerda } else { Icone::Direita };
        icones::pintar(ui.painter(), icone, centro, 26.0, tinta);
        if resposta.clicked() {
            let delta = (saida.inner_rect.width() * 0.8) * if esquerda { -1.0 } else { 1.0 };
            ui.ctx().data_mut(|d| d.insert_temp(id.with("empurrar"), delta));
        }
    }
}

/// Um cartão, em qualquer dos quatro formatos.
pub(super) fn desenhar_cartao(app: &mut App, ui: &mut egui::Ui, rect: Rect, cartao: &Cartao, tipo: Tipo, focado: bool, id: egui::Id) -> egui::Response {
    let resposta = ui.interact(rect, id, Sense::click());
    let t = aceso(ui, id.with("aceso"), focado || resposta.hovered());
    let pintor = ui.painter().clone();
    moldura_de_foco(&pintor, rect, t, 14.0);
    let (_, imagem) = medidas(tipo);
    let quadro = Rect::from_min_size(rect.min + egui::vec2(6.0, 6.0), imagem);
    match tipo {
        Tipo::Secao => secao(app, ui, quadro, cartao),
        Tipo::Evento => jogo(app, ui, quadro, cartao),
        _ => imagem_do_cartao(app, ui, quadro, cartao, tipo),
    }
    if tipo != Tipo::Secao {
        let x = quadro.left();
        let y = quadro.bottom() + 7.0;
        if tipo == Tipo::Capa {
            texto(&pintor, &cartao.titulo, normal(14.5), TEXTO, Pos2::new(x, y), quadro.width(), 2, false);
        } else {
            texto(&pintor, &cartao.titulo, forte(15.5), TEXTO, Pos2::new(x, y), quadro.width(), 1, false);
            if !cartao.sub.is_empty() {
                texto(&pintor, &cartao.sub, normal(13.5), SECUNDARIO, Pos2::new(x, y + 21.0), quadro.width(), 1, false);
            }
        }
    }
    let dica = match &cartao.acao {
        Acao::Canal(_) => "Clique ou Enter assiste ao vivo",
        Acao::Radio(_) => "Clique ou Enter para ouvir",
        Acao::Pagina(_) | Acao::Explorar(..) => "Clique ou Enter abre",
        _ if cartao.favorito => "Clique abre a ficha · botão direito ou S tira dos favoritos",
        _ => "Clique abre a ficha · botão direito ou S favorita",
    };
    resposta.on_hover_text_at_pointer(dica).on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn imagem_do_cartao(app: &mut App, ui: &egui::Ui, quadro: Rect, cartao: &Cartao, tipo: Tipo) {
    let pintor = ui.painter();
    pintor.rect_filled(quadro, 12.0, ELEVADO);
    let textura = match &cartao.imagem {
        Imagem::Url(url) => app.imagem(url, 342),
        Imagem::Capa(titulo, serie) => app.capa(titulo, *serie),
        Imagem::Logo(url) => app.imagem(url, 256),
        Imagem::Nenhuma => None,
    };
    match (textura, &cartao.imagem) {
        (Some(t), Imagem::Logo(_)) => pintar_contido(ui, &t, quadro.shrink2(egui::vec2(quadro.width() * 0.2, quadro.height() * 0.17))),
        (Some(t), _) => pintar_cobrindo(ui, &t, quadro, 12.0, 0.5, 1.0),
        (None, _) => {
            let marca = match cartao.icone {
                Some(icone) => {
                    icones::pintar(pintor, icone, quadro.center(), 44.0, ACENTO);
                    return;
                }
                None => inicial(&cartao.titulo),
            };
            pintor.text(quadro.center(), Align2::CENTER_CENTER, marca, forte(40.0), ACENTO);
        }
    }
    if let Some((selo, vivo)) = cartao.selo.as_ref() {
        selo_vivo(pintor, quadro.min + egui::vec2(10.0, 10.0), selo, *vivo, 11.5);
    }
    if cartao.favorito && tipo == Tipo::Capa {
        let centro = quadro.right_top() + egui::vec2(-18.0, 18.0);
        pintor.circle_filled(centro, 14.0, Color32::from_black_alpha(160));
        icones::pintar(pintor, Icone::Estrela, centro, 18.0, super::AMARELO);
    }
    if let Some(fracao) = cartao.progresso.filter(|f| *f > 0.0) {
        let trilho = Rect::from_min_size(Pos2::new(quadro.left() + 8.0, quadro.bottom() - 10.0), egui::vec2(quadro.width() - 16.0, 4.0));
        andamento(pintor, trilho, fracao);
    }
}

/// O cartão de categoria: colagem de capas, a cor vinda do canto, o véu e o
/// nome com o ícone redondo (`item_secao.xml`).
fn secao(app: &mut App, ui: &egui::Ui, quadro: Rect, cartao: &Cartao) {
    let pintor = ui.painter().clone();
    pintor.rect_filled(quadro, 12.0, ELEVADO);
    let n = cartao.colagem.len().max(1);
    let largura = quadro.width() / n as f32;
    for (k, url) in cartao.colagem.iter().enumerate() {
        let parte = Rect::from_min_size(quadro.min + egui::vec2(k as f32 * largura, 0.0), egui::vec2(largura, quadro.height()));
        if let Some(t) = app.imagem(url, 342) {
            let uv = super::uv_cobrindo(t.size_vec2(), parte.size(), 0.3);
            // Só as pontas da colagem levam os cantos redondos do cartão.
            let cantos = egui::CornerRadius {
                nw: if k == 0 { 12 } else { 0 },
                sw: if k == 0 { 12 } else { 0 },
                ne: if k + 1 == n { 12 } else { 0 },
                se: if k + 1 == n { 12 } else { 0 },
            };
            egui::Image::new(&t).uv(uv).corner_radius(cantos).paint_at(ui, parte);
        }
    }
    // A cor da categoria entrando pelo canto de baixo.
    let mut malha = egui::Mesh::default();
    let cor = cartao.cor;
    malha.colored_vertex(quadro.left_bottom(), cor.gamma_multiply(0.7));
    malha.colored_vertex(quadro.left_top(), cor.gamma_multiply(0.25));
    malha.colored_vertex(quadro.right_top(), Color32::TRANSPARENT);
    malha.colored_vertex(quadro.right_bottom(), cor.gamma_multiply(0.25));
    malha.add_triangle(0, 1, 2);
    malha.add_triangle(0, 2, 3);
    pintor.add(egui::Shape::mesh(malha));
    degrade3(&pintor, quadro, fundo_com(26), fundo_com(153), 0.5, fundo_com(242), false);

    let centro = Pos2::new(quadro.left() + 40.0, quadro.bottom() - 40.0);
    pintor.circle_filled(centro, 22.0, cor);
    if let Some(icone) = cartao.icone {
        icones::pintar(&pintor, icone, centro, 24.0, FUNDO);
    }
    let x = centro.x + 34.0;
    let largura = quadro.right() - x - 12.0;
    if cartao.sub.is_empty() {
        texto_centrado(&pintor, &cartao.titulo, forte(22.0), Pos2::new(x, centro.y), largura);
    } else {
        texto(&pintor, &cartao.titulo, forte(22.0), TEXTO, Pos2::new(x, centro.y - 26.0), largura, 1, false);
        texto(&pintor, &cartao.sub, normal(14.0), SECUNDARIO, Pos2::new(x, centro.y + 3.0), largura, 1, false);
    }
}

fn texto_centrado(pintor: &egui::Painter, conteudo: &str, fonte: egui::FontId, esquerda_meio: Pos2, largura: f32) {
    super::texto_numa_linha(pintor, conteudo, fonte, TEXTO, esquerda_meio, largura);
}

/// O cartão do jogo (`item_evento_cartao.xml`): selo, logo da liga e os
/// escudos com o "x" entre eles.
fn jogo(app: &mut App, ui: &egui::Ui, quadro: Rect, cartao: &Cartao) {
    let pintor = ui.painter();
    pintor.rect_filled(quadro, 12.0, ELEVADO);
    let [casa, fora, liga] = &cartao.escudos;
    let partes: Vec<&str> = cartao.titulo.splitn(2, " x ").collect();
    for (k, url) in [casa, fora].iter().enumerate() {
        let cx = quadro.left() + quadro.width() * if k == 0 { 0.28 } else { 0.72 };
        let area = Rect::from_center_size(Pos2::new(cx, quadro.center().y + 8.0), egui::vec2(78.0, 78.0));
        match (!url.is_empty()).then(|| app.imagem(url, 128)).flatten() {
            Some(t) => pintar_contido(ui, &t, area),
            None => {
                pintor.circle_filled(area.center(), 34.0, branco(26));
                let nome = partes.get(k).copied().unwrap_or("");
                pintor.text(area.center(), Align2::CENTER_CENTER, inicial(nome), forte(26.0), SECUNDARIO);
            }
        }
    }
    pintor.text(Pos2::new(quadro.center().x, quadro.center().y + 8.0), Align2::CENTER_CENTER, "x", forte(26.0), APAGADO);
    if !liga.is_empty() {
        if let Some(t) = app.imagem(liga, 64) {
            pintar_contido(ui, &t, Rect::from_center_size(quadro.right_top() + egui::vec2(-26.0, 24.0), egui::vec2(30.0, 30.0)));
        }
    }
    if let Some((selo, vivo)) = cartao.selo.as_ref() {
        selo_vivo(pintor, quadro.min + egui::vec2(10.0, 10.0), selo, *vivo, 12.5);
    }
    // A logo do canal onde passa, no pé do cartão.
    if let Imagem::Logo(url) = &cartao.imagem {
        if let Some(t) = app.imagem(url, 128) {
            pintar_contido(ui, &t, Rect::from_min_size(quadro.left_bottom() + egui::vec2(12.0, -36.0), egui::vec2(48.0, 26.0)));
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn numero_com_ponto_de_milhar() {
        assert_eq!(numero(7), "7");
        assert_eq!(numero(1234), "1.234");
        assert_eq!(numero(30000), "30.000");
        assert_eq!(numero(1234567), "1.234.567");
    }

    #[test]
    fn ano_do_titulo_ou_do_campo() {
        assert_eq!(ano_de("Duna", "2021"), 2021);
        assert_eq!(ano_de("Duna (2021)", ""), 2021);
        assert_eq!(ano_de("Blade Runner 2049", ""), 0);
    }
}
