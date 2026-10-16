# Changelog

## 1.0.0 — WordMat tab (first release)

Danish school-math inside WordCraft, powered by Maxima (same engine WordMat uses).

- **WordMat ribbon tab**: Beregn, Løs, Definér, Enheder, LaTeX, Matrix, Statistik, Calculus, Formler, Graf, Mere — all Danish, with Alt+M / Alt+B / Alt+L shortcuts
- **Answers in green** inside the same equation (`= result`); re-running after edits replaces the old answer
- **Solvers**: exact, numeric, isolate-variable with variable picker, inequalities, systems, ODEs
- **Units** (Danish decimal commas), definitions (`:=`), matrices, limits, Taylor, statistics
- **Graphs**: function plots (up to 3 curves, gaps at poles) rendered to PNG in the document
- **Formula collection** plus personal saved equations; LaTeX export
- **Equation highlighting** across structure boundaries, copy-paste keeps equations as equations
- Maxima runs bounded (30 s timeout); remote-code patterns in pasted equations are rejected

Requires Maxima installed separately (`sudo pacman -S maxima` on Arch/CachyOS).
Optional SolveReal (`solvereal.mac`, GPL, user-installed, never bundled) for real-domain solving.
