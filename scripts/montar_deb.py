#!/usr/bin/env python3
"""Monta o .deb do Saimo TV no Mac, sem dpkg-deb.

O .deb é um arquivo `ar` com três membros: debian-binary, control.tar.gz e
data.tar.gz. Tudo sai como dono root e com data fixa, para o mesmo binário dar
o mesmo pacote.

Uso: montar_deb.py <binário> <versão> <amd64|arm64> <saída.deb>
"""
from __future__ import annotations

import gzip
import io
import sys
import tarfile
from pathlib import Path

RAIZ = Path(__file__).resolve().parent.parent
DATA = 1_700_000_000

DESKTOP = """[Desktop Entry]
Type=Application
Name=Saimo TV
GenericName=TV ao vivo, filmes e séries
Comment=Canais ao vivo, filmes e séries das suas listas
Exec=saimo-tv
Icon=saimo-tv
Terminal=false
Categories=AudioVideo;Video;Player;TV;
Keywords=tv;iptv;m3u;canais;filmes;series;
StartupWMClass=saimo-tv
"""


def tar_gz(arquivos: list[tuple[str, bytes, int]]) -> bytes:
    """(caminho, conteúdo, modo) -> tar.gz com as pastas criadas no caminho."""
    bruto = io.BytesIO()
    with tarfile.open(fileobj=bruto, mode="w", format=tarfile.GNU_FORMAT) as tar:
        pastas: set[str] = set()
        for caminho, _, _ in arquivos:
            partes = caminho.strip("./").split("/")[:-1]
            for i in range(1, len(partes) + 1):
                pastas.add("./" + "/".join(partes[:i]))
        for pasta in sorted(pastas):
            info = tarfile.TarInfo(pasta)
            info.type, info.mode, info.mtime = tarfile.DIRTYPE, 0o755, DATA
            tar.addfile(info)
        for caminho, conteudo, modo in arquivos:
            info = tarfile.TarInfo(caminho)
            info.size, info.mode, info.mtime = len(conteudo), modo, DATA
            tar.addfile(info, io.BytesIO(conteudo))
    return gzip.compress(bruto.getvalue(), mtime=DATA)


def membro_ar(nome: str, conteudo: bytes) -> bytes:
    cabecalho = f"{nome:<16}{DATA:<12}{0:<6}{0:<6}{0o100644:<8o}{len(conteudo):<10}`\n".encode()
    return cabecalho + conteudo + (b"\n" if len(conteudo) % 2 else b"")


def main() -> int:
    binario, versao, arquitetura, saida = sys.argv[1:5]
    executavel = Path(binario).read_bytes()
    icone = (RAIZ / "assets/logo.png").read_bytes()
    dados = [
        ("./usr/bin/saimo-tv", executavel, 0o755),
        ("./usr/share/applications/saimo-tv.desktop", DESKTOP.encode(), 0o644),
        ("./usr/share/icons/hicolor/128x128/apps/saimo-tv.png", icone, 0o644),
    ]
    tamanho_kb = sum(len(c) for _, c, _ in dados) // 1024 + 1
    controle = f"""Package: saimo-tv
Version: {versao}
Architecture: {arquitetura}
Maintainer: Saimo TV <saimo.tv.gs@gmail.com>
Installed-Size: {tamanho_kb}
Depends: libmpv2 | libmpv1, libxkbcommon-x11-0, libegl1 | libgl1
Recommends: zenity | kdialog, xdg-utils
Section: video
Priority: optional
Homepage: https://saimo-tv.pages.dev
Description: Canais ao vivo, filmes e séries das suas listas
 Saimo TV para Linux: o mesmo app do Windows, com guia de programação,
 acervo de filmes e séries, rádios, eventos e o Saimo Connect. Lê listas M3U
 e contas com usuário e senha (Xtream).
"""
    pos = """#!/bin/sh
set -e
command -v update-desktop-database >/dev/null && update-desktop-database -q /usr/share/applications || true
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t /usr/share/icons/hicolor || true
"""
    ctrl = tar_gz([("./control", controle.encode(), 0o644), ("./postinst", pos.encode(), 0o755)])
    pacote = b"!<arch>\n" + membro_ar("debian-binary", b"2.0\n") + membro_ar("control.tar.gz", ctrl) + membro_ar("data.tar.gz", tar_gz(dados))
    Path(saida).write_bytes(pacote)
    print(f"{saida}: {len(pacote) // 1024} KB")
    return 0


if __name__ == "__main__":
    sys.exit(main())
