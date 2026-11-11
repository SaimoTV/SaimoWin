#!/bin/bash
# Monta o Saimo TV para Linux a partir do Mac, para PC comum (x86_64) e ARM (aarch64):
#
#   dist/linux/SaimoTV-Linux-x86_64.AppImage    roda em qualquer distro (2022 para cá),
#   dist/linux/SaimoTV-Linux-aarch64.AppImage   já com a libmpv dentro
#   dist/linux/saimo-tv_<versão>_amd64.deb      Ubuntu, Debian, Mint, Pop!_OS...
#   dist/linux/saimo-tv_<versão>_arm64.deb      (usa a libmpv da distribuição)
#
# É o mesmo código do Windows, então a tela é a mesma. Precisa de:
#   cargo-zigbuild e zig (compila para Linux sem máquina Linux),
#   mksquashfs (brew install squashfs),
#   mpv/linux-<arq>/   libmpv do Ubuntu 22.04 (scripts/montar_mpv_linux.sh),
#   mpv/appimage/runtime-<arq>   runtime oficial do AppImage (AppImage/type2-runtime).
set -euo pipefail
cd "$(dirname "$0")"

VERSAO=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
SAIDA=dist/linux
rm -rf "$SAIDA"
mkdir -p "$SAIDA"

for arq in x86_64 aarch64; do
  [ -f "mpv/linux-$arq/libmpv.so.1" ] || { echo "falta mpv/linux-$arq (rode scripts/montar_mpv_linux.sh)"; exit 1; }
  [ -f "mpv/appimage/runtime-$arq" ] || { echo "falta mpv/appimage/runtime-$arq"; exit 1; }
done
command -v mksquashfs >/dev/null || { echo "falta o mksquashfs (brew install squashfs)"; exit 1; }

# Mensagens de erro do Rust guardam o caminho de quem compilou; fora do binário.
export RUSTFLAGS="${RUSTFLAGS:-} --remap-path-prefix=$HOME=/build"

# glibc 2.31: Ubuntu 20.04, Debian 11 e tudo depois.
for arq in x86_64 aarch64; do
  echo "==> compilando para Linux $arq"
  cargo zigbuild --release --target "$arq-unknown-linux-gnu.2.31" -q
done

DESKTOP='[Desktop Entry]
Type=Application
Name=Saimo TV
GenericName=TV ao vivo, filmes e séries
Comment=Canais ao vivo, filmes e séries das suas listas
Exec=saimo-tv
Icon=saimo-tv
Terminal=false
Categories=AudioVideo;Video;Player;TV;
Keywords=tv;iptv;m3u;canais;filmes;series;
StartupWMClass=saimo-tv'

for arq in x86_64 aarch64; do
  bin="target/$arq-unknown-linux-gnu/release/saimo-tv"
  if grep -q -a "$HOME" "$bin"; then
    echo "o binário ainda tem o caminho da pasta pessoal; nada foi gerado"; exit 1
  fi

  echo "==> AppImage $arq"
  app=$(mktemp -d)/SaimoTV.AppDir
  mkdir -p "$app/usr/bin" "$app/usr/lib" "$app/usr/share/applications" "$app/usr/share/icons/hicolor/128x128/apps"
  cp "$bin" "$app/usr/bin/saimo-tv"
  cp mpv/linux-$arq/*.so* "$app/usr/lib/"
  printf '%s\n' "$DESKTOP" > "$app/saimo-tv.desktop"
  printf '%s\nX-AppImage-Version=%s\n' "$DESKTOP" "$VERSAO" > "$app/usr/share/applications/saimo-tv.desktop"
  cp assets/logo.png "$app/saimo-tv.png"
  cp assets/logo.png "$app/usr/share/icons/hicolor/128x128/apps/saimo-tv.png"
  ln -s saimo-tv.png "$app/.DirIcon"
  cat > "$app/AppRun" <<'RUN'
#!/bin/sh
# As bibliotecas do pacote se acham sozinhas (RUNPATH=$ORIGIN): nada de
# LD_LIBRARY_PATH, que vazaria para o navegador e o seletor de arquivo.
AQUI="$(dirname "$(readlink -f "$0")")"
exec "$AQUI/usr/bin/saimo-tv" "$@"
RUN
  chmod 755 "$app/AppRun" "$app/usr/bin/saimo-tv"
  imagem="$SAIDA/SaimoTV-Linux-$arq.AppImage"
  mksquashfs "$app" "$SAIDA/.fs" -root-owned -no-xattrs -noappend -comp zstd -Xcompression-level 19 -quiet >/dev/null
  cat "mpv/appimage/runtime-$arq" "$SAIDA/.fs" > "$imagem"
  rm -f "$SAIDA/.fs"
  chmod 755 "$imagem"

  echo "==> .deb $arq"
  deb_arq=$([ "$arq" = x86_64 ] && echo amd64 || echo arm64)
  python3 scripts/montar_deb.py "$bin" "$VERSAO" "$deb_arq" "$SAIDA/saimo-tv_${VERSAO}_${deb_arq}.deb"
done

ls -la "$SAIDA"
