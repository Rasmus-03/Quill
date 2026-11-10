# WordCraft cloud collaboration server — real-time co-authoring
#
# The collaboration server runs on Node.js with Express + WebSocket (port 3001).
# It stores operational-transform document state in memory (`docs` Map).
#
#   npm install        # installs express + ws
#   npm start          # node src/index.js (port 3001)
#
# Document API:
#   GET  /api/doc/:id     -> { id, text, version }
#   POST /api/doc/:id     -> { text }          (create/reset)
#
# WebSocket API (path: /ws?docId=X):
#   client -> server:  { type: "op", op: { kind, text, off } }
#   server -> client:  { type: "snapshot", id, text, version }
#   server -> client:  { type: "op", op, version }
#
# Operational transform (simplified, appends only):
#   { kind: "insert", text: "hello", off: 0 }   -> insert at offset
#   { kind: "delete", count: 5 }                -> delete count chars at offset (off=0)
#   For true OT, each client would compute the transform against concurrent
#   operations; this server keeps the naive version for the baseline.
#
# WordCraft integration (`wordcraft-cli` / `wordcraft-cli run`):
#   cloud.connect                    -> connect to the server
#   cloud.create_doc                 -> { docId, text }
#   cloud.snapshot                   -> { docId, text, version }
#   cloud.op                         -> { docId, op }
#   cloud.subscribe                  -> { docId }  (WebSocket)
#   cloud.unsubscribe                -> { docId }
#
# Example: two editors on one document
#   1. wordcraft-cli run --cmd 'cloud.connect' --cmd 'cloud.create_doc' --arg '{"docId":"report.docx","text":"Hello"}'
#   2. Editor A:   wordcraft-cli run --cmd 'cloud.op' --arg '{"docId":"report.docx","op":{"kind":"insert","text":" world","off":0}}'
#   3. Editor B (same doc): receives the op over WebSocket and the text becomes "Hello world" in one shot.

folder ./cloud-server  [if not exists]
cd cloud-server
npm install
