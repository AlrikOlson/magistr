# Support inbox — recorded Magistr build

A working local inbox for a small support team. It receives mail through an HTTP endpoint, keeps replies in a conversation, ignores duplicate deliveries, searches messages, assigns conversations and marks them resolved.

## Run

Requires Python 3.11 or newer. No third-party Python dependencies.

```sh
python3 server.py --db inbox.sqlite --port 8765
```

Open http://127.0.0.1:8765. The inbox starts empty. Use **Load sample mail** to receive three fictional customer conversations through the real intake endpoint.

```sh
python3 -m unittest discover -s tests -v
```

## Ownership and workflow

1. **Storage:** `store.py` owns persisted conversations, messages, deduplication and update versions.
2. **API:** `server.py` validates HTTP input and calls the store. Every request opens its own SQLite connection.
3. **Interface:** `static/` reads the API. It has no substitute database or hard-coded success responses.
4. **Checks:** integration tests cover duplicate deliveries, threading, persistence, search, assignment, stale updates and invalid input. Browser checks exercise the same server.

Incoming mail has an `external_id`, `thread_id`, `sender`, `subject` and `body`. Its external ID is unique across the workspace. Receiving a new message reopens a resolved conversation. Retrying that same delivery changes nothing. Assignment and resolution require the last-read version, so another teammate's changes cannot be silently overwritten.

## HTTP endpoints

- `POST /api/mail`: receive one message; 201 for a new message, 200 for an identical retry, 409 if an external ID is reused for different content.
- `GET /api/conversations?q=&status=&assignee=`: filter by text, open/resolved status or assignee.
- `GET /api/conversations/<id>`: conversation and ordered messages.
- `PATCH /api/conversations/<id>`: `version`, plus `assignee` and/or `status`; 409 if stale.
- `GET /api/health`: readiness check.

## Scope

This is a completed local demo, not an Internet-facing mail service. It binds only to loopback and has one workspace. Customer names and messages are sample data. It does not send email, authenticate users, process attachments or connect to a live mail provider. Those are separate projects, not implied features of this recording.

The implementation was built in one Codex session using Magistr. The accompanying website recording contains the actual planning, writes and check results, with timestamps. Playback shortens the gaps between actions; it does not invent additional sessions or outcomes.
