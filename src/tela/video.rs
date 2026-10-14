//! O vídeo no fundo da janela, a multi tela e a abertura.
//!
//! Igual ao que já funcionava: o desenho novo da interface não mexe no
//! caminho do quadro do mpv até a tela.

use crate::App;
use eframe::egui::{self, Color32, Pos2, Rect};
use eframe::glow::HasContext;
use std::sync::Arc;


/// O vídeo, no fundo de tudo.
///
/// Era aqui a tela preta do Windows. O vídeo morava numa `Area` de ordem
/// `Background`, na crença de que isso o deixaria embaixo de tudo — mas os
/// painéis (`SidePanel`, `CentralPanel`) desenham nessa mesma ordem, e entre
/// camadas de mesma ordem o egui põe por último a que foi criada por último.
/// A `Area` do vídeo nascia a cada quadro, então ia parar em cima: o retângulo
/// preto que ela pinta antes do quadro cobria a interface inteira.
///
/// Só acontecia onde o mpv carrega — sem ele esta função sai na primeira
/// linha, nada é pintado por cima e a tela aparece. Por isso o defeito parecia
/// vir do vídeo, e mexer na decodificação nunca resolveu.
///
/// A correção é pintar no próprio fundo dos painéis, antes deles. Dentro de
/// uma camada vale a ordem de inserção, e `video` é a primeira coisa que
/// `desenhar` chama — o quadro fica embaixo, e a interface por cima.
pub fn video(app: &mut App, ctx: &egui::Context) {
    let Some(mpv) = app.mpv.clone() else { return };
    let rect = ctx.content_rect();
    let pintor = ctx.layer_painter(egui::LayerId::background());
    pintor.rect_filled(rect, 0.0, Color32::BLACK);
    if let Some(outra) = app.outra_tela.as_ref() {
        let (em_foco, fora) = telas_da_multi_tela(rect, app.foco_a_direita);
        video_numa_parte(&pintor, mpv, em_foco);
        video_numa_parte(&pintor, outra.mpv.clone(), fora);
        return;
    }
    pintor.add(egui::PaintCallback {
        rect,
        callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, pintor| {
            if let Err(erro) = mpv.ligar_video() {
                crate::telemetria::erro(erro);
                return;
            }
            let gl = pintor.gl();
            let fbo = unsafe { gl.get_parameter_i32(eframe::glow::DRAW_FRAMEBUFFER_BINDING) };
            let [largura, altura] = info.screen_size_px;
            unsafe { estado_padrao_de_opengl(gl) };
            mpv.desenhar(fbo, largura as i32, altura as i32);
        })),
    });
}

/// A abertura cobrindo a janela: o azul-marinho e, assim que o mpv dela tem
/// quadro, o vídeo por cima; `veu` é o azul que dissolve a marca no fim.
pub fn abertura(ctx: &egui::Context, mpv: Option<Arc<crate::mpv::Mpv>>, veu: f32) {
    let rect = ctx.content_rect();
    let pintor = ctx.layer_painter(egui::LayerId::background());
    pintor.rect_filled(rect, 0.0, marinho(1.0));
    if let Some(mpv) = mpv {
        pintor.add(egui::PaintCallback {
            rect,
            callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, pintor| {
                if mpv.ligar_video().is_err() {
                    return;
                }
                let gl = pintor.gl();
                let fbo = unsafe { gl.get_parameter_i32(eframe::glow::DRAW_FRAMEBUFFER_BINDING) };
                let [largura, altura] = info.screen_size_px;
                unsafe { estado_padrao_de_opengl(gl) };
                mpv.desenhar(fbo, largura as i32, altura as i32);
            })),
        });
    }
    veu_da_abertura(ctx, veu);
}

/// O azul da abertura por cima de tudo, com a opacidade pedida.
pub fn veu_da_abertura(ctx: &egui::Context, veu: f32) {
    if veu <= 0.0 {
        return;
    }
    let pintor = ctx.layer_painter(egui::LayerId::new(egui::Order::Debug, egui::Id::new("veu-da-abertura")));
    pintor.rect_filled(ctx.content_rect(), 0.0, marinho(veu));
}

fn marinho(opacidade: f32) -> Color32 {
    let [r, g, b] = crate::abertura::MARINHO;
    Color32::from_rgb(r, g, b).gamma_multiply(opacidade.clamp(0.0, 1.0))
}

/// As duas metades da janela na multi tela: (a em foco, a outra).
pub fn telas_da_multi_tela(janela: Rect, foco_a_direita: bool) -> (Rect, Rect) {
    let meio = janela.center().x;
    let esquerda = Rect::from_min_max(janela.min, Pos2::new(meio - 1.0, janela.max.y));
    let direita = Rect::from_min_max(Pos2::new(meio + 1.0, janela.min.y), janela.max);
    if foco_a_direita { (direita, esquerda) } else { (esquerda, direita) }
}

/// O alvo próprio de cada mpv na multi tela: textura e framebuffer do
/// tamanho da metade dele.
struct Alvo {
    fbo: eframe::glow::Framebuffer,
    textura: eframe::glow::Texture,
    largura: i32,
    altura: i32,
}

thread_local! {
    /// Os alvos por mpv (pelo endereço dele). O desenho é sempre na thread
    /// da janela, então não precisa de trava.
    static ALVOS: std::cell::RefCell<std::collections::HashMap<usize, Alvo>> = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Desenha um mpv só numa parte da janela.
///
/// A API de render do mpv desenha sempre o framebuffer inteiro que recebe, a
/// partir do canto. Para pôr duas imagens lado a lado, cada mpv desenha num
/// framebuffer próprio, do tamanho da metade dele, que depois é copiado
/// (`glBlitFramebuffer`) para o lugar certo da janela.
fn video_numa_parte(pintor: &egui::Painter, mpv: Arc<crate::mpv::Mpv>, parte: Rect) {
    pintor.add(egui::PaintCallback {
        rect: parte,
        callback: Arc::new(eframe::egui_glow::CallbackFn::new(move |info, pintor| {
            if let Err(erro) = mpv.ligar_video() {
                crate::telemetria::erro(erro);
                return;
            }
            let gl = pintor.gl();
            let vista = info.viewport_in_pixels();
            let (largura, altura) = (vista.width_px, vista.height_px);
            if largura <= 0 || altura <= 0 {
                return;
            }
            unsafe {
                let janela = gl.get_parameter_i32(eframe::glow::DRAW_FRAMEBUFFER_BINDING);
                let janela = std::num::NonZeroU32::new(janela as u32).map(eframe::glow::NativeFramebuffer);
                let chave = Arc::as_ptr(&mpv) as usize;
                let Some(fbo) = ALVOS.with_borrow_mut(|alvos| alvo(gl, alvos, chave, largura, altura)) else { return };
                estado_padrao_de_opengl(gl);
                mpv.desenhar(fbo.0.get() as i32, largura, altura);
                // A cópia obedece à tesoura, e o mpv não promete deixá-la desligada.
                gl.disable(eframe::glow::SCISSOR_TEST);
                gl.bind_framebuffer(eframe::glow::READ_FRAMEBUFFER, Some(fbo));
                gl.bind_framebuffer(eframe::glow::DRAW_FRAMEBUFFER, janela);
                gl.blit_framebuffer(
                    0, 0, largura, altura,
                    vista.left_px, vista.from_bottom_px, vista.left_px + largura, vista.from_bottom_px + altura,
                    eframe::glow::COLOR_BUFFER_BIT, eframe::glow::LINEAR,
                );
                gl.bind_framebuffer(eframe::glow::FRAMEBUFFER, janela);
            }
        })),
    });
}

/// O framebuffer do mpv `chave`, criado ou refeito no tamanho pedido.
unsafe fn alvo(
    gl: &eframe::glow::Context,
    alvos: &mut std::collections::HashMap<usize, Alvo>,
    chave: usize,
    largura: i32,
    altura: i32,
) -> Option<eframe::glow::Framebuffer> {
    use eframe::glow;
    if let Some(a) = alvos.get(&chave) {
        if a.largura == largura && a.altura == altura {
            return Some(a.fbo);
        }
    }
    if let Some(velho) = alvos.remove(&chave) {
        gl.delete_framebuffer(velho.fbo);
        gl.delete_texture(velho.textura);
    }
    let textura = gl.create_texture().ok()?;
    gl.bind_texture(glow::TEXTURE_2D, Some(textura));
    gl.tex_image_2d(
        glow::TEXTURE_2D, 0, glow::RGBA8 as i32, largura, altura, 0,
        glow::RGBA, glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(None),
    );
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::LINEAR as i32);
    gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::LINEAR as i32);
    gl.bind_texture(glow::TEXTURE_2D, None);
    let fbo = gl.create_framebuffer().ok()?;
    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(fbo));
    gl.framebuffer_texture_2d(glow::FRAMEBUFFER, glow::COLOR_ATTACHMENT0, glow::TEXTURE_2D, Some(textura), 0);
    alvos.insert(chave, Alvo { fbo, textura, largura, altura });
    Some(fbo)
}

/// Devolve o OpenGL ao estado padrão antes de entregar o quadro ao mpv.
///
/// A documentação do mpv é explícita: `mpv_render_context_render` exige o
/// contexto "reasonably set to OpenGL standard defaults", e cita
/// `GL_SCISSOR_TEST` e `GL_BLEND` nos valores de fábrica — desligados.
///
/// O egui chama o callback com o oposto disso, porque é assim que ele desenha
/// a própria interface: teste de tesoura ligado, caixa de recorte posta e
/// mistura ligada. Medido aqui, o mpv recebia `SCISSOR_TEST=1 BLEND=1`. Ele
/// não desliga nada — confia no que a documentação pede — e desenha o quadro
/// em várias passagens, cada uma num alvo próprio, limpando-as com `glClear`,
/// que obedece à tesoura. O que sai daí depende da placa e do tamanho do
/// vídeo: imagem recortada, cor lavada pela mistura, ou nada.
///
/// Não era esta a causa da tela preta — essa era a camada, acima — mas é um
/// contrato quebrado que estragaria o quadro assim que ele voltasse a
/// aparecer.
///
/// Depois do callback o egui refaz o estado dele sozinho
/// (`prepare_painting`), e a caixa de recorte volta a cada primitiva, então
/// não há o que restaurar aqui.
unsafe fn estado_padrao_de_opengl(gl: &eframe::glow::Context) {
    gl.disable(eframe::glow::SCISSOR_TEST);
    gl.disable(eframe::glow::BLEND);
    gl.disable(eframe::glow::CULL_FACE);
    gl.disable(eframe::glow::DEPTH_TEST);
    gl.disable(eframe::glow::STENCIL_TEST);
    gl.color_mask(true, true, true, true);
    gl.depth_mask(true);
    gl.stencil_mask(0xFF);
    // O egui deixa o programa e o VAO dele ligados; o padrão é nenhum.
    gl.use_program(None);
    gl.bind_vertex_array(None);
    gl.bind_buffer(eframe::glow::ARRAY_BUFFER, None);
    gl.bind_buffer(eframe::glow::ELEMENT_ARRAY_BUFFER, None);
    gl.active_texture(eframe::glow::TEXTURE0);
    gl.bind_texture(eframe::glow::TEXTURE_2D, None);
}

