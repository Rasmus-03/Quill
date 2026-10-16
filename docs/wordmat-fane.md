# WordMat-fanen i WordCraft

Alt det, du kender fra WordMat — som en fane i WordCraft. Kræver programmet
Maxima (samme regnemotor WordMat bruger).

## Genveje

| Tast | Gør |
|---|---|
| Alt+M | Ny ligning ved cursoren |
| Alt+B | Beregn det markerede |
| Alt+L | Isolér variabel i det markerede |

## Arbejdsgange

**Regn på en ligning:** Stil cursoren i den, eller markér den (eller noget
af den), og tryk på knappen. Svaret sættes på med `=` i grønt — inde i
samme ligning. Kør igen efter redigering: det gamle svar erstattes.

**Flere variable:** En dialog spørger hvilken variabel (`x`, `y`, …).
Én variabel vælges selv. Kommandoer tager også `{"var": "y"}`.

**Enheder:** Markér f.eks. `5 km/h to m/s` → Konverter. Danske
decimalkommaer virker overalt (`2,5`).

**Definitioner:** Markér `g := 9.82` → Definér. Bruges i alle beregninger
i dokumentet, indtil du sletter dem.

**Gemte ligninger:** Højreklik en ligning → Gem ligning. Den ligger under
Formelsamling → Mine. Gem igen for at fjerne.

**Formelsamling:** Rente, andengrad og geometri — klik sætter formlen ind
som ny ligning.

## Knapperne

- **Beregn** — talværdi (Eksakt/Decimal-tilstand fra Mere-gruppen)
- **Reducér, Faktorisér, Udvid, Differentier, Stamfunktion**
- **Løs ligning, Løs numerisk, Isolér, Løs diff.ligning**
- **Enheder, Decimaltal, Til LaTeX**
- **Determinant, Invers, Egenværdier, Løs system** (skriv `matrix([1,2],[3,4])`)
- **Grænseværdi** (`udtryk, var, punkt`), **Taylorrække** (`udtryk, var, punkt, orden`)
- **Middel, Spredning** (`[1, 2, 3, 4]` — punktum-decimaler i lister)
- **Mere:** Maxima-kommando (rå Maxima), Forrige resultat, Eksakt, Decimal

## Grafer

Markér f.eks. `x^2, -5, 5` og tryk **Tegn graf** (Graf-gruppen). Flere
kurver adskilles med `;`: `x^2; 2*x+1, -5, 5`. Udtryk som `f(x)=x^2`
eller `y=x^2` virker også; `e^x` og `ln(x)` forstås. Uden interval
tegnes -10..10. Brøkstreger/polers (f.eks. `1/x`) bliver til slip i
kurven, ikke vrøvl. Grafen erstatter markeringen som billede (PNG).

## Krav

- **Maxima** i PATH (`sudo pacman -S maxima` på Arch/CachyOS).
  Uden den melder knapperne pænt fejl i stedet for at crashe.
- **SolveReal** (valgfri, bedre løsninger over reelle tal):
  læg `solvereal.mac` i `~/.local/share/mathcraft/packages/` og sæt
  `MATHCRAFT_SOLVEREAL` til stien. Uden den bruges Maximas egen solve().
  (SolveReal er GPL-licenseret og medfølger aldrig appen — du henter og
  installerer den selv.)

## Første start

WordMat-fanen opretter sig selv ved første start. Slettes den ved et
uheld, kommer den igen ved næste start.
