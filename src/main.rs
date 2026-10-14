//! Saimo TV para Windows.
//!
//! Aplicativo nativo: a janela, a lista e o letreiro são desenhados pela GPU
//! (egui sobre OpenGL) e o vídeo é do mpv, que desenha no mesmo framebuffer.
//! Não há navegador nem página HTML em lugar nenhum.
//!
//! A lista é a mesma dos outros aplicativos, baixada do repositório publicado.

// Sem console preto atrás da janela quando alguém abre pelo Explorer.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod abertura;
mod atualizacao;
mod fontes_desativadas;
mod fonte_preferida;
mod destaques;
mod generos;
mod ficha;
mod catalogo;
mod mpv;
mod progresso;
mod pulos;
mod radios;
mod legendas;
mod midia;
mod bateria;
mod rede;
mod telemetria;

use catalogo::Canal;
use eframe::egui;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

pub const VERSAO: &str = env!("CARGO_PKG_VERSION");
pub const AGENTE: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0 Safari/537.36";
/// Código que revela os canais restritos, o mesmo dos outros aplicativos.
const CODIGO: &str = "1010";
/// Uma fonte que não abre neste tempo está fora: vai para a próxima.
const ESPERA_DA_FONTE: Duration = Duration::from_secs(14);
const SUMIR_LETREIRO: Duration = Duration::from_secs(6);

fn main() -> eframe::Result<()> {
    let mut janela = egui::ViewportBuilder::default()
        .with_title("Saimo TV")
        .with_inner_size([1280.0, 720.0])
        .with_min_inner_size([800.0, 480.0]);
    if std::env::var("SAIMO_FOTO").is_ok() {
        // Foto de teste: abre sem tomar o foco de quem está usando a máquina.
        janela = janela.with_active(false);
    }
    let opcoes = eframe::NativeOptions {
        viewport: janela,
        ..Default::default()
    };
    eframe::run_native("Saimo TV", opcoes, Box::new(|cc| Ok(Box::new(App::novo(cc)))))
}

/// Recados que chegam das threads de fundo.
enum Recado {
    Catalogo(Vec<Canal>),
    Restritos(Vec<Canal>),
    /// A agenda com os placares; `None` quando a agenda não veio.
    Eventos(Option<Vec<eventos::Jogo>>),
    Logo(String, egui::ColorImage),
    /// O pedido da imagem saiu da fila antes de ser baixado: pode ser pedido de novo.
    LogoEsquecido(String),
    Atualizacao(atualizacao::Versao),
    Guia,
    Gavetas(Vec<vod::Gaveta>),
    Filmes(String, Vec<vod::Filme>),
    Series(String, Vec<vod::Serie>),
    Colecao(Aba, Vec<(vod::Serie, Vec<vod::Episodio>)>),
    Destaques(Vec<destaques::Fila>),
    Generos(generos::Generos),
    Episodios(String, Vec<vod::Episodio>),
    Acervo(Vec<vod::Achado>),
    /// A lista de servidores desligados mudou: a lista de canais muda junto.
    FontesDesativadas,
    /// A ficha de um título chegou: sinopse, duração, gêneros e elenco.
    Ficha(String, bool, Option<ficha::Ficha>),
    /// Os trabalhos de um ator, como o TMDB os devolve. O cruzamento com o
    /// acervo é feito na thread do desenho, que é onde o índice está.
    Filmografia(u32, Vec<ficha::Credito>),
    /// As legendas do OpenSubtitles para o título que está tocando.
    Legendas(String, Vec<legendas::Opcao>),
    /// O arquivo de uma legenda foi baixado (ou não).
    LegendaPronta(String, legendas::Opcao, Option<std::path::PathBuf>),
    /// Quem é o ator aberto: foto, biografia, de onde é.
    Perfil(u32, Option<ficha::Perfil>),
    /// Onde pular no que está tocando, pelo TheIntroDB.
    Marcas(String, pulos::Marcas),
    /// A lista de rádios publicada.
    Radios(Vec<radios::Radio>),
    /// A ficha do título em destaque chegou (fica guardada no `ficha`).
    FichaDoDestaque,
    /// Os episódios de uma temporada, pelo TMDB (ficam guardados no `ficha`).
    Temporada,
    /// A procura pedida nos Ajustes não achou versão nova.
    SemAtualizacao,
}

/// O que ocupa a área principal: o vídeo, ou uma das seções do acervo —
/// como no Mac, onde o acervo abre por cima do player.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Aba {
    Canais,
    /// A porta de entrada do acervo: fileiras de capa, no lugar da grade.
    Inicio,
    Filmes,
    Series,
    Animes,
    Doramas,
    Favoritos,
    Extras,
    Eventos,
    /// As quatro categorias juntas, como o On Demand da TV Box.
    OnDemand,
    Radios,
    Buscar,
    Ajustes,
}

impl Aba {
    /// As páginas de um tipo do acervo, que abrem em fileiras e têm a grade
    /// de A a Z por dentro.
    pub fn de_tipo(self) -> bool {
        matches!(self, Aba::Filmes | Aba::Series | Aba::Animes | Aba::Doramas)
    }

    /// As seções que mostram filmes (e não séries).
    pub fn de_filmes(self) -> bool {
        matches!(self, Aba::Filmes | Aba::Favoritos | Aba::Extras)
    }
}

/// Um cartão da grade: o nome, o que vai escrito embaixo dele, e a letra onde
/// mora o endereço — que só é baixada quando alguém abre o título.
#[derive(Clone, Debug)]
pub struct ItemNaTela {
    pub titulo: String,
    pub detalhe: String,
    pub serie: bool,
    pub letra: String,
}

/// Quantas capas grandes ficam na memória: bem mais que as de uma tela
/// cheia, e ainda assim uns 200 MB no máximo.
const MAXIMO_DE_CAPAS: usize = 280;

/// De quanto em quanto tempo a tela de Eventos busca a agenda de novo.
const ATUALIZAR_EVENTOS: Duration = Duration::from_secs(60);

/// Para onde o "Assistir" da ficha leva.
///
/// Um título da grade vai pelo caminho de sempre (`abrir_item`); um destaque
/// da tela inicial, pelo dele — que sabe achar anime e dorama nas coleções.
#[derive(Clone)]
pub enum Destino {
    Item(ItemNaTela),
    /// Um destaque publicado: a fila e a posição dele em `filas`.
    Fileira(usize, usize),
}

/// O que está tocando quando não é canal: filme ou episódio.
pub struct TocandoVod {
    pub titulo: String,
    pub urls: Vec<String>,
    pub fonte: usize,
    pub desde: Instant,
    pub confirmado: bool,
    /// Quando é episódio: a série, o número e a lista para achar o seguinte.
    pub episodio: Option<EpisodioTocando>,
    /// Abertura, recapitulação e créditos, quando o TheIntroDB conhece.
    pub marcas: Option<pulos::Marcas>,
    /// Legendas do OpenSubtitles, a escolhida (com o arquivo baixado) e o atraso.
    pub legendas: Vec<legendas::Opcao>,
    pub legenda_escolhida: Option<(legendas::Opcao, String)>,
    pub legenda_atraso: f64,
    pub legenda_aviso: Option<String>,
    /// A última posição vista (posição, duração): no fim o mpv já não diz.
    pub visto: (f64, f64),
    /// O cartão do próximo episódio: quando subiu, e se foi dispensado.
    pub proximo_desde: Option<Instant>,
    pub proximo_dispensado: bool,
}

#[derive(Clone)]
pub struct EpisodioTocando {
    pub serie: String,
    pub temporada: u32,
    pub numero: u32,
    pub versao: String,
    pub lista: Vec<vod::Episodio>,
}

impl EpisodioTocando {
    /// O episódio seguinte, de preferência na mesma versão (dublado/legendado).
    pub fn seguinte(&self) -> Option<vod::Episodio> {
        let agora = (self.temporada, self.numero);
        let chave = self
            .lista
            .iter()
            .map(|e| (e.temporada, e.numero))
            .filter(|k| *k > agora)
            .min()?;
        let candidatos: Vec<&vod::Episodio> =
            self.lista.iter().filter(|e| (e.temporada, e.numero) == chave).collect();
        candidatos
            .iter()
            .find(|e| e.versao == self.versao)
            .or_else(|| candidatos.first())
            .map(|e| (*e).clone())
    }
}

/// Quanto tempo o cartão do próximo episódio espera antes de seguir sozinho.
pub const ESPERA_DO_PROXIMO: Duration = Duration::from_secs(10);

/// A rádio no ar: toca no mesmo mpv do vídeo, só com o som.
pub struct TocandoRadio {
    pub nome: String,
    pub url: String,
    pub logo: Option<String>,
    pub desde: Instant,
    pub confirmado: bool,
}

pub struct Tocando {
    pub canal: usize,
    pub fonte: usize,
    pub desde: Instant,
    pub confirmado: bool,
}

struct App {
    /// A fonte em que a série no ar vem tocando; vale só para esta sessão.
    fonte_preferida: Option<fonte_preferida::Memoria>,
    mpv: Option<std::sync::Arc<mpv::Mpv>>,
    erro_do_mpv: Option<String>,
    canais: Vec<Canal>,
    restritos: Vec<Canal>,
    liberado: bool,
    favoritos: Vec<String>,
    busca: String,
    foco: usize,
    tocando: Option<Tocando>,
    lista_aberta: bool,
    letreiro_ate: Instant,
    digitado: String,
    digitado_em: Instant,
    volume: i32,
    mudo: bool,
    pausado: bool,
    logos: HashMap<String, Option<egui::TextureHandle>>,
    /// Chave no mapa, endereço e o lado máximo em pixels.
    pedidos_de_logo: Vec<(String, String, u32)>,
    /// As capas grandes na ordem em que chegaram, para soltar as mais antigas.
    ordem_das_capas: std::collections::VecDeque<String>,
    recados: Receiver<Recado>,
    emissor: Sender<Recado>,
    nova_versao: Option<atualizacao::Versao>,
    ultima_checagem: Instant,
    aviso: Option<(String, Instant)>,
    aba: Aba,
    pip: bool,
    guia_aberto: bool,
    /// A lista publicada como ela chegou, sem nada peneirado: é dela que a
    /// lista visível é remontada quando um servidor é religado.
    publicados: Vec<Canal>,
    gavetas: Vec<vod::Gaveta>,
    letra: String,
    filmes: Vec<vod::Filme>,
    series: Vec<vod::Serie>,
    serie_aberta: Option<vod::Serie>,
    episodios: Vec<vod::Episodio>,
    episodios_colecao: HashMap<String, Vec<vod::Episodio>>,
    /// As fileiras publicadas, baixadas uma vez por abertura do programa.
    filas: Vec<destaques::Fila>,
    /// Os gêneros publicados, e o escolhido na régua (vazio = todos).
    generos: generos::Generos,
    genero: String,
    /// Em qual fileira o teclado está. A coluna é o `foco_vod` de sempre.
    foco_fila: usize,
    carregando_vod: bool,
    /// Todo o acervo só com nome, tipo e letra: é dele que a grade se serve.
    acervo: Vec<vod::Achado>,
    /// O título que espera a letra dele terminar de chegar para abrir.
    abrir_ao_chegar: Option<(String, bool)>,
    favoritos_vod: Vec<String>,
    /// Busca do acervo, separada da busca de canais da barra lateral.
    busca_vod: String,
    quadro: u32,
    ultimo_pulo: Option<Instant>,
    /// Colunas da grade na última pintura, para as setas andarem por linha.
    pub colunas_vod: usize,
    ultimo_progresso: Instant,
    foco_vod: usize,
    tocando_vod: Option<TocandoVod>,
    /// Até quando a barra de controles fica à vista; mexer o mouse renova.
    controles_ate: Instant,
    ajuda_aberta: bool,
    velocidade: f32,
    preencher: bool,
    ultimo_mouse: Option<egui::Pos2>,
    /// Já voltou ao último canal nesta abertura: a lista recarrega e não pode
    /// arrancar a pessoa do que ela escolheu depois.
    retomou_canal: bool,
    /// As fileiras já foram pedidas: Filmes e Séries também as usam.
    carregando_filas: bool,
    /// O título cuja ficha está aberta: nome, se é série e o detalhe do cartão.
    pub ficha_aberta: Option<(String, bool)>,
    /// Para onde o "Assistir" da ficha aberta leva.
    pub ficha_destino: Option<Destino>,
    /// A ficha já baixada do título aberto, quando chegou.
    pub ficha: Option<ficha::Ficha>,
    /// Enquanto o pedido não volta, a janela mostra que está a caminho.
    pub ficha_carregando: bool,
    /// O ator aberto dentro da ficha, e o que ele tem no acervo.
    pub ficha_ator: Option<(u32, String)>,
    /// A foto pequena do elenco, que já está carregada: aparece na hora.
    pub ficha_ator_foto: Option<String>,
    pub perfil: Option<ficha::Perfil>,
    pub filmografia: Vec<ficha::Trabalho>,
    pub filmografia_carregando: bool,
    pub eventos: Vec<eventos::Jogo>,
    pub eventos_carregados: bool,
    /// A última busca da agenda falhou: a tela diz, e tenta de novo.
    pub eventos_erro: bool,
    /// Quando a agenda foi pedida da última vez, e se ainda está a caminho.
    eventos_pedidos: Option<Instant>,
    buscando_eventos: bool,
    /// Sobe a cada recado com dados novos (acervo, letra, gêneros, fileiras):
    /// é parte da chave dos caches da grade e das fileiras.
    pub versao_dados: u64,
    /// A grade montada da última vez e a chave com que foi montada. Montar
    /// filtra e ordena o acervo inteiro (trinta mil nomes); feito a cada
    /// quadro, deixava o acervo arrastado, e com um canal tocando por trás
    /// eram sessenta vezes por segundo.
    cache_itens: std::cell::RefCell<Option<(u64, std::rc::Rc<Vec<ItemNaTela>>)>>,
    /// As capas pedidas e ainda não baixadas, as mais novas no fim.
    fila_de_imagens: std::sync::Arc<FilaDeImagens>,
    /// O canal que estava no ar quando a pessoa foi para o acervo: o acervo
    /// cobre a tela inteira e o canal parava de tocar por trás; na volta ao
    /// "Ao vivo" ele recomeça.
    canal_parado: Option<String>,
    /// Multi tela: a tela que não está em foco. A em foco é sempre `mpv` e
    /// `tocando`, então teclado, lista, fontes e faixa valem para ela sem
    /// mudar nada; trocar o foco é trocar as duas de lugar.
    pub outra_tela: Option<OutraTela>,
    /// Na multi tela, a tela em foco é a da direita.
    pub foco_a_direita: bool,
    /// O mpv da segunda tela entre um uso e outro: subir um mpv custa.
    mpv_reserva: Option<std::sync::Arc<mpv::Mpv>>,
    /// Animes e doramas já baixados nesta abertura: voltar à seção não
    /// baixa a coleção de novo.
    colecoes: HashMap<u8, Vec<(vod::Serie, Vec<vod::Episodio>)>>,
    /// As séries da letra, guardadas enquanto uma coleção ocupa `series`.
    /// Sem isto, ir a Animes e voltar a Séries deixava a lista da coleção no
    /// lugar, e abrir uma série ficava esperando uma letra que nunca vinha.
    series_da_letra: Option<Vec<vod::Serie>>,
    /// O quadro de "tocando agora" do Windows.
    midia: midia::Midia,
    /// Na página de um tipo, a grade de A a Z está aberta (no lugar das fileiras).
    pub explorar: bool,
    /// A grade em ordem alfabética, em vez dos mais novos primeiro.
    pub ordem_az: bool,
    /// O que o destaque do topo das páginas mostra: o cartão em foco.
    pub heroi: Option<tela::Heroi>,
    /// Fichas já pedidas para o destaque, para não pedir duas vezes.
    fichas_pedidas: std::collections::HashSet<String>,
    /// Temporadas já pedidas ao TMDB (id, número).
    temporadas_pedidas: std::collections::HashSet<(u32, u32)>,
    pub radios: Vec<radios::Radio>,
    buscando_radios: bool,
    pub radio: Option<TocandoRadio>,
    colecoes_pedidas: bool,
    /// No player, o painel de episódios aberto.
    pub episodios_no_player: bool,
    /// Na página da série, a temporada escolhida.
    pub temporada_aberta: u32,
    /// O campo da busca pede o teclado no próximo quadro.
    pub pedir_foco_na_busca: bool,
    /// A abertura animada, enquanto está na tela.
    abertura: Option<abertura::Abertura>,
}

/// A segunda tela da multi tela: outro mpv, outro canal, sem som.
pub struct OutraTela {
    pub mpv: std::sync::Arc<mpv::Mpv>,
    pub tocando: Option<Tocando>,
}

/// Os pedidos de imagem esperando um dos poucos baixadores.
///
/// Era uma thread por capa: rolando a grade, centenas baixavam e
/// decodificavam ao mesmo tempo e a interface engasgava. Agora são poucas, e
/// a última capa pedida sai primeiro — é a que está na tela; a que ficou para
/// trás na rolagem é esquecida e volta a ser pedida se reaparecer.
#[derive(Default)]
struct FilaDeImagens {
    pedidos: std::sync::Mutex<std::collections::VecDeque<(String, String, u32)>>,
    chegou: std::sync::Condvar,
}

/// Quantas imagens baixam e decodificam ao mesmo tempo.
const BAIXADORES_DE_IMAGEM: usize = 6;
/// Pedidos além disto são os mais velhos, de capas que já saíram da tela.
const MAXIMO_NA_FILA_DE_IMAGENS: usize = 120;

impl App {
    fn novo(cc: &eframe::CreationContext<'_>) -> Self {
        let (emissor, recados) = channel();
        tela::aplicar_estilo(&cc.egui_ctx);

        let (mpv, erro_do_mpv) = match mpv::Mpv::novo(&caminho_do_mpv()) {
            Ok(m) => (Some(std::sync::Arc::new(m)), None),
            Err(e) => (None, Some(e)),
        };

        telemetria::iniciar();

        let canais = catalogo::em_cache("catalogo.txt");
        let restritos = catalogo::em_cache("restritos.txt");
        let favoritos = ler_favoritos();

        // Lista publicada e versão nova, sem segurar a abertura.
        for (arquivo, monta) in [
            ("catalogo.txt", Recado::Catalogo as fn(Vec<Canal>) -> Recado),
            ("restritos.txt", Recado::Restritos as fn(Vec<Canal>) -> Recado),
        ] {
            let emissor = emissor.clone();
            std::thread::spawn(move || {
                if let Some(lista) = catalogo::baixar(arquivo) {
                    let _ = emissor.send(monta(lista));
                }
            });
        }
        {
            let emissor = emissor.clone();
            std::thread::spawn(move || {
                if let Some(v) = atualizacao::procurar() {
                    let _ = emissor.send(Recado::Atualizacao(v));
                }
            });
        }
        {
            // Os servidores desligados no painel, de dois em dois minutos: é o
            // tempo que alguém aguenta um canal quebrado.
            let emissor = emissor.clone();
            std::thread::spawn(move || loop {
                if fontes_desativadas::atualizar() {
                    let _ = emissor.send(Recado::FontesDesativadas);
                }
                std::thread::sleep(std::time::Duration::from_secs(120));
            });
        }
        {
            // O índice do acervo é pequeno e dá as bases dos endereços; sem ele
            // nenhum filme abre.
            let emissor = emissor.clone();
            std::thread::spawn(move || {
                let gavetas = vod::indice();
                if !gavetas.is_empty() {
                    let _ = emissor.send(Recado::Gavetas(gavetas));
                }
                // O índice de busca são uns 800 KB para trinta mil títulos:
                // procurar no acervo inteiro sem baixar o acervo.
                let acervo = vod::busca();
                if !acervo.is_empty() {
                    let _ = emissor.send(Recado::Acervo(acervo));
                }
                // Os gêneros vêm do mesmo lugar, e só servem para a régua do
                // acervo: podem chegar depois de tudo.
                let lidos = generos::baixar();
                if !lidos.vazio() {
                    let _ = emissor.send(Recado::Generos(lidos));
                }
            });
        }

        let mut app = App {
            mpv,
            erro_do_mpv,
            canais: canais.clone(),
            restritos,
            liberado: false,
            favoritos,
            busca: String::new(),
            foco: 0,
            tocando: None,
            lista_aberta: true,
            letreiro_ate: Instant::now(),
            digitado: String::new(),
            digitado_em: Instant::now(),
            volume: 100,
            mudo: false,
            pausado: false,
            logos: HashMap::new(),
            pedidos_de_logo: Vec::new(),
            ordem_das_capas: std::collections::VecDeque::new(),
            recados,
            emissor,
            nova_versao: None,
            ultima_checagem: Instant::now(),
            aviso: None,
            aba: Aba::Canais,
            pip: false,
            guia_aberto: false,
            publicados: canais.clone(),
            gavetas: Vec::new(),
            letra: "A".into(),
            filmes: Vec::new(),
            series: Vec::new(),
            serie_aberta: None,
            episodios: Vec::new(),
            episodios_colecao: HashMap::new(),
            filas: Vec::new(),
            generos: generos::Generos::default(),
            genero: String::new(),
            foco_fila: 0,
            carregando_vod: false,
            acervo: Vec::new(),
            abrir_ao_chegar: None,
            favoritos_vod: ler_lista("favoritos-vod.txt"),
            busca_vod: String::new(),
            quadro: 0,
            ultimo_pulo: None,
            colunas_vod: 5,
            ultimo_progresso: Instant::now(),
            foco_vod: 0,
            fonte_preferida: None,
            tocando_vod: None,
            controles_ate: Instant::now() + Duration::from_secs(6),
            ajuda_aberta: false,
            velocidade: 1.0,
            preencher: false,
            ultimo_mouse: None,
            retomou_canal: false,
            carregando_filas: false,
            ficha_aberta: None,
            ficha_destino: None,
            ficha: None,
            ficha_carregando: false,
            ficha_ator: None,
            ficha_ator_foto: None,
            perfil: None,
            filmografia: Vec::new(),
            filmografia_carregando: false,
            eventos: Vec::new(),
            eventos_carregados: false,
            eventos_erro: false,
            eventos_pedidos: None,
            buscando_eventos: false,
            versao_dados: 0,
            cache_itens: std::cell::RefCell::new(None),
            fila_de_imagens: std::sync::Arc::new(FilaDeImagens::default()),
            canal_parado: None,
            outra_tela: None,
            foco_a_direita: false,
            mpv_reserva: None,
            colecoes: HashMap::new(),
            series_da_letra: None,
            midia: midia::Midia::nova(cc),
            abertura: abertura::Abertura::nova(&caminho_do_mpv()),
            explorar: false,
            ordem_az: false,
            heroi: None,
            fichas_pedidas: std::collections::HashSet::new(),
            temporadas_pedidas: std::collections::HashSet::new(),
            radios: radios::locais(),
            buscando_radios: false,
            radio: None,
            colecoes_pedidas: false,
            episodios_no_player: false,
            temporada_aberta: 0,
            pedir_foco_na_busca: false,
        };
        app.ligar_baixadores_de_imagem(&cc.egui_ctx);
        app.reordenar();
        // Só para conferir telas fora do Windows: SAIMO_ABA=inicio|filmes|series.
        if let Ok(aba) = std::env::var("SAIMO_ABA") {
            match aba.as_str() {
                "inicio" => app.abrir_secao(Aba::Inicio),
                "ondemand" => app.abrir_secao(Aba::OnDemand),
                "filmes" => app.abrir_secao(Aba::Filmes),
                "series" => app.abrir_secao(Aba::Series),
                "radios" => app.abrir_secao(Aba::Radios),
                "buscar" => app.abrir_secao(Aba::Buscar),
                "ajustes" => app.abrir_secao(Aba::Ajustes),
                "eventos" => app.abrir_secao(Aba::Eventos),
                _ => {}
            }
        }
        app
    }

    /// A lista que está na tela: catálogo (mais os restritos quando liberados),
    /// na ordem das seções, filtrada pela busca.
    pub fn visiveis(&self) -> Vec<usize> {
        let busca = catalogo::chave_de_ordem(self.busca.trim());
        self.canais
            .iter()
            .enumerate()
            .filter(|(_, c)| {
                busca.is_empty()
                    || catalogo::chave_de_ordem(&c.nome).contains(&busca)
                    || catalogo::chave_de_ordem(c.secao()).contains(&busca)
            })
            .map(|(i, _)| i)
            .collect()
    }

    fn reordenar(&mut self) {
        let atual = self.tocando.as_ref().and_then(|t| self.canais.get(t.canal)).map(|c| c.nome.clone());
        // Sempre da lista publicada: peneirar a que já está na tela faria o
        // servidor religado não voltar nunca, porque suas fontes já teriam
        // sido jogadas fora.
        let mut lista = fontes_desativadas::peneirar_canais(self.publicados.clone());
        if self.liberado {
            let ja: Vec<String> = lista.iter().map(|c| c.nome.clone()).collect();
            lista.extend(
                fontes_desativadas::peneirar_canais(self.restritos.clone())
                    .into_iter()
                    .filter(|r| !ja.contains(&r.nome)),
            );
        } else {
            lista.retain(|c| c.secao() != "Adulto");
        }
        catalogo::ordenar(&mut lista, &self.favoritos);
        self.canais = lista;
        if let Some(nome) = atual {
            if let Some(novo) = self.canais.iter().position(|c| c.nome == nome) {
                if let Some(t) = self.tocando.as_mut() {
                    t.canal = novo;
                }
            }
        }
        self.foco = self.foco.min(self.canais.len().saturating_sub(1));
    }

    /// Manda montar o guia com a lista que está na tela.
    fn pedir_guia(&self) {
        let canais = self.canais.clone();
        let emissor = self.emissor.clone();
        epg::carregar(canais, move || {
            let _ = emissor.send(Recado::Guia);
        });
    }

    fn trocar_lista(&mut self, base: Vec<Canal>, restrita: bool) {
        if restrita {
            self.restritos = base;
        } else {
            // A lista de canais visíveis é montada a cada reordenação; esta
            // é a publicada pura, e guardá-la evita duplicar restrito a cada
            // recarga.
            self.publicados = base;
        }
        self.reordenar();
        self.pedir_guia();
        self.voltar_ao_ultimo_canal();
    }

    /// Abre no canal que estava no ar quando o programa fechou, como a TV.
    fn voltar_ao_ultimo_canal(&mut self) {
        if self.retomou_canal || self.tocando.is_some() || self.tocando_vod.is_some() || self.mpv.is_none() {
            return;
        }
        // Com a abertura na frente o canal esperaria por trás dela, com som:
        // volta quando ela sair (`cuidar_da_abertura`).
        if self.abertura.as_ref().is_some_and(|a| a.cobrindo()) {
            return;
        }
        let Some(nome) = ler_lista("ultimo-canal.txt").into_iter().next() else {
            self.retomou_canal = true;
            return;
        };
        // A restrita chega depois: espera por ela se o canal é de lá.
        let Some(indice) = self.canais.iter().position(|c| c.nome == nome) else { return };
        self.retomou_canal = true;
        self.foco = indice;
        self.tocar(indice, 0, true);
    }

    // MARK: - Filmes e séries

    /// Troca a letra do acervo e busca as duas listas dela.
    pub fn abrir_letra(&mut self, letra: String) {
        if self.letra == letra && !self.filmes.is_empty() {
            return;
        }
        self.letra = letra.clone();
        self.filmes.clear();
        self.series.clear();
        self.serie_aberta = None;
        self.episodios.clear();
        self.foco_vod = 0;
        self.carregando_vod = true;
        let liberado = self.liberado;
        for serie in [false, true] {
            let emissor = self.emissor.clone();
            let letra = letra.clone();
            std::thread::spawn(move || {
                let recado = if serie {
                    Recado::Series(letra.clone(), vod::series(&letra))
                } else {
                    let mut lista = vod::filmes(&letra);
                    // Os reservados só existem depois do código, como os canais.
                    if liberado {
                        lista.extend(vod::reservados(&letra));
                        lista.sort_by(|a, b| {
                            catalogo::chave_de_ordem(&a.titulo).cmp(&catalogo::chave_de_ordem(&b.titulo))
                        });
                    }
                    Recado::Filmes(letra.clone(), lista)
                };
                let _ = emissor.send(recado);
            });
        }
    }

    fn mostrar_colecao(&mut self, lista: Vec<(vod::Serie, Vec<vod::Episodio>)>) {
        self.episodios_colecao = lista.iter()
            .map(|(serie, episodios)| (serie.titulo.clone(), episodios.clone()))
            .collect();
        self.series = lista.into_iter().map(|(serie, _)| serie).collect();
        self.carregando_vod = false;
    }

    fn abrir_serie(&mut self, serie: vod::Serie) {
        self.serie_aberta = Some(serie.clone());
        self.episodios.clear();
        self.temporada_aberta = 0;
        // A página da série tem o fundo, a nota e a sinopse da ficha.
        self.pedir_ficha_do_destaque(&serie.titulo, true);
        if let Some(lista) = self.episodios_colecao.get(&serie.titulo).cloned() {
            self.episodios = lista;
            self.carregando_vod = false;
            return;
        }
        self.carregando_vod = true;
        let emissor = self.emissor.clone();
        let letra = self.letra.clone();
        std::thread::spawn(move || {
            let lista = vod::episodios(&letra, &serie);
            let _ = emissor.send(Recado::Episodios(serie.titulo.clone(), lista));
        });
    }

    fn tocar_vod(&mut self, titulo: String, urls: Vec<String>, fonte: usize, nova: bool) {
        // Trocar de filme fecha a conta do anterior antes de perder a posição.
        self.guardar_progresso(true);
        // Filme ocupa a janela inteira e tem som próprio; o canal parado
        // para ir ao acervo não volta por cima dele.
        self.sair_da_multi_tela();
        self.canal_parado = None;
        self.radio = None;
        self.episodios_no_player = false;
        let Some(mpv) = self.mpv.as_ref() else { return };
        let Some(url) = urls.get(fonte).cloned() else {
            telemetria::caiu("vod", &titulo, urls.len());
            self.aviso = Some((format!("{titulo}: nenhuma fonte abriu"), Instant::now()));
            self.tocando_vod = None;
            return;
        };
        // Trocar de fonte do mesmo título mantém o episódio, as marcas e a
        // legenda escolhida — que segue junto para o arquivo novo.
        let anterior = self.tocando_vod.take().filter(|t| t.titulo == titulo);
        let legenda = anterior.as_ref().and_then(|t| t.legenda_escolhida.as_ref().map(|(_, c)| c.clone()));
        mpv.tocar_com_legenda(&url, None, None, legenda.as_deref());
        mpv.pausa(false);
        self.pausado = false;
        self.tocando = None;
        telemetria::comecou("vod", &titulo, &url, fonte + 1, nova);
        let (episodio, marcas, legendas, legenda_escolhida) = anterior
            .map(|t| (t.episodio, t.marcas, t.legendas, t.legenda_escolhida))
            .unwrap_or((None, None, Vec::new(), None));
        self.tocando_vod = Some(TocandoVod {
            titulo,
            urls,
            fonte,
            desde: Instant::now(),
            confirmado: false,
            episodio,
            marcas,
            legendas,
            legenda_escolhida,
            legenda_atraso: 0.0,
            legenda_aviso: None,
            visto: (0.0, 0.0),
            proximo_desde: None,
            proximo_dispensado: false,
        });
        self.letreiro_ate = Instant::now() + SUMIR_LETREIRO;
        self.aba = Aba::Canais;
        self.mostrar_controles();
    }

    fn tocar_episodio(&mut self, serie: &str, episodio: vod::Episodio, lista: Vec<vod::Episodio>) {
        let titulo = format!("{} · T{} E{}", serie, episodio.temporada, episodio.numero);
        let urls = self.fontes_preferidas(serie, &episodio.urls);
        // Some antes de tocar: senão a troca de título apagaria o episódio.
        self.tocando_vod = None;
        self.tocar_vod(titulo.clone(), urls, 0, true);
        let Some(t) = self.tocando_vod.as_mut() else { return };
        t.episodio = Some(EpisodioTocando {
            serie: serie.to_string(),
            temporada: episodio.temporada,
            numero: episodio.numero,
            versao: episodio.versao.clone(),
            lista,
        });
        self.pedir_legendas(titulo.clone(), self.generos.id(serie, true), true, episodio.temporada, episodio.numero);
        self.pedir_marcas(titulo, self.generos.id(serie, true), episodio.temporada, episodio.numero);
        self.pedir_logo(serie, true);
    }

    /// Guarda a fonte que está de fato no ar (a escolhida, a de fábrica ou a
    /// que o player desceu sozinho) e devolve as do próximo episódio da série
    /// com a de sempre na frente. Noutra série, ou sem nada lembrado, a lista
    /// segue como veio.
    fn fontes_preferidas(&mut self, serie: &str, urls: &[String]) -> Vec<String> {
        if self.fonte_preferida.as_ref().map(|m| m.serie != serie).unwrap_or(false) {
            self.fonte_preferida = None;
        }
        if let Some(t) = self.tocando_vod.as_ref() {
            if let (Some(ep), Some(url)) = (t.episodio.as_ref(), t.urls.get(t.fonte)) {
                if ep.serie == serie {
                    // A posição é na lista do próprio episódio, não na reordenada.
                    let posicao = ep
                        .lista
                        .iter()
                        .find(|e| e.temporada == ep.temporada && e.numero == ep.numero && e.versao == ep.versao)
                        .and_then(|e| e.urls.iter().position(|u| u == url))
                        .unwrap_or(t.fonte);
                    self.fonte_preferida = Some(fonte_preferida::Memoria {
                        serie: serie.to_string(),
                        host: fonte_preferida::host(url),
                        posicao,
                    });
                }
            }
        }
        match self.fonte_preferida.as_ref() {
            Some(m) => fonte_preferida::ordenar(urls, &m.host, m.posicao),
            None => urls.to_vec(),
        }
    }

    /// Toca um filme e já pede as legendas dele.
    fn tocar_filme(&mut self, titulo: String, urls: Vec<String>) {
        self.fonte_preferida = None;
        self.tocar_vod(titulo.clone(), urls, 0, true);
        self.pedir_legendas(titulo.clone(), self.generos.id(&titulo, false), false, 0, 0);
        self.pedir_logo(&titulo, false);
    }

    /// Busca as legendas numa thread; chegam pelo recado. Se a pessoa já
    /// escolhera um idioma antes, a primeira versão dele liga sozinha.
    fn pedir_legendas(&self, titulo: String, tmdb: Option<u32>, serie: bool, temporada: u32, numero: u32) {
        let Some(tmdb) = tmdb else { return };
        let emissor = self.emissor.clone();
        std::thread::spawn(move || {
            let lista = legendas::buscar(tmdb, serie, temporada, numero);
            if !lista.is_empty() {
                let _ = emissor.send(Recado::Legendas(titulo, lista));
            }
        });
    }

    /// A pessoa escolheu uma legenda: baixa o arquivo e liga no mpv.
    pub fn escolher_legenda(&mut self, opcao: legendas::Opcao) {
        let Some(t) = self.tocando_vod.as_mut() else { return };
        t.legenda_aviso = Some("Baixando a legenda…".to_string());
        let (titulo, emissor) = (t.titulo.clone(), self.emissor.clone());
        std::thread::spawn(move || {
            let caminho = legendas::baixar(&opcao);
            let _ = emissor.send(Recado::LegendaPronta(titulo, opcao, caminho));
        });
    }

    pub fn desligar_legendas(&mut self) {
        if let Some(mpv) = self.mpv.as_ref() {
            mpv.escolher_faixa("sub", None);
        }
        if let Some(t) = self.tocando_vod.as_mut() {
            t.legenda_escolhida = None;
            t.legenda_aviso = None;
        }
        legendas::guardar_idioma("");
    }

    /// Soma (ou tira) segundos do atraso da legenda escolhida.
    pub fn ajustar_atraso_legenda(&mut self, delta: f64) {
        let Some(t) = self.tocando_vod.as_mut() else { return };
        t.legenda_atraso = ((t.legenda_atraso + delta) * 100.0).round() / 100.0;
        if let Some(mpv) = self.mpv.as_ref() {
            mpv.atraso_da_legenda(t.legenda_atraso);
        }
    }

    /// Busca as marcas de pular numa thread; chegam pelo recado.
    fn pedir_marcas(&self, titulo: String, tmdb: Option<u32>, temporada: u32, numero: u32) {
        let Some(tmdb) = tmdb else { return };
        let emissor = self.emissor.clone();
        std::thread::spawn(move || {
            if let Some(marcas) = pulos::buscar(tmdb, temporada, numero) {
                let _ = emissor.send(Recado::Marcas(titulo, marcas));
            }
        });
    }

    /// O trecho que dá para pular agora, se houver.
    pub fn pulo_na_tela(&self) -> Option<pulos::Trecho> {
        let t = self.tocando_vod.as_ref()?;
        let (posicao, duracao) = t.visto;
        t.marcas.as_ref()?.em(posicao, duracao).cloned()
    }

    pub fn pular_trecho(&mut self) {
        let Some(trecho) = self.pulo_na_tela() else { return };
        let duracao = self.tocando_vod.as_ref().map(|t| t.visto.1).unwrap_or(0.0);
        match trecho.fim {
            Some(fim) => self.ir_para(fim),
            // Até o fim do vídeo: é o mesmo que acabar.
            None => self.ir_para((duracao - 1.0).max(0.0)),
        }
    }

    /// O episódio seguinte, quando o cartão dele está à vista.
    pub fn proximo_na_tela(&self) -> Option<(vod::Episodio, u64)> {
        let t = self.tocando_vod.as_ref()?;
        let desde = t.proximo_desde?;
        if t.proximo_dispensado {
            return None;
        }
        let seguinte = t.episodio.as_ref()?.seguinte()?;
        let resta = ESPERA_DO_PROXIMO.saturating_sub(desde.elapsed()).as_secs() + 1;
        Some((seguinte, resta))
    }

    pub fn tocar_proximo(&mut self) -> bool {
        let Some(t) = self.tocando_vod.as_ref() else { return false };
        let Some(ep) = t.episodio.clone() else { return false };
        let Some(seguinte) = ep.seguinte() else { return false };
        // O que acabou não fica no "continuar assistindo".
        progresso::esquecer(&t.titulo);
        self.tocar_episodio(&ep.serie, seguinte, ep.lista);
        true
    }

    pub fn dispensar_proximo(&mut self) {
        if let Some(t) = self.tocando_vod.as_mut() {
            t.proximo_dispensado = true;
        }
    }

    /// Enter com o botão de pular ou o cartão do próximo à vista.
    fn acao_na_tela(&mut self) -> bool {
        if self.pulo_na_tela().is_some() {
            self.pular_trecho();
            return true;
        }
        if self.proximo_na_tela().is_some() {
            return self.tocar_proximo();
        }
        false
    }

    /// Posição, cartão do próximo episódio e a volta sozinha, a cada quadro.
    fn acompanhar_vod(&mut self) {
        let Some(mpv) = self.mpv.as_ref() else { return };
        let visto = mpv.posicao();
        let Some(t) = self.tocando_vod.as_mut() else { return };
        if !t.confirmado {
            return;
        }
        if let Some(v) = visto {
            t.visto = v;
        }
        let (posicao, duracao) = t.visto;
        let tem_seguinte = t.episodio.as_ref().and_then(|e| e.seguinte()).is_some();
        if !tem_seguinte || t.proximo_dispensado {
            return;
        }
        // Os créditos marcados; sem marca, os últimos 30 s de um episódio.
        let creditos = t
            .marcas
            .as_ref()
            .and_then(|m| m.creditos())
            .unwrap_or(if duracao > 300.0 { duracao - 30.0 } else { f64::MAX });
        if posicao >= creditos && t.proximo_desde.is_none() {
            t.proximo_desde = Some(Instant::now());
        } else if posicao < creditos - 5.0 {
            // Voltou para antes dos créditos: o cartão some.
            t.proximo_desde = None;
        }
        let venceu = t.proximo_desde.map(|d| d.elapsed() >= ESPERA_DO_PROXIMO).unwrap_or(false);
        if venceu && !self.pausado {
            self.tocar_proximo();
        }
    }

    /// Grava onde o filme parou. `agora` força, em vez de esperar os 15 s.
    pub fn guardar_progresso(&mut self, agora: bool) {
        if !agora && self.ultimo_progresso.elapsed() < Duration::from_secs(15) {
            return;
        }
        self.ultimo_progresso = Instant::now();
        let (Some(filme), Some(mpv)) = (self.tocando_vod.as_ref(), self.mpv.as_ref()) else { return };
        if !filme.confirmado {
            return;
        }
        if let Some((posicao, duracao)) = mpv.posicao() {
            progresso::salvar(&filme.titulo, posicao, duracao);
        }
    }

    /// Volta ao ponto em que parou, quando há um.
    fn retomar(&mut self) {
        let (Some(filme), Some(mpv)) = (self.tocando_vod.as_ref(), self.mpv.as_ref()) else { return };
        let Some(marca) = progresso::onde_parou(&filme.titulo) else { return };
        mpv.ir_para(marca.posicao);
        let minutos = (marca.posicao / 60.0).round() as i64;
        self.aviso = Some((format!("continuando de {minutos} min"), Instant::now()));
    }

    /// Abre a ficha de um título: sinopse, duração, gêneros e elenco.
    ///
    /// O pedido vai para uma thread; enquanto não volta, a janela já está no
    /// ar dizendo que está a caminho. Sem id do TMDB não há o que pedir — e é
    /// melhor dizer isso do que abrir uma janela que nunca preenche.
    pub fn abrir_ficha(&mut self, titulo: String, serie: bool, destino: Destino) {
        self.ficha_destino = Some(destino);
        self.ficha_ator = None;
        self.filmografia.clear();
        self.ficha = ficha::conhecida(&titulo, serie);
        self.ficha_aberta = Some((titulo.clone(), serie));
        if self.ficha.is_some() {
            self.ficha_carregando = false;
            return;
        }
        let Some(id) = self.generos.id(&titulo, serie) else {
            self.ficha_carregando = false;
            return;
        };
        self.ficha_carregando = true;
        let emissor = self.emissor.clone();
        std::thread::spawn(move || {
            let achada = ficha::baixar(&titulo, serie, id);
            let _ = emissor.send(Recado::Ficha(titulo, serie, achada));
        });
    }

    /// Abre, dentro da ficha, quem é o ator e o que ele tem neste acervo.
    pub fn abrir_ator(&mut self, id: u32, nome: String, foto: Option<String>) {
        self.ficha_ator = Some((id, nome));
        self.ficha_ator_foto = foto;
        self.perfil = None;
        self.filmografia.clear();
        self.filmografia_carregando = true;
        // A rede vai para threads; o cruzamento com o acervo é feito na
        // volta, aqui, que é onde o índice e as fichas moram.
        let emissor = self.emissor.clone();
        std::thread::spawn(move || {
            let _ = emissor.send(Recado::Filmografia(id, ficha::creditos_de(id)));
        });
        let emissor = self.emissor.clone();
        std::thread::spawn(move || {
            let _ = emissor.send(Recado::Perfil(id, ficha::perfil(id)));
        });
    }

    /// "Assistir" na ficha: fecha a ficha e segue pelo caminho do título.
    pub fn assistir_da_ficha(&mut self) {
        let destino = self.ficha_destino.take();
        self.fechar_ficha();
        match destino {
            Some(Destino::Item(item)) => self.abrir_item(item),
            Some(Destino::Fileira(fila, indice)) => tela::seguir_destaque(self, fila, indice),
            None => {}
        }
    }

    /// Fecha a ficha e solta as imagens grandes dela: o fundo tem 1280 pixels
    /// e ficaria na memória de vídeo para sempre a cada ficha aberta.
    pub fn fechar_ficha(&mut self) {
        self.ficha_aberta = None;
        self.ficha_ator = None;
        self.perfil = None;
        self.filmografia.clear();
        self.logos.retain(|chave, _| !chave.starts_with("1280|") && !chave.starts_with("632|"));
    }

    pub fn favoritar_vod(&mut self, titulo: String) {
        if let Some(pos) = self.favoritos_vod.iter().position(|f| *f == titulo) {
            self.favoritos_vod.remove(pos);
        } else {
            self.favoritos_vod.push(titulo);
        }
        gravar_lista("favoritos-vod.txt", &self.favoritos_vod);
    }

    /// A capa do título: devolve o que já se sabe e manda procurar o resto.
    /// A capa de um título.
    ///
    /// O endereço vem da ficha publicada, resolvida pelo id do TMDB — não mais
    /// de uma busca por nome feita aqui, que era lenta e trocava filmes de nome
    /// igual. Quem não tem ficha fica sem capa, e a tela põe uma marca no
    /// lugar.
    pub fn capa(&mut self, titulo: &str, serie: bool) -> Option<egui::TextureHandle> {
        let endereco = self.endereco_da_capa(titulo, serie)?;
        // 342 pixels, o tamanho que o arquivo de fichas publica: a capa era
        // reduzida a 96, o tamanho das logos de canal, e ficava borrada num
        // cartão de 150x225 — pior ainda em tela com escala de 150%.
        self.imagem(&endereco, 342)
    }

    /// A ficha publicada primeiro; sem ela, a capa que veio com a fileira de
    /// destaque (animes e doramas quase nunca têm ficha).
    pub fn endereco_da_capa(&self, titulo: &str, serie: bool) -> Option<String> {
        match self.generos.capa(titulo, serie) {
            Some(url) => Some(url.to_string()),
            None => self
                .filas
                .iter()
                .flat_map(|f| &f.itens)
                .find(|i| i.titulo == titulo && i.serie() == serie && !i.capa.is_empty())
                .map(|i| i.capa.clone()),
        }
    }

    /// O quadro de "tocando agora" do Windows: canal com a logo e o programa
    /// do guia, filme e série com a capa. Os botões dele só pausam filme e
    /// série — o `alternar_pausa` já recusa pausar o ao vivo.
    fn cuidar_da_midia(&mut self) {
        for botao in self.midia.botoes() {
            let querer_pausa = botao == midia::Botao::Pausar;
            if self.tocando_vod.is_some() && self.pausado != querer_pausa {
                self.alternar_pausa();
            }
        }
        let canal = self.tocando.as_ref().and_then(|t| self.canais.get(t.canal));
        let titulo = match (self.tocando_vod.as_ref(), canal, self.radio.as_ref()) {
            (Some(t), _, _) => Some(t.titulo.as_str()),
            (None, Some(c), _) => Some(c.nome.as_str()),
            (None, None, Some(r)) => Some(r.nome.as_str()),
            _ => None,
        };
        // A ficha da série, no episódio; a do filme, no filme.
        let obra = self.tocando_vod.as_ref().map(|t| match t.episodio.as_ref() {
            Some(e) => (e.serie.as_str(), true),
            None => (t.titulo.as_str(), false),
        });
        let logo = obra.and_then(|(nome, serie)| ficha::logo(nome, serie));
        if self.midia.em_dia(titulo, logo.as_deref(), self.pausado) {
            return;
        }
        let no_ar = match (self.tocando_vod.as_ref(), canal) {
            (Some(t), _) => {
                let imagem = logo.or_else(|| obra.and_then(|(nome, serie)| self.endereco_da_capa(nome, serie)));
                Some(midia::NoAr { titulo: t.titulo.clone(), subtitulo: "Saimo TV".into(), imagem, ao_vivo: false })
            }
            (None, Some(c)) => Some(midia::NoAr {
                titulo: c.nome.clone(),
                subtitulo: epg::agora_e_depois(&c.nome).map(|(p, _)| p.titulo).unwrap_or_else(|| "Saimo TV".into()),
                imagem: c.logo.clone(),
                ao_vivo: true,
            }),
            _ => self.radio.as_ref().map(|r| midia::NoAr {
                titulo: r.nome.clone(),
                subtitulo: "Rádio ao vivo".into(),
                imagem: r.logo.clone(),
                ao_vivo: true,
            }),
        };
        self.midia.mostrar(no_ar, self.pausado);
    }

    /// Busca a ficha do que começou a tocar, só pela logo do quadro de
    /// "tocando agora"; até ela chegar (ou sem ela), vai a capa.
    fn pedir_logo(&self, titulo: &str, serie: bool) {
        if ficha::conhecida(titulo, serie).is_some() {
            return;
        }
        let Some(id) = self.generos.id(titulo, serie) else { return };
        let titulo = titulo.to_string();
        std::thread::spawn(move || {
            ficha::baixar(&titulo, serie, id);
        });
    }

    fn proxima_fonte_vod(&mut self, motivo: &str) {
        let Some(t) = self.tocando_vod.as_ref() else { return };
        let (titulo, fonte, urls) = (t.titulo.clone(), t.fonte, t.urls.clone());
        telemetria::falhou("vod", &titulo, urls.get(fonte).map(String::as_str).unwrap_or(""), fonte + 1, motivo);
        if fonte + 1 >= urls.len() {
            telemetria::caiu("vod", &titulo, urls.len());
            telemetria::parou();
            self.aviso = Some((format!("{titulo}: nenhuma das {} fontes abriu", urls.len()), Instant::now()));
            self.tocando_vod = None;
            if let Some(mpv) = self.mpv.as_ref() {
                mpv.parar();
            }
            return;
        }
        self.aviso = Some((format!("{titulo}: trocando para a fonte {}", fonte + 2), Instant::now()));
        self.tocar_vod(titulo, urls, fonte + 1, false);
    }

    pub fn tocar(&mut self, canal: usize, fonte: usize, nova: bool) {
        self.radio = None;
        self.fonte_preferida = None;
        let Some(mpv) = self.mpv.as_ref() else { return };
        let Some(c) = self.canais.get(canal) else { return };
        let Some(f) = c.fontes.get(fonte) else {
            telemetria::caiu("live", &c.nome, c.fontes.len());
            self.aviso = Some((format!("{}: nenhuma fonte abriu", c.nome), Instant::now()));
            self.tocando = None;
            return;
        };
        mpv.tocar(&f.url, f.referer.as_deref(), f.agente.as_deref());
        mpv.pausa(false);
        self.pausado = false;
        telemetria::comecou("live", &c.nome, &f.url, fonte + 1, nova);
        self.tocando = Some(Tocando { canal, fonte, desde: Instant::now(), confirmado: false });
        self.letreiro_ate = Instant::now() + SUMIR_LETREIRO;
    }

    fn proxima_fonte(&mut self, motivo: &str) {
        let Some(t) = self.tocando.as_ref() else { return };
        let (canal, fonte) = (t.canal, t.fonte);
        let (nome, url, total) = {
            let Some(c) = self.canais.get(canal) else { return };
            (c.nome.clone(), c.fontes.get(fonte).map(|f| f.url.clone()).unwrap_or_default(), c.fontes.len())
        };
        telemetria::falhou("live", &nome, &url, fonte + 1, motivo);
        if fonte + 1 >= total {
            telemetria::caiu("live", &nome, total);
            telemetria::parou();
            self.aviso = Some((format!("{nome}: nenhuma das {total} fontes abriu"), Instant::now()));
            self.tocando = None;
            if let Some(mpv) = self.mpv.as_ref() {
                mpv.parar();
            }
            return;
        }
        self.aviso = Some((format!("{nome}: trocando para a fonte {}", fonte + 2), Instant::now()));
        self.tocar(canal, fonte + 1, false);
    }

    fn escolher_fonte_seguinte(&mut self) {
        let Some(t) = self.tocando.as_ref() else { return };
        let (canal, fonte) = (t.canal, t.fonte);
        let total = self.canais.get(canal).map(|c| c.fontes.len()).unwrap_or(0);
        if total <= 1 {
            return;
        }
        self.tocar(canal, (fonte + 1) % total, false);
    }

    fn cuidar_do_mpv(&mut self, ctx: &egui::Context) {
        let avisos: Vec<mpv::Aviso> = self.mpv.as_ref().map(|m| m.avisos().collect()).unwrap_or_default();
        for aviso in avisos {
            if let Some(r) = self.radio.as_mut() {
                match aviso {
                    mpv::Aviso::Tocando => r.confirmado = true,
                    mpv::Aviso::Falhou | mpv::Aviso::Fim => {
                        self.aviso = Some((format!("{}: a rádio está fora do ar", r.nome), Instant::now()));
                        self.radio = None;
                    }
                }
                continue;
            }
            if self.tocando_vod.is_some() {
                match aviso {
                    mpv::Aviso::Tocando => {
                        if let Some(t) = self.tocando_vod.as_mut() {
                            if !t.confirmado {
                                t.confirmado = true;
                                let (titulo, fonte, ms) =
                                    (t.titulo.clone(), t.fonte, t.desde.elapsed().as_millis());
                                let url = t.urls.get(fonte).cloned().unwrap_or_default();
                                telemetria::tocou("vod", &titulo, &url, fonte + 1, ms);
                                self.retomar();
                            }
                        }
                    }
                    mpv::Aviso::Falhou => self.proxima_fonte_vod("mpv: erro na fonte"),
                    // Filme que termina é fim mesmo; canal que termina é fonte caindo.
                    mpv::Aviso::Fim => {
                        let (acabou, visto) = self
                            .tocando_vod
                            .as_ref()
                            .map(|t| (t.confirmado, t.visto))
                            .unwrap_or((false, (0.0, 0.0)));
                        // "Terminar" nos primeiros segundos é fonte quebrada, não
                        // fim de filme: tenta a próxima em vez de fechar.
                        let cedo = visto.0 < 30.0 || (visto.1 > 0.0 && visto.1 < 120.0);
                        if acabou && cedo {
                            self.proxima_fonte_vod(&format!("terminou em {} s", visto.0 as u64));
                        } else if acabou && self.tocar_proximo() {
                            // Série: o episódio seguinte já está a caminho.
                        } else if acabou {
                            // Chegou ao fim: nada a retomar da próxima vez.
                            if let Some(filme) = self.tocando_vod.as_ref() {
                                progresso::esquecer(&filme.titulo);
                            }
                            telemetria::parou();
                            self.tocando_vod = None;
                            self.lista_aberta = true;
                        } else {
                            self.proxima_fonte_vod("a fonte não abriu");
                        }
                    }
                }
                continue;
            }
            match aviso {
                mpv::Aviso::Tocando => {
                    if let Some(t) = self.tocando.as_mut() {
                        if !t.confirmado {
                            t.confirmado = true;
                            let ms = t.desde.elapsed().as_millis();
                            let (canal, fonte) = (t.canal, t.fonte);
                            self.retomou_canal = true;
                            if let Some(c) = self.canais.get(canal) {
                                gravar_lista("ultimo-canal.txt", std::slice::from_ref(&c.nome));
                                let url = c.fontes.get(fonte).map(|f| f.url.clone()).unwrap_or_default();
                                telemetria::tocou("live", &c.nome, &url, fonte + 1, ms);
                            }
                        }
                    }
                }
                mpv::Aviso::Falhou => self.proxima_fonte("mpv: erro na fonte"),
                mpv::Aviso::Fim => self.proxima_fonte("a transmissão terminou"),
            }
        }
        // Fonte que nunca abre não manda erro nenhum: o relógio é quem decide.
        let estourou = self
            .tocando
            .as_ref()
            .map(|t| !t.confirmado && t.desde.elapsed() > ESPERA_DA_FONTE)
            .unwrap_or(false);
        if estourou {
            self.proxima_fonte("a fonte não abriu em 14 s");
        }
        let estourou_vod = self
            .tocando_vod
            .as_ref()
            .map(|t| !t.confirmado && t.desde.elapsed() > ESPERA_DA_FONTE)
            .unwrap_or(false);
        if estourou_vod {
            self.proxima_fonte_vod("a fonte não abriu em 14 s");
        }
        if self.radio.as_ref().is_some_and(|r| !r.confirmado && r.desde.elapsed() > ESPERA_DA_FONTE) {
            let nome = self.radio.as_ref().map(|r| r.nome.clone()).unwrap_or_default();
            self.aviso = Some((format!("{nome}: a rádio não abriu"), Instant::now()));
            self.parar_radio();
        }
        self.cuidar_da_outra_tela();
        let outra_no_ar = self.outra_tela.as_ref().is_some_and(|o| o.tocando.is_some());
        if self.tocando.is_some() || self.tocando_vod.is_some() || outra_no_ar || self.radio.is_some() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
        self.acompanhar_vod();
        self.contar_video();
        self.guardar_progresso(false);
    }

    /// O monitor só conta o tempo com o vídeo andando de verdade.
    fn contar_video(&mut self) {
        let rodando = self.tocando.as_ref().map(|t| t.confirmado).unwrap_or(false)
            || self.tocando_vod.as_ref().map(|t| t.confirmado).unwrap_or(false);
        let (carregando, qualidade) = match (rodando, self.mpv.as_ref()) {
            (true, Some(mpv)) => {
                let pulando = mpv.ler("seeking").as_deref() == Some("yes");
                if pulando {
                    self.ultimo_pulo = Some(Instant::now());
                }
                let carregando = pulando || mpv.ler("paused-for-cache").as_deref() == Some("yes");
                // A altura só muda quando troca a variante: não precisa ler todo quadro.
                let qualidade = if self.quadro.is_multiple_of(120) {
                    mpv.ler("height").and_then(|h| h.parse::<u32>().ok()).filter(|h| *h > 0).map(|h| format!("{h}p"))
                } else {
                    None
                };
                (carregando, qualidade)
            }
            _ => (false, None),
        };
        self.quadro = self.quadro.wrapping_add(1);
        // Carregar logo depois de pular para outro ponto do filme é normal, não travamento.
        let pulou = self.ultimo_pulo.map(|t| t.elapsed() < Duration::from_secs(3)).unwrap_or(false);
        telemetria::video(rodando, self.pausado, carregando, pulou, qualidade);
        if !self.busca.trim().is_empty() {
            telemetria::busca(0, &self.busca, !self.visiveis().is_empty());
        }
    }

    fn cuidar_dos_recados(&mut self, ctx: &egui::Context) {
        let mut chegou_algo = false;
        while let Ok(recado) = self.recados.try_recv() {
            chegou_algo = true;
            if !matches!(recado, Recado::Logo(..) | Recado::LogoEsquecido(_) | Recado::FichaDoDestaque | Recado::Temporada) {
                self.versao_dados = self.versao_dados.wrapping_add(1);
            }
            match recado {
                Recado::LogoEsquecido(chave) => {
                    if matches!(self.logos.get(&chave), Some(None)) {
                        self.logos.remove(&chave);
                    }
                }
                Recado::Catalogo(lista) => self.trocar_lista(lista, false),
                Recado::Restritos(lista) => self.trocar_lista(lista, true),
                Recado::Eventos(lidos) => {
                    self.buscando_eventos = false;
                    self.eventos_carregados = true;
                    // Falha de rede não apaga a agenda que já estava na tela.
                    self.eventos_erro = lidos.is_none();
                    if let Some(jogos) = lidos {
                        self.eventos = jogos;
                    }
                }
                Recado::Logo(url, imagem) => {
                    let textura = ctx.load_texture(&url, imagem, egui::TextureOptions::LINEAR);
                    if url.starts_with("342|") {
                        self.ordem_das_capas.push_back(url.clone());
                        // Cada capa em 342 pixels ocupa uns 700 KB. Rolando o
                        // acervo inteiro isso cresceria sem fim; as mais
                        // antigas saem e voltam da rede se aparecerem de novo.
                        while self.ordem_das_capas.len() > MAXIMO_DE_CAPAS {
                            if let Some(velha) = self.ordem_das_capas.pop_front() {
                                self.logos.remove(&velha);
                            }
                        }
                    }
                    self.logos.insert(url, Some(textura));
                }
                Recado::Atualizacao(v) => self.nova_versao = Some(v),
                Recado::SemAtualizacao => {
                    self.aviso = Some((format!("Você já está na versão mais nova ({VERSAO})"), Instant::now()));
                }
                Recado::Guia | Recado::FichaDoDestaque | Recado::Temporada => {}
                Recado::Radios(lista) => {
                    self.buscando_radios = false;
                    self.radios = lista;
                }
                Recado::Gavetas(lista) => self.gavetas = lista,
                Recado::Acervo(lista) => self.acervo = lista,
                Recado::FontesDesativadas => {
                    self.reordenar();
                    self.pedir_guia();
                }
                Recado::Ficha(titulo, serie, achada) => {
                    // A janela pode ter sido fechada, ou trocada de título,
                    // enquanto o pedido corria: só vale a resposta do aberto.
                    if self.ficha_aberta.as_ref() == Some(&(titulo, serie)) {
                        self.ficha = achada;
                        self.ficha_carregando = false;
                    }
                }
                Recado::Filmografia(ator, creditos) => {
                    if self.ficha_ator.as_ref().map(|(id, _)| *id) == Some(ator) {
                        self.filmografia = ficha::no_acervo(&creditos, &self.generos, &self.acervo);
                        self.filmografia_carregando = false;
                    }
                }
                Recado::Legendas(titulo, lista) => {
                    let idioma = legendas::idioma_guardado();
                    let automatica = lista.iter().find(|o| o.idioma == idioma).cloned();
                    if let Some(t) = self.tocando_vod.as_mut().filter(|t| t.titulo == titulo) {
                        t.legendas = lista;
                        if !idioma.is_empty() && t.legenda_escolhida.is_none() {
                            if let Some(o) = automatica {
                                self.escolher_legenda(o);
                            }
                        }
                    }
                }
                Recado::LegendaPronta(titulo, opcao, caminho) => {
                    if let (Some(t), Some(mpv)) = (
                        self.tocando_vod.as_mut().filter(|t| t.titulo == titulo),
                        self.mpv.as_ref(),
                    ) {
                        match caminho {
                            Some(c) => {
                                let c = c.to_string_lossy().to_string();
                                mpv.legenda_externa(&c, &opcao.rotulo, &opcao.idioma[..2]);
                                mpv.atraso_da_legenda(0.0);
                                legendas::guardar_idioma(&opcao.idioma);
                                t.legenda_escolhida = Some((opcao, c));
                                t.legenda_atraso = 0.0;
                                t.legenda_aviso = None;
                            }
                            None => {
                                t.legenda_aviso = Some("Não foi possível baixar esta legenda. Tente outra versão.".to_string());
                            }
                        }
                    }
                }
                Recado::Marcas(titulo, marcas) => {
                    if let Some(t) = self.tocando_vod.as_mut().filter(|t| t.titulo == titulo) {
                        t.marcas = Some(marcas);
                    }
                }
                Recado::Perfil(ator, perfil) => {
                    if self.ficha_ator.as_ref().map(|(id, _)| *id) == Some(ator) {
                        self.perfil = perfil;
                    }
                }
                Recado::Filmes(letra, lista) => {
                    if letra == self.letra {
                        self.filmes = lista;
                        self.carregando_vod = false;
                        self.abrir_o_que_esperava();
                    }
                }
                Recado::Series(letra, lista) => {
                    if letra == self.letra && matches!(self.aba, Aba::Animes | Aba::Doramas) {
                        // A letra chegou com uma coleção na tela: fica para a volta.
                        self.series_da_letra = Some(lista);
                    } else if letra == self.letra {
                        self.series = lista;
                        self.carregando_vod = false;
                        self.abrir_o_que_esperava();
                    }
                }
                Recado::Generos(lidos) => {
                    self.generos = lidos;
                }
                Recado::Destaques(lista) => {
                    self.filas = lista;
                    self.carregando_filas = false;
                    if self.aba == Aba::Inicio {
                        self.carregando_vod = false;
                    }
                }
                Recado::Colecao(aba, lista) => {
                    // Lista vazia é falha de rede: não fica guardada.
                    if !lista.is_empty() {
                        self.colecoes.insert(aba as u8, lista.clone());
                    }
                    if self.aba == aba {
                        self.mostrar_colecao(lista);
                    }
                }
                Recado::Episodios(serie, lista) => {
                    if self.serie_aberta.as_ref().map(|s| s.titulo == serie).unwrap_or(false) {
                        self.episodios = lista;
                        self.carregando_vod = false;
                    }
                }
            }
        }
        // Sem isto, a lista só redesenharia no próximo movimento do mouse — e a
        // capa que acabou de chegar ficaria invisível até lá.
        if chegou_algo || !self.pedidos_de_logo.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
        if self.ultima_checagem.elapsed() > Duration::from_secs(3600) {
            self.ultima_checagem = Instant::now();
            let emissor = self.emissor.clone();
            std::thread::spawn(move || {
                if let Some(v) = atualizacao::procurar() {
                    let _ = emissor.send(Recado::Atualizacao(v));
                }
            });
        }
        telemetria::no_relogio();
    }


    /// Uma imagem da rede, reduzida para caber em `lado` pixels.
    ///
    /// O carregador era só para logo de canal, e reduzia tudo a 96 pixels —
    /// uma capa grande na ficha ficaria borrada. O tamanho entra na chave, então
    /// a mesma capa pode existir pequena na grade e grande na ficha.
    pub fn imagem(&mut self, url: &str, lado: u32) -> Option<egui::TextureHandle> {
        let chave = if lado == 96 { url.to_string() } else { format!("{lado}|{url}") };
        if let Some(guardado) = self.logos.get(&chave) {
            return guardado.clone();
        }
        self.logos.insert(chave.clone(), None);
        self.pedidos_de_logo.push((chave, url.to_string(), lado));
        None
    }

    fn baixar_logos_pendentes(&mut self) {
        let novos = std::mem::take(&mut self.pedidos_de_logo);
        if novos.is_empty() {
            return;
        }
        let mut pedidos = self.fila_de_imagens.pedidos.lock().unwrap();
        pedidos.extend(novos);
        while pedidos.len() > MAXIMO_NA_FILA_DE_IMAGENS {
            if let Some((chave, _, _)) = pedidos.pop_front() {
                let _ = self.emissor.send(Recado::LogoEsquecido(chave));
            }
        }
        drop(pedidos);
        self.fila_de_imagens.chegou.notify_all();
    }

    /// Sobe os baixadores de imagem, que vivem enquanto o programa viver.
    fn ligar_baixadores_de_imagem(&self, ctx: &egui::Context) {
        for _ in 0..BAIXADORES_DE_IMAGEM {
            let fila = self.fila_de_imagens.clone();
            let emissor = self.emissor.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || loop {
                let (chave, url, lado) = {
                    let mut pedidos = fila.pedidos.lock().unwrap();
                    loop {
                        // A mais nova primeiro: é a que está na tela agora.
                        if let Some(pedido) = pedidos.pop_back() {
                            break pedido;
                        }
                        pedidos = fila.chegou.wait(pedidos).unwrap();
                    }
                };
                let cores = rede::bytes(&url, 8 * 1024 * 1024)
                    .and_then(|bytes| image::load_from_memory(&bytes).ok())
                    .map(|imagem| {
                        let imagem = imagem.thumbnail(lado, lado).to_rgba8();
                        let tamanho = [imagem.width() as usize, imagem.height() as usize];
                        egui::ColorImage::from_rgba_unmultiplied(tamanho, imagem.as_raw())
                    });
                // A que não veio fica sem imagem, como antes: pedir de novo a
                // cada quadro só martelaria um endereço quebrado.
                let Some(cores) = cores else { continue };
                if emissor.send(Recado::Logo(chave, cores)).is_err() {
                    return;
                }
                ctx.request_repaint();
            });
        }
    }

    fn teclado(&mut self, ctx: &egui::Context) {
        let (eventos, texto_digitado, mouse) = ctx.input(|i| {
            (
                i.events.clone(),
                i.events
                    .iter()
                    .filter_map(|e| match e {
                        egui::Event::Text(t) => Some(t.clone()),
                        _ => None,
                    })
                    .collect::<String>(),
                i.pointer.hover_pos(),
            )
        });
        // Mexer o mouse traz os controles de volta, como no Mac.
        if mouse.is_some() && mouse != self.ultimo_mouse {
            self.ultimo_mouse = mouse;
            self.mostrar_controles();
        }
        let escrevendo = ctx.memory(|m| m.focused().is_some());

        // Número de canal, e o código que revela a lista restrita.
        if !escrevendo && self.aba == Aba::Canais {
            for c in texto_digitado.chars().filter(|c| c.is_ascii_digit()) {
                if self.digitado_em.elapsed() > Duration::from_millis(1500) {
                    self.digitado.clear();
                }
                self.digitado.push(c);
                self.digitado_em = Instant::now();
            }
            if texto_digitado.contains('?') {
                self.ajuda_aberta = !self.ajuda_aberta;
            }
        }
        if !self.digitado.is_empty() && self.digitado_em.elapsed() > Duration::from_millis(1200) {
            let digitado = std::mem::take(&mut self.digitado);
            if digitado == CODIGO {
                self.liberado = !self.liberado;
                let aviso = if self.liberado { "lista completa liberada" } else { "lista restrita escondida" };
                self.aviso = Some((aviso.into(), Instant::now()));
                self.reordenar();
                let letra = self.letra.clone();
                self.letra.clear();
                self.abrir_letra(letra);
                if !self.liberado && self.aba == Aba::Extras {
                    self.abrir_secao(Aba::Filmes);
                }
            } else if let Ok(numero) = digitado.parse::<usize>() {
                if numero >= 1 && numero <= self.canais.len() {
                    self.tocar(numero - 1, 0, true);
                    self.foco = numero - 1;
                }
            }
        }

        for evento in eventos {
            let egui::Event::Key { key, pressed: true, modifiers, .. } = evento else { continue };
            // Com a busca aberta as letras são texto; só as teclas de sair e de
            // descer para a lista passam.
            if escrevendo && !matches!(key, egui::Key::Escape | egui::Key::Enter | egui::Key::ArrowDown) {
                continue;
            }
            if escrevendo && matches!(key, egui::Key::Escape | egui::Key::Enter | egui::Key::ArrowDown) {
                ctx.memory_mut(|m| m.request_focus(egui::Id::NULL));
                if key != egui::Key::Enter {
                    continue;
                }
            }
            // Enter com o botão de pular à vista faz o que o botão diz; Esc
            // dispensa o cartão do próximo episódio.
            if self.aba == Aba::Canais && self.tocando_vod.is_some() {
                if key == egui::Key::Enter && self.acao_na_tela() {
                    continue;
                }
                if key == egui::Key::Escape && self.proximo_na_tela().is_some() {
                    self.dispensar_proximo();
                    continue;
                }
            }
            match key {
                egui::Key::Space => self.alternar_pausa(),
                egui::Key::F if modifiers.ctrl => {
                    ctx.memory_mut(|m| m.request_focus(egui::Id::new("busca-canais")));
                    self.lista_aberta = true;
                }
                egui::Key::F | egui::Key::F11 => self.alternar_tela_cheia(ctx),
                egui::Key::F1 | egui::Key::H => self.ajuda_aberta = !self.ajuda_aberta,
                egui::Key::G => {
                    self.guia_aberto = !self.guia_aberto;
                    self.abrir_secao(Aba::Canais);
                }
                egui::Key::L => self.lista_aberta = !self.lista_aberta,
                egui::Key::M => self.alternar_mudo(),
                egui::Key::Plus | egui::Key::Equals => self.mudar_volume(5),
                egui::Key::Minus => self.mudar_volume(-5),
                egui::Key::PageUp => self.pular_canal(-1),
                egui::Key::PageDown => self.pular_canal(1),
                egui::Key::D if self.aba == Aba::Canais => {
                    if self.outra_tela.is_some() {
                        self.sair_da_multi_tela();
                    } else {
                        self.entrar_na_multi_tela();
                    }
                }
                egui::Key::T if self.outra_tela.is_some() => self.trocar_tela_em_foco(),
                egui::Key::P => self.alternar_janelinha(ctx),
                egui::Key::E if self.aba == Aba::Canais && self.tocando_vod.as_ref().is_some_and(|t| t.episodio.is_some()) => {
                    self.episodios_no_player = !self.episodios_no_player;
                }
                egui::Key::Tab => {
                    // A ordem do menu do topo, como na TV Box.
                    let proxima = match self.aba {
                        Aba::Buscar => Aba::Inicio,
                        Aba::Inicio => Aba::Canais,
                        Aba::Canais => Aba::Eventos,
                        Aba::Eventos => Aba::OnDemand,
                        Aba::OnDemand | Aba::Filmes | Aba::Series | Aba::Animes | Aba::Doramas => Aba::Favoritos,
                        Aba::Favoritos => Aba::Radios,
                        Aba::Radios => Aba::Ajustes,
                        _ => Aba::Buscar,
                    };
                    self.abrir_secao(proxima);
                }
                egui::Key::Escape => {
                    if self.ficha_aberta.is_some() {
                        if self.ficha_ator.is_some() {
                            self.ficha_ator = None;
                            self.filmografia.clear();
                        } else {
                            self.fechar_ficha();
                        }
                    } else if self.ajuda_aberta {
                        self.ajuda_aberta = false;
                    } else if self.episodios_no_player {
                        self.episodios_no_player = false;
                    } else if self.serie_aberta.is_some() {
                        self.serie_aberta = None;
                    } else if self.explorar && self.aba.de_tipo() {
                        self.explorar = false;
                        self.foco_vod = 0;
                    } else if matches!(self.aba, Aba::Filmes | Aba::Series | Aba::Animes | Aba::Doramas) {
                        // Página de um tipo: volta ao On Demand, de onde ela abriu.
                        self.abrir_secao(Aba::OnDemand);
                    } else if self.aba != Aba::Canais {
                        self.abrir_secao(Aba::Canais);
                    } else if self.guia_aberto {
                        self.guia_aberto = false;
                    } else if ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)) {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                    } else {
                        self.lista_aberta = !self.lista_aberta;
                    }
                }
                _ if self.aba != Aba::Canais => self.tecla_no_acervo(key),
                egui::Key::ArrowDown | egui::Key::ArrowUp => {
                    let visiveis = self.visiveis();
                    let atual = visiveis.iter().position(|i| *i == self.foco).unwrap_or(0);
                    let novo = if key == egui::Key::ArrowDown {
                        (atual + 1).min(visiveis.len().saturating_sub(1))
                    } else {
                        atual.saturating_sub(1)
                    };
                    if let Some(i) = visiveis.get(novo) {
                        self.foco = *i;
                        self.lista_aberta = true;
                    }
                }
                egui::Key::Enter => {
                    let foco = self.foco;
                    if self.visiveis().contains(&foco) {
                        self.tocar(foco, 0, true);
                    }
                }
                egui::Key::ArrowRight => {
                    if self.tocando_vod.is_some() {
                        self.avancar(10.0);
                    } else {
                        self.escolher_fonte_seguinte();
                    }
                }
                egui::Key::ArrowLeft => {
                    if self.tocando_vod.is_some() {
                        self.avancar(-10.0);
                    }
                }
                egui::Key::S => {
                    if let Some(canal) = self.canais.get(self.foco).map(|c| c.nome.clone()) {
                        self.favoritar(canal);
                    }
                }
                _ => {}
            }
        }
    }

    /// Setas, Enter e estrela dentro das fileiras da tela inicial.
    ///
    /// Esquerda e direita andam dentro da fileira; cima e baixo trocam de
    /// fileira mantendo a coluna, que é como toda TV se comporta e como a
    /// pessoa espera depois de dois segundos mexendo.
    fn tecla_nas_fileiras(&mut self, key: egui::Key) {
        let tamanhos = tela::tamanho_das_filas(self);
        if tamanhos.is_empty() {
            return;
        }
        self.foco_fila = self.foco_fila.min(tamanhos.len() - 1);
        let nesta = tamanhos[self.foco_fila];
        match key {
            egui::Key::ArrowRight => {
                self.foco_vod = (self.foco_vod + 1).min(nesta.saturating_sub(1));
            }
            egui::Key::ArrowLeft => self.foco_vod = self.foco_vod.saturating_sub(1),
            egui::Key::ArrowDown => {
                if self.foco_fila + 1 < tamanhos.len() {
                    self.foco_fila += 1;
                    self.foco_vod = self.foco_vod.min(tamanhos[self.foco_fila].saturating_sub(1));
                }
            }
            egui::Key::ArrowUp => {
                if self.foco_fila > 0 {
                    self.foco_fila -= 1;
                    self.foco_vod = self.foco_vod.min(tamanhos[self.foco_fila].saturating_sub(1));
                }
            }
            egui::Key::Enter => tela::abrir_em_foco_na_fileira(self),
            egui::Key::S => tela::favoritar_em_foco_na_fileira(self),
            // I abre a ficha: sinopse, duração, gêneros e elenco, sem abrir
            // o filme para descobrir do que ele trata.
            egui::Key::I => tela::ficha_em_foco_na_fileira(self),
            _ => {}
        }
    }

    /// Setas, Enter e estrela dentro da grade do acervo.
    fn tecla_no_acervo(&mut self, key: egui::Key) {
        if self.aba == Aba::Ajustes || (self.aba == Aba::Buscar && self.busca_vod.trim().is_empty()) {
            return;
        }
        if self.serie_aberta.is_none() && tela::em_fileiras(self) {
            self.tecla_nas_fileiras(key);
            return;
        }
        let colunas = self.colunas_vod.max(1);
        let total = self.itens_do_acervo();
        match key {
            egui::Key::ArrowRight => self.foco_vod = (self.foco_vod + 1).min(total.saturating_sub(1)),
            egui::Key::ArrowLeft => self.foco_vod = self.foco_vod.saturating_sub(1),
            egui::Key::ArrowDown => {
                let passo = if self.serie_aberta.is_some() { 1 } else { colunas };
                self.foco_vod = (self.foco_vod + passo).min(total.saturating_sub(1));
            }
            egui::Key::ArrowUp => {
                let passo = if self.serie_aberta.is_some() { 1 } else { colunas };
                self.foco_vod = self.foco_vod.saturating_sub(passo);
            }
            egui::Key::Enter => self.abrir_do_acervo(),
            egui::Key::S => {
                if let Some(titulo) = self.titulo_em_foco() {
                    self.favoritar_vod(titulo);
                }
            }
            // I abre a ficha: sinopse, duração, gêneros e elenco, sem abrir
            // o filme para descobrir do que ele trata.
            egui::Key::I => {
                if let Some(item) = self.itens_na_tela().get(self.foco_vod).cloned() {
                    self.abrir_ficha(item.titulo.clone(), item.serie, Destino::Item(item));
                }
            }
            _ => {}
        }
    }

    /// O que a grade mostra: um título, o bastante para desenhar o cartão, e a
    /// letra onde mora o endereço dele.
    ///
    /// O acervo é publicado por letra porque são 30 MB, mas o índice de busca
    /// tem os trinta mil nomes em 800 KB — então a grade lista tudo a partir
    /// dele e só baixa a letra quando alguém abre um título. O 18+ fica de
    /// fora: ele não entra no índice, e continua vindo uma letra por vez.
    pub fn itens_na_tela(&self) -> std::rc::Rc<Vec<ItemNaTela>> {
        let chave = self.chave_da_grade();
        if let Some((guardada, itens)) = self.cache_itens.borrow().as_ref() {
            if *guardada == chave {
                return itens.clone();
            }
        }
        let itens = std::rc::Rc::new(self.montar_itens());
        *self.cache_itens.borrow_mut() = Some((chave, itens.clone()));
        itens
    }

    /// Tudo de que a grade depende, resumido num número: mudou um, remonta.
    fn chave_da_grade(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.versao_dados.hash(&mut h);
        (self.aba as u8).hash(&mut h);
        self.busca_vod.hash(&mut h);
        self.genero.hash(&mut h);
        self.letra.hash(&mut h);
        self.liberado.hash(&mut h);
        self.ordem_az.hash(&mut h);
        self.favoritos_vod.hash(&mut h);
        (self.acervo.len(), self.filmes.len(), self.series.len()).hash(&mut h);
        h.finish()
    }

    fn montar_itens(&self) -> Vec<ItemNaTela> {
        let busca = catalogo::chave_de_ordem(self.busca_vod.trim());
        let serie = matches!(self.aba, Aba::Series | Aba::Animes | Aba::Doramas);

        // Coleções e 18+ já estão inteiros na memória; o resto vem do índice.
        if matches!(self.aba, Aba::Animes | Aba::Doramas) {
            return self
                .series_na_tela()
                .into_iter()
                .map(|s| {
                    let ano = if s.ano.is_empty() { String::new() } else { format!("{} · ", s.ano) };
                    ItemNaTela {
                        detalhe: format!("{ano}{} episódios", s.episodios),
                        titulo: s.titulo,
                        serie: true,
                        letra: self.letra.clone(),
                    }
                })
                .collect();
        }
        if self.aba == Aba::Extras {
            return self
                .filmes_na_tela()
                .into_iter()
                .map(|f| ItemNaTela {
                    detalhe: f.versoes.iter().map(|(v, _)| v.to_uppercase()).collect::<Vec<_>>().join(" · "),
                    titulo: f.titulo,
                    serie: false,
                    letra: self.letra.clone(),
                })
                .collect();
        }

        let mut itens: Vec<ItemNaTela> = self.acervo
            .iter()
            .filter(|a| a.serie == serie || self.aba == Aba::Favoritos)
            // Nome sem nenhuma letra nem número ("(2024)") é lixo da lista de
            // origem: abria a grade como primeiro cartão, sem capa.
            .filter(|a| tem_nome(&a.titulo))
            .filter(|a| self.aba != Aba::Favoritos || self.favoritos_vod.contains(&a.titulo))
            .filter(|a| busca.is_empty() || catalogo::chave_de_ordem(&a.titulo).contains(&busca))
            .filter(|a| self.generos.tem(&a.titulo, a.serie, &self.genero))
            .map(|a| ItemNaTela {
                titulo: a.titulo.clone(),
                detalhe: a.ano.clone(),
                serie: a.serie,
                letra: a.letra.clone(),
            })
            .collect();
        // Sem busca, o mais novo primeiro: é o que a pessoa procura ao abrir
        // Filmes, e não "#Alive" só porque "#" vem antes de "A".
        if busca.is_empty() && self.ordem_az {
            let simbolo = |i: &ItemNaTela| !i.titulo.chars().next().is_some_and(|c| c.is_alphanumeric());
            itens.sort_by_cached_key(|i| (simbolo(i), catalogo::chave_de_ordem(&i.titulo)));
        } else if busca.is_empty() {
            let ano = |i: &ItemNaTela| {
                i.detalhe.get(..4).and_then(|a| a.parse::<u32>().ok()).or_else(|| {
                    let t = i.titulo.trim_end();
                    t.ends_with(')').then_some(t.len()).filter(|n| *n >= 6).and_then(|n| t.get(n - 5..n - 1)?.parse().ok())
                })
                // "Blade Runner 2049" não é de 2049.
                .filter(|a| (1900..=2027).contains(a))
                .unwrap_or(0)
            };
            let simbolo = |i: &ItemNaTela| !i.titulo.chars().next().is_some_and(|c| c.is_alphanumeric());
            // A chave de cada um calculada uma vez só: no sort_by ela era
            // refeita a cada comparação, centenas de milhares de vezes.
            itens.sort_by_cached_key(|i| {
                (simbolo(i), std::cmp::Reverse(ano(i)), catalogo::chave_de_ordem(&i.titulo))
            });
        }
        itens
    }

    /// Abre um título da grade: a letra dele pode ainda não ter chegado, e aí
    /// a abertura fica esperando o recado com a lista.
    pub fn abrir_item(&mut self, item: ItemNaTela) {
        if self.letra != item.letra && self.aba != Aba::Extras && !matches!(self.aba, Aba::Animes | Aba::Doramas) {
            self.abrir_letra(item.letra.clone());
        }
        if item.serie {
            if let Some(s) = self.series.iter().find(|s| s.titulo == item.titulo).cloned() {
                self.foco_vod = 0;
                self.abrir_serie(s);
                return;
            }
        } else if let Some(f) = self.filmes.iter().find(|f| f.titulo == item.titulo).cloned() {
            if let Some((_, urls)) = f.versoes.first().cloned() {
                self.tocar_filme(f.titulo, urls);
            }
            return;
        }
        self.abrir_ao_chegar = Some((item.titulo, item.serie));
        self.carregando_vod = true;
    }

    /// Chamado quando a lista de uma letra chega: se alguém estava esperando
    /// um título dela, abre agora.
    fn abrir_o_que_esperava(&mut self) {
        let Some((titulo, serie)) = self.abrir_ao_chegar.clone() else { return };
        if serie {
            if let Some(s) = self.series.iter().find(|s| s.titulo == titulo).cloned() {
                self.abrir_ao_chegar = None;
                self.foco_vod = 0;
                self.abrir_serie(s);
            }
        } else if let Some(f) = self.filmes.iter().find(|f| f.titulo == titulo).cloned() {
            self.abrir_ao_chegar = None;
            if let Some((_, urls)) = f.versoes.first().cloned() {
                self.tocar_filme(f.titulo, urls);
            }
        }
    }

    /// Filmes da seção aberta, já filtrados pela busca e pela seção.
    pub fn filmes_na_tela(&self) -> Vec<vod::Filme> {
        // A lista de gêneros guarda o título como o acervo o escreve, sem o
        // ano que às vezes vem colado no nome.

        let busca = catalogo::chave_de_ordem(self.busca_vod.trim());
        self.filmes
            .iter()
            .filter(|f| match self.aba {
                Aba::Favoritos => self.favoritos_vod.contains(&f.titulo),
                Aba::Extras => f.reservado,
                _ => !f.reservado || self.liberado,
            })
            .filter(|f| busca.is_empty() || catalogo::chave_de_ordem(&f.titulo).contains(&busca))
            .filter(|f| self.generos.tem(&f.titulo, false, &self.genero))
            .cloned()
            .collect()
    }

    pub fn series_na_tela(&self) -> Vec<vod::Serie> {
        let busca = catalogo::chave_de_ordem(self.busca_vod.trim());
        self.series
            .iter()
            .filter(|s| self.aba != Aba::Favoritos || self.favoritos_vod.contains(&s.titulo))
            .filter(|s| busca.is_empty() || catalogo::chave_de_ordem(&s.titulo).contains(&busca))
            .filter(|s| self.generos.tem(&s.titulo, true, &self.genero))
            .cloned()
            .collect()
    }

    fn itens_do_acervo(&self) -> usize {
        if self.serie_aberta.is_none() && tela::em_fileiras(self) {
            return tela::tamanho_das_filas(self).get(self.foco_fila).copied().unwrap_or(0);
        }
        if self.serie_aberta.is_some() {
            self.episodios.len()
        } else {
            self.itens_na_tela().len()
        }
    }

    fn titulo_em_foco(&self) -> Option<String> {
        self.itens_na_tela().get(self.foco_vod).map(|i| i.titulo.clone())
    }

    /// Enter no acervo: toca o filme ou o episódio, ou abre a série.
    pub fn abrir_do_acervo(&mut self) {
        if let Some(serie) = self.serie_aberta.clone() {
            let Some(episodio) = self.episodios.get(self.foco_vod).cloned() else { return };
            let lista = self.episodios.clone();
            self.tocar_episodio(&serie.titulo, episodio, lista);
            return;
        }
        // O título abre a ficha, como no celular e na TV Box: saber do que o
        // filme trata antes de gastar dois minutos abrindo a fonte.
        if let Some(item) = self.itens_na_tela().get(self.foco_vod).cloned() {
            self.abrir_ficha(item.titulo.clone(), item.serie, Destino::Item(item));
        }
    }

    // MARK: - Eventos

    /// Com a tela de Eventos aberta, a agenda é buscada na hora e de novo a
    /// cada minuto: o placar anda.
    fn cuidar_dos_eventos(&mut self, ctx: &egui::Context) {
        if self.aba != Aba::Eventos {
            return;
        }
        let falta = self
            .eventos_pedidos
            .map(|quando| ATUALIZAR_EVENTOS.saturating_sub(quando.elapsed()))
            .unwrap_or(Duration::ZERO);
        // Sem isto, a próxima busca (e a troca de grupo de quem começou) só
        // aconteceria no próximo movimento do mouse.
        ctx.request_repaint_after(falta.max(Duration::from_secs(1)));
        if !falta.is_zero() || self.buscando_eventos {
            return;
        }
        // Canal adulto não entra na agenda.
        let canais: Vec<(String, Option<String>)> = self
            .canais
            .iter()
            .filter(|c| c.secao() != "Adulto")
            .map(|c| (c.nome.clone(), c.logo.clone()))
            .collect();
        // A lista de canais ainda não chegou: sem ela não há jogo para mostrar.
        if canais.is_empty() {
            return;
        }
        self.eventos_pedidos = Some(Instant::now());
        self.buscando_eventos = true;
        let emissor = self.emissor.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = emissor.send(Recado::Eventos(eventos::buscar(&canais)));
            ctx.request_repaint();
        });
    }

    /// Os jogos da tela de Eventos, na ordem em que aparecem.
    pub fn jogos_na_tela(&self) -> Vec<(usize, Vec<eventos::Jogo>)> {
        eventos::agrupar(&self.eventos, eventos::agora(), &self.busca_vod)
    }

    /// O cartão do jogo abre o canal ao vivo, como a lista de canais.
    pub fn abrir_jogo(&mut self, canal: &str) {
        let Some(indice) = self.canais.iter().position(|c| c.nome == canal) else {
            self.aviso = Some((format!("{canal} não está na lista agora"), Instant::now()));
            return;
        };
        self.foco = indice;
        self.tocar(indice, 0, true);
        self.abrir_secao(Aba::Canais);
    }

    // MARK: - Comandos da barra

    pub fn mostrar_controles(&mut self) {
        self.controles_ate = Instant::now() + Duration::from_secs(3);
    }

    pub fn controles_visiveis(&self) -> bool {
        Instant::now() < self.controles_ate || self.pausado
    }

    pub fn alternar_pausa(&mut self) {
        // Ao vivo não pausa: só filme e série param.
        self.pausado = self.tocando_vod.is_some() && !self.pausado;
        if let Some(mpv) = self.mpv.as_ref() {
            mpv.pausa(self.pausado);
        }
        self.mostrar_controles();
    }

    pub fn alternar_mudo(&mut self) {
        self.mudo = !self.mudo;
        if let Some(mpv) = self.mpv.as_ref() {
            mpv.mudo(self.mudo);
        }
        self.mostrar_controles();
    }

    pub fn definir_volume(&mut self, valor: i32) {
        self.volume = valor.clamp(0, 130);
        if let Some(mpv) = self.mpv.as_ref() {
            mpv.volume(self.volume);
        }
    }

    /// Canal vizinho na lista que está na tela; -1 é o de cima.
    pub fn pular_canal(&mut self, passo: i64) {
        let visiveis = self.visiveis();
        if visiveis.is_empty() {
            return;
        }
        let atual = self
            .tocando
            .as_ref()
            .and_then(|t| visiveis.iter().position(|i| *i == t.canal))
            .unwrap_or(0) as i64;
        let novo = (atual + passo).rem_euclid(visiveis.len() as i64) as usize;
        let canal = visiveis[novo];
        self.foco = canal;
        self.tocar(canal, 0, true);
    }

    /// A janelinha por cima de tudo, sem moldura. É de uma tela só.
    pub fn alternar_janelinha(&mut self, ctx: &egui::Context) {
        self.sair_da_multi_tela();
        self.pip = !self.pip;
        let nivel = if self.pip { egui::viewport::WindowLevel::AlwaysOnTop } else { egui::viewport::WindowLevel::Normal };
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(nivel));
        ctx.send_viewport_cmd(egui::ViewportCommand::Decorations(!self.pip));
        if self.pip {
            self.lista_aberta = false;
            // A escala da janela grande ficaria enorme na pequena.
            ctx.set_zoom_factor(1.0);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize([400.0, 225.0].into()));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize([1280.0, 720.0].into()));
        }
    }

    /// O voltar do player: para o filme (guardando onde parou) e volta à
    /// página de onde ele saiu — a da série, no episódio.
    pub fn voltar_do_player(&mut self) {
        let serie = self.tocando_vod.as_ref().and_then(|t| t.episodio.as_ref()).map(|e| e.serie.clone());
        let destino = if serie.is_some() { Aba::Series } else { Aba::Filmes };
        self.abrir_secao(destino);
        if let Some(nome) = serie {
            if let Some(s) = self.series.iter().find(|s| s.titulo == nome).cloned() {
                self.foco_vod = 0;
                self.abrir_serie(s);
            }
        }
    }

    /// Outro episódio da mesma série, escolhido no painel do player.
    pub fn trocar_de_episodio(&mut self, episodio: vod::Episodio) {
        let Some(ep) = self.tocando_vod.as_ref().and_then(|t| t.episodio.clone()) else { return };
        self.tocar_episodio(&ep.serie, episodio, ep.lista);
    }

    pub fn alternar_tela_cheia(&self, ctx: &egui::Context) {
        let cheia = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!cheia));
    }

    /// Avança ou volta no filme (segundos negativos voltam).
    pub fn avancar(&mut self, segundos: f64) {
        let Some(mpv) = self.mpv.as_ref() else { return };
        if let Some((posicao, duracao)) = mpv.posicao() {
            mpv.ir_para((posicao + segundos).clamp(0.0, duracao - 1.0));
        }
        self.mostrar_controles();
    }

    pub fn ir_para(&mut self, segundos: f64) {
        if let Some(mpv) = self.mpv.as_ref() {
            mpv.ir_para(segundos);
        }
    }

    pub fn definir_velocidade(&mut self, velocidade: f32) {
        self.velocidade = velocidade;
        if let Some(mpv) = self.mpv.as_ref() {
            mpv.definir("speed", &format!("{velocidade}"));
        }
        self.aviso = Some((format!("velocidade {velocidade}×"), Instant::now()));
    }

    /// Preencher a tela corta as bordas em vez de deixar faixas pretas.
    pub fn alternar_preencher(&mut self) {
        self.preencher = !self.preencher;
        if let Some(mpv) = self.mpv.as_ref() {
            mpv.definir("panscan", if self.preencher { "1.0" } else { "0.0" });
        }
    }

    pub fn escolher_fonte(&mut self, fonte: usize) {
        if let Some(canal) = self.tocando.as_ref().map(|t| t.canal) {
            self.tocar(canal, fonte, false);
        } else if let Some(filme) = self.tocando_vod.as_ref() {
            let (titulo, urls) = (filme.titulo.clone(), filme.urls.clone());
            self.tocar_vod(titulo, urls, fonte, false);
        }
    }

    pub fn abrir_secao(&mut self, aba: Aba) {
        if self.aba == aba {
            return;
        }
        if aba == Aba::Canais {
            self.retomar_canal_parado();
        } else if self.aba == Aba::Canais {
            self.parar_o_que_toca();
        }
        let colecao = |a: Aba| matches!(a, Aba::Animes | Aba::Doramas);
        if colecao(aba) && !colecao(self.aba) {
            self.series_da_letra = Some(std::mem::take(&mut self.series));
        } else if !colecao(aba) && colecao(self.aba) {
            if let Some(series) = self.series_da_letra.take() {
                self.series = series;
            }
            self.episodios_colecao.clear();
        }
        self.aba = aba;
        self.busca_vod.clear();
        self.foco_vod = 0;
        self.serie_aberta = None;
        self.foco_fila = 0;
        self.explorar = false;
        self.heroi = None;
        self.episodios_no_player = false;
        // As fileiras de destaque abrem as páginas do acervo e a busca.
        let com_destaques = matches!(aba, Aba::Inicio | Aba::OnDemand | Aba::Buscar) || aba.de_tipo();
        if com_destaques && self.filas.is_empty() && !self.carregando_filas {
            self.carregando_filas = true;
            let emissor = self.emissor.clone();
            std::thread::spawn(move || {
                let _ = emissor.send(Recado::Destaques(destaques::filas()));
            });
        }
        if matches!(aba, Aba::Inicio | Aba::OnDemand) {
            if self.filas.is_empty() {
                self.carregando_vod = true;
            }
            // Os cartões de Animes e Doramas do On Demand contam os títulos
            // das coleções: elas vêm por trás, sem trocar de seção.
            if aba == Aba::OnDemand {
                self.pedir_colecoes();
            }
            return;
        }
        if aba == Aba::Radios {
            self.buscar_radios();
            return;
        }
        if matches!(aba, Aba::Buscar | Aba::Ajustes) {
            return;
        }
        if matches!(aba, Aba::Animes | Aba::Doramas) {
            if let Some(lista) = self.colecoes.get(&(aba as u8)).cloned() {
                self.mostrar_colecao(lista);
                return;
            }
            self.series.clear();
            self.episodios_colecao.clear();
            self.carregando_vod = true;
            let emissor = self.emissor.clone();
            std::thread::spawn(move || {
                let tipo = if aba == Aba::Animes { "animes" } else { "doramas" };
                let _ = emissor.send(Recado::Colecao(aba, vod::colecao(tipo)));
            });
            return;
        }
        if aba == Aba::Eventos {
            // Volta para a tela: a agenda é buscada de novo na hora.
            self.eventos_pedidos = None;
            return;
        }
        if aba != Aba::Canais && self.filmes.is_empty() && self.series.is_empty() {
            let letra = self.letra.clone();
            self.letra.clear();
            self.abrir_letra(letra);
        }
    }

    /// A coleção de animes ou de doramas, quando já chegou.
    pub fn colecao(&self, aba: Aba) -> Option<&Vec<(vod::Serie, Vec<vod::Episodio>)>> {
        self.colecoes.get(&(aba as u8))
    }

    /// Quais coleções já chegaram: parte da chave das fileiras.
    pub fn colecoes_carregadas(&self) -> (usize, usize) {
        (
            self.colecao(Aba::Animes).map(|l| l.len()).unwrap_or(0),
            self.colecao(Aba::Doramas).map(|l| l.len()).unwrap_or(0),
        )
    }

    /// "Procurar atualização" dos Ajustes: a mesma procura de toda hora, já.
    pub fn procurar_atualizacao_agora(&mut self) {
        self.aviso = Some(("Procurando versão nova…".into(), Instant::now()));
        let emissor = self.emissor.clone();
        std::thread::spawn(move || {
            let _ = emissor.send(match atualizacao::procurar() {
                Some(v) => Recado::Atualizacao(v),
                None => Recado::SemAtualizacao,
            });
        });
    }

    pub fn buscando_radios(&self) -> bool {
        self.buscando_radios
    }

    /// Baixa animes e doramas que ainda não estão guardados, sem mostrá-los.
    fn pedir_colecoes(&mut self) {
        if self.colecoes_pedidas {
            return;
        }
        self.colecoes_pedidas = true;
        for aba in [Aba::Animes, Aba::Doramas] {
            if self.colecoes.contains_key(&(aba as u8)) {
                continue;
            }
            let emissor = self.emissor.clone();
            std::thread::spawn(move || {
                let tipo = if aba == Aba::Animes { "animes" } else { "doramas" };
                let _ = emissor.send(Recado::Colecao(aba, vod::colecao(tipo)));
            });
        }
    }

    // MARK: - Rádios

    /// A lista do repositório por trás; a guardada já está na tela.
    fn buscar_radios(&mut self) {
        if self.buscando_radios {
            return;
        }
        self.buscando_radios = true;
        let emissor = self.emissor.clone();
        std::thread::spawn(move || {
            if let Some(lista) = radios::baixar() {
                let _ = emissor.send(Recado::Radios(lista));
            }
        });
    }

    /// Toca a rádio no mpv do vídeo, só com o som. O que tocava para.
    pub fn tocar_radio(&mut self, radio: radios::Radio) {
        self.sair_da_multi_tela();
        if self.tocando_vod.is_some() {
            self.guardar_progresso(true);
            self.tocando_vod = None;
        }
        self.tocando = None;
        self.canal_parado = None;
        let Some(mpv) = self.mpv.as_ref() else {
            self.aviso = Some(("O player não abriu: a rádio não tem como tocar".into(), Instant::now()));
            return;
        };
        mpv.tocar(&radio.url, None, None);
        mpv.pausa(false);
        self.pausado = false;
        radios::ouviu(&radio.nome);
        self.versao_dados = self.versao_dados.wrapping_add(1);
        self.radio = Some(TocandoRadio {
            nome: radio.nome,
            url: radio.url,
            logo: radio.logo,
            desde: Instant::now(),
            confirmado: false,
        });
    }

    pub fn parar_radio(&mut self) {
        if self.radio.take().is_some() {
            if let Some(mpv) = self.mpv.as_ref() {
                mpv.parar();
            }
        }
    }

    // MARK: - Fichas do destaque

    /// Pede a ficha de um título para o destaque do topo, uma vez só.
    pub fn pedir_ficha_do_destaque(&mut self, titulo: &str, serie: bool) {
        if ficha::conhecida(titulo, serie).is_some() {
            return;
        }
        let chave = format!("{serie}|{titulo}");
        if !self.fichas_pedidas.insert(chave) {
            return;
        }
        let Some(id) = self.generos.id(titulo, serie) else { return };
        let (titulo, emissor) = (titulo.to_string(), self.emissor.clone());
        std::thread::spawn(move || {
            ficha::baixar(&titulo, serie, id);
            let _ = emissor.send(Recado::FichaDoDestaque);
        });
    }

    /// Os episódios de uma temporada pelo TMDB: nome, sinopse e imagem.
    pub fn pedir_temporada(&mut self, serie: &str, temporada: u32) -> Option<Vec<ficha::EpisodioTmdb>> {
        let id = self.generos.id(serie, true)?;
        if let Some(lista) = ficha::temporada_conhecida(id, temporada) {
            return Some(lista);
        }
        if self.temporadas_pedidas.insert((id, temporada)) {
            let emissor = self.emissor.clone();
            std::thread::spawn(move || {
                ficha::temporada(id, temporada);
                let _ = emissor.send(Recado::Temporada);
            });
        }
        None
    }

    /// O acervo cobre a tela inteira: o que estava tocando para de vez, em
    /// vez de seguir por trás gastando rede, processador e som.
    fn parar_o_que_toca(&mut self) {
        self.sair_da_multi_tela();
        if self.tocando_vod.is_some() {
            self.guardar_progresso(true);
            self.tocando_vod = None;
        } else if let Some(t) = self.tocando.take() {
            self.canal_parado = self.canais.get(t.canal).map(|c| c.nome.clone());
        } else {
            return;
        }
        telemetria::parou();
        if let Some(mpv) = self.mpv.as_ref() {
            mpv.parar();
        }
    }

    /// Volta ao canal que `parar_o_que_toca` parou, se nada mais começou.
    fn retomar_canal_parado(&mut self) {
        let Some(nome) = self.canal_parado.take() else { return };
        if self.tocando.is_some() || self.tocando_vod.is_some() {
            return;
        }
        if let Some(indice) = self.canais.iter().position(|c| c.nome == nome) {
            self.foco = indice;
            self.tocar(indice, 0, true);
        }
    }

    // MARK: - Multi tela

    /// Abre a segunda tela. A tela nova fica em foco, vazia, com a lista
    /// aberta: o próximo canal escolhido vai para ela. A que já tocava segue
    /// à esquerda, sem som.
    pub fn entrar_na_multi_tela(&mut self) {
        if self.outra_tela.is_some() || self.mpv.is_none() {
            return;
        }
        if self.tocando_vod.is_some() {
            self.aviso = Some(("A multi tela é para os canais ao vivo".into(), Instant::now()));
            return;
        }
        let nova = match self.mpv_reserva.take() {
            Some(mpv) => mpv,
            None => match mpv::Mpv::novo(&caminho_do_mpv()) {
                Ok(mpv) => std::sync::Arc::new(mpv),
                Err(erro) => {
                    telemetria::erro(erro);
                    self.aviso = Some(("Não deu para abrir a segunda tela".into(), Instant::now()));
                    return;
                }
            },
        };
        let Some(antiga) = self.mpv.replace(nova) else { return };
        let tocando = self.tocando.take();
        self.outra_tela = Some(OutraTela { mpv: antiga, tocando });
        self.foco_a_direita = true;
        self.ajustar_som_das_telas();
        self.aba = Aba::Canais;
        self.lista_aberta = true;
        self.guia_aberto = false;
        self.aviso = Some(("Multi tela: escolha o canal da segunda tela".into(), Instant::now()));
    }

    /// Fecha a tela que não está em foco; a em foco ocupa a janela de novo.
    pub fn sair_da_multi_tela(&mut self) {
        let Some(outra) = self.outra_tela.take() else { return };
        outra.mpv.parar();
        self.mpv_reserva = Some(outra.mpv);
        self.foco_a_direita = false;
        self.ajustar_som_das_telas();
    }

    /// Põe em foco a outra tela: o som, o teclado e a lista passam para ela.
    pub fn trocar_tela_em_foco(&mut self) {
        let Some(outra) = self.outra_tela.as_mut() else { return };
        let Some(atual) = self.mpv.as_mut() else { return };
        std::mem::swap(atual, &mut outra.mpv);
        std::mem::swap(&mut self.tocando, &mut outra.tocando);
        self.foco_a_direita = !self.foco_a_direita;
        if let Some(t) = self.tocando.as_ref() {
            self.foco = t.canal;
        }
        self.ajustar_som_das_telas();
        self.letreiro_ate = Instant::now() + SUMIR_LETREIRO;
    }

    /// Só a tela em foco tem som: duas transmissões falando juntas não
    /// servem a ninguém.
    fn ajustar_som_das_telas(&self) {
        if let Some(mpv) = self.mpv.as_ref() {
            mpv.volume(self.volume);
            mpv.mudo(self.mudo);
        }
        if let Some(outra) = self.outra_tela.as_ref() {
            outra.mpv.mudo(true);
        }
    }

    /// A tela fora de foco cuida das fontes dela como a principal: a que não
    /// abre ou cai dá lugar à seguinte do mesmo canal.
    fn cuidar_da_outra_tela(&mut self) {
        let Some(outra) = self.outra_tela.as_mut() else { return };
        let avisos: Vec<mpv::Aviso> = outra.mpv.avisos().collect();
        let mut trocar = avisos.iter().any(|a| matches!(a, mpv::Aviso::Falhou | mpv::Aviso::Fim));
        if avisos.contains(&mpv::Aviso::Tocando) {
            if let Some(t) = outra.tocando.as_mut() {
                t.confirmado = true;
            }
        }
        if let Some(t) = outra.tocando.as_ref() {
            trocar |= !t.confirmado && t.desde.elapsed() > ESPERA_DA_FONTE;
        }
        if !trocar {
            return;
        }
        let Some(t) = outra.tocando.take() else { return };
        let Some(canal) = self.canais.get(t.canal) else { return };
        match canal.fontes.get(t.fonte + 1) {
            Some(fonte) => {
                outra.mpv.tocar(&fonte.url, fonte.referer.as_deref(), fonte.agente.as_deref());
                outra.mpv.mudo(true);
                outra.tocando = Some(Tocando { canal: t.canal, fonte: t.fonte + 1, desde: Instant::now(), confirmado: false });
            }
            None => {
                outra.mpv.parar();
                self.aviso = Some((format!("{}: nenhuma fonte abriu na outra tela", canal.nome), Instant::now()));
            }
        }
    }

    fn mudar_volume(&mut self, passo: i32) {
        self.volume = (self.volume + passo).clamp(0, 130);
        if let Some(mpv) = self.mpv.as_ref() {
            mpv.volume(self.volume);
        }
        self.aviso = Some((format!("volume {}%", self.volume), Instant::now()));
    }

    pub fn favoritar(&mut self, canal: String) {
        if let Some(pos) = self.favoritos.iter().position(|f| *f == canal) {
            self.favoritos.remove(pos);
        } else {
            self.favoritos.push(canal);
        }
        gravar_favoritos(&self.favoritos);
        self.reordenar();
    }
}

impl App {
    /// A abertura animada. Devolve `true` enquanto ela cobre a janela: aí o
    /// app não é desenhado nem ouve o teclado — a tecla que pula a abertura
    /// não pode, de quebra, trocar de canal.
    fn cuidar_da_abertura(&mut self, ctx: &egui::Context) -> bool {
        let Some(a) = self.abertura.as_mut() else { return false };
        let cobria = a.cobrindo();
        if cobria {
            let pulou = ctx.input(|i| {
                i.pointer.any_click()
                    || i.events.iter().any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))
            });
            if pulou {
                a.pular();
            }
        }
        let segue = a.andar();
        let cobrindo = a.cobrindo();
        ctx.request_repaint();
        if cobrindo {
            tela::abertura(ctx, a.mpv.clone().filter(|_| a.com_quadro()), a.veu());
            return true;
        }
        if !segue {
            self.abertura = None;
        }
        if cobria {
            // Saiu da frente agora: o último canal, que esperava, entra.
            self.voltar_ao_ultimo_canal();
        }
        false
    }
}

/// Se o programa foi aberto de dentro do ZIP, sem descompactar.
///
/// Clicando no executável dentro da janela do compactador, o Windows copia só
/// ele para uma pasta temporária e deixa o resto para trás — a libmpv-2.dll
/// fica no ZIP, e o vídeo não tem como abrir. O caminho denuncia: ele passa
/// por uma pasta que termina em ".zip", dentro do Temp do usuário.
///
///   C:\Users\...\AppData\Local\Temp\<código>_SaimoTV-Windows.zip\SaimoTV\
///
/// Vale a pena reconhecer isso em vez de só dizer que falta um arquivo: quem
/// abriu assim não fez nada de errado, e a saída é uma frase, não uma caçada.
pub fn aberto_de_dentro_do_zip() -> bool {
    let Ok(exe) = std::env::current_exe() else { return false };
    let Some(pasta) = exe.parent() else { return false };
    let caminho = pasta.to_string_lossy().to_ascii_lowercase();
    let dentro_de_zip = pasta
        .ancestors()
        .filter_map(|p| p.file_name())
        .any(|nome| nome.to_string_lossy().to_ascii_lowercase().ends_with(".zip"));
    // O Temp sozinho não basta: há quem descompacte ali de propósito.
    dentro_de_zip || caminho.contains("\\temp\\") && caminho.contains(".zip")
}

/// A pasta de onde o programa está rodando, para mostrar no aviso.
pub fn pasta_do_programa() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.display().to_string()))
        .unwrap_or_default()
}

/// No Windows a biblioteca vai no pacote, ao lado do executável. No Mac (só
/// para testar a interface antes de publicar) vale a do sistema.
fn caminho_do_mpv() -> std::path::PathBuf {
    if let Ok(escolhido) = std::env::var("SAIMO_MPV") {
        return std::path::PathBuf::from(escolhido);
    }
    if cfg!(windows) {
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|p| p.join("libmpv-2.dll")))
            .unwrap_or_else(|| std::path::PathBuf::from("libmpv-2.dll"))
    } else {
        [
            "/opt/homebrew/lib/libmpv.dylib",
            "/opt/homebrew/opt/mpv/lib/libmpv.dylib",
            "/usr/local/lib/libmpv.dylib",
        ]
            .iter()
            .map(std::path::PathBuf::from)
            .find(|p| p.exists())
            .unwrap_or_else(|| std::path::PathBuf::from("libmpv.dylib"))
    }
}

/// Conferência visual sem depender de quem está na frente da tela:
/// `SAIMO_FOTO=arquivo.png` salva a própria janela depois de alguns segundos e
/// fecha. `SAIMO_FOTO_TELA` escolhe o que mostrar (filmes, series, eventos, guia,
/// atalhos). Não faz nada sem a variável.
fn foto_de_teste(app: &mut App, ctx: &egui::Context) {
    let Ok(destino) = std::env::var("SAIMO_FOTO") else { return };
    static INICIO: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    static PEDIDA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let inicio = *INICIO.get_or_init(Instant::now);
    ctx.request_repaint_after(Duration::from_millis(200));

    let espera = std::env::var("SAIMO_FOTO_ESPERA").ok().and_then(|v| v.parse().ok()).unwrap_or(18);
    static ESCOLHIDA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    let antes = std::env::var("SAIMO_FOTO_ANTES").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    if inicio.elapsed() > Duration::from_secs(antes) && !ESCOLHIDA.swap(true, std::sync::atomic::Ordering::SeqCst) {
        match std::env::var("SAIMO_FOTO_TELA").unwrap_or_default().as_str() {
            "inicio" => app.abrir_secao(Aba::Inicio),
            "ondemand" => app.abrir_secao(Aba::OnDemand),
            "filmes" => app.abrir_secao(Aba::Filmes),
            "series" => app.abrir_secao(Aba::Series),
            "explorar" => {
                app.abrir_secao(Aba::Filmes);
                app.explorar = true;
            }
            "eventos" => app.abrir_secao(Aba::Eventos),
            "radios" => app.abrir_secao(Aba::Radios),
            "favoritos" => app.abrir_secao(Aba::Favoritos),
            "buscar" => {
                app.abrir_secao(Aba::Buscar);
                app.busca_vod = std::env::var("SAIMO_FOTO_BUSCA").unwrap_or_else(|_| "amor".into());
            }
            "ajustes" => app.abrir_secao(Aba::Ajustes),
            "favoritos-vazio" => app.abrir_secao(Aba::Favoritos),
            "aovivo" => app.lista_aberta = true,
            "aovivo-sem-lista" => app.lista_aberta = false,
            "serie" => {
                let titulo = std::env::var("SAIMO_FOTO_TITULO").unwrap_or_else(|_| "Ted Lasso".into());
                if let Some(a) = app.acervo.iter().find(|a| a.titulo == titulo && a.serie).cloned() {
                    app.abrir_secao(Aba::Series);
                    app.abrir_item(ItemNaTela { titulo: a.titulo, detalhe: a.ano, serie: true, letra: a.letra });
                }
            }
            "player" => {
                // Só a cara do player, sem vídeo: um episódio de mentira.
                app.lista_aberta = false;
                app.tocando_vod = Some(TocandoVod {
                    titulo: "Ted Lasso · T1 E2".into(),
                    urls: vec!["https://exemplo/video.m3u8".into()],
                    fonte: 0,
                    desde: Instant::now(),
                    confirmado: true,
                    episodio: Some(EpisodioTocando { serie: "Ted Lasso".into(), temporada: 1, numero: 2, versao: "dub".into(),
                        lista: (1..=10).map(|n| vod::Episodio { temporada: 1, numero: n, versao: "dub".into(), urls: Vec::new() }).collect() }),
                    marcas: None,
                    legendas: Vec::new(),
                    legenda_escolhida: None,
                    legenda_atraso: 0.0,
                    legenda_aviso: None,
                    visto: (0.0, 0.0),
                    proximo_desde: None,
                    proximo_dispensado: false,
                });
                app.episodios_no_player = std::env::var("SAIMO_FOTO_EPISODIOS").is_ok();
                app.pedir_ficha_do_destaque("Ted Lasso", true);
            }
            "guia" => app.guia_aberto = true,
            "atalhos" => app.ajuda_aberta = true,
            "ficha" => {
                app.abrir_secao(Aba::Inicio);
                let titulo = std::env::var("SAIMO_FOTO_TITULO").unwrap_or_else(|_| "Ted Lasso".into());
                let serie = std::env::var("SAIMO_FOTO_FILME").is_err();
                let item = ItemNaTela { titulo: titulo.clone(), detalhe: String::new(), serie, letra: String::new() };
                app.abrir_ficha(titulo, serie, Destino::Item(item));
            }
            _ => {}
        }
    }
    if inicio.elapsed() > Duration::from_secs(espera) {
        app.mostrar_controles();
        if !PEDIDA.swap(true, std::sync::atomic::Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
    }
    let imagem = ctx.input(|i| {
        i.events.iter().find_map(|e| match e {
            egui::Event::Screenshot { image, .. } => Some(image.clone()),
            _ => None,
        })
    });
    if let Some(imagem) = imagem {
        let [w, h] = imagem.size;
        let bytes: Vec<u8> = imagem.pixels.iter().flat_map(|c| c.to_array()).collect();
        if let Some(buffer) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
            let _ = buffer.save(&destino);
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }
}

fn ler_favoritos() -> Vec<String> {
    ler_lista("favoritos.txt")
}

fn gravar_favoritos(favoritos: &[String]) {
    gravar_lista("favoritos.txt", favoritos);
}

pub fn ler_lista(nome: &str) -> Vec<String> {
    std::fs::read_to_string(catalogo::pasta().join(nome))
        .map(|t| t.lines().map(str::to_string).filter(|l| !l.is_empty()).collect())
        .unwrap_or_default()
}

fn gravar_lista(nome: &str, itens: &[String]) {
    let _ = std::fs::write(catalogo::pasta().join(nome), itens.join("\n"));
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.abertura.is_some() {
            let [r, g, b] = abertura::MARINHO;
            return [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0];
        }
        [0.0, 0.0, 0.0, 1.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = &ui.ctx().clone();
        self.cuidar_dos_recados(ctx);
        self.cuidar_dos_eventos(ctx);
        self.cuidar_do_mpv(ctx);
        self.cuidar_da_midia();
        if self.cuidar_da_abertura(ctx) {
            self.baixar_logos_pendentes();
            return;
        }
        self.teclado(ctx);
        self.baixar_logos_pendentes();

        foto_de_teste(self, ctx);
        tela::desenhar(self, ui);
        if let Some(a) = self.abertura.as_ref() {
            tela::veu_da_abertura(ctx, a.veu());
        }

        if self.pip
            && ui.rect_contains_pointer(ui.max_rect()) && ui.input(|i| i.pointer.primary_down())
                && !ctx.egui_wants_pointer_input() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }

        // Capa e logo pedidos durante o desenho só chegam no quadro seguinte:
        // sem este pedido de redesenho, a lista ficaria parada até alguém mexer.
        if !self.pedidos_de_logo.is_empty() {
            ctx.request_repaint_after(Duration::from_millis(120));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.guardar_progresso(true);
        telemetria::fechando();
    }
}

mod epg;
mod vod;
mod tela;
mod eventos;


/// Um nome de título de verdade tem algo além do ano entre parênteses.
pub fn tem_nome(titulo: &str) -> bool {
    let sem_ano = match titulo.rfind(" (") {
        Some(i) if titulo.ends_with(')') => &titulo[..i],
        _ if titulo.starts_with('(') && titulo.ends_with(')') => "",
        _ => titulo,
    };
    sem_ano.chars().any(|c| c.is_alphanumeric())
}

#[cfg(test)]
mod testes_nome {
    #[test]
    fn so_ano_nao_e_nome() {
        assert!(!super::tem_nome("(2024)"));
        assert!(super::tem_nome("#Alive (2020)"));
        assert!(super::tem_nome("1917"));
    }
}
