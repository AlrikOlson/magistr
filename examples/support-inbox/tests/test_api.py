import json
import tempfile
import threading
import unittest
from pathlib import Path
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from server import make_server
from test_store import mail


class ApiTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.directory = tempfile.TemporaryDirectory()
        cls.server = make_server(Path(cls.directory.name) / "api.sqlite")
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.origin = f"http://127.0.0.1:{cls.server.server_port}"

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join()
        cls.directory.cleanup()

    def request(self, method, path, payload=None, headers=None):
        data = json.dumps(payload).encode() if payload is not None else None
        request = Request(self.origin + path, data=data, method=method,
                          headers={"Content-Type": "application/json", **(headers or {})})
        try:
            response = urlopen(request, timeout=5)
        except HTTPError as error:
            response = error
        with response:
            return response.status, json.load(response)

    def test_full_intake_search_assign_resolve_workflow(self):
        status, delivered = self.request("POST", "/api/mail", mail("api-1", "api-thread"))
        self.assertEqual(status, 201)
        item = delivered["conversation"]
        self.assertEqual(self.request("POST", "/api/mail", mail("api-1", "api-thread"))[0], 200)
        status, rows = self.request("GET", "/api/conversations?q=Friday")
        self.assertEqual(status, 200)
        self.assertEqual(len(rows), 1)
        path = f"/api/conversations/{item['id']}"
        status, assigned = self.request("PATCH", path, {"version": item["version"], "assignee": "Maya"})
        self.assertEqual(status, 200)
        self.assertEqual(assigned["assignee"], "Maya")
        self.assertEqual(self.request("PATCH", path, {"version": item["version"], "status": "resolved"})[0], 409)
        status, resolved = self.request("PATCH", path, {"version": assigned["version"], "status": "resolved"})
        self.assertEqual(status, 200)
        self.assertEqual(resolved["status"], "resolved")
        self.assertEqual(self.request("GET", path)[1]["status"], "resolved")

    def test_invalid_mail_and_missing_conversation(self):
        self.assertEqual(self.request("POST", "/api/mail", {"body": "missing fields"})[0], 400)
        self.assertEqual(self.request("POST", "/api/mail", [1, 2])[0], 400)
        self.assertEqual(self.request("GET", "/api/conversations/99999")[0], 404)
        self.assertEqual(self.request("GET", "/api/conversations?status=deleted")[0], 400)

    def test_foreign_origin_and_hostname_are_rejected(self):
        self.assertEqual(self.request("POST", "/api/mail", mail(), {"Origin": "https://elsewhere.example"})[0], 403)
        self.assertEqual(self.request("GET", "/api/health", headers={"Host": "elsewhere.example"})[0], 403)
        self.assertEqual(self.request("GET", "/api/health"), (200, {"ready": True}))


if __name__ == "__main__":
    unittest.main()
