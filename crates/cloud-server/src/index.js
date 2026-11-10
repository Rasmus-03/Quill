// WordCraft collaboration server: Express + WebSocket + operational transform.
//
// Runs on port 3001. Document IDs are paths (e.g. "docs/report.docx").
// Each client connects over WebSocket: "connect", then "snapshot" (full doc),
// then the server applies operations in order and broadcasts them.
"use strict";
const express = require("express");
const { WebSocketServer, WebSocket } = require("ws");
const http = require("http");
const url = require("url");

const app = express();
const server = http.createServer(app);
const wss = new WebSocketServer({ server, path: "/ws" });

// Document store: DocId -> { text, ops[], sessions[] }
const docs = new Map();

app.use(express.json());

// ---- REST API ----

app.get("/api/doc/:id", (req, res) => {
  const doc = docs.get(req.params.id);
  if (!doc) return res.status(404).json({ error: "not found" });
  res.json({ id: doc.id, text: doc.text, version: doc.ops.length });
});

app.post("/api/doc/:id", (req, res) => {
  const id = req.params.id;
  const text = req.body.text ?? "";
  if (typeof text !== "string") return res.status(400).json({ error: "text required" });
  const doc = { id, text, ops: [], sessions: [] };
  docs.set(id, doc);
  res.json({ id: doc.id, text: doc.text, version: 0 });
});

// ---- WebSocket ----

function send(ws, msg) {
  if (ws.readyState === WebSocket.OPEN) ws.send(JSON.stringify(msg));
}

wss.on("connection", (ws, req) => {
  const q = url.parse(req.url, true).query;
  const docId = q.docId;

  if (!docId) {
    ws.close(4001, "no docId");
    return;
  }

  // Find or create the document.
  let doc = docs.get(docId);
  if (!doc) {
    doc = { id: docId, text: "", ops: [], sessions: [] };
    docs.set(docId, doc);
  }
  doc.sessions.push(ws);

  ws.docId = docId;

  // Send the full snapshot.
  send(ws, { type: "snapshot", id: doc.id, text: doc.text, version: doc.ops.length });

  ws.on("message", (raw) => {
    let msg;
    try {
      msg = JSON.parse(raw.toString());
    } catch {
      return;
    }
    switch (msg.type) {
      case "op": {
        // Apply the operation (operational transform simplified: appends only).
        const { op } = msg;
        if (op.kind === "insert") {
          doc.text = doc.text.slice(0, op.off) + op.text + doc.text.slice(op.off);
        } else if (op.kind === "delete") {
          doc.text = doc.text.slice(0, op.off) + doc.text.slice(op.off + op.count);
        }
        doc.ops.push(op);
        // Broadcast to all sessions viewing this doc.
        for (const s of doc.sessions) {
          if (s !== ws && s.readyState === WebSocket.OPEN) {
            send(s, { type: "op", op, version: doc.ops.length });
          }
        }
        break;
      }
      case "snapshot": {
        send(ws, { type: "snapshot", id: doc.id, text: doc.text, version: doc.ops.length });
        break;
      }
    }
  });

  ws.on("close", () => {
    const idx = doc.sessions.indexOf(ws);
    if (idx >= 0) doc.sessions.splice(idx, 1);
  });
});

const PORT = process.env.CLOUD_SERVER_PORT || 3001;
server.listen(PORT, () => {
  console.log(`wordcraft-cloud-server listening on :${PORT}`);
});
