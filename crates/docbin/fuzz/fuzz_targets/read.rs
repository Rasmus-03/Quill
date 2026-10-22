#![no_main]

use libfuzzer_sys::fuzz_target;

// The .doc compound-file reader and its piece table: untrusted bytes in, a document or an error.
fuzz_target!(|data: &[u8]| {
    let _ = wordcraft_docbin::read(data);
});
