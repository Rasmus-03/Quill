# The Math tab

The tab emulates WordMat (the Danish mathematics add-in) with English commands.

Everything you know from WordMat — as a tab in Quill. Needs the
Maxima program (the same calculation engine WordMat uses).

## Shortcuts

| Key | Does |
|---|---|
| Alt+M | New equation at the cursor |
| Alt+B | Calculate the selection |
| Alt+L | Isolate a variable in the selection |

## Workflows

**Calculate an equation:** put the cursor in it, or select it (or part of
it), and press the button. The answer is appended with `=` in green — inside
the same equation. Re-run after editing: the old answer is replaced.

**Several variables:** a dialog asks which variable (`x`, `y`, …).
One variable is picked automatically. Commands also take `{"var": "y"}`.

**Units:** select e.g. `5 km/h to m/s` → Convert. Danish decimal
commas work everywhere (`2,5`).

**Definitions:** select `g := 9.82` → Define. Used in every calculation
in the document until you delete them.

**Saved equations:** right-click an equation → Save equation. It shows up under
Formula collection → Mine. Save again to remove.

**Formula collection:** interest, quadratics and geometry — click inserts
the formula as a new equation.

## The buttons

- **Calculate** — numeric value (Exact/Decimal mode from the More group)
- **Simplify, Factor, Expand, Differentiate, Antiderivative**
- **Solve equation, Solve numeric, Isolate, Solve ODE**
- **Units, Decimal, To LaTeX**
- **Determinant, Inverse, Eigenvalues, Solve system** (write `matrix([1,2],[3,4])`)
- **Limit** (`expression, var, point`), **Taylor series** (`expression, var, point, order`)
- **Mean, Standard deviation** (`[1, 2, 3, 4]` — dotted decimals inside lists)
- **More:** Maxima command (raw Maxima), Previous result, Exact, Decimal

## Graphs

Select e.g. `x^2, -5, 5` and press **Plot graph** (the Graph group). Several
curves are separated with `;`: `x^2; 2*x+1, -5, 5`. Forms like `f(x)=x^2`
or `y=x^2` work too; `e^x` and `ln(x)` are understood. Without a range
-10..10 is drawn. Poles (e.g. `1/x`) become gaps in the curve, not garbage.
The graph replaces the selection as a picture (PNG).

## Requirements

- **Maxima** in PATH (`sudo pacman -S maxima` on Arch/CachyOS).
  Without it the buttons report a clean error instead of crashing.
- Real answers are the default (solved over the reals with stock Maxima).
  Experts may still point `MATHCRAFT_SOLVEREAL` at a `solvereal.mac` file
  for its solver; it is GPL-licensed and never ships with the app.

## First launch

The Math tab creates itself on first launch. If deleted by accident,
it comes back on next launch.
