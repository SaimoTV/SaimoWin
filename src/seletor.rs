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

/// No Linux não há um seletor do sistema que todo desktop tenha: usa o do GNOME
/// (zenity), o do KDE (kdialog) ou o yad, o primeiro que existir.
#[cfg(target_os = "linux")]
pub fn escolher_lista() -> Option<PathBuf> {
    use std::process::Command;
    let tentativas: [(&str, Vec<&str>); 3] = [
        ("zenity", vec!["--file-selection", "--title=Escolher lista", "--file-filter=Listas | *.m3u *.m3u8 *.txt", "--file-filter=Todos os arquivos | *"]),
        ("kdialog", vec!["--title", "Escolher lista", "--getopenfilename", ".", "*.m3u *.m3u8 *.txt|Listas"]),
        ("yad", vec!["--file", "--title=Escolher lista", "--file-filter=Listas | *.m3u *.m3u8 *.txt"]),
    ];
    for (programa, args) in tentativas {
        let Ok(saida) = Command::new(programa).args(&args).output() else { continue };
        // Rodou e a pessoa cancelou: não abre outro seletor por cima.
        if !saida.status.success() {
            return None;
        }
        let caminho = String::from_utf8_lossy(&saida.stdout).trim().to_string();
        return (!caminho.is_empty()).then(|| PathBuf::from(caminho)).filter(|p| p.is_file());
    }
    None
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn escolher_lista() -> Option<PathBuf> {
    None
}
