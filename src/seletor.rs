//! O seletor de arquivo nativo do Windows (GetOpenFileNameW), sem crate extra.
//! Bloqueia até a pessoa escolher: chame de uma thread, nunca da do desenho.
//! Fora do Windows não há seletor (devolve `None`).

use std::path::PathBuf;

#[cfg(windows)]
pub fn escolher_lista() -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt;
    use windows::core::{w, PCWSTR, PWSTR};
    use windows::Win32::UI::Controls::Dialogs::{GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY, OFN_NOCHANGEDIR, OFN_PATHMUSTEXIST, OPENFILENAMEW};

    // Pares "texto\0padrão\0", terminados por um \0 extra.
    let filtro: Vec<u16> = "Listas (*.m3u;*.m3u8;*.txt)\0*.m3u;*.m3u8;*.txt\0Todos os arquivos (*.*)\0*.*\0\0".encode_utf16().collect();
    let mut buf = vec![0u16; 4096];
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        lpstrFilter: PCWSTR(filtro.as_ptr()),
        nFilterIndex: 1,
        lpstrFile: PWSTR(buf.as_mut_ptr()),
        nMaxFile: buf.len() as u32,
        lpstrTitle: w!("Escolher lista"),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY | OFN_NOCHANGEDIR,
        ..Default::default()
    };
    // SAFETY: `ofn` aponta para `filtro` e `buf`, que vivem até o fim da chamada.
    if !unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
        return None;
    }
    let fim = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(PathBuf::from(std::ffi::OsString::from_wide(&buf[..fim])))
}

#[cfg(not(windows))]
pub fn escolher_lista() -> Option<PathBuf> {
    None
}
