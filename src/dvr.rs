//! DVR local: arquivos e agendamentos por aparelho, sem pressupor unidade C/J/E.
//! Gravação usa FFmpeg externo e independente da reprodução do mpv.
//! O agendador funciona com o Saimo aberto; não promete acordar um PC desligado.
use crate::{catalogo, epg, mpv};
use chrono::{Local, NaiveDateTime, TimeZone};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Sender, Receiver};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaseGravacao {
    Parado,
    Iniciando,
    Gravando,
    Processando,
    Concluido,
    Erro,
}

use std::process::{Command, Stdio};

const CONFIG: &str = "gravacoes-config.json";
const AGENDA: &str = "gravacoes-agendadas.json";

#[derive(Default, Serialize, Deserialize)]
struct Config {
    pasta: Option<PathBuf>,
}

fn home() -> PathBuf {
    #[cfg(windows)]
    { return std::env::var_os("USERPROFILE").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".")); }
    #[cfg(not(windows))]
    { std::env::var_os("HOME").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(".")) }
}

/// Pasta de vídeo do usuário, em vez de assumir C:, J: ou E:.
pub fn pasta_padrao() -> PathBuf {
    // Instalações portáteis e testes podem fornecer outro volume sem alterar
    // a preferência persistida. Ex.: SAIMO_GRAVACOES_DIR=J:\\SaimoTV\\Gravacoes.
    if let Some(dir) = std::env::var_os("SAIMO_GRAVACOES_DIR")
        .map(PathBuf::from).filter(|p| p.is_absolute()) {
        return dir;
    }
    #[cfg(target_os = "windows")]
    { home().join("Videos").join("Saimo TV") }
    #[cfg(target_os = "macos")]
    { home().join("Movies").join("Saimo TV") }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let base = std::env::var("XDG_VIDEOS_DIR").ok()
            .map(|s| s.replace("$HOME", &home().to_string_lossy()))
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home().join("Videos"));
        base.join("Saimo TV")
    }
}

pub fn pasta() -> PathBuf {
    fs::read_to_string(catalogo::pasta().join(CONFIG)).ok()
        .and_then(|s| serde_json::from_str::<Config>(&s).ok())
        .and_then(|c| c.pasta)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(pasta_padrao)
}

pub fn configurar_pasta(texto: &str) -> Result<PathBuf, String> {
    let destino = if texto.trim().is_empty() {
        pasta_padrao()
    } else {
        PathBuf::from(texto.trim().trim_matches('"'))
    };
    if !destino.is_absolute() {
        return Err("Informe um caminho absoluto, como D:\\\\Gravacoes ou /home/usuario/Videos".into());
    }
    fs::create_dir_all(&destino).map_err(|e| format!("Pasta inacessível: {e}"))?;
    // Confirmar permissão de escrita antes de salvar a preferência.
    let teste = destino.join(format!(".saimo-verificacao-{}", std::process::id()));
    fs::write(&teste, b"ok").map_err(|e| format!("Sem permissão para gravar nesta pasta: {e}"))?;
    let _ = fs::remove_file(teste);
    let json = serde_json::to_string_pretty(&Config { pasta: Some(destino.clone()) })
        .map_err(|e| e.to_string())?;
    fs::write(catalogo::pasta().join(CONFIG), json).map_err(|e| e.to_string())?;
    Ok(destino)
}

/// Abre o destino; no Windows destaca o arquivo validado quando disponível.
pub fn abrir_destino(arquivo: Option<&Path>) -> Result<PathBuf, String> {
    let raiz = pasta().join("Gravacoes");
    fs::create_dir_all(&raiz).map_err(|e| format!("Pasta de gravações inacessível: {e}"))?;
    let selecionado = arquivo.filter(|p| p.is_file() && p.parent() == Some(raiz.as_path()));

    #[cfg(target_os = "windows")]
    let mut comando = Command::new("explorer.exe");
    #[cfg(target_os = "macos")]
    let mut comando = Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut comando = Command::new("xdg-open");

    #[cfg(target_os = "windows")]
    if let Some(arquivo) = selecionado {
        comando.arg(format!("/select,{}", arquivo.display()));
    } else {
        comando.arg(&raiz);
    }
    #[cfg(target_os = "macos")]
    if let Some(arquivo) = selecionado {
        comando.arg("-R").arg(arquivo);
    } else {
        comando.arg(&raiz);
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    comando.arg(&raiz);
    comando.spawn().map_err(|e| format!("Não foi possível abrir a pasta: {e}"))?;
    Ok(selecionado.unwrap_or(raiz.as_path()).to_path_buf())
}

fn nome_seguro(nome: &str) -> String {
    let limpo: String = nome.chars().map(|c| {
        if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' { c } else { '_' }
    }).take(70).collect();
    let limpo = limpo.trim().trim_matches('.').to_string();
    if limpo.is_empty() { "Canal".into() } else { limpo }
}

fn arquivo_novo(canal: &str, extensao: &str, subpasta: &str) -> Result<PathBuf, String> {
    let destino = pasta().join(subpasta);
    fs::create_dir_all(&destino).map_err(|e| format!("Não foi possível criar a pasta: {e}"))?;
    for n in 0..1000 {
        let data = Local::now().format("%Y-%m-%d_%H-%M-%S");
        let sufixo = if n == 0 { String::new() } else { format!("-{n}") };
        let arquivo = destino.join(format!("{}_{}{}.{}", nome_seguro(canal), data, sufixo, extensao));
        if !arquivo.exists() { return Ok(arquivo); }
    }
    Err("Não foi possível gerar nome exclusivo para o arquivo".into())
}

pub fn captura(mpv: &mpv::Mpv, canal: &str) -> Result<PathBuf, String> {
    let arquivo = arquivo_novo(canal, "png", "Capturas")?;
    mpv.capturar_para(&arquivo)?;
    Ok(arquivo)
}

/// Localiza o FFmpeg disponibilizado com o aplicativo ou no PATH.
fn ffmpeg() -> Result<PathBuf, String> {
    let nome = if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" };
    if let Some(p) = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join(nome))) {
        if p.is_file() { return Ok(p); }
    }
    if let Some(dir) = std::env::var_os("SAIMO_FFMPEG") {
        let p = PathBuf::from(dir);
        if p.is_file() { return Ok(p); }
    }
    let disponivel = Command::new(nome).arg("-version")
        .stdout(Stdio::null()).stderr(Stdio::null()).status()
        .map(|s| s.success()).unwrap_or(false);
    if disponivel { Ok(PathBuf::from(nome)) }
    else { Err("FFmpeg não encontrado. Coloque ffmpeg ao lado do executável ou instale-o no PATH.".into()) }
}


#[derive(Clone, Serialize, Deserialize)]
pub struct Agendamento {
    pub canal: String,
    pub fonte: usize,
    pub inicio: i64,
    pub fim: i64,
    #[serde(default)]
    pub executado: bool,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub programa: Option<String>,
}

fn agendamento_visivel(a: &Agendamento) -> bool {
    !a.executado || matches!(
        a.status.as_str(), "iniciando" | "gravando" | "processando" | "concluida"
    )
}

/// O DVR nunca recodifica vídeo nem áudio; cada sessão contém partes
/// independentes. Não precisa manter a janela Saimo visível para continuar.
pub struct Dvr {
    ativos: Vec<crate::dvr_stream::GravacaoStream>,
    agendados: Vec<Agendamento>,
    avisos_iniciais: Vec<String>,
    eventos_tx: Sender<crate::dvr_stream::ResultadoFinal>,
    eventos_rx: Receiver<crate::dvr_stream::ResultadoFinal>,
    processando: HashSet<String>,
    recente: HashMap<String, (FaseGravacao, Instant)>,
    manuais: HashSet<String>,
    manuais_concluidas: VecDeque<PathBuf>,
}

impl Dvr {
    pub fn novo() -> Self {
        let mut agendados = fs::read_to_string(catalogo::pasta().join(AGENDA)).ok()
            .and_then(|s| serde_json::from_str::<Vec<Agendamento>>(&s).ok())
            .unwrap_or_default();
        let mut recuperar_agenda = false;
        for a in &mut agendados {
            if a.executado && matches!(
                a.status.as_str(), "iniciando" | "gravando" | "processando"
            ) {
                a.status = "interrompida".into();
                recuperar_agenda = true;
            }
        }
        // Histórico encerrado sem gravação concluída não ocupa a programação.
        let quantidade = agendados.len();
        agendados.retain(|a| !a.executado || matches!(
            a.status.as_str(), "iniciando" | "gravando" | "processando" | "concluida" | "interrompida"
        ));
        if agendados.len() != quantidade { recuperar_agenda = true; }
        let mut avisos_iniciais = Vec::new();
        let (eventos_tx, eventos_rx) = mpsc::channel();
        if let Ok(binario) = ffmpeg() {
            let recuperadas = crate::dvr_stream::recuperar_sessoes(
                &pasta().join("Gravacoes"), &binario, eventos_tx.clone()
            );
            if recuperadas > 0 {
                avisos_iniciais.push(format!(
                    "DVR organizando/recuperando {recuperadas} gravação(ões) anteriores."
                ));
            }
        }
        let dvr = Self {
            ativos: Vec::new(), agendados, avisos_iniciais,
            eventos_tx, eventos_rx, processando: HashSet::new(),
            recente: HashMap::new(),
            manuais: HashSet::new(), manuais_concluidas: VecDeque::new(),
        };
        if recuperar_agenda { dvr.salvar_agenda(); }
        dvr
    }

    fn salvar_agenda(&self) {
        if let Ok(texto) = serde_json::to_string_pretty(&self.agendados) {
            let temp = catalogo::pasta().join("gravacoes-agendadas.tmp");
            if fs::write(&temp, texto).is_ok() {
                let _ = fs::rename(temp, catalogo::pasta().join(AGENDA));
            }
        }
    }

    pub fn gravando(&self) -> bool {
        !self.ativos.is_empty() || !self.processando.is_empty()
    }

    pub fn fase_do_canal(&self, canal: &str) -> FaseGravacao {
        if let Some(r) = self.ativos.iter().find(|r| r.canal == canal) {
            if r.parada_solicitada { return FaseGravacao::Processando; }
            return if r.tem_dados() { FaseGravacao::Gravando }
                else { FaseGravacao::Iniciando };
        }
        if self.processando.contains(canal) { return FaseGravacao::Processando; }
        if let Some((fase, desde)) = self.recente.get(canal) {
            if desde.elapsed() < Duration::from_secs(14) { return *fase; }
        }
        FaseGravacao::Parado
    }

    pub fn ativos_totais(&self) -> usize { self.ativos.len() }

    pub fn ha_agenda(&self) -> bool { self.agendados.iter().any(|a| !a.executado) }
    pub fn quantidade_agendada(&self) -> usize {
        self.agendados.iter().filter(|a| !a.executado).count()
    }

    pub fn agendamentos_do_canal(&self, canal: &str) -> Vec<(usize, Agendamento)> {
        let mut lista: Vec<_> = self.agendados.iter().enumerate()
            .filter(|(_, a)| a.canal == canal && agendamento_visivel(a))
            .map(|(i, a)| (i, a.clone()))
            .collect();
        lista.sort_by_key(|(_, a)| std::cmp::Reverse(a.inicio));
        lista.truncate(10);
        lista
    }

    pub fn pendentes_no_canal(&self, canal: &str) -> usize {
        self.agendados.iter().filter(|a| a.canal == canal && !a.executado).count()
    }

    pub fn cancelar_agendamento(&mut self, indice: usize) -> Result<(), String> {
        let a = self.agendados.get(indice).ok_or("Agendamento não encontrado")?;
        if a.executado { return Err("Este agendamento já foi executado".into()); }
        self.agendados.remove(indice);
        self.salvar_agenda();
        Ok(())
    }

    /// Confirma que a conclusão foi vista. Não modifica o vídeo gravado.
    pub fn confirmar_conclusao(&mut self, indice: usize) -> Result<(), String> {
        let a = self.agendados.get_mut(indice).ok_or("Gravação não encontrada")?;
        if a.status != "concluida" { return Err("A gravação ainda não foi concluída".into()); }
        a.status = "confirmada".into();
        self.salvar_agenda();
        Ok(())
    }

    /// Exclui somente o registro da programação, sem apagar a gravação.
    pub fn excluir_conclusao(&mut self, indice: usize) -> Result<(), String> {
        let a = self.agendados.get(indice).ok_or("Gravação não encontrada")?;
        if a.status != "concluida" { return Err("A gravação ainda não foi concluída".into()); }
        self.agendados.remove(indice);
        self.salvar_agenda();
        Ok(())
    }

    pub fn ativo_no_canal(&self, nome: &str) -> bool {
        self.ativos.iter().any(|r| r.canal == nome)
    }

    pub fn iniciar(&mut self, canal: &catalogo::Canal, indice: usize, duracao: Option<u64>) -> Result<PathBuf, String> {
        if self.ativos.len() >= 3 { return Err("Limite de três gravações simultâneas".into()); }
        if self.ativo_no_canal(&canal.nome) || self.processando.contains(&canal.nome) {
            return Err("Este canal já está gravando ou finalizando um arquivo".into());
        }
        let ffmpeg = ffmpeg()?;
        let rec = crate::dvr_stream::GravacaoStream::iniciar(
            canal, indice, duracao, ffmpeg, &pasta().join("Gravacoes"),
            self.eventos_tx.clone()
        )?;
        self.recente.insert(canal.nome.clone(), (FaseGravacao::Iniciando, Instant::now()));
        let pasta = rec.pasta.clone();
        self.ativos.push(rec);
        Ok(pasta)
    }

    pub fn iniciar_manual(
        &mut self, canal: &catalogo::Canal, indice: usize, duracao: Option<u64>,
    ) -> Result<PathBuf, String> {
        let pasta = self.iniciar(canal, indice, duracao)?;
        self.manuais.insert(canal.nome.clone());
        Ok(pasta)
    }

    pub fn retirar_finalizacao_manual(&mut self) -> Option<PathBuf> {
        self.manuais_concluidas.pop_front()
    }

    pub fn parar(&mut self, canal: &str) -> Result<(), String> {
        let ativo = self.ativos.iter_mut().find(|a| a.canal == canal)
            .ok_or("Nenhuma gravação em andamento neste canal")?;
        ativo.solicitar_parada()
    }

    pub fn agendar(
        &mut self, canal: &str, fonte: usize, inicio: &str, minutos: u32,
        programa: Option<String>,
    ) -> Result<(), String> {
        let formato = NaiveDateTime::parse_from_str(inicio.trim(), "%Y-%m-%d %H:%M")
            .map_err(|_| "Data inválida. Use AAAA-MM-DD HH:MM".to_string())?;
        let data = Local.from_local_datetime(&formato).single()
            .ok_or("Horário inexistente ou ambíguo no fuso local")?.timestamp();
        let agora = epg::agora();
        if data < agora || data > agora + 90 * 24 * 3600 {
            return Err("Escolha uma data futura dentro de 90 dias.".into());
        }
        if !(1..=480).contains(&minutos) {
            return Err("Duração permitida: 1 a 480 minutos.".into());
        }
        if self.agendados.iter().any(|a|
            !a.executado && a.canal == canal && a.inicio == data
        ) {
            return Err("Já existe agendamento desse canal no horário".into());
        }
        self.agendados.push(Agendamento {
            canal: canal.into(), fonte, inicio: data,
            fim: data + (minutos as i64) * 60, executado: false,
            status: "aguardando".into(), programa,
        });
        self.salvar_agenda();
        Ok(())
    }

    /// Roda a cada segundo, sem bloquear a interface.
    pub fn atualizar(&mut self, canais: &[catalogo::Canal]) -> Vec<String> {
        let mut avisos = std::mem::take(&mut self.avisos_iniciais);
        let mut i = 0;
        let mut agenda_alterada = false;
        while i < self.ativos.len() {
            let (ativa, mensagens) = self.ativos[i].atualizar();
            avisos.extend(mensagens);
            if self.ativos[i].tem_dados() {
                for a in &mut self.agendados {
                    if a.canal == self.ativos[i].canal && a.status == "iniciando" {
                        a.status = "gravando".into();
                        agenda_alterada = true;
                    }
                }
            }
            if ativa {
                i += 1;
            } else {
                let concluida = self.ativos.remove(i);
                if concluida.tem_dados() && concluida.tem_finalizacao() {
                    self.processando.insert(concluida.canal.clone());
                    for a in &mut self.agendados {
                        if a.canal == concluida.canal
                            && (a.status == "iniciando" || a.status == "gravando") {
                            a.status = "processando".into();
                        }
                    }
                    self.salvar_agenda();
                    self.recente.insert(
                        concluida.canal.clone(), (FaseGravacao::Processando, Instant::now())
                    );
                } else {
                    self.manuais.remove(&concluida.canal);
                    for a in &mut self.agendados {
                        if a.canal == concluida.canal
                            && (a.status == "iniciando" || a.status == "gravando") {
                            a.status = "falhou".into();
                        }
                    }
                    self.salvar_agenda();
                    self.recente.insert(
                        concluida.canal.clone(), (FaseGravacao::Erro, Instant::now())
                    );
                }
            }
        }
        if agenda_alterada { self.salvar_agenda(); }
        while let Ok(evento) = self.eventos_rx.try_recv() {
            self.processando.remove(&evento.canal);
            if self.manuais.remove(&evento.canal) && evento.sucesso {
                if let Some(arquivo) = evento.arquivo.as_ref() {
                    self.manuais_concluidas.push_back(arquivo.clone());
                }
            }
            let fase = if evento.sucesso {
                FaseGravacao::Concluido
            } else {
                FaseGravacao::Erro
            };
            self.recente.insert(evento.canal.clone(), (fase, Instant::now()));
            for a in &mut self.agendados {
                if a.canal == evento.canal && (a.status == "iniciando" || a.status == "gravando"
                    || a.status == "processando" || a.status == "interrompida") {
                    a.status = if evento.sucesso { "concluida" } else { "falhou" }.into();
                }
            }
            self.salvar_agenda();
            avisos.push(evento.mensagem);
        }
        self.recente.retain(|_, (_, t)| t.elapsed() < Duration::from_secs(30));

        let agora = epg::agora();
        let mut salvar = false;
        for i in 0..self.agendados.len() {
            if self.agendados[i].executado { continue; }
            let a = self.agendados[i].clone();
            if agora >= a.fim {
                self.agendados[i].executado = true;
                self.agendados[i].status = "perdida".into();
                salvar = true;
                avisos.push(format!(
                    "Gravação programada perdida (Saimo fechado/desligado?): {}", a.canal
                ));
            } else if agora >= a.inicio {
                // Marcar como iniciada antes de criar o processo impede disparo
                // duplicado no mesmo instante; sessões parcialmente gravadas
                // continuam disponíveis mesmo após encerramento inesperado.
                self.agendados[i].executado = true;
                salvar = true;
                let resultado = if let Some(c) = canais.iter().find(|c| c.nome == a.canal) {
                    self.iniciar(c, a.fonte, Some((a.fim - agora) as u64))
                        .map(|_| ())
                } else {
                    Err(format!("Canal agendado não encontrado: {}", a.canal))
                };
                match resultado {
                    Ok(()) => {
                        self.agendados[i].status = "iniciando".into();
                        avisos.push(format!("DVR: agendamento iniciado — {}", a.canal));
                    }
                    Err(e) => {
                        self.agendados[i].status = "falhou".into();
                        avisos.push(format!("DVR: agendamento falhou — {e}"));
                    }
                }
            }
        }
        let quantidade = self.agendados.len();
        self.agendados.retain(|a| !a.executado || matches!(
            a.status.as_str(), "iniciando" | "gravando" | "processando" | "concluida" | "interrompida"
        ));
        if self.agendados.len() != quantidade { salvar = true; }
        if salvar { self.salvar_agenda(); }
        avisos
    }
}

/// Não destruir os arquivos se o usuário fechar o Saimo durante a gravação.
impl Drop for Dvr {
    fn drop(&mut self) {
        for rec in &mut self.ativos {
            let _ = rec.solicitar_parada();
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn sanitiza_nome_para_arquivos() {
        assert_eq!(nome_seguro("SBT: São Paulo / HD"), "SBT_ São Paulo _ HD");
    }

    #[test]
    fn agenda_legada_sem_status_ou_programa_continua_abrindo() {
        let json = r#"{"canal":"SBT SP","fonte":0,"inicio":1794484800,
            "fim":1794488400,"executado":false}"#;
        let a: Agendamento = serde_json::from_str(json).unwrap();
        assert_eq!(a.canal, "SBT SP");
        assert!(!a.executado);
        assert!(a.status.is_empty());
        assert!(a.programa.is_none());
    }

    #[test]
    fn agenda_nova_guarda_status_e_programacao() {
        let a = Agendamento {
            canal: "SBT SP".into(), fonte: 0, inicio: 1794484800,
            fim: 1794488400, executado: false,
            status: "aguardando".into(), programa: Some("Jornal".into()),
        };
        let json = serde_json::to_string(&a).unwrap();
        let b: Agendamento = serde_json::from_str(&json).unwrap();
        assert_eq!(b.status, "aguardando");
        assert_eq!(b.programa.as_deref(), Some("Jornal"));
    }

    #[test]
    fn agenda_exibe_ativas_e_concluidas_nao_confirmadas() {
        let mut a = Agendamento {
            canal: "SBT".into(), fonte: 0, inicio: 1, fim: 2,
            executado: false, status: "aguardando".into(), programa: None,
        };
        assert!(agendamento_visivel(&a));
        a.executado = true;
        for status in ["iniciando", "gravando", "processando", "concluida"] {
            a.status = status.into();
            assert!(agendamento_visivel(&a), "{status}");
        }
        for status in ["falhou", "perdida", "cancelada", "interrompida", "confirmada"] {
            a.status = status.into();
            assert!(!agendamento_visivel(&a), "{status}");
        }
    }

    #[test]
    fn pasta_padrao_por_dispositivo() {
        assert!(pasta_padrao().is_absolute() || !home().is_absolute());
    }
}
