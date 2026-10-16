# Quill

Gratis tekstbehandling med dansk skolematematik indbygget.

Quill er et Word-kompatibelt skriveprogram (.docx ud og ind) med en **WordMat-fane** til
matematik: regn, løs ligninger, enheder, matricer, grafer — med svar i grønt direkte i ligningen.

## Installér (Linux)

1. Hent `quill-1.0.0-linux-x86_64.AppImage` under [Releases](../../releases).
2. Installér regnemotoren Maxima (medfølger ikke):
   - Arch/CachyOS: `sudo pacman -S maxima`
   - Debian/Ubuntu: `sudo apt install maxima`
   - Fedora: `sudo dnf install maxima`
3. Kør AppImage-filen. WordMat-fanen opretter sig selv ved første start.

Valgfrit (bedre løsninger over reelle tal): læg `solvereal.mac` i
`~/.local/share/mathcraft/packages/` og sæt `MATHCRAFT_SOLVEREAL` til stien.
SolveReal er GPL og hentes separat — den bundtes aldrig med Quill.

## WordMat-fanen

| Tast | Gør |
|---|---|
| Alt+M | Ny ligning |
| Alt+B | Beregn markeringen |
| Alt+L | Isolér variabel |

Markér f.eks. `x^2-4=0` → **Isolér** giver `x = -2 eller x = 2` i grønt.
Markér `x^2, -5, 5` → **Tegn graf**. Se alle arbejdsgange i
[`docs/wordmat-fane.md`](docs/wordmat-fane.md).

## Licens

Gratis og open source (MIT OR Apache-2.0 — se `LICENSE-MIT`, `LICENSE-APACHE`).
Bygger på WordCraft fra ArtCraft-teamet og bidragydere (se `ATTRIBUTION.md`).
Original matematik-kode i `crates/engine/src/cmd/mathcraft.rs`,
`mathgraf.rs`, `crates/doc/src/math_edit.rs`.
