#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use wordcraft_fonts::{FontDb, decode_font_file};

// One database for the process: loading the bundled faces per input would dominate the run.
static DB: OnceLock<FontDb> = OnceLock::new();

// The TrueType/OpenType/collection parser, reached the way a dropped font reaches it: check the
// file, then add every face it names. WOFF/WOFF2 unwrap first and then take the same path.
fuzz_target!(|data: &[u8]| {
    if let Ok(bytes) = decode_font_file(data) {
        let db = DB.get_or_init(|| FontDb::with_font_dirs(Vec::new()));
        let _ = db.add_font(bytes);
    }
});
