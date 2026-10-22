//! A tiny synthetic Danish Hunspell dictionary for tests (written for these tests; not taken
//! from any real dictionary). Real dictionaries are installed on the system, never bundled.
//!
//! It exercises what Danish dictionaries use: `FLAG UTF-8`, `FORBIDDENWORD` (the common
//! misspelling `ihvertfald`), the plural/definite suffixes (`hunde`, `biler`, `skolen`), a
//! prefix (`urolig`), and the suggestion tables (`TRY`, `MAP` for `å`/`a`, `ø`/`o`, `é`/`e`).

pub const AFF: &str = "SET UTF-8
FLAG UTF-8
FORBIDDENWORD F
TRY abcdefghijklmnopqrstuvwxyzæøåé
MAP 3
MAP aå
MAP oø
MAP eé
SFX A Y 1
SFX A 0 e .
SFX B Y 1
SFX B 0 er .
SFX C Y 1
SFX C e en e
SFX D Y 1
SFX D 0 en .
PFX X Y 1
PFX X 0 u .
";

/// Stems (the count line first, as in Hunspell files).
pub const DIC: &str = "207
hej
verden
Danmark
København
dansk
sprog
skrive
skriver
sjovt
mail
læse
bog
hus
hund/A
kat
æble
øl
vand
brød
smør
ost
fisk
fugl
træ
blå
grøn
rød
glæde
kærlighed
arbejde
computer
telefon
avis
morgen
aften
nat
dag/D
uge
år
tid
menneske
samfund
skole/C
lærer
elev
by
land
vej
gade
sommer
vinter
forår
efterår
sol
måne
stjerne
himmel
hav
strand
skov
mark
blomst
vindue
kvinde
mand
barn
dreng
pige
familie
ven
veninde
kær
god
stor
lille
ny
gammel
ung
glad
trist
hurtig
langsom
smuk
klog
stærk
svag
rig
fattig
varm
kold
lys
mørk
åben
lukket
rigtig
forkert
korrekt
ihvertfald/F
hverdag
fællesskab
retskrivning
stavekontrol
sætning
ord
bogstav
lyd
tale
skrift
historie
fremtid
fortid
nutid
jord
luft
ild
regn
sne
vind
vejr
sky
torden
lyn
sommerferie
ferie
arbejdsplads
skolebog
børnehave
bil/B
rolig/X
og
i
at
det
den
en
et
er
var
har
havde
kan
skal
vil
må
ikke
meget
også
kun
her
der
hvor
hvad
hvem
hvorfor
hvordan
når
så
men
fordi
hvis
som
til
fra
med
uden
på
under
over
ved
efter
før
mellem
gennem
mod
om
af
for
jeg
du
han
hun
vi
de
mig
dig
sig
os
jer
dem
min
din
hans
hendes
vores
jeres
deres
to
tre
fire
fem
seks
syv
otte
ni
ti
hvert
fald
";

/// Make [`AFF`]/[`DIC`] the Danish dictionary (every test that needs one calls this; they all
/// install the same one, so tests running in parallel agree).
pub fn install() {
    if let Err(e) = super::install_dictionary(AFF, DIC) {
        panic!("test dictionary: {e}");
    }
}
