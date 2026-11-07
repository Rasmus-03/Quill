#![no_main]

use libfuzzer_sys::fuzz_target;

// The ZIP/XML reader, straight from bytes: a package is the boundary a document arrives through.
fuzz_target!(|data: &[u8]| {
    let _ = wordcraft_docx::read(data);
});
