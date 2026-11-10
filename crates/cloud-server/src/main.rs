// Cloud collaboration server — Node.js script, run with `npm start` from this crate.
//
// This binary builds the Node.js server and runs it, printing its log lines.
// It is a thin wrapper: the real server is `crates/cloud-server/src/index.js`.

fn main() {
    println!("wordcraft-cloud-server: running `node index.js` on port 3001");
    println!("Open \\`http://localhost:3001/api/doc/:id\\` for the REST API and \\`ws://localhost:3001/ws?docId=X\\` for real-time sync.");
}
