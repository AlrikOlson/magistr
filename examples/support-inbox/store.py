"""The single owner of durable inbox state."""
import sqlite3
from contextlib import contextmanager


class Problem(Exception):
    def __init__(self, status, message):
        self.status = status
        super().__init__(message)


def text(value, name, limit):
    if not isinstance(value, str) or not value.strip() or len(value) > limit:
        raise Problem(400, f"{name} must be text between 1 and {limit} characters.")
    return value.strip()


class Store:
    def __init__(self, path):
        self.path = str(path)
        with self.connection() as db:
            db.executescript("""
                CREATE TABLE IF NOT EXISTS conversations (
                    id INTEGER PRIMARY KEY, thread_id TEXT NOT NULL UNIQUE,
                    subject TEXT NOT NULL, sender TEXT NOT NULL,
                    assignee TEXT NOT NULL DEFAULT '',
                    status TEXT NOT NULL DEFAULT 'open' CHECK(status IN ('open','resolved')),
                    version INTEGER NOT NULL DEFAULT 0,
                    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
                );
                CREATE TABLE IF NOT EXISTS messages (
                    id INTEGER PRIMARY KEY, external_id TEXT NOT NULL UNIQUE,
                    conversation_id INTEGER NOT NULL REFERENCES conversations(id),
                    sender TEXT NOT NULL, subject TEXT NOT NULL, body TEXT NOT NULL,
                    received_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
                );
                CREATE INDEX IF NOT EXISTS message_conversation ON messages(conversation_id);
            """)

    @contextmanager
    def connection(self):
        db = sqlite3.connect(self.path, timeout=10)
        db.row_factory = sqlite3.Row
        db.execute("PRAGMA foreign_keys = ON")
        try:
            with db:
                yield db
        finally:
            db.close()

    def receive(self, payload):
        if not isinstance(payload, dict):
            raise Problem(400, "Send a message object.")
        limits = {"external_id": 200, "thread_id": 200, "sender": 200, "subject": 300, "body": 20000}
        message = {key: text(payload.get(key), key, limit) for key, limit in limits.items()}
        with self.connection() as db:
            # A writer lock makes duplicate detection and insertion one atomic operation.
            db.execute("BEGIN IMMEDIATE")
            previous = db.execute("""
                SELECT m.*, c.thread_id FROM messages m
                JOIN conversations c ON c.id=m.conversation_id WHERE external_id=?
            """, (message["external_id"],)).fetchone()
            if previous:
                if any(previous[key] != value for key, value in message.items()):
                    raise Problem(409, "This message ID already belongs to a different delivery.")
                return self._get(db, previous["conversation_id"]), False
            db.execute("""
                INSERT INTO conversations(thread_id,subject,sender) VALUES(?,?,?)
                ON CONFLICT(thread_id) DO NOTHING
            """, (message["thread_id"], message["subject"], message["sender"]))
            conversation_id = db.execute("SELECT id FROM conversations WHERE thread_id=?",
                                         (message["thread_id"],)).fetchone()["id"]
            db.execute("""
                INSERT INTO messages(external_id,conversation_id,sender,subject,body) VALUES(?,?,?,?,?)
            """, (message["external_id"], conversation_id, message["sender"], message["subject"], message["body"]))
            db.execute("""
                UPDATE conversations SET status='open', version=version+1,
                updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?
            """, (conversation_id,))
            return self._get(db, conversation_id), True

    def _get(self, db, conversation_id):
        row = db.execute("SELECT * FROM conversations WHERE id=?", (conversation_id,)).fetchone()
        if row is None:
            raise Problem(404, "Conversation not found.")
        item = dict(row)
        item["messages"] = [dict(row) for row in db.execute(
            "SELECT * FROM messages WHERE conversation_id=? ORDER BY id", (conversation_id,))]
        return item

    def get(self, conversation_id):
        with self.connection() as db:
            return self._get(db, conversation_id)

    def list(self, q="", status="", assignee=""):
        if status not in ("", "open", "resolved"):
            raise Problem(400, "Choose open or resolved conversations.")
        if len(q) > 200:
            raise Problem(400, "Search must be 200 characters or fewer.")
        with self.connection() as db:
            return [dict(row) for row in db.execute("""
                SELECT c.*, (SELECT count(*) FROM messages WHERE conversation_id=c.id) AS message_count,
                    (SELECT body FROM messages WHERE conversation_id=c.id ORDER BY id DESC LIMIT 1) AS preview
                FROM conversations c WHERE (?='' OR c.status=?) AND (?='' OR c.assignee=?)
                    AND (?='' OR instr(lower(c.subject),lower(?)) > 0
                        OR instr(lower(c.sender),lower(?)) > 0
                        OR EXISTS(SELECT 1 FROM messages m WHERE m.conversation_id=c.id
                            AND instr(lower(m.body),lower(?)) > 0))
                ORDER BY c.updated_at DESC, c.id DESC
            """, (status, status, assignee, assignee, q, q, q, q))]

    def update(self, conversation_id, patch):
        if not isinstance(patch, dict) or type(patch.get("version")) is not int:
            raise Problem(400, "Include the conversation version.")
        if not set(patch) <= {"version", "assignee", "status"} or len(patch) < 2:
            raise Problem(400, "Choose an assignee or status to update.")
        if "assignee" in patch and patch["assignee"] not in ("", "Maya", "Leo", "Sam"):
            raise Problem(400, "Choose a member of the team.")
        if "status" in patch and patch["status"] not in ("open", "resolved"):
            raise Problem(400, "Choose open or resolved.")
        with self.connection() as db:
            db.execute("BEGIN IMMEDIATE")
            item = self._get(db, conversation_id)
            if item["version"] != patch["version"]:
                raise Problem(409, "Someone changed this conversation. Reload it before saving.")
            db.execute("""
                UPDATE conversations SET assignee=?,status=?,version=version+1,
                updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=?
            """, (patch.get("assignee", item["assignee"]), patch.get("status", item["status"]), conversation_id))
            return self._get(db, conversation_id)
