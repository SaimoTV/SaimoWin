//! Saimo Connect: o celular manda neste computador como manda na TV Box.
//!
//! O mesmo servidor do `Connect.kt` da TV Box, na porta 8999:
//!
//! 1. `GET /info` — o celular varre a rede pedindo isto a cada endereço e
//!    reconhece o app pelo `"app": "saimo-tv"`.
//! 2. `GET /hls` — a lista do canal na régua comum ([`hls`], [`historico`]),
//!    para o celular tocar o som junto com a imagem daqui. O mpv do próprio
//!    app também toca o ao vivo por ela.
//! 3. `/ws` — o WebSocket do controle remoto: o estado do que está no ar a
//!    cada mudança, os comandos do celular e, enquanto algum celular toca o
//!    som, onde cada tela está na régua, a cada segundo.
//!
//! Celular novo só entra depois de alguém clicar em "Permitir" aqui; a
//! aprovação fica lembrada, e os Ajustes esquecem todos. Sem o anúncio mDNS
//! da TV Box: o app do celular não procura por ele, só pela varredura do
//! `/info`.
//!
//! O que é do app (canais, filmes, som) fica no app: os comandos chegam para
//! ele como [`Pedido`], tratados na thread do desenho, e o estado volta para
//! cá por [`publicar_estado`].

pub mod historico;
pub mod hls;
mod ws;

use eframe::egui;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// A porta que o celular procura. Não muda: o app do celular só conhece esta.
pub const PORTA: u16 = 8999;

/// A porta em que o servidor sobe: a [`PORTA`], ou `SAIMO_CONNECT_PORTA` para
/// testar num computador onde ela já está em uso (o celular não acharia).
pub fn porta() -> u16 {
    static ESCOLHIDA: OnceLock<u16> = OnceLock::new();
    *ESCOLHIDA.get_or_init(|| std::env::var("SAIMO_CONNECT_PORTA").ok().and_then(|p| p.parse().ok()).unwrap_or(PORTA))
}
/// Quanto um celular espera pela resposta de "Permitir" antes de ser recusado.
pub const ESPERA_DA_APROVACAO: Duration = Duration::from_secs(60);
const ARQUIVO_ID: &str = "connect-id.txt";
const ARQUIVO_CELULARES: &str = "connect-celulares.txt";

/// O que o servidor pede ao app.
#[derive(Debug, Clone)]
pub enum Pedido {
    /// Um celular novo quer entrar: o app pergunta e responde por [`responder`].
    Aprovar { cliente: u64, nome: String },
    /// O pedido de aprovação acabou sem resposta (o celular saiu, ou o tempo
    /// passou): a pergunta sai da tela.
    Desistiu { cliente: u64 },
    /// Um celular entrou (o aviso curto na tela).
    Conectou(String),
    /// Um comando do celular: o tipo (`tune`, `key`...) e a mensagem inteira.
    Comando(String, Value),
    /// Mudou quem toca o som no celular.
    SomNoCelular,
}

/// Uma tela tocando, para o aviso de posição.
#[derive(Clone)]
pub struct TelaNoAr {
    pub idx: u32,
    pub mpv: Arc<crate::mpv::Mpv>,
    /// Filme ou série: a régua é a posição no arquivo.
    pub vod: bool,
}

struct Cliente {
    id: u64,
    cid: Option<String>,
    nome: String,
    autenticado: bool,
    envio: Sender<Vec<u8>>,
    socket: TcpStream,
}

#[derive(Clone)]
struct SomNoCelular {
    cid: String,
    nome: String,
    src: String,
}

struct Servidor {
    tv_id: String,
    pedidos: Sender<Pedido>,
    ctx: egui::Context,
    clientes: Mutex<Vec<Cliente>>,
    confiaveis: Mutex<Vec<String>>,
    /// "0" (tela 1), "1" (tela 2) ou "mute".
    tv_audio: Mutex<String>,
    som_no_celular: Mutex<Vec<SomNoCelular>>,
    /// O estado que o app publicou por último, sem o som.
    estado: Mutex<Value>,
    /// O que foi enviado da última vez (sem a posição do filme) e quando.
    enviado: Mutex<(String, Instant)>,
    telas: Mutex<Vec<TelaNoAr>>,
    aprovacoes: Mutex<HashMap<u64, Instant>>,
    proximo_cliente: AtomicU64,
    /// O texto que o celular deve abrir no teclado dele, quando há.
    texto: Mutex<Option<(String, String)>>,
}

static SERVIDOR: OnceLock<Arc<Servidor>> = OnceLock::new();
static ATIVO: AtomicBool = AtomicBool::new(false);

fn agora_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// Um identificador novo, no formato de UUID (v4).
fn uuid_novo() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut bytes = [0u8; 16];
    for (i, pedaco) in bytes.chunks_mut(8).enumerate() {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0));
        h.write_u32(std::process::id());
        h.write_usize(i);
        pedaco.copy_from_slice(&h.finish().to_le_bytes());
    }
    bytes[6] = (bytes[6] & 0x0F) | 0x40;
    bytes[8] = (bytes[8] & 0x3F) | 0x80;
    let h: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &h[..8], &h[8..12], &h[12..16], &h[16..20], &h[20..])
}

fn ler_ou_criar_id() -> String {
    let caminho = crate::catalogo::pasta().join(ARQUIVO_ID);
    if let Ok(id) = std::fs::read_to_string(&caminho) {
        let id = id.trim().to_string();
        if !id.is_empty() {
            return id;
        }
    }
    let id = uuid_novo();
    let _ = std::fs::write(&caminho, &id);
    id
}

/// O nome que aparece no celular.
pub fn nome() -> String {
    let maquina = std::env::var("COMPUTERNAME").ok().map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    match maquina {
        Some(m) => format!("Saimo TV - {m}"),
        None => "Saimo TV - Windows".to_string(),
    }
}

/// O endereço deste computador na rede local (o que o celular acha).
/// Guardado por alguns segundos: os Ajustes o mostram a cada quadro.
pub fn ip_local() -> Option<Ipv4Addr> {
    static GUARDADO: Mutex<Option<(Instant, Option<Ipv4Addr>)>> = Mutex::new(None);
    let mut guardado = GUARDADO.lock().unwrap();
    if let Some((quando, ip)) = *guardado {
        if quando.elapsed() < Duration::from_secs(10) {
            return ip;
        }
    }
    let ip = descobrir_ip_local();
    *guardado = Some((Instant::now(), ip));
    ip
}

fn descobrir_ip_local() -> Option<Ipv4Addr> {
    // Nenhum pacote sai: "conectar" UDP só escolhe a placa de rede da rota.
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("10.255.255.255:9").or_else(|_| socket.connect("8.8.8.8:53")).ok()?;
    match socket.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => Some(ip),
        _ => None,
    }
}

/// Sobe o servidor. Os pedidos chegam pelo canal devolvido; `ctx` acorda a
/// tela quando chega um. Sem a porta (outro programa nela), o app segue sem o
/// Connect e o ao vivo toca direto, como antes.
pub fn iniciar(ctx: egui::Context) -> Receiver<Pedido> {
    let (envio, recebe) = channel();
    if std::env::var("SAIMO_SEM_CONNECT").is_ok() {
        return recebe;
    }
    let servidor = Arc::new(Servidor {
        tv_id: ler_ou_criar_id(),
        pedidos: envio,
        ctx,
        clientes: Mutex::new(Vec::new()),
        confiaveis: Mutex::new(crate::ler_lista(ARQUIVO_CELULARES)),
        tv_audio: Mutex::new("0".into()),
        som_no_celular: Mutex::new(Vec::new()),
        estado: Mutex::new(Value::Null),
        enviado: Mutex::new((String::new(), Instant::now())),
        telas: Mutex::new(Vec::new()),
        aprovacoes: Mutex::new(HashMap::new()),
        proximo_cliente: AtomicU64::new(1),
        texto: Mutex::new(None),
    });
    // Todas as placas: o celular vem pela rede, o mpv daqui por 127.0.0.1.
    // É aqui que o Firewall do Windows pergunta, na primeira vez.
    let ouvinte = match TcpListener::bind(SocketAddr::from(([0, 0, 0, 0], porta()))) {
        Ok(o) => o,
        Err(e) => {
            crate::telemetria::erro(format!("Saimo Connect sem a porta {}: {e}", porta()));
            return recebe;
        }
    };
    if SERVIDOR.set(servidor.clone()).is_err() {
        return recebe;
    }
    ATIVO.store(true, Ordering::SeqCst);
    {
        let servidor = servidor.clone();
        std::thread::Builder::new()
            .name("saimo-connect".into())
            .spawn(move || {
                for conexao in ouvinte.incoming() {
                    let Ok(socket) = conexao else { continue };
                    let servidor = servidor.clone();
                    let _ = std::thread::Builder::new()
                        .name("saimo-connect-cliente".into())
                        .spawn(move || servidor.atender(socket));
                }
            })
            .ok();
    }
    {
        let servidor = servidor.clone();
        std::thread::Builder::new()
            .name("saimo-connect-relogio".into())
            .spawn(move || loop {
                std::thread::sleep(Duration::from_millis(1000));
                servidor.a_cada_segundo();
            })
            .ok();
    }
    recebe
}

fn servidor() -> Option<&'static Arc<Servidor>> {
    SERVIDOR.get().filter(|_| ATIVO.load(Ordering::SeqCst))
}

/// Se o servidor está no ar.
pub fn ativo() -> bool {
    servidor().is_some()
}

/// O endereço da régua para o mpv `tela` tocar uma lista ao vivo, ou nada
/// quando não vale (não é HLS, ou o servidor não subiu).
pub fn pela_regua(url: &str, referer: Option<&str>, agente: Option<&str>, tela: u64) -> Option<String> {
    servidor()?;
    if !hls::parece_hls(url) {
        return None;
    }
    Some(hls::endereco(&format!("127.0.0.1:{}", porta()), url, referer, agente, Some(tela)))
}

/// Quem está ligado agora, autorizado.
pub fn tem_celular() -> bool {
    servidor().is_some_and(|s| s.clientes.lock().unwrap().iter().any(|c| c.autenticado))
}

pub fn quantos_autorizados() -> usize {
    servidor().map(|s| s.confiaveis.lock().unwrap().len()).unwrap_or_else(|| crate::ler_lista(ARQUIVO_CELULARES).len())
}

/// Esquece todos os celulares autorizados e desliga os que estão conectados.
pub fn esquecer_todos() {
    let _ = std::fs::remove_file(crate::catalogo::pasta().join(ARQUIVO_CELULARES));
    let Some(s) = servidor() else { return };
    s.confiaveis.lock().unwrap().clear();
    for c in s.clientes.lock().unwrap().iter() {
        let _ = c.socket.shutdown(Shutdown::Both);
    }
}

/// A resposta à pergunta "Permitir?" feita pelo [`Pedido::Aprovar`].
pub fn responder(cliente: u64, permitido: bool) {
    if let Some(s) = servidor() {
        s.aprovado(cliente, permitido);
    }
}

/// "0", "1" ou "mute": que tela tem som aqui.
pub fn tv_audio() -> String {
    servidor().map(|s| s.tv_audio.lock().unwrap().clone()).unwrap_or_else(|| "0".into())
}

pub fn definir_tv_audio(valor: &str) {
    if let Some(s) = servidor() {
        *s.tv_audio.lock().unwrap() = valor.to_string();
    }
}

/// Algum celular tocando o som de uma das telas.
pub fn som_no_celular() -> bool {
    servidor().is_some_and(|s| s.alguem_ouvindo())
}

/// Quem ouve, e o quê ("0", "1" ou "both"), para o aviso na tela.
pub fn quem_ouve() -> Vec<(String, String)> {
    servidor()
        .map(|s| {
            s.som_no_celular
                .lock()
                .unwrap()
                .iter()
                .filter(|a| a.src != "none")
                .map(|a| (a.nome.clone(), a.src.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// O app publica o que está no ar (sem `tvAudio` e `phoneAudio`, que são
/// daqui). Vai para os celulares quando mudou; `forcar` manda de qualquer jeito.
pub fn publicar_estado(estado: Value, forcar: bool) {
    let Some(s) = servidor() else { return };
    *s.estado.lock().unwrap() = estado;
    s.espalhar_estado(forcar);
}

/// As telas no ar, para o aviso de posição de cada segundo.
pub fn publicar_telas(telas: Vec<TelaNoAr>) {
    if let Some(s) = servidor() {
        *s.telas.lock().unwrap() = telas;
    }
}

/// Um campo de texto abriu (ou fechou, com `None`) aqui: o celular abre o
/// teclado dele.
pub fn pedir_texto(pedido: Option<(&str, &str)>) {
    let Some(s) = servidor() else { return };
    let novo = pedido.map(|(dica, valor)| (dica.to_string(), valor.to_string()));
    let mut atual = s.texto.lock().unwrap();
    // Com o campo já aberto, o texto mudando (digitado aqui ou vindo do
    // próprio celular) não reabre o teclado de lá: só fica guardado.
    if atual.is_some() == novo.is_some() {
        *atual = novo;
        return;
    }
    let mensagem = match &novo {
        Some((dica, valor)) => json!({"t": "textRequest", "hint": dica, "value": valor}),
        None => json!({"t": "textDone"}),
    };
    *atual = novo;
    drop(atual);
    s.para_todos(&mensagem.to_string());
}

/// Onde o mpv de uma tela está na régua comum (o `st`), lendo a posição dele
/// agora: ao vivo, pelo PTS ([`historico::instante_da_tela`]); filme, a
/// posição. Também devolve o quanto está atrás da ponta ao vivo.
pub fn instante(tela: &TelaNoAr) -> Option<(i64, Option<i64>)> {
    let posicao = tela.mpv.ler("time-pos")?.parse::<f64>().ok()?;
    if tela.vod {
        return Some(((posicao * 1000.0).round() as i64, None));
    }
    let r = historico::instante_da_tela(tela.mpv.id(), posicao, agora_ms())?;
    Some((r.instante, Some((r.ponta - r.instante).max(0))))
}

impl Servidor {
    fn pedir(&self, pedido: Pedido) {
        let _ = self.pedidos.send(pedido);
        self.ctx.request_repaint();
    }

    fn alguem_ouvindo(&self) -> bool {
        self.som_no_celular.lock().unwrap().iter().any(|a| matches!(a.src.as_str(), "0" | "1" | "both"))
    }

    fn eh_confiavel(&self, cid: &str) -> bool {
        self.confiaveis.lock().unwrap().iter().any(|c| c == cid)
    }

    fn confiar(&self, cid: &str) {
        let mut lista = self.confiaveis.lock().unwrap();
        if !lista.iter().any(|c| c == cid) {
            lista.push(cid.to_string());
            let _ = std::fs::write(crate::catalogo::pasta().join(ARQUIVO_CELULARES), lista.join("\n"));
        }
    }

    fn enviar(&self, cliente: u64, texto: &str) {
        if let Some(c) = self.clientes.lock().unwrap().iter().find(|c| c.id == cliente) {
            let _ = c.envio.send(ws::quadro_de_texto(texto));
        }
    }

    fn para_todos(&self, texto: &str) {
        let quadro = ws::quadro_de_texto(texto);
        for c in self.clientes.lock().unwrap().iter().filter(|c| c.autenticado) {
            let _ = c.envio.send(quadro.clone());
        }
    }

    fn fechar(&self, cliente: u64) {
        if let Some(c) = self.clientes.lock().unwrap().iter().find(|c| c.id == cliente) {
            let _ = c.socket.shutdown(Shutdown::Both);
        }
    }

    /// O estado completo: o do app mais o som.
    fn estado_completo(&self) -> Value {
        let mut estado = self.estado.lock().unwrap().clone();
        if !estado.is_object() {
            estado = json!({"t": "state", "mode": "idle", "playing": false, "multi": false, "active": 0, "screens": []});
        }
        estado["tvAudio"] = json!(self.tv_audio.lock().unwrap().clone());
        let ouvintes: Vec<Value> = self
            .som_no_celular
            .lock()
            .unwrap()
            .iter()
            .map(|a| json!({"cid": a.cid, "name": a.nome, "src": a.src}))
            .collect();
        estado["phoneAudio"] = Value::Array(ouvintes);
        estado
    }

    /// Manda o estado a quem está conectado quando ele mudou. A posição do
    /// filme muda sempre: ela sozinha só vai a cada 5 s.
    fn espalhar_estado(&self, forcar: bool) {
        let estado = self.estado_completo();
        let mut sem_posicao = estado.clone();
        if let Some(vod) = sem_posicao.get_mut("vod").and_then(Value::as_object_mut) {
            vod.remove("pos");
        }
        let chave = sem_posicao.to_string();
        let filme = estado.get("vod").is_some();
        {
            let mut enviado = self.enviado.lock().unwrap();
            let velho = enviado.1.elapsed() >= Duration::from_secs(5);
            if !forcar && enviado.0 == chave && !(filme && velho) {
                return;
            }
            *enviado = (chave, Instant::now());
        }
        self.para_todos(&estado.to_string());
    }

    fn a_cada_segundo(&self) {
        // Pergunta sem resposta há um minuto: recusada, como a TV Box.
        let vencidas: Vec<u64> = {
            let aprovacoes = self.aprovacoes.lock().unwrap();
            aprovacoes.iter().filter(|(_, desde)| desde.elapsed() >= ESPERA_DA_APROVACAO).map(|(id, _)| *id).collect()
        };
        for id in vencidas {
            self.aprovado(id, false);
            self.pedir(Pedido::Desistiu { cliente: id });
        }
        // Enquanto algum celular toca o som, a posição de cada tela vai para
        // ele, com a hora daqui: é o que o celular usa para ficar junto.
        if !self.alguem_ouvindo() || !self.clientes.lock().unwrap().iter().any(|c| c.autenticado) {
            return;
        }
        let telas = self.telas.lock().unwrap().clone();
        let mut lista = Vec::new();
        for tela in &telas {
            let Some((st, _)) = instante(tela) else { continue };
            // "core-idle": pausado ou esperando dados — não está andando.
            let andando = tela.mpv.ler("core-idle").as_deref() == Some("no");
            lista.push(json!({"idx": tela.idx, "st": st, "playing": andando}));
        }
        if lista.is_empty() {
            return;
        }
        self.para_todos(&json!({"t": "pos", "wall": agora_ms(), "screens": lista}).to_string());
    }

    fn aprovado(&self, cliente: u64, permitido: bool) {
        if self.aprovacoes.lock().unwrap().remove(&cliente).is_none() {
            return;
        }
        let dados = self.clientes.lock().unwrap().iter().find(|c| c.id == cliente).map(|c| (c.cid.clone(), c.nome.clone()));
        let Some((Some(cid), nome)) = dados else { return };
        if permitido {
            self.confiar(&cid);
            self.boas_vindas(cliente, &nome);
        } else {
            self.enviar(cliente, &json!({"t": "denied"}).to_string());
            // Um instante para o "denied" sair antes de fechar.
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(300));
                if let Some(s) = servidor() {
                    s.fechar(cliente);
                }
            });
        }
    }

    fn boas_vindas(&self, cliente: u64, nome: &str) {
        if let Some(c) = self.clientes.lock().unwrap().iter_mut().find(|c| c.id == cliente) {
            c.autenticado = true;
        }
        self.enviar(cliente, &json!({"t": "welcome", "tv": {"id": self.tv_id, "name": crate::connect::nome()}}).to_string());
        self.enviar(cliente, &self.estado_completo().to_string());
        if let Some((dica, valor)) = self.texto.lock().unwrap().clone() {
            self.enviar(cliente, &json!({"t": "textRequest", "hint": dica, "value": valor}).to_string());
        }
        self.pedir(Pedido::Conectou(nome.to_string()));
    }

    /// Uma conexão nova: lê o pedido HTTP e decide a rota.
    fn atender(self: Arc<Self>, mut socket: TcpStream) {
        let _ = socket.set_nodelay(true);
        let _ = socket.set_read_timeout(Some(Duration::from_secs(10)));
        let _ = socket.set_write_timeout(Some(Duration::from_secs(10)));
        let local = socket.peer_addr().map(|a| a.ip().is_loopback()).unwrap_or(false);
        let Some(cabecalho) = ler_cabecalho(&mut socket) else { return };
        let primeira = cabecalho.lines().next().unwrap_or("").to_string();
        let mut partes = primeira.split_whitespace();
        let metodo = partes.next().unwrap_or("");
        let alvo = partes.next().unwrap_or("/");
        let campo = |nome: &str| -> Option<String> {
            cabecalho.lines().skip(1).find_map(|l| {
                let (k, v) = l.split_once(':')?;
                k.trim().eq_ignore_ascii_case(nome).then(|| v.trim().to_string())
            })
        };

        if metodo == "OPTIONS" {
            let _ = socket.write_all(
                b"HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, OPTIONS\r\n\
                  Access-Control-Allow-Headers: *\r\nConnection: close\r\n\r\n",
            );
            return;
        }
        if metodo == "GET" && (alvo == "/info" || alvo.starts_with("/info?")) {
            let corpo = json!({
                "app": "saimo-tv",
                "id": self.tv_id,
                "name": nome(),
                "version": crate::VERSAO,
                "port": porta(),
                "multi": true,
                "platform": "windows",
            })
            .to_string();
            responder_http(&mut socket, 200, "application/json; charset=utf-8", corpo.as_bytes(), None);
            return;
        }
        if metodo == "GET" && (alvo == "/hls" || alvo.starts_with("/hls?")) {
            let host = campo("Host").unwrap_or_else(|| {
                let ip = ip_local().map(|i| i.to_string()).unwrap_or_else(|| "127.0.0.1".into());
                format!("{ip}:{}", porta())
            });
            let r = hls::responder(alvo, &host, local, agora_ms());
            responder_http(&mut socket, r.codigo, r.tipo, &r.corpo, r.destino.as_deref());
            return;
        }
        let upgrade = campo("Upgrade").is_some_and(|u| u.eq_ignore_ascii_case("websocket"));
        if metodo == "GET" && upgrade {
            if let Some(chave) = campo("Sec-WebSocket-Key") {
                let aperto = format!(
                    "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {}\r\n\r\n",
                    ws::aceite(&chave)
                );
                if socket.write_all(aperto.as_bytes()).is_ok() {
                    self.sessao(socket);
                }
                return;
            }
        }
        let _ = socket.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    }

    /// A conversa com um celular, até ele sair.
    fn sessao(self: &Arc<Self>, socket: TcpStream) {
        let id = self.proximo_cliente.fetch_add(1, Ordering::SeqCst);
        // O celular manda ping a cada 5 s: meio minuto calado é conexão morta.
        let _ = socket.set_read_timeout(Some(Duration::from_secs(30)));
        let (Ok(mut leitura), Ok(mut escrita), Ok(controle)) = (socket.try_clone(), socket.try_clone(), socket.try_clone()) else {
            return;
        };
        let (envio, fila) = channel::<Vec<u8>>();
        // Quem escreve é uma thread só dele: um celular lento não segura a
        // tela nem os outros.
        let escritor = std::thread::spawn(move || {
            for quadro in fila {
                if escrita.write_all(&quadro).is_err() {
                    let _ = escrita.shutdown(Shutdown::Both);
                    break;
                }
            }
        });
        self.clientes.lock().unwrap().push(Cliente {
            id,
            cid: None,
            nome: "Celular".into(),
            autenticado: false,
            envio: envio.clone(),
            socket: controle,
        });

        let mut leitor = ws::Leitor::default();
        let mut despedida: Vec<u8> = Vec::new();
        loop {
            match leitor.ler(&mut leitura) {
                Ok(ws::Mensagem::Texto(texto)) => self.mensagem(id, &texto),
                Ok(ws::Mensagem::Ping(dados)) => {
                    let _ = envio.send(ws::quadro(0xA, &dados));
                }
                Ok(ws::Mensagem::Outro) => {}
                Ok(ws::Mensagem::Fechar(codigo)) => {
                    // O aceite do fechamento leva o mesmo código de volta.
                    despedida = codigo.get(..2).map(<[u8]>::to_vec).unwrap_or_default();
                    break;
                }
                Err(_) => break,
            }
        }
        let _ = envio.send(ws::quadro(0x8, &despedida));
        let saiu = {
            let mut clientes = self.clientes.lock().unwrap();
            let posicao = clientes.iter().position(|c| c.id == id);
            posicao.map(|p| clientes.remove(p))
        };
        // Sem ninguém mais para mandar, o escritor esvazia a fila e sai: o
        // fechamento chega ao celular antes da conexão cair.
        drop(envio);
        let cid_que_saiu = saiu.and_then(|c| c.cid);
        let _ = escritor.join();
        let _ = socket.shutdown(Shutdown::Both);
        if self.aprovacoes.lock().unwrap().remove(&id).is_some() {
            self.pedir(Pedido::Desistiu { cliente: id });
        }
        if let Some(cid) = cid_que_saiu {
            let tinha_som = {
                let mut ouvintes = self.som_no_celular.lock().unwrap();
                let antes = ouvintes.len();
                ouvintes.retain(|a| a.cid != cid);
                antes != ouvintes.len()
            };
            if tinha_som {
                self.pedir(Pedido::SomNoCelular);
            }
            self.espalhar_estado(true);
        }
    }

    fn mensagem(&self, cliente: u64, texto: &str) {
        let Ok(msg) = serde_json::from_str::<Value>(texto) else { return };
        let tipo = msg.get("t").and_then(Value::as_str).unwrap_or("").to_string();
        match tipo.as_str() {
            "hello" => {
                let Some(cid) = msg.get("cid").and_then(Value::as_str).filter(|c| !c.trim().is_empty()) else { return };
                let nome: String = msg
                    .get("name")
                    .and_then(Value::as_str)
                    .map(|n| n.trim().chars().take(40).collect::<String>())
                    .filter(|n| !n.is_empty())
                    .unwrap_or_else(|| "Celular".into());
                {
                    let mut clientes = self.clientes.lock().unwrap();
                    let Some(c) = clientes.iter_mut().find(|c| c.id == cliente) else { return };
                    // Um "hello" repetido na mesma conexão não pergunta de novo.
                    if c.autenticado {
                        return;
                    }
                    c.cid = Some(cid.to_string());
                    c.nome = nome.clone();
                }
                if self.eh_confiavel(cid) {
                    self.boas_vindas(cliente, &nome);
                    return;
                }
                // Primeira vez deste celular: pergunta aqui. Antes qualquer
                // aparelho na mesma rede (Wi-Fi de prédio, de condomínio)
                // entraria sozinho e mandaria no computador.
                self.enviar(cliente, &json!({"t": "pending"}).to_string());
                let nova = self.aprovacoes.lock().unwrap().insert(cliente, Instant::now()).is_none();
                if nova {
                    self.pedir(Pedido::Aprovar { cliente, nome });
                }
            }
            "ping" => {
                // Com o relógio daqui: o celular estima a diferença entre os
                // dois relógios pela menor ida e volta. O `c0` volta como veio.
                let mut pong = json!({"t": "pong", "tv": agora_ms()});
                if let Some(c0) = msg.get("c0") {
                    pong["c0"] = c0.clone();
                }
                self.enviar(cliente, &pong.to_string());
            }
            _ => {
                let autorizado = self.clientes.lock().unwrap().iter().any(|c| c.id == cliente && c.autenticado);
                if !autorizado {
                    return;
                }
                self.comando(cliente, tipo, msg);
            }
        }
    }

    fn comando(&self, cliente: u64, tipo: String, msg: Value) {
        match tipo.as_str() {
            "sync" | "getState" => {
                self.enviar(cliente, &self.estado_completo().to_string());
                self.pedir(Pedido::Comando(tipo, msg));
            }
            "tvAudio" => {
                let src = msg.get("src").and_then(Value::as_str).unwrap_or("0");
                if matches!(src, "0" | "1" | "mute") {
                    *self.tv_audio.lock().unwrap() = src.to_string();
                    self.pedir(Pedido::Comando(tipo, msg.clone()));
                    self.espalhar_estado(true);
                }
            }
            "phoneAudio" => {
                let src = msg.get("src").and_then(Value::as_str).unwrap_or("none");
                let src = if matches!(src, "0" | "1" | "both") { src } else { "none" };
                let dados = self.clientes.lock().unwrap().iter().find(|c| c.id == cliente).and_then(|c| Some((c.cid.clone()?, c.nome.clone())));
                let Some((cid, nome)) = dados else { return };
                {
                    let mut ouvintes = self.som_no_celular.lock().unwrap();
                    match ouvintes.iter_mut().find(|a| a.cid == cid) {
                        Some(a) => {
                            a.src = src.to_string();
                            a.nome = nome;
                        }
                        None => ouvintes.push(SomNoCelular { cid, nome, src: src.to_string() }),
                    }
                }
                self.pedir(Pedido::SomNoCelular);
                self.espalhar_estado(true);
            }
            _ => self.pedir(Pedido::Comando(tipo, msg)),
        }
    }
}

/// O cabeçalho do pedido HTTP (até a linha em branco, no máximo 8 KB).
fn ler_cabecalho(socket: &mut TcpStream) -> Option<String> {
    let mut lido = Vec::with_capacity(1024);
    let mut byte = [0u8; 1];
    while lido.len() < 8192 {
        match socket.read(&mut byte) {
            Ok(1) => {
                lido.push(byte[0]);
                if lido.ends_with(b"\r\n\r\n") {
                    return Some(String::from_utf8_lossy(&lido).into_owned());
                }
            }
            _ => return None,
        }
    }
    None
}

fn responder_http(socket: &mut TcpStream, codigo: u16, tipo: &str, corpo: &[u8], destino: Option<&str>) {
    let status = match codigo {
        200 => "OK",
        302 => "Found",
        400 => "Bad Request",
        _ => "Bad Gateway",
    };
    let mut cabecalho = format!(
        "HTTP/1.1 {codigo} {status}\r\nContent-Type: {tipo}\r\nCache-Control: no-cache\r\nAccess-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, OPTIONS\r\nConnection: close\r\nContent-Length: {}\r\n",
        corpo.len()
    );
    if let Some(d) = destino {
        cabecalho.push_str(&format!("Location: {d}\r\n"));
    }
    cabecalho.push_str("\r\n");
    let _ = socket.write_all(cabecalho.as_bytes());
    let _ = socket.write_all(corpo);
    let _ = socket.flush();
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Sobe o servidor de verdade na porta 8999 por `SAIMO_TESTE_SEGUNDOS`
    /// (padrão 60) fazendo o papel do app: aprova quem pedir, publica um
    /// estado com o canal de `SAIMO_TESTE_CANAL` e imprime os comandos. Para
    /// conferir de fora, com um cliente WebSocket e o ffprobe:
    ///
    /// `cargo test servidor_de_verdade -- --ignored --nocapture`
    #[test]
    #[ignore = "abre a porta 8999"]
    fn servidor_de_verdade() {
        let pedidos = iniciar(egui::Context::default());
        assert!(ativo(), "a porta {} está ocupada", porta());
        let segundos = std::env::var("SAIMO_TESTE_SEGUNDOS").ok().and_then(|s| s.parse().ok()).unwrap_or(60);
        let canal = std::env::var("SAIMO_TESTE_CANAL").unwrap_or_else(|_| "https://exemplo/canal.m3u8".into());
        let fim = Instant::now() + Duration::from_secs(segundos);
        let mut volume = 0.5;
        while Instant::now() < fim {
            while let Ok(p) = pedidos.try_recv() {
                println!("pedido: {p:?}");
                match p {
                    Pedido::Aprovar { cliente, .. } => {
                        // Só neste teste: no app quem responde é a pessoa.
                        std::thread::sleep(Duration::from_millis(300));
                        responder(cliente, std::env::var("SAIMO_TESTE_RECUSAR").is_err());
                    }
                    Pedido::Comando(tipo, msg) if tipo == "volume" => {
                        volume = msg.get("set").and_then(Value::as_f64).unwrap_or(volume);
                    }
                    _ => {}
                }
            }
            publicar_estado(
                json!({"t": "state", "mode": "live", "screen": "live", "playing": true, "multi": false, "active": 0,
                    "volume": volume, "muted": false,
                    "screens": [{"idx": 0, "channel": "Teste", "num": 1, "logo": "", "program": "", "sourceIndex": 0,
                        "source": {"url": canal, "referer": null, "userAgent": null, "keyId": null, "key": null}, "liveOffset": 12000}]}),
                false,
            );
            std::thread::sleep(Duration::from_millis(100));
        }
        for (chave, seq, pts, inicio, url) in historico::depurar() {
            println!("trecho {chave} {seq} pts={pts:?} inicio={inicio} {url}");
        }
    }

    #[test]
    fn uuid_tem_o_formato() {
        let id = uuid_novo();
        assert_eq!(id.len(), 36);
        assert_eq!(&id[14..15], "4");
        assert_ne!(id, uuid_novo());
    }
}
