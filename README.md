# Quill

Free word processing with Danish mathematics built in.

Quill is a Word-compatible writing app (.docx in and out) with a **WordMat tab** for
mathematics: calculate, solve equations, units, matrices, plots — with answers in green
right inside the equation.

![Quill writing](docs/images/quill-writing.png)

![Quill solving a quadratic equation](docs/images/quill-math.png)

## Install (Linux)

1. Download `quill-1.0.0-linux-x86_64.AppImage` from [Releases](../../releases).
2. Install the Maxima calculation engine (not bundled):
   - Arch/CachyOS: `sudo pacman -S maxima`
   - Debian/Ubuntu: `sudo apt install maxima`
   - Fedora: `sudo dnf install maxima`
3. Run the AppImage. The WordMat tab creates itself on first launch.

Optional (better real-domain solving): place `solvereal.mac` in
`~/.local/share/mathcraft/packages/` and point `MATHCRAFT_SOLVEREAL` at it.
SolveReal is GPL and never ships with Quill — you fetch it yourself.

## The WordMat tab

| Key | Does |
|---|---|
| Alt+M | New equation |
| Alt+B | Calculate the selection |
| Alt+L | Isolate a variable |

Select e.g. `x^2-4=0` → **Isolér** gives `x = -2 eller x = 2` in green.
Select `x^2, -5, 5` → **Tegn graf**. All workflows are in
[`docs/wordmat-fane.md`](docs/wordmat-fane.md).

## License

Free and open source (MIT OR Apache-2.0 — see `LICENSE-MIT`, `LICENSE-APACHE`).
Builds on WordCraft by the ArtCraft team and contributors (see `ATTRIBUTION.md`).
Original mathematics code in `crates/engine/src/cmd/mathcraft.rs`,
`mathgraf.rs`, `crates/doc/src/math_edit.rs`.
