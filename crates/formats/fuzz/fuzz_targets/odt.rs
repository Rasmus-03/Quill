#![no_main]

use libfuzzer_sys::fuzz_target;

// The OpenDocument reader: a ZIP package, its content.xml and any pictures it names.
fuzz_target!(|data: &[u8]| {
    let _ = wordcraft_formats::import("odt", data);
});
