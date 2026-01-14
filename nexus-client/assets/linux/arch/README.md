# Arch Linux Packages

PKGBUILD files for building Nexus BBS client on Arch Linux.

## AUR Packages

These packages are also maintained in the [Arch User Repository (AUR)](https://aur.archlinux.org/):

- [nexus-client](https://aur.archlinux.org/packages/nexus-client) - Release version
- [nexus-client-git](https://aur.archlinux.org/packages/nexus-client-git) - Git version (latest from main branch)

## Installing from AUR

Using an AUR helper like `yay` or `paru`:

```bash
# Release version
yay -S nexus-client

# Git version (latest development)
yay -S nexus-client-git
```

## Building Manually

```bash
# Release version
makepkg -si

# Git version
makepkg -si -p PKGBUILD-git
```

## Files

- `PKGBUILD` - Builds from tagged release tarballs
- `PKGBUILD-git` - Builds from latest git commit

## Updating

When a new version is released:

1. Update `pkgver` in `PKGBUILD`
2. Update `sha256sums` (run `makepkg -g` to generate)
3. Reset `pkgrel` to `1`
4. Push changes to AUR