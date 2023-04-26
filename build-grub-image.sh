# Builds the GRUB image used in the ISO and writes the directory tree to
# third_party/grub-image. This only needs to be re-run if the set of required
# modules changes. The resulting files should be committed.

grub-mkimage -C auto -d /usr/lib/grub/i386-pc -O i386-pc-eltorito \
    -o third_party/grub-image/boot/grub/i386-pc/eltorito.img -p '/boot/grub' \
    biosdisk iso9660 normal vga vbe multiboot multiboot2 normal
