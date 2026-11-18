//! Conta Saimo (opcional): o computador entra pelo celular (QR) ou com e-mail e
//! senha, e sincroniza perfis, onde parou, favoritos, listas e addons pela
//! internet. Sem conta, nada muda: os dados ficam aqui e o Saimo Connect segue
//! na rede de casa. O que viaja é o mesmo pacote do Connect
//! (`App::pacote_de_dados` / `App::aplicar_dados`). Servidor: SaimoConta (README
//! lá). `SAIMO_CONTA_API` no ambiente aponta para um servidor de teste.
//!
//! A rede corre em threads; a tela (`App::passo_da_conta`, a cada quadro) recebe
//! o resultado por canal e é quem aplica e monta o pacote.

use crate::App;
use serde_json::{json, Value};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

const DEZ_MIN: Duration = Duration::from_secs(600);

pub fn api() -> String {
    std::env::var("SAIMO_CONTA_API")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| "https://conta.saimo-tv.com.br".into())
        .trim_end_matches('/')
        .to_string()
}

fn agora_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Sessao {
    token: Option<String>,
    email: Option<String>,
    #[serde(default)]
    sincronizada_em: i64,
}

fn caminho() -> std::path::PathBuf {
    crate::catalogo::pasta().join("conta.json")
}

pub struct PedidoTv {
    pub aparelho_codigo: String,
    pub codigo: String,
    pub url: String,
    pub expira: Instant,
    pub intervalo: Duration,
    proxima: Instant,
}

enum Evento {
    Pedido(Result<PedidoTv, String>),
    /// Resposta do "já aprovou?": `Some` entrou (token, e-mail); `None` ainda espera.
    Estado(Result<Option<(String, Option<String>)>, String>),
    Vencido,
    Entrou(Result<(String, Option<String>), String>),
    Remoto(Result<Option<Value>, String>),
    Enviado(Result<String, String>),
}

struct Falha {
    status: u16,
    mensagem: String,
}

fn chamar(metodo: &str, rota: &str, corpo: Option<Value>, token: Option<&str>) -> Result<Value, Falha> {
    let agente: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(crate::AGENTE)
        .http_status_as_error(false)
        .build()
        .into();
    let url = format!("{}{rota}", api());
    let sem_rede = |_| Falha { status: 0, mensagem: "Sem conexão com a conta Saimo agora".into() };
    let auth = token.map(|t| format!("Bearer {t}"));
    let resposta = match metodo {
        "GET" => {
            let mut r = agente.get(&url);
            if let Some(a) = &auth {
                r = r.header("Authorization", a);
            }
            r.call().map_err(sem_rede)?
        }
        _ => {
            let texto = corpo.map(|c| c.to_string()).unwrap_or_default();
            let mut r = if metodo == "PUT" { agente.put(&url) } else { agente.post(&url) };
            r = r.header("Content-Type", "application/json");
            if let Some(a) = &auth {
                r = r.header("Authorization", a);
            }
            r.send(texto.as_bytes()).map_err(sem_rede)?
        }
    };
    let status = resposta.status().as_u16();
    let texto = resposta.into_body().with_config().limit(4 * 1024 * 1024).read_to_string().unwrap_or_default();
    let j: Value = serde_json::from_str(&texto).unwrap_or(Value::Null);
    if !(200..300).contains(&status) {
        let mensagem = j.get("mensagem").and_then(Value::as_str).unwrap_or("Não deu certo: tente de novo").to_string();
        return Err(Falha { status, mensagem });
    }
    Ok(j)
}

fn nome_do_aparelho() -> String {
    let sistema = if cfg!(target_os = "linux") { "Linux" } else { "Windows" };
    let maquina = std::env::var("COMPUTERNAME").or_else(|_| std::env::var("HOSTNAME")).unwrap_or_default();
    let nome = if maquina.trim().is_empty() { format!("Saimo no {sistema}") } else { format!("Saimo no {sistema} ({})", maquina.trim()) };
    nome.chars().take(60).collect()
}

fn token_e_email(j: &Value) -> Option<(String, Option<String>)> {
    let t = j.get("token")?.as_str()?.to_string();
    let e = j.get("conta").and_then(|c| c.get("email")).and_then(Value::as_str).map(str::to_string);
    Some((t, e))
}

pub struct Conta {
    sessao: Sessao,
    pub erro: Option<String>,
    pub sincronizando: bool,
    /// A janela da conta está aberta (Ajustes > Conta Saimo).
    pub aberta: bool,
    pub pedido: Option<PedidoTv>,
    pedindo: bool,
    consultando: bool,
    pub situacao: String,
    /// Formulário de e-mail e senha.
    pub com_senha: bool,
    pub email_form: String,
    pub senha_form: String,
    pub aviso_form: String,
    pub entrando: bool,
    pub qr_textura: Option<eframe::egui::TextureHandle>,
    pub qr_url: String,
    ultimo_enviado: String,
    forcar: bool,
    proxima_sync: Instant,
    ultima_olhada: Instant,
    tx: Sender<Evento>,
    rx: Receiver<Evento>,
}

impl Conta {
    pub fn carregar() -> Conta {
        let sessao: Sessao = std::fs::read_to_string(caminho()).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default();
        let (tx, rx) = channel();
        Conta {
            sessao,
            erro: None,
            sincronizando: false,
            aberta: false,
            pedido: None,
            pedindo: false,
            consultando: false,
            situacao: String::new(),
            com_senha: false,
            email_form: String::new(),
            senha_form: String::new(),
            aviso_form: String::new(),
            entrando: false,
            qr_textura: None,
            qr_url: String::new(),
            ultimo_enviado: String::new(),
            forcar: false,
            // A primeira sincronia vem alguns segundos depois de abrir.
            proxima_sync: Instant::now() + Duration::from_secs(5),
            ultima_olhada: Instant::now(),
            tx,
            rx,
        }
    }

    fn gravar(&self) {
        let _ = std::fs::create_dir_all(crate::catalogo::pasta());
        if self.sessao.token.is_none() {
            let _ = std::fs::remove_file(caminho());
        } else if let Ok(t) = serde_json::to_string(&self.sessao) {
            let _ = std::fs::write(caminho(), t);
        }
    }

    pub fn logado(&self) -> bool {
        self.sessao.token.is_some()
    }

    pub fn email(&self) -> Option<&str> {
        self.sessao.email.as_deref()
    }

    pub fn ha_quanto(&self) -> String {
        let ms = self.sessao.sincronizada_em;
        if ms == 0 {
            return "ainda não sincronizou".into();
        }
        let min = (agora_ms() - ms) / 60_000;
        match min {
            m if m < 1 => "sincronizado agora".into(),
            m if m < 60 => format!("sincronizado há {m} min"),
            m if m < 24 * 60 => format!("sincronizado há {} h", m / 60),
            m => format!("sincronizado há {} d", m / (24 * 60)),
        }
    }

    pub fn resumo(&self) -> String {
        if !self.logado() {
            return "Entrar pelo celular (QR) ou com e-mail e senha".into();
        }
        let estado = if self.sincronizando { "sincronizando…".to_string() } else { self.erro.clone().unwrap_or_else(|| self.ha_quanto()) };
        format!("{} · {estado}", self.email().unwrap_or("Conta Saimo"))
    }

    fn entrou(&mut self, token: String, email: Option<String>) {
        self.sessao = Sessao { token: Some(token), email, sincronizada_em: 0 };
        self.gravar();
        self.pedido = None;
        self.erro = None;
        self.ultimo_enviado.clear();
        self.forcar = true;
        self.proxima_sync = Instant::now();
    }

    fn limpar(&mut self) {
        self.sessao = Sessao::default();
        self.gravar();
        self.ultimo_enviado.clear();
        self.erro = None;
    }

    /// Abre a janela (sem conta: começa a pedir o código do QR).
    pub fn abrir(&mut self) {
        self.aberta = true;
        self.com_senha = false;
        self.aviso_form.clear();
    }

    pub fn fechar(&mut self) {
        self.aberta = false;
        self.pedido = None;
        self.senha_form.clear();
    }

    pub fn entrar_com_senha(&mut self) {
        if self.entrando {
            return;
        }
        let email = self.email_form.trim().to_string();
        let senha = self.senha_form.clone();
        if !email.contains('@') || senha.is_empty() {
            self.aviso_form = "Digite o e-mail e a senha".into();
            return;
        }
        self.entrando = true;
        self.aviso_form = "Entrando…".into();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let r = chamar("POST", "/v1/senha/entrar", Some(json!({"email": email, "senha": senha, "aparelho": nome_do_aparelho()})), None)
                .map_err(|f| f.mensagem)
                .and_then(|j| token_e_email(&j).ok_or_else(|| "Resposta inesperada da conta".to_string()));
            let _ = tx.send(Evento::Entrou(r));
        });
    }

    pub fn sair(&mut self) {
        if let Some(t) = self.sessao.token.clone() {
            std::thread::spawn(move || {
                let _ = chamar("POST", "/v1/sair", None, Some(&t));
            });
        }
        self.limpar();
    }

    fn pedir_codigo(&mut self) {
        if self.pedindo {
            return;
        }
        self.pedindo = true;
        self.situacao = "Pedindo um código…".into();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let r = chamar("POST", "/v1/tv/pedir", Some(json!({"aparelho": nome_do_aparelho()})), None)
                .map_err(|f| f.mensagem)
                .and_then(|j| {
                    let s = |k: &str| j.get(k).and_then(Value::as_str).map(str::to_string);
                    let (Some(a), Some(c), Some(u)) = (s("aparelhoCodigo"), s("codigo"), s("url")) else {
                        return Err("Resposta inesperada da conta".to_string());
                    };
                    let resta = (j.get("expiraEm").and_then(Value::as_i64).unwrap_or(0) - agora_ms()).clamp(30_000, 600_000);
                    let intervalo = Duration::from_secs(j.get("intervalo").and_then(Value::as_u64).unwrap_or(5).clamp(2, 30));
                    Ok(PedidoTv {
                        aparelho_codigo: a,
                        codigo: c,
                        url: u,
                        expira: Instant::now() + Duration::from_millis(resta as u64),
                        intervalo,
                        proxima: Instant::now() + intervalo,
                    })
                });
            let _ = tx.send(Evento::Pedido(r));
        });
    }

    fn conferir(&mut self) {
        let Some(p) = self.pedido.as_mut() else { return };
        if self.consultando || Instant::now() < p.proxima {
            return;
        }
        p.proxima = Instant::now() + p.intervalo;
        self.consultando = true;
        let codigo = p.aparelho_codigo.clone();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let r = chamar("POST", "/v1/tv/estado", Some(json!({"aparelhoCodigo": codigo})), None);
            let ev = match r {
                Ok(j) => match j.get("estado").and_then(Value::as_str) {
                    Some("aprovado") => Evento::Estado(Ok(token_e_email(&j))),
                    Some("vencido") => Evento::Vencido,
                    _ => Evento::Estado(Ok(None)),
                },
                Err(f) => Evento::Estado(Err(f.mensagem)),
            };
            let _ = tx.send(ev);
        });
    }

    fn baixar(&mut self) {
        let Some(t) = self.sessao.token.clone() else { return };
        self.sincronizando = true;
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let r = chamar("GET", "/v1/dados", None, Some(&t));
            let ev = match r {
                Ok(j) => Evento::Remoto(Ok(j.get("dados").filter(|d| d.is_object()).cloned())),
                Err(f) if f.status == 401 => Evento::Remoto(Err("401".into())),
                Err(f) => Evento::Remoto(Err(f.mensagem)),
            };
            let _ = tx.send(ev);
        });
    }

    fn enviar(&mut self, pacote: Value, texto: String) {
        let Some(t) = self.sessao.token.clone() else { return };
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            let r = chamar("PUT", "/v1/dados", Some(json!({ "dados": pacote })), Some(&t));
            let _ = tx.send(Evento::Enviado(match r {
                Ok(_) => Ok(texto),
                Err(f) if f.status == 401 => Err("401".into()),
                Err(f) => Err(f.mensagem),
            }));
        });
    }

    fn terminou(&mut self, erro: Option<String>) {
        self.sincronizando = false;
        self.forcar = false;
        match erro {
            None => {
                self.sessao.sincronizada_em = agora_ms();
                self.gravar();
                self.erro = None;
                self.proxima_sync = Instant::now() + DEZ_MIN;
            }
            Some(e) if e == "401" => {
                // Sessão encerrada em outro lugar (sair de todos, conta apagada).
                self.limpar();
            }
            Some(e) => {
                self.erro = Some(e);
                self.proxima_sync = Instant::now() + Duration::from_secs(120);
            }
        }
    }
}

impl App {
    /// Sincronizar agora (o botão da janela).
    pub(crate) fn sincronizar_conta_agora(&mut self) {
        if self.conta.logado() && !self.conta.sincronizando {
            self.conta.forcar = true;
            self.conta.baixar();
        }
    }

    /// A cada quadro: resultados da rede, QR da janela aberta e quando sincronizar.
    pub(crate) fn passo_da_conta(&mut self) {
        while let Ok(ev) = self.conta.rx.try_recv() {
            match ev {
                Evento::Pedido(r) => {
                    self.conta.pedindo = false;
                    match r {
                        Ok(p) => {
                            self.conta.situacao = "Esperando o celular…".into();
                            self.conta.pedido = Some(p);
                        }
                        Err(e) => self.conta.situacao = e,
                    }
                }
                Evento::Estado(r) => {
                    self.conta.consultando = false;
                    if let Ok(Some((t, e))) = r {
                        self.conta.situacao = "Pronto! Sincronizando…".into();
                        self.conta.entrou(t, e);
                        self.aviso = Some((format!("Entrou na conta {}", self.conta.email().unwrap_or("")), Instant::now()));
                    }
                }
                Evento::Vencido => {
                    self.conta.consultando = false;
                    self.conta.pedido = None;
                }
                Evento::Entrou(r) => {
                    self.conta.entrando = false;
                    match r {
                        Ok((t, e)) => {
                            self.conta.senha_form.clear();
                            self.conta.aviso_form.clear();
                            self.conta.com_senha = false;
                            self.conta.entrou(t, e);
                            self.aviso = Some((format!("Entrou na conta {}", self.conta.email().unwrap_or("")), Instant::now()));
                        }
                        Err(e) => {
                            self.conta.senha_form.clear();
                            self.conta.aviso_form = e;
                        }
                    }
                }
                Evento::Remoto(Ok(remoto)) => {
                    if let Some(v) = remoto {
                        self.aplicar_dados(&v);
                    }
                    let pacote = self.pacote_de_dados();
                    let texto = pacote.to_string();
                    if self.conta.forcar || texto != self.conta.ultimo_enviado {
                        self.conta.enviar(pacote, texto);
                    } else {
                        self.conta.terminou(None);
                    }
                }
                Evento::Remoto(Err(e)) => self.conta.terminou(Some(e)),
                Evento::Enviado(Ok(texto)) => {
                    self.conta.ultimo_enviado = texto;
                    self.conta.terminou(None);
                }
                Evento::Enviado(Err(e)) => self.conta.terminou(Some(e)),
            }
        }

        // Janela aberta sem conta: mostra um código e pergunta se o celular aprovou.
        if self.conta.aberta && !self.conta.logado() && !self.conta.com_senha {
            match &self.conta.pedido {
                Some(p) if Instant::now() >= p.expira => self.conta.pedido = None,
                Some(_) => self.conta.conferir(),
                None => self.conta.pedir_codigo(),
            }
        }

        if !self.conta.logado() || self.conta.sincronizando {
            return;
        }
        // A cada 10 minutos, logo depois de entrar, e quando algo mudou aqui
        // (o pacote é comparado a cada 30 s com o último enviado).
        let agora = Instant::now();
        if agora >= self.conta.proxima_sync {
            self.conta.baixar();
        } else if agora.duration_since(self.conta.ultima_olhada) > Duration::from_secs(30) {
            self.conta.ultima_olhada = agora;
            if !self.conta.ultimo_enviado.is_empty() && self.pacote_de_dados().to_string() != self.conta.ultimo_enviado {
                self.conta.baixar();
            }
        }
    }
}
