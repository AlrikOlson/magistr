import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from store import Problem, Store


def mail(external_id="mail-1", thread_id="thread-1", **extra):
    return dict(external_id=external_id, thread_id=thread_id,
                sender="Nora <nora@example.test>", subject="Brake repair",
                body="Can I bring my bicycle in on Friday?", **extra)


class StorageTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.path = Path(self.directory.name) / "inbox.sqlite"
        self.store = Store(self.path)

    def tearDown(self):
        self.directory.cleanup()

    def test_delivery_is_persistent_and_retry_changes_nothing(self):
        item, created = self.store.receive(mail())
        retry, created_again = self.store.receive(mail())
        self.assertTrue(created)
        self.assertFalse(created_again)
        self.assertEqual(item, retry)
        reopened = Store(self.path).get(item["id"])
        self.assertEqual(len(reopened["messages"]), 1)
        self.assertEqual(reopened["messages"][0]["body"], mail()["body"])

    def test_replies_join_and_reopen_the_original_conversation(self):
        item, _ = self.store.receive(mail())
        resolved = self.store.update(item["id"], {"version": item["version"], "status": "resolved"})
        duplicate, _ = self.store.receive(mail())
        self.assertEqual(duplicate["status"], "resolved")
        reply, _ = self.store.receive(mail("mail-2"))
        self.assertEqual(reply["id"], item["id"])
        self.assertEqual(reply["status"], "open")
        self.assertGreater(reply["version"], resolved["version"])
        self.assertEqual(len(reply["messages"]), 2)

    def test_external_id_reuse_for_different_content_is_rejected(self):
        self.store.receive(mail())
        changed = mail()
        changed["body"] = "Different delivery"
        with self.assertRaises(Problem) as error:
            self.store.receive(changed)
        self.assertEqual(error.exception.status, 409)
        self.assertEqual(len(self.store.list()), 1)

    def test_concurrent_duplicate_deliveries_create_one_message(self):
        with ThreadPoolExecutor(max_workers=4) as pool:
            results = list(pool.map(lambda _: self.store.receive(mail()), range(8)))
        self.assertEqual(sum(created for _, created in results), 1)
        self.assertEqual(len(self.store.get(results[0][0]["id"])["messages"]), 1)

    def test_search_matches_message_body_and_treats_wildcards_literally(self):
        self.store.receive(mail())
        second = mail("mail-2", "thread-2")
        second["body"] = "Your 10% discount"
        self.store.receive(second)
        self.assertEqual(len(self.store.list(q="friday")), 1)
        self.assertEqual(len(self.store.list(q="%")), 1)
        self.assertEqual(len(self.store.list(q="' OR 1=1 --")), 0)

    def test_stale_assignment_cannot_overwrite_a_teammate(self):
        item, _ = self.store.receive(mail())
        self.store.update(item["id"], {"version": item["version"], "assignee": "Maya"})
        with self.assertRaises(Problem) as error:
            self.store.update(item["id"], {"version": item["version"], "assignee": "Leo"})
        self.assertEqual(error.exception.status, 409)
        self.assertEqual(self.store.get(item["id"])["assignee"], "Maya")
        self.assertEqual(len(self.store.list(assignee="Maya", status="open")), 1)
        self.assertEqual(self.store.list(assignee="Leo"), [])

    def test_invalid_input_leaves_no_partial_conversation(self):
        for invalid in [{}, {**mail(), "body": ""}, {**mail(), "thread_id": 3}]:
            with self.assertRaises(Problem):
                self.store.receive(invalid)
        self.assertEqual(self.store.list(), [])
        item, _ = self.store.receive(mail())
        for patch in [{"version": True, "status": "resolved"}, {"version": 1, "assignee": "Nobody"}, {"version": 1, "status": "gone"}]:
            with self.assertRaises(Problem):
                self.store.update(item["id"], patch)
        self.assertEqual(self.store.get(item["id"])["status"], "open")


if __name__ == "__main__":
    unittest.main()
