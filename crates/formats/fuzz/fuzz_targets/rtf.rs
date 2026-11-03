#![no_main]

use libfuzzer_sys::fuzz_target;

// The RTF reader: groups, destinations, fields, pictures and tables from hostile bytes.
fuzz_target!(|data: &[u8]| {
    let _ = wordcraft_formats::import("rtf", data);
});
