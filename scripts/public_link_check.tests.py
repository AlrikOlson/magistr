#!/usr/bin/env python3
"""Real loopback HTTP checks for deployed-copy and reporting-route failures."""
from html import escape
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
import threading
import unittest
from urllib.parse import quote

import public_link_check as gate


class FixtureHandler(BaseHTTPRequestHandler):
    def log_message(self, *args):
        pass

    def do_GET(self):
        self.server.requests.append(self.path)
        status, body, headers = self.server.routes.get(self.path, (404, "Missing", {}))
        self.send_response(status)
        for key, value in headers.items():
            self.send_header(key, value)
        payload = body.encode()
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


class PublicReadinessTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.server = ThreadingHTTPServer(("127.0.0.1", 0), FixtureHandler)
        cls.thread = threading.Thread(target=cls.server.serve_forever, daemon=True)
        cls.thread.start()
        cls.base = f"http://127.0.0.1:{cls.server.server_port}"

    @classmethod
    def tearDownClass(cls):
        cls.server.shutdown()
        cls.server.server_close()
        cls.thread.join()

    def setUp(self):
        self.server.requests = []
        self.server.routes = {}
        self.repository = "owner/repo"
        self.site = self.base + "/site"
        self.github = self.base + "/github"
        self.form = self.github + "/owner/repo/security/advisories/new"
        self.set_route("/site/", "Website")
        self.guide()
        for suffix in ("", "/releases", "/issues", "/security/policy"):
            self.set_route("/github/owner/repo" + suffix, "Public GitHub page")
        self.set_route("/api/repos/owner/repo/private-vulnerability-reporting", '{"enabled":true}')
        self.set_route("/github/owner/repo/security/advisories/new", "", status=302,
                       headers={"Location": self.github + "/login?return_to=" + quote(self.form, safe="")})
        self.set_route("/npm/magistr/latest", '{"name":"magistr","version":"0.1.0"}')
        self.set_route("/cargo/api/v1/crates/magistr-cli/0.1.0",
                       '{"version":{"crate":"magistr-cli","num":"0.1.0","yanked":false}}')

    def set_route(self, path, body, status=200, headers=None):
        self.server.routes[path] = (status, body, headers or {})

    def guide(self, pages=None, strings=None, extra=""):
        """Serve every manual guide; `strings` and `extra` address the installation guide."""
        pages = dict(gate.GUIDE_CONTRACT if pages is None else pages)
        if strings is not None:
            pages["installation"] = strings
        for name, items in pages.items():
            # Simulate Shiki's nested tokens and HTML entities without removing semantic whitespace.
            text = "".join("<p>" + "<span>".join(
                escape(item).replace(" ", "&nbsp;", 1).split("~")) + "</p>" for item in items)
            text = text.replace("magistr", "<span>magistr</span>")
            tail = extra if name == "installation" else ""
            self.set_route(f"/site/docs/{name}/", "<html><body>" + text + tail + "</body></html>")

    def audit(self, stage="release", version="0.1.0"):
        return gate.audit(stage, version, site=self.site, repository=self.repository,
                          github=self.github, api=self.base + "/api",
                          npm=self.base + "/npm", cargo=self.base + "/cargo")

    def failures(self, receipt):
        return [check["name"] for check in receipt["checks"] if not check["passed"]]

    def test_current_public_release_and_correct_reporting_signin_pass(self):
        self.assertEqual(self.failures(self.audit()), [])

    def test_old_guide_returning_200_is_blocked(self):
        self.guide(strings=(), extra="<p>Claude Code setup is always written.</p>")
        self.assertIn("Current installation guide", self.failures(self.audit("website")))

    def test_missing_contract_is_blocked_even_when_other_steps_and_status_pass(self):
        self.guide(pages={**gate.GUIDE_CONTRACT, "agents": gate.GUIDE_CONTRACT["agents"][:-1]})
        self.assertEqual(self.failures(self.audit("website")), ["Current agents guide"])

    def test_every_manual_guide_is_checked_for_its_own_commands(self):
        self.server.routes.pop("/site/docs/uninstall/")
        self.assertEqual(self.failures(self.audit("website")),
                         ["Uninstall guide", "Current uninstall guide"])

    def test_script_text_cannot_fake_visible_instructions(self):
        self.guide(strings=(), extra="<script>" + " ".join(gate.GUIDE_CONTRACT["installation"]) + "</script>")
        self.assertIn("Current installation guide", self.failures(self.audit("website")))

    def test_html_entities_and_spans_are_read_but_styles_are_ignored(self):
        self.guide(extra="<style>.old::after { content: 'Claude Code setup is always written'; }</style>")
        self.assertEqual(self.failures(self.audit("website")), [])

    def test_website_preparation_does_not_require_public_repo_or_registry_packages(self):
        self.server.routes = {key: value for key, value in self.server.routes.items()
                              if key.startswith("/site/")}
        self.guide(extra="<p>registry packages are not published yet</p>")
        self.assertTrue(self.audit("website")["passed"])
        self.assertTrue(all(path.startswith("/site/") for path in self.server.requests))

    def test_publication_readiness_allows_unpublished_packages_but_checks_support(self):
        self.guide(extra="<p>registry packages are not published yet</p>")
        self.server.routes.pop("/npm/magistr/latest")
        self.assertTrue(self.audit("publication")["passed"])
        self.assertFalse(any(path.startswith(("/npm/", "/cargo/")) for path in self.server.requests))

    def test_public_release_blocks_unpublished_copy(self):
        self.guide(extra="<p>registry packages are not published yet</p>")
        self.assertEqual(self.failures(self.audit()), ["Published installation instructions"])

    def test_public_release_blocks_wrong_registry_version(self):
        self.set_route("/npm/magistr/latest", '{"name":"magistr","version":"0.0.1"}')
        self.assertEqual(self.failures(self.audit()), ["Published npm version"])

    def test_private_repo_routes_are_not_hidden_by_passing_website(self):
        self.set_route("/github/owner/repo", "Missing", status=404)
        self.assertIn("Repository", self.failures(self.audit("publication")))

    def test_disabled_reporting_blocks_even_when_form_returns_a_login(self):
        self.set_route("/api/repos/owner/repo/private-vulnerability-reporting", '{"enabled":false}')
        self.assertEqual(self.failures(self.audit("publication")),
                         ["Private vulnerability reporting", "Private report form"])

    def test_login_redirect_must_return_to_the_private_report_form(self):
        self.set_route("/github/owner/repo/security/advisories/new", "", status=302,
                       headers={"Location": self.github + "/login?return_to=%2F"})
        self.assertEqual(self.failures(self.audit("publication")), ["Private report form"])

    def test_redirect_to_an_unrelated_200_page_is_not_a_working_route(self):
        self.set_route("/github/owner/repo/issues", "", status=302,
                       headers={"Location": self.site + "/"})
        self.assertEqual(self.failures(self.audit("publication")), ["Issues"])

    def test_pre_release_is_verified_on_the_beta_tag_not_latest(self):
        self.set_route("/npm/magistr/latest", '{"name":"magistr","version":"1.0.0-beta.1"}')
        self.set_route("/cargo/api/v1/crates/magistr-cli/1.0.0-beta.1",
                       '{"version":{"crate":"magistr-cli","num":"1.0.0-beta.1","yanked":false}}')
        self.assertEqual(self.failures(self.audit(version="1.0.0-beta.1")), ["Published npm version"])
        self.assertNotIn("/npm/magistr/latest", self.server.requests)
        self.set_route("/npm/magistr/beta", '{"name":"magistr","version":"1.0.0-beta.1"}')
        receipt = self.audit(version="1.0.0-beta.1")
        self.assertEqual(self.failures(receipt), [])
        self.assertEqual(receipt["npmTag"], "beta")
        self.assertEqual(self.audit()["npmTag"], "latest")

    def test_yanked_cargo_version_does_not_pass_release(self):
        self.set_route("/cargo/api/v1/crates/magistr-cli/0.1.0",
                       '{"version":{"crate":"magistr-cli","num":"0.1.0","yanked":true}}')
        self.assertEqual(self.failures(self.audit()), ["Published Cargo version"])


if __name__ == "__main__":
    unittest.main()
