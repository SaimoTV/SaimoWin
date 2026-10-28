//! A abertura animada: o vídeo da marca, uma vez, quando o programa abre.
//!
//! O vídeo vai dentro do executável (uns 800 KB) e é gravado na pasta
//! temporária na primeira vez: assim não há arquivo para o pacote esquecer.
//! Toca num mpv só dele, sem som e cobrindo a janela inteira; o último quadro
//! é a marca pronta sobre o azul-marinho, que se dissolve no mesmo azul e dali
//! abre o app. Clique ou qualquer tecla pula. Se o mpv não começar o vídeo em
//! 1,5 s, a abertura sai da frente sozinha: ela nunca pode ser o motivo de o
//! programa não abrir.

use crate::mpv::Mpv;
use std::sync::Arc;
use std::time::{Duration, Instant};

const VIDEO: &[u8] = include_bytes!("../assets/abertura.mp4");

/// O azul-marinho do fundo do vídeo (#0b0f2a): é a cor da janela enquanto a
/// abertura está na frente, para não piscar preto antes do primeiro quadro.
pub const MARINHO: [u8; 3] = [11, 15, 42];

/// Quanto o mpv tem para mostrar o primeiro quadro antes de a abertura desistir.
const ESPERA_DO_INICIO: Duration = Duration::from_millis(1500);
/// Teto da abertura inteira, caso o fim do vídeo nunca seja avisado.
const TETO: Duration = Duration::from_secs(9);
/// A marca se dissolvendo no azul, no fim do vídeo.
const DISSOLVER: Duration = Duration::from_millis(280);
/// O app aparecendo por baixo do azul.
const REVELAR: Duration = Duration::from_millis(450);

pub enum Fase {
    /// O vídeo tocando (ou ainda abrindo).
    Video { desde: Instant, comecou: bool },
    /// O vídeo acabou: o azul cobre a marca aos poucos.
    Dissolvendo { desde: Instant },
    /// O app já está desenhado, e o azul por cima vai sumindo.
    Revelando { desde: Instant },
}

pub struct Abertura {
    pub mpv: Option<Arc<Mpv>>,
    pub fase: Fase,
}

impl Abertura {
    /// Sobe a abertura, ou nada quando não há como (sem mpv, foto de teste,
    /// `SAIMO_SEM_ABERTURA`).
    pub fn nova(caminho_do_mpv: &std::path::Path) -> Option<Self> {
        // Modo seguro (o app caiu na abertura anterior): sem o segundo mpv da abertura.
        if std::env::var("SAIMO_FOTO").is_ok() || std::env::var("SAIMO_SEM_ABERTURA").is_ok() || crate::diagnostico::nivel() >= 1 {
            return None;
        }
        let arquivo = gravar_video()?;
        let mpv = Mpv::novo(caminho_do_mpv).ok()?;
        // Sem som, cobrindo a janela (corta as bordas em vez de faixas), e
        // parado no último quadro: é ele que se dissolve no azul.
        mpv.definir("mute", "yes");
        mpv.definir("aid", "no");
        mpv.definir("panscan", "1.0");
        mpv.definir("keep-open", "yes");
        mpv.tocar(&arquivo.to_string_lossy(), None, None);
        Some(Abertura {
            mpv: Some(Arc::new(mpv)),
            fase: Fase::Video { desde: Instant::now(), comecou: false },
        })
    }

    /// Se a abertura ainda esconde o app (vídeo ou dissolvendo).
    pub fn cobrindo(&self) -> bool {
        !matches!(self.fase, Fase::Revelando { .. })
    }

    /// O vídeo já tem quadro para mostrar.
    pub fn com_quadro(&self) -> bool {
        matches!(self.fase, Fase::Video { comecou: true, .. } | Fase::Dissolvendo { .. })
    }

    /// Pula direto para o app aparecendo.
    pub fn pular(&mut self) {
        if self.cobrindo() {
            self.revelar();
        }
    }

    fn revelar(&mut self) {
        self.fase = Fase::Revelando { desde: Instant::now() };
        // O mpv da abertura não serve para mais nada.
        if let Some(mpv) = self.mpv.take() {
            mpv.parar();
        }
    }

    /// Anda a abertura um quadro. Devolve `false` quando acabou de vez.
    pub fn andar(&mut self) -> bool {
        match self.fase {
            Fase::Video { desde, comecou } => {
                let tempo = self.mpv.as_ref().and_then(|m| m.ler("time-pos")).and_then(|t| t.parse::<f64>().ok());
                let fim = self.mpv.as_ref().and_then(|m| m.ler("eof-reached")).as_deref() == Some("yes");
                if !comecou && tempo.is_some() {
                    self.fase = Fase::Video { desde, comecou: true };
                } else if !comecou && desde.elapsed() > ESPERA_DO_INICIO {
                    // O mpv não conseguiu: o app aparece sem esperar mais.
                    self.revelar();
                } else if fim || desde.elapsed() > TETO {
                    self.fase = Fase::Dissolvendo { desde: Instant::now() };
                }
                true
            }
            Fase::Dissolvendo { desde } => {
                if desde.elapsed() >= DISSOLVER {
                    self.revelar();
                }
                true
            }
            Fase::Revelando { desde } => desde.elapsed() < REVELAR,
        }
    }

    /// Quanto do azul vai por cima de tudo, de 0 a 1.
    pub fn veu(&self) -> f32 {
        match self.fase {
            Fase::Video { comecou, .. } => if comecou { 0.0 } else { 1.0 },
            Fase::Dissolvendo { desde } => suave(desde.elapsed().as_secs_f32() / DISSOLVER.as_secs_f32()),
            Fase::Revelando { desde } => 1.0 - suave(desde.elapsed().as_secs_f32() / REVELAR.as_secs_f32()),
        }
    }
}

/// Entrada e saída macias, em vez de uma rampa reta.
fn suave(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Grava o vídeo na pasta temporária, se ainda não estiver lá igual.
fn gravar_video() -> Option<std::path::PathBuf> {
    let caminho = std::env::temp_dir().join("saimo-tv-abertura.mp4");
    let igual = std::fs::metadata(&caminho).map(|m| m.len() == VIDEO.len() as u64).unwrap_or(false);
    if !igual {
        std::fs::write(&caminho, VIDEO).ok()?;
    }
    Some(caminho)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn video_vai_no_executavel() {
        // Um MP4 começa com a caixa "ftyp".
        assert_eq!(&VIDEO[4..8], b"ftyp");
    }

    #[test]
    fn rampa_suave_nas_pontas() {
        assert_eq!(suave(-1.0), 0.0);
        assert_eq!(suave(0.0), 0.0);
        assert_eq!(suave(1.0), 1.0);
        assert_eq!(suave(2.0), 1.0);
        assert!((suave(0.5) - 0.5).abs() < 1e-6);
    }
}
