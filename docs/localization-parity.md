# Localization parity with Microsoft Word

> **Last reviewed:** 2026-10-23 · **Last updated:** 2026-10-23 · **Change:** minor (a Danish pass over all 2,495 entries: three entries that read wrong fixed — Highlighter, Vault Tags, Default Paragraph Font — and four left open for a native speaker; previously minor: Danish interface catalog 2,495 entries and Danish proofing; 20 interface languages; previously trivial: Turkish interface catalog completed, 2,325 entries, #774; previously minor: Turkish interface catalog completed, 2,325 entries, #774; previously minor: Hungarian interface added; previously trivial: sorting and table numbers follow the text's language, #836, #837; previously minor: Polish interface; Polish proofing — spelling, grammar, hyphenation — and a proofing language per run; previously minor: Turkish interface added; previously major: first version; catalogs measured from `crates/ui-egui/src/i18n`, Word's languages from its bundle listing) · **Target:** Microsoft Word (Microsoft 365) for Mac 16.113.4

Interface languages, script support and proofing per language. How translation works (catalog
format, adding a language, clean-room rule): `crates/ui-egui/src/i18n/mod.rs`.

**Dimension: ~28% (estimated; 27% before Portuguese proofing, 25% before Polish and per-run proofing languages), 70–115 h to full
for the twelve key languages** plus ~45 h for the other ~17 languages Word ships, plus
native-speaker review (human).

## What Word ships (measured by listing the bundle)

- **30 interface languages** (31 `.lproj` folders incl. Base): ar, cs, da, de, el, en, en-GB, es,
  es-MX, fi, fr, fr-CA, he, hu, id, it, ja, ko, nl, no, pl, pt (BR), pt-PT, ru, sk, sv, th, tr,
  zh-CN, zh-TW. Arabic and Hebrew get a mirrored interface. No Hindi or Vietnamese interface on Mac.
- **120 proofing tools** (`SharedSupport/Proofing Tools`): spellers for ~50 languages, grammar
  checkers for ~20, hyphenators for ~16, thesauri for ~20.

## What WordCraft ships (measured)

- **20 interface languages**: English plus `zh-hans`, `zh-hant`, `ja`, `uk`, `es`, `pt-br`,
  Serbian `sr`/`sr-latn` (#250), Estonian `et`, Turkish `tr`, Russian `ru`, Italian `it` (#594), German `de` (#574), Polish `pl`, Czech `cs`, Hungarian `hu`, French `fr`, Bulgarian `bg` and **Danish `da`** (2,495 entries, every key the interface uses). Catalogs have 2,057–2,495 entries
  each; Polish has every key of every other catalog, including the built-in style names and the equation gallery, except the citation style names only Hungarian and Bulgarian translate so far (a test checks it). A test (`every_tab_and_command_is_translated`) enforces that every ribbon tab, command
  label and ribbon location is translated in every language.
  `pl`, `pl-PL` and `pl_PL.UTF-8` system locales pick Polish automatically.
- Of the 675 English strings a scan extracts from `tl!(…)` calls and command labels/locations, each
  catalog covers **661 (98%)**. Some newer dialog and pane strings (Equation tab tooltips, Zotero,
  Read Aloud, Backstage greetings) aren't wrapped in `tl!` yet, so they show in English in every
  language: the true share of visible strings is **~93–95%** (estimated).
- Pending PRs: Norwegian Bokmål (#208) and Nynorsk (#210), Hebrew with RTL (#20).
- **Proofing: English, Polish, Portuguese and Danish**, chosen per document from its language (`w:lang`, resolved through
  the styles; Review › Language and the status bar set it); Danish spelling uses an installed Hunspell Danish dictionary, with our own conservative grammar rules and hyph-utf8 hyphenation. Languages without proofing tools get
  no spelling or grammar marks (as in Word without the language's tools) instead of being flagged
  as English. Detail in [Proofing per language](#proofing-per-language).
- Document scripts: shaping via HarfRust (HarfBuzz port) for every script; bidi (UAX #9) and Arabic
  shaping tested (#207); IME on all desktop platforms (winit fixes pending, #155–#164).

## The twelve key languages

Status: `full` = every user-visible string translated and the script renders and edits correctly;
`partial`; `menus only`; `none`. Hours are to `full`, Opus 5.5 agent wall-clock, excluding the
shared items below.

| Language | Code | UI strings translated | Dialogs / tooltips / help | Script support | Native review | Status | Hours to full |
|---|---|---|---|---|---|---|---|
| English | en | source (100%) | ✅ | Latin ✅ | yes | **full** | 0 |
| Simplified Chinese (Mandarin) | zh-hans | 929 entries; 98% of extracted, ~95% of visible | most; newer panes English | CJK ✅, IME ✅; Windows UI font fixed (#248); interface font chain covers every catalog character, installed CJK font after the embedded ones when it doesn't, PingFang found on macOS 12 (#486); font list by Chinese names (宋体, 微软雅黑, #505); no vertical text | no | partial | 3–5 |
| Spanish | es | ~930; 98% / ~95% | most | Latin ✅ | no | partial | 1–2 |
| Hindi | hi | 0 | ❌ | Devanagari shaping via HarfRust, untested; no hyphenation | no | none | 4–6 |
| Arabic | ar | 0 | ❌ | shaping, joining, lam-alef, bidi ✅ (#207); no kashida; **no mirrored UI** | no | none | 6–8 |
| French | fr | 2,033 entries | most; newer dialogs partly English | Latin ✅ | no | partial | 1–2 |
| Portuguese | pt-br (pt-PT none) | ~930; 98% / ~95% (pt-BR) | most | Latin ✅ | no | partial | 2–4 (pt-BR finish + pt-PT) |
| Indonesian | id | 0 | ❌ | Latin ✅ | no | none | 3–4 |
| Japanese | ja | ~930; 98% / ~95% | most | CJK ✅, IME ✅; no vertical text, ruby or document grid | no | partial | 3–5 |
| German | de | 2,021 entries; same key set as zh-hans (#574); 98% of extracted, ~95% of visible | most | Latin ✅ | no | partial | 1–2 |
| Korean | ko | 0 | ❌ | Hangul ✅; macOS Korean IME fixes pending (#163, #164) | no | none | 3–5 |
| Vietnamese | vi | 0 | ❌ | Latin with stacked diacritics, untested | no | none | 3–4 |

Other shipped languages: **14** — Traditional Chinese (`zh-hant`), Ukrainian (`uk`), Serbian
Cyrillic and Latin (`sr`, `sr-latn`, #250), Estonian (`et`), Turkish (`tr`), Russian (`ru`), Italian (`it`, #594), Polish (`pl`), Czech (`cs`), Hungarian (`hu`), French (`fr`), Bulgarian (`bg`) and Danish (`da`, full: interface and proofing), all partial unless noted.
Estonian uses the existing fonts, follows `et-EE` system locales, and is available in Options or
through `ui.language` with `{"value":"et"}`. Its 1,307 entries cover the existing interface catalog.
Estonian spelling, grammar, dates and document templates are not added by the interface catalog.
Turkish works the same way (`tr-TR` and `tr-CY` system locales, `{"value":"tr"}`) with all 2,325
entries, the same keys as the Russian catalog. Command search folds the Turkish dotted and dotless I (`italik` finds İtalik, `YAZI TİPİ`
finds Yazı Tipi) while English labels and ASCII command ids keep their usual matching. Turkish
proofing is not included; terminology and checks: [`localization-tr.md`](localization-tr.md).
Bulgarian (`bg-BG` system locales, `{"value":"bg"}`) has 2,419 entries: every key of the reference
catalog, the equation gallery's labels and everything the interface audit finds (the audit checks
Bulgarian as it does Russian). Counts use count-neutral wording (`Думи: 21`). Bulgarian text is
hyphenated by the Bulgarian syllable rules (#826); Bulgarian spelling and grammar are not included.
Hungarian follows `hu-HU` system locales and `{"value":"hu"}`, with a complete catalog of 2,419 entries: every key of
the Russian catalog, built-in style names shown translated, plus the citation style names. Hungarian proofing is not included; terminology and conventions:
[`localization-hu.md`](localization-hu.md).

Polish is the only one besides English with proofing tools, and a Polish interface writes new
documents in Polish (`pl-PL`).

**Danish read through once (2026-10-23).** An agent pass over all 2,495 entries, reading the
ribbon tabs and groups, the Home, Insert, References and Review labels, the dialogs and the error
sentences as a Dane would. It is not the native-speaker review this table's last-but-two column
asks for; it is what can be done before one.

What it found: no English left inside a Danish sentence (checked word by word), no calque
sentences, and the everyday vocabulary is right — `Brevfletning`, `Punktopstilling`, `Ordoptælling`,
`Spor ændringer`, `Sidehoved`/`Sidefod`, `Figurer`, `Billedtekst`, `Linjeafstand 1,5`,
`Margener`/`Retning`. Three entries read wrong and were fixed:

| English | Was | Now | Why |
|---|---|---|---|
| `Highlighter` | Overstregningstusch | Tekstmarker | Reads as a pen for striking text over, next to `Overstregning` (Overbar) and `Gennemstregning` (Strikethrough) in the same menu; every other catalog uses the marker word (Textmarker, Surligneur, Zakreślacz) |
| `Vault Tags` | Bokstags | Boks-tags | The one word in the vault group without the hyphen its sibling `Boks-tilbagelinks` uses; `bokstags` is not a Danish word |
| `Default Paragraph Font` | Standardafsnitstypografi | Standardafsnitsskrift | `-typografi` is a *style* in this catalog's own vocabulary (Styles = Typografier); the entry names a font |

Four are left open for a native speaker, because the call is a matter of taste rather than of
right and wrong: `Match Fields` → “Match felter” (a borrowed verb where “Tilknyt felter” says what
happens), `Cloud` → “Cloud” (a core pane; “Skylagring” is the ordinary Danish word), `Collapse
Selection` → “Skjul markering” (reads as “hide the selection”), `Editor` → “Editor” (kept — the
feature is known by that name in Danish use too).

Judged and kept: `Ceiling` → “Loft” and `Floor` → “Gulv” are the standard Danish mathematical
terms, as are `Differential` and `Differential θ`; so are `Hyperlink`, `Matrix`, `Sum`, `Zoom`.

**The Math tab.** It is Danish at every point a student meets it: the command labels and
locations in the registry, the tab's own controls, the four outcomes of a solve, the error
messages (`Kunne ikke isolere x: …`, `Taylor-rækkens orden må højst være 1000`), the formula
gallery's groups and its answers (`ingen reel løsning`, `2 < x eller x < -2`). Written in Danish
in the source rather than put through the catalogs, so the other 19 interfaces show Danish here
— the tab is the 1.2 Danish-school feature, and `every_tab_and_command_is_translated` skips it
for that reason (`crates/ui-egui/src/i18n/tests.rs`). That is the open item, not English prose:
a German or Turkish user reaching the tab gets Danish.

## Proofing per language

| Language | Spelling | Suggestions | Grammar | Hyphenation |
|---|---|---|---|---|
| English (`en`, any region; text with no language) | Moby word list (public domain, ~160k words) with inflection rules | edit distance | repeated word, `a`/`an`, sentence capital, space before punctuation, double space | Moby dictionary, our Liang patterns, heuristic |
| Polish (`pl`) | The Hunspell `pl_PL` dictionary installed on the system (OS package, Homebrew, LibreOffice, or the `WORDCRAFT_DICTIONARIES` folder; not bundled, so no Polish spelling marks without it), read at runtime by a Hunspell-style affix engine (`crates/proof/src/affix.rs`); names capitalized, compounds part by part, `Kennedy'ego` | missing diacritics first (`zolw` → `żółw`), the dictionary's confusion table (`żeka` → `rzeka`, `chuśtawka` → `huśtawka`), missing space (`napewno` → `na pewno`), one typo, then a stem scan within two typos | Polish messages: repeated word, space before punctuation, double space, sentence capital (knows `np.`, `r.`, initials, ordinals, dialogue dashes), missing comma before `że`/`który`/`ponieważ`/`gdyż`/`aby`/`żeby`/`jeśli`/`jeżeli`/`gdyby`/`lecz` (not after `mimo`, `chyba`, `tak`, prepositions…), `we`/`ze` (`we wtorek`, `ze szkoły`), doubled punctuation | TeX Polish patterns (Kołodziejska, Jackowski, Ryćko; MIT option) with their 20 exception words; a Polish syllable heuristic for letters outside the alphabet |
| Portuguese (`pt`, any region) | The Hunspell `pt_BR` dictionary installed on the system — VERO and its derivatives (same places as Polish; not bundled, LGPL/MPL), read at runtime by the same affix engine, which reads its `FLAG UTF-8`, 106 flags, continuation classes and `FORBIDDENWORD` spellings; names capitalized, compounds part by part (`guarda-roupa`, `ex-marido`) | missing accents first (`coraçao` → `coração`, `saude` → `saúde`), the dictionary's confusion table (`seje` → `seja`, `excessão` → `exceção`), missing space (`nãoé` → `não é`), one typo, then an accent-folding stem scan | Portuguese messages: repeated word (the emphatic doubles are allowed), space before punctuation, double space, sentence capital (knows `etc.`, `pág.`, `Dr.`, initials, ordinals, dialogue dashes), doubled punctuation; no comma rule — `que` opens a clause only sometimes | TeX Portuguese patterns (Rezende, Almeida, Araujo, Benevides; BSD 3-clause) with their 2 exception words |
| Any other | — (no marks) | — | — | the English patterns, as before |

Measured with the SJP.PL dictionary: every one of the ~4.5 million forms an independent expansion of its affix file
produces is accepted, and 200,000 one-edit mutations that aren't words are all rejected; a check
takes ~1 µs, a suggestion list 1–35 ms (release build). Not yet: other languages' dictionaries,
a thesaurus for Polish, Word's style and clarity refinements, Polish-specific AutoCorrect.

Italian (`it`, #594) follows `it`, `it-IT` and `it-CH` system locales. Its catalog has every key of
the others plus 51 strings that are wrapped in `tl!` but were in no catalog yet (File › Info
properties, the File › Home greeting, list levels, Equation tab gallery and tooltips). Ribbon tabs,
groups and command names use the names Italian Word users know (feature names, as `AGENTS.md`
allows); all other text is written from the meaning of the English. No Italian proofing yet.
## Sorting and numbers in the text's language

Sorts follow the alphabet of the text's language (`w:lang`, else the document default):
Table › Sort, Home › Sort, the index and its letter headings, the bibliography and the Table of
Authorities. Letters with diacritics sort with their base letter (`é` with `e`) unless the
language makes them letters of their own: Polish `ą ć ę ł ń ó ś ź ż`, Czech/Slovak `č ch ř š ž`
(and Slovak `ä ô`), Hungarian `cs dz dzs gy ly ny sz ty zs ö ü`, Swedish/Finnish `å ä ö` and
Danish/Norwegian `æ ø å` after `z`, Spanish `ñ`, Turkish, Croatian/Bosnian/Serbian Latin,
Romanian, Estonian, Latvian, Lithuanian, Slovenian and Icelandic (`crates/doc/src/collate.rs`,
a hand-written per-language alphabet table; full CLDR collation is not used). Table sort and
table formulas read numbers with the language's decimal and group separators (`1 250,50` in
Polish, `1.250,50` in German, `1,250.50` in English) and write formula results with its decimal
separator (`crates/doc/src/locale_number.rs`). Interface lists (styles, fonts) still sort by code
point.

## Region conventions (#838)

As in Word, a new blank document takes its paper from the system **region**, not the interface
language: US Letter in the territories CLDR lists for it (US, Canada, Mexico, the Philippines,
Chile, Colombia, Venezuela and a few Central American ones), A4 everywhere else. Lengths are shown
in inches in the US (and Liberia, Myanmar) and centimetres elsewhere, written with the region's
decimal separator (`2,54 cm` in Poland, Germany, France…); fields take either separator and any
unit (`1 in` in a centimetre field). An explicit File › Options › Advanced › Measurement units
choice still wins; its Automatic entry follows the region again.

Where the region comes from:
- **File › Options › Advanced › Region** (saved; agents: `file.options {"region": "pl-PL"}`, `auto`
  returns to the system's);
- else `WORDCRAFT_REGION` or `LC_ALL`;
- else, on Unix, each convention from its own locale: numbers from `LC_NUMERIC` (else `LANG`),
  paper from `LC_PAPER`, measurement from `LC_MEASUREMENT`;
- else the platform's preferred locale: the macOS preferred locale (`en-PL`), the browser's
  language on the web, and on Windows the home location (Settings › Time & language › Region)
  with the display language. Word follows Windows' regional format instead, so an en-US format
  set by someone living in Germany gives A4 and a decimal comma here; reading the regional format
  needs Win32 calls that are `unsafe` in Rust, which WordCraft forbids.

Runs without preferences (`WORDCRAFT_NO_PREFS`) stay `en-US` unless `WORDCRAFT_REGION` is set, so
agents and screenshots give the same result on every machine. The decimal separator belongs to
the language's own number format (CLDR): a decimal-comma language uses a comma everywhere except
where CLDR says otherwise (`de-CH`), and English, whose `en-XX` locales follow the territory, uses a
comma in decimal-comma territories (`en-DE`, `en-PL`). Other decimal-point languages keep the point
in any territory (`ja-PL` is `2.54 cm`), as CLDR's fallback to the language gives.

Templates (Letter, Resume, Report) keep their own size. Not yet: Word's "Default paper size"
preference of its own, and Word's locale-specific Normal margins (2,5 cm in some regions;
WordCraft keeps 2,54 cm).

## Shared work (not in the per-language hours)

| Item | Hours | Notes |
|---|---|---|
| Wrap the remaining English literals in `tl!` and translate them in every catalog | 4–6 | Makes the six partial languages full except script items |
| Mirrored (right-to-left) interface for Arabic and Hebrew | 15–25 | Ribbon, panes, dialogs, rulers |
| Vertical text, ruby, document grid (Chinese, Japanese, Korean) | counted in [layout-parity.md](layout-parity.md) | |
| Proofing dictionaries, grammar and hyphenation for the 11 non-English key languages | 20–40 | Open dictionaries only (licence check per dictionary); issues #25, #40, #100. Per-run languages, the Hunspell-style affix engine and the TeX pattern reader landed with Polish, so a language with an open Hunspell dictionary and hyphenation patterns is mostly data plus grammar rules |
| Native-speaker review of every catalog | human | None reviewed yet |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-23 | trivial | A Danish read-through of all 2,495 entries (an agent pass, not the native-speaker review the table asks for): no English left inside a Danish sentence and no calque sentences, three entries fixed — `Highlighter` “Overstregningstusch” → “Tekstmarker”, `Vault Tags` “Bokstags” → “Boks-tags”, `Default Paragraph Font` “Standardafsnitstypografi” → “Standardafsnitsskrift” — and four left open for a native speaker (`Match Fields`, `Cloud`, `Collapse Selection`, `Editor`). `Ceiling`/`Floor` → “Loft”/“Gulv” and the `Differential` entries are the standard Danish mathematical terms and stay |
| 2026-10-11 | minor | Turkish interface catalog completed (2,325 entries, the same keys as Russian): equation gallery, built-in style names, watermarks, paper sizes and preview tiles; the interface audit checks Turkish too (#774) |
| 2026-10-12 | trivial | Region conventions: File › Options › Advanced › Region (saved) and an Automatic units entry; Unix `LC_PAPER`/`LC_MEASUREMENT`/`LC_NUMERIC` each decide their own convention; the web reads the browser's locale; `WORDCRAFT_NO_PREFS` runs stay en-US; where the decimal separator comes from (#838 review) |
| 2026-10-12 | minor | New blank documents follow the system region: A4 outside the Letter territories, centimetres outside the US, the region's decimal separator in length fields; explicit Measurement units choice wins (#838) |
| 2026-10-11 | minor | Czech (`cs`) interface catalog added |
| 2026-10-12 | minor | Portuguese proofing: pt-BR spelling from the installed VERO dictionary (the affix engine grew `FLAG UTF-8`, 128 flag bits, continuation classes and `FORBIDDENWORD`), accent-first suggestions, conservative grammar with Portuguese messages, TeX Portuguese hyphenation patterns (BSD 3-clause); Review › Language offers Português (Brasil); a pt-br interface writes pt-BR documents; dimension ~27% → ~28% |
| 2026-10-11 | minor | Polish interface (13 languages) and Polish proofing: SJP.PL spelling with suggestions, Polish grammar rules, TeX Polish hyphenation; proofing language per run; dimension ~25% → ~27% |
| 2026-10-11 | minor | German interface catalog (#574): 2,021 entries covering the full zh-hans key set; locale, saved-preference and bundled-font tests; no German proofing |
| 2026-10-11 | minor | Italian interface catalog (#594), locale selection, saved preference and font coverage checks; count labels put the noun first so they read right at 1 |
| 2026-10-12 | minor | Language-aware sorting (tables, paragraphs, index, bibliography, Table of Authorities) and table numbers/formulas in the text's language (#836, #837) |
| 2026-10-11 | minor | Font dialog sets the Asian text font (`w:eastAsia`) and the Latin text font separately; the Asian list holds the installed Chinese, Japanese and Korean fonts by their interface-language names (#505) |
| 2026-10-11 | minor | French interface catalog (2,033 entries, @pechnikov2026-star), `fr-*` system locales, saved preference and font coverage checks; no French proofing |
| 2026-10-11 | minor | Font list shows installed fonts by their names in the interface language (宋体 for SimSun in Chinese, Japanese and Korean names likewise) and finds them by either name; a Chinese, Japanese or Korean font is stored as `w:eastAsia` too (#505) |
| 2026-10-11 | minor | Chinese/Japanese interface fonts: a test checks every catalog character has a glyph; an installed CJK font follows the embedded faces whenever they miss one; macOS font asset folders searched for PingFang (#486) |
| 2026-10-10 | minor | Russian interface (#300; 1,524 entries, built-in style names shown translated; covers everything the interface audit finds) |
| 2026-10-12 | minor | Bulgarian interface catalog (2,419 entries), locale selection, saved preference, audit coverage and font checks |
| 2026-10-11 | minor | Hungarian interface catalog (2,419 entries, built-in style names shown translated; covers everything the interface audit finds), locale selection, saved preference and font coverage checks |
| 2026-10-11 | minor | Turkish interface catalog (1,307 entries), locale selection, saved preference, Turkish casing in command search and font coverage checks; catalog sizes re-counted |
| 2026-10-11 | trivial | English spelling (#520): our supplement `assets/spelling/en-extra.txt` adds ~220 common modern words the Moby list lacks ("desktop", "download", "email"…) |
| 2026-10-10 | minor | Estonian interface catalog, locale selection, saved preference and font coverage checks; count labels put the noun first so they read right at 1 |
| 2026-10-10 | minor | Serbian (Cyrillic and Latin) interface merged (#250) |
| 2026-10-10 | major | First version: twelve-language table, catalog coverage measured, Word's 30 UI languages and 120 proofing tools listed |
