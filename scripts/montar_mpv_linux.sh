#!/bin/bash
# Monta a libmpv que vai dentro do AppImage do Linux, a partir do Ubuntu 22.04
# (glibc 2.35): o que sai daqui roda em qualquer distro de 2022 para cá.
#
# Roda num Linux com debootstrap (a VM de teste: limactl shell saimo-linux).
# Para outra arquitetura, precisa do qemu-user-binfmt.
#
#   sudo bash scripts/montar_mpv_linux.sh amd64 /tmp/mpv-linux-x86_64.tar.gz
#   sudo bash scripts/montar_mpv_linux.sh arm64 /tmp/mpv-linux-aarch64.tar.gz
#
# Depois, no Mac: tar -xzf <arquivo> -C mpv/   (fica mpv/linux-x86_64/ ou mpv/linux-aarch64/)
set -euo pipefail
ARCH="${1:?amd64 ou arm64}"
SAIDA="${2:?arquivo .tar.gz de saída}"
case "$ARCH" in
  amd64) DIR=linux-x86_64; TRIPLA=x86_64-linux-gnu; ESPELHO=http://archive.ubuntu.com/ubuntu ;;
  arm64) DIR=linux-aarch64; TRIPLA=aarch64-linux-gnu; ESPELHO=http://ports.ubuntu.com/ubuntu-ports ;;
  *) echo "arquitetura desconhecida: $ARCH"; exit 1 ;;
esac
RAIZ="/srv/jammy-$ARCH"

if [ ! -x "$RAIZ/bin/bash" ]; then
  debootstrap --arch="$ARCH" --variant=minbase --include=ca-certificates jammy "$RAIZ" "$ESPELHO"
fi
printf 'deb %s jammy main universe\ndeb %s jammy-updates main universe\n' "$ESPELHO" "$ESPELHO" > "$RAIZ/etc/apt/sources.list"
chroot "$RAIZ" apt-get update -qq
chroot "$RAIZ" env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends libmpv1 >/dev/null

# Fica do sistema de quem instala: base do glibc, driver de vídeo e tudo que
# conversa com ele (X11, Wayland, DRM, VA-API, VDPAU), som (ALSA carrega os
# plugins do sistema), fontes e as bibliotecas que toda distro traz. Levar uma
# versão velha de qualquer uma delas quebra o driver de quem tem distro nova.
FICA='^(libc|libm|libdl|libpthread|librt|libresolv|libutil|libnsl|libcrypt|ld-linux.*|libgcc_s|libstdc\+\+|libGL.*|libEGL|libGLX|libGLdispatch|libOpenGL|libdrm.*|libgbm|libX.*|libxcb.*|libxkbcommon.*|libwayland.*|libxshmfence|libasound|libva.*|libvdpau|libfontconfig|libfreetype|libexpat|libz|libuuid|libblkid|libmount|libselinux|libsystemd|libudev|libdbus-1|libcap|libgcrypt|libgpg-error|liblzma|libzstd|libbz2|libpcre.*|libffi|libglib-2\.0|libgobject-2\.0|libgio-2\.0|libgmodule-2\.0|libpulse.*|libsndfile|libpipewire.*|libjack)\.so'

DESTINO=$(mktemp -d)/$DIR
mkdir -p "$DESTINO"
LIBMPV="/usr/lib/$TRIPLA/libmpv.so.1"
cp -L "$RAIZ$LIBMPV" "$DESTINO/libmpv.so.1"
chroot "$RAIZ" ldd "$LIBMPV" | awk '/=> \//{print $1, $3}' | while read -r nome caminho; do
  if ! echo "$nome" | grep -q -E "$FICA"; then
    # Links do "alternatives" apontam para caminhos absolutos: resolver lá dentro.
    real=$(chroot "$RAIZ" readlink -f "$caminho")
    cp "$RAIZ$real" "$DESTINO/$nome"
  fi
done
# Cada biblioteca procura as outras na própria pasta ($ORIGIN). Sem isso seria
# preciso LD_LIBRARY_PATH, que vaza para o que o app abre (seletor de arquivo,
# navegador) e faz o programa do sistema carregar a biblioteca velha daqui.
command -v patchelf >/dev/null || apt-get install -y -qq patchelf >/dev/null
for so in "$DESTINO"/*.so*; do
  patchelf --set-rpath '$ORIGIN' "$so"
done
VERSAO=$(chroot "$RAIZ" dpkg-query -W -f='${Version}' libmpv1)
cat > "$DESTINO/LEIAME.txt" <<TXT
libmpv1 $VERSAO (Ubuntu 22.04, $ARCH) e as dependências que nem toda distro tem.
Montado por scripts/montar_mpv_linux.sh em $(date -u +%Y-%m-%d).
TXT
tar -C "$(dirname "$DESTINO")" -czf "$SAIDA" "$DIR"
echo "$(ls "$DESTINO" | wc -l) bibliotecas, $(du -sh "$DESTINO" | cut -f1) -> $SAIDA"
