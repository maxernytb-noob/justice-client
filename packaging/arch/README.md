# Justice Client pro Arch Linux

Stáhni hotový balíček z https://ertenix.net/justice/ a nainstaluj ho:

```
sudo pacman -U justice-client.pkg.tar.zst
```

Aktualizace: launcher ukáže, že je nová verze — stáhni nový balíček a spusť stejný příkaz.
Logy: spusť `justice-client` z terminálu, nebo otevři `~/.config/JusticeClient/launcher.log`.

## Jak vzniká

`release.ps1` pošle zdrojáky na VPS a `vps-build.sh` je zkompiluje v Arch kontejneru (`Dockerfile.arch`)
přes `makepkg` s tímhle `PKGBUILD`. Ručně ze zdrojáků: rozbal `justice-client-arch.tar.gz` a spusť `makepkg -si`.
