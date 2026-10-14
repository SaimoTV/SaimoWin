//! O quadro de "tocando agora" do Windows: o que aparece junto do volume, na
//! tela de bloqueio e em programas que mostram o que está tocando.
//!
//! O libmpv tem os controles dele, mas só ligados no mpv de linha de comando,
//! e de lá sairia o nome do arquivo (um endereço de HLS) sem logo nenhuma.
//! Aqui o app diz o que está no ar: o canal com a logo e o programa do guia, ou
//! o filme e a série com a capa.
//!
//! Fora do Windows tudo isto é vazio: o desenvolvimento roda no Mac.

use std::time::{Duration, Instant};

/// O que vai no quadro.
#[derive(Clone, PartialEq, Debug)]
pub struct NoAr {
    pub titulo: String,
    pub subtitulo: String,
    /// Logo do canal, ou logo do título (capa quando ele não tem); o próprio
    /// Windows a baixa.
    pub imagem: Option<String>,
    /// Canal ao vivo: sem pausa no quadro, porque o app não pausa ao vivo.
    pub ao_vivo: bool,
}

/// Os botões do quadro que o app atende.
/// Fora do Windows ninguém aperta: o quadro não existe.
#[derive(Clone, Copy, PartialEq, Debug)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum Botao {
    Tocar,
    Pausar,
}

/// O guia muda de programa sozinho: de tempos em tempos o quadro é refeito
/// mesmo sem troca de canal.
const CONFERIR: Duration = Duration::from_secs(20);

pub struct Midia {
    mostrado: Option<(NoAr, bool)>,
    conferido: Instant,
    #[cfg(windows)]
    janela: Option<janela::Controles>,
}

impl Midia {
    /// Pega os controles da janela do app. Sem eles (Windows antigo, ou fora
    /// do Windows) o resto vira nada.
    #[allow(unused_variables)]
    pub fn nova(cc: &eframe::CreationContext<'_>) -> Self {
        Midia {
            mostrado: None,
            conferido: Instant::now(),
            #[cfg(windows)]
            janela: janela::Controles::da_janela(cc),
        }
    }

    /// Se o quadro já mostra este título: o app só monta o que vai nele (e
    /// consulta o guia) quando algo mudou ou de tempos em tempos. A `logo`,
    /// quando dada, também precisa ser a do quadro: ela chega depois do play,
    /// e entra no lugar da capa assim que chega.
    pub fn em_dia(&self, titulo: Option<&str>, logo: Option<&str>, pausado: bool) -> bool {
        let igual = match (&self.mostrado, titulo) {
            (None, None) => true,
            (Some((no_ar, p)), Some(titulo)) => {
                no_ar.titulo == titulo && *p == pausado && (logo.is_none() || no_ar.imagem.as_deref() == logo)
            }
            _ => false,
        };
        igual && self.conferido.elapsed() < CONFERIR
    }

    /// Põe no quadro o que está tocando, ou o limpa quando nada toca.
    pub fn mostrar(&mut self, no_ar: Option<NoAr>, pausado: bool) {
        self.conferido = Instant::now();
        let novo = no_ar.map(|n| (n, pausado));
        if self.mostrado == novo {
            return;
        }
        #[cfg(windows)]
        if let Some(janela) = self.janela.as_ref() {
            match novo.as_ref() {
                Some((no_ar, pausado)) => janela.mostrar(no_ar, *pausado),
                None => janela.limpar(),
            }
        }
        self.mostrado = novo;
    }

    /// Os botões apertados no quadro desde a última vez.
    pub fn botoes(&self) -> Vec<Botao> {
        #[cfg(windows)]
        if let Some(janela) = self.janela.as_ref() {
            return janela.apertados.try_iter().collect();
        }
        Vec::new()
    }
}

#[cfg(windows)]
mod janela {
    use super::{Botao, NoAr};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use std::sync::mpsc::{channel, Receiver};
    use windows::core::{factory, Ref, Result, HSTRING};
    use windows::Foundation::{TypedEventHandler, Uri};
    use windows::Media::{
        MediaPlaybackStatus, MediaPlaybackType, SystemMediaTransportControls,
        SystemMediaTransportControlsButton, SystemMediaTransportControlsButtonPressedEventArgs,
    };
    use windows::Storage::Streams::RandomAccessStreamReference;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::WinRT::ISystemMediaTransportControlsInterop;

    pub struct Controles {
        smtc: SystemMediaTransportControls,
        pub apertados: Receiver<Botao>,
    }

    impl Controles {
        pub fn da_janela(cc: &eframe::CreationContext<'_>) -> Option<Self> {
            let RawWindowHandle::Win32(janela) = cc.window_handle().ok()?.as_raw() else { return None };
            let ctx = cc.egui_ctx.clone();
            Self::ligar(HWND(janela.hwnd.get() as *mut _), ctx).ok()
        }

        fn ligar(hwnd: HWND, ctx: egui::Context) -> Result<Self> {
            let interop = factory::<SystemMediaTransportControls, ISystemMediaTransportControlsInterop>()?;
            let smtc: SystemMediaTransportControls = unsafe { interop.GetForWindow(hwnd)? };
            let (emissor, apertados) = channel();
            // O aperto chega numa thread do Windows: vira recado, e a tela
            // acorda para atendê-lo.
            smtc.ButtonPressed(&TypedEventHandler::new(
                move |_: Ref<SystemMediaTransportControls>,
                      args: Ref<SystemMediaTransportControlsButtonPressedEventArgs>| {
                    let botao = match args.ok()?.Button()? {
                        SystemMediaTransportControlsButton::Play => Botao::Tocar,
                        SystemMediaTransportControlsButton::Pause => Botao::Pausar,
                        _ => return Ok(()),
                    };
                    let _ = emissor.send(botao);
                    ctx.request_repaint();
                    Ok(())
                },
            ))?;
            Ok(Controles { smtc, apertados })
        }

        pub fn mostrar(&self, no_ar: &NoAr, pausado: bool) {
            let _ = self.tentar_mostrar(no_ar, pausado);
        }

        fn tentar_mostrar(&self, no_ar: &NoAr, pausado: bool) -> Result<()> {
            let smtc = &self.smtc;
            smtc.SetIsEnabled(true)?;
            // Ao vivo não pausa: o quadro nem oferece o botão.
            smtc.SetIsPlayEnabled(!no_ar.ao_vivo)?;
            smtc.SetIsPauseEnabled(!no_ar.ao_vivo)?;
            smtc.SetPlaybackStatus(if pausado { MediaPlaybackStatus::Paused } else { MediaPlaybackStatus::Playing })?;

            let quadro = smtc.DisplayUpdater()?;
            quadro.ClearAll()?;
            // Como música e não como vídeo: título e artista são os campos
            // que todo programa de "tocando agora" lê; o subtítulo de vídeo
            // muitos ignoram.
            quadro.SetType(MediaPlaybackType::Music)?;
            let musica = quadro.MusicProperties()?;
            musica.SetTitle(&HSTRING::from(no_ar.titulo.as_str()))?;
            musica.SetArtist(&HSTRING::from(no_ar.subtitulo.as_str()))?;
            musica.SetAlbumTitle(&HSTRING::from("Saimo TV"))?;
            if let Some(imagem) = no_ar.imagem.as_deref() {
                // Endereço que o Windows não aceita só deixa o quadro sem
                // imagem; o título vai mesmo assim.
                if let Ok(uri) = Uri::CreateUri(&HSTRING::from(imagem)) {
                    quadro.SetThumbnail(&RandomAccessStreamReference::CreateFromUri(&uri)?)?;
                }
            }
            quadro.Update()
        }

        pub fn limpar(&self) {
            let _ = (|| -> Result<()> {
                self.smtc.SetPlaybackStatus(MediaPlaybackStatus::Closed)?;
                self.smtc.DisplayUpdater()?.ClearAll()?;
                self.smtc.SetIsEnabled(false)
            })();
        }
    }
}
