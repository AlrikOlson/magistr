#!/usr/bin/env python3
"""Check deployed installation behavior and public launch routes without authentication."""
import argparse
from datetime import datetime, timezone
from html.parser import HTMLParser
from http.client import HTTPException
import json
from pathlib import Path
import re
import urllib.error
import urllib.parse
import urllib.request

from version import npm_dist_tag, product_version

REPOSITORY = "AlrikOlson/magistr"
SITE = "https://magistr.dev"
# The deployed manual splits the launch instructions across four guides. Each
# guide must still show its current commands and the planning boundary; an old
# deployment or an unrelated redirect fails even when it answers 200.
GUIDE_CONTRACT = {
    "installation": (
        "magistr --version",
        "Apple Silicon macOS 15 or later",
    ),
    "agents": (
        "magistr init . --agent codex --yes",
        "magistr init . --agent claude --yes",
        "magistr init . --agent portable --yes",
        "magistr doctor .",
        "It does not start implementation.",
    ),
    "upgrades": ("magistr repair . --dry-run",),
    "uninstall": ("magistr remove . --dry-run",),
}
OBSOLETE_GUIDE = ("Claude Code setup is always written", "think-and-ship", "workbench-mcp")


class VisibleText(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.parts, self.hidden = [], 0

    def handle_starttag(self, tag, attrs):
        if tag in ("script", "style"):
            self.hidden += 1
        elif tag in ("p", "div", "li", "pre", "br", "h1", "h2", "h3", "h4"):
            self.parts.append(" ")

    def handle_endtag(self, tag):
        if tag in ("script", "style"):
            self.hidden = max(0, self.hidden - 1)
        elif tag in ("p", "div", "li", "pre", "h1", "h2", "h3", "h4"):
            self.parts.append(" ")

    def handle_data(self, data):
        if not self.hidden:
            self.parts.append(data)

    def text(self):
        return " ".join("".join(self.parts).split())


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def fetch(url, *, follow=True):
    request = urllib.request.Request(url, headers={
        "User-Agent": "magistr-public-readiness (https://github.com/AlrikOlson/magistr)",
        "Accept": "text/html,application/json",
    })
    opener = urllib.request.build_opener() if follow else urllib.request.build_opener(NoRedirect())
    try:
        with opener.open(request, timeout=20) as response:
            return {"status": response.status, "url": response.url,
                    "location": response.headers.get("Location"),
                    "body": response.read(4 * 1024 * 1024).decode("utf-8", errors="replace")}
    except urllib.error.HTTPError as error:
        with error:
            return {"status": error.code, "url": url, "location": error.headers.get("Location"), "body": ""}
    except (OSError, TimeoutError, HTTPException):
        return {"status": 0, "url": url, "location": None, "body": ""}


def same_route(actual, expected):
    left, right = urllib.parse.urlsplit(actual), urllib.parse.urlsplit(expected)
    return (left.scheme, left.netloc, left.path.rstrip("/")) == (
        right.scheme, right.netloc, right.path.rstrip("/"))


def audit(stage, version, *, site=SITE, repository=REPOSITORY, github="https://github.com",
          api="https://api.github.com", npm="https://registry.npmjs.org", request=fetch):
    if stage not in ("website", "publication", "release"):
        raise ValueError("Unknown public readiness stage")
    if not re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", repository):
        raise ValueError("Invalid GitHub repository")
    # A pre-release is published on the `beta` dist-tag and must be verified
    # there; `latest` serving the same version would be a mis-tagged upload.
    npm_tag = npm_dist_tag(version)
    checks = []

    def record(name, url, response, passed, detail):
        checks.append({"name": name, "url": url, "status": response["status"],
                       "passed": bool(passed), "detail": detail})

    def page(name, url):
        response = request(url)
        ok = response["status"] == 200 and same_route(response["url"], url)
        record(name, url, response, ok, "Reachable at the intended public route" if ok
               else "Not anonymously reachable at the intended route")
        return response

    page("Website", site.rstrip("/") + "/")
    guide_url = site.rstrip("/") + "/docs/installation/"
    rendered = ""
    for name, required in GUIDE_CONTRACT.items():
        url = site.rstrip("/") + f"/docs/{name}/"
        guide = page(f"{name.capitalize()} guide", url)
        parser = VisibleText()
        parser.feed(guide["body"])
        text = parser.text()
        if name == "installation":
            rendered = text
        missing = [item for item in required if item not in text]
        obsolete = [item for item in OBSOLETE_GUIDE if item.casefold() in text.casefold()]
        record(f"Current {name} guide", url, guide, not missing and not obsolete,
               json.dumps({"missing": missing, "obsolete": obsolete}) if missing or obsolete
               else "Current commands and planning boundary are deployed")

    if stage in ("publication", "release"):
        repo_url = github.rstrip("/") + "/" + repository
        for label, suffix in (("Repository", ""), ("Releases", "/releases"), ("Issues", "/issues"),
                              ("Security policy", "/security/policy")):
            page(label, repo_url + suffix)
        reporting_url = api.rstrip("/") + "/repos/" + repository + "/private-vulnerability-reporting"
        reporting = request(reporting_url)
        try:
            enabled = reporting["status"] == 200 and json.loads(reporting["body"]).get("enabled") is True
        except (ValueError, TypeError, AttributeError):
            enabled = False
        record("Private vulnerability reporting", reporting_url, reporting, enabled,
               "Private reporting is enabled" if enabled else "Enable private vulnerability reporting")

        form_url = repo_url + "/security/advisories/new"
        form = request(form_url, follow=False)
        direct = form["status"] == 200 and same_route(form["url"], form_url)
        login = urllib.parse.urlsplit(urllib.parse.urljoin(form_url, form.get("location") or ""))
        destination = urllib.parse.parse_qs(login.query).get("return_to", [""])[0]
        destination = urllib.parse.urljoin(github.rstrip("/") + "/", destination)
        login_ok = (form["status"] in (301, 302, 303, 307, 308)
                    and same_route(urllib.parse.urlunsplit(login._replace(query="", fragment="")),
                                   github.rstrip("/") + "/login")
                    and same_route(destination, form_url))
        record("Private report form", form_url, form, enabled and (direct or login_ok),
               "Report form or sign-in returning to that form is reachable" if enabled and (direct or login_ok)
               else "Private report form does not reach the intended report route")

    if stage == "release":
        unpublished = "registry packages are not published yet" in rendered.casefold()
        record("Published installation instructions", guide_url, guide, not unpublished,
               "Installation page still says packages are unpublished" if unpublished
               else "Installation instructions no longer claim packages are unpublished")
        # npm is the only registry: the CLI is not published to crates.io.
        for label, url, identity in (
            ("Published npm version", npm.rstrip("/") + f"/magistr/{npm_tag}",
             lambda value: value.get("name") == "magistr" and value.get("version") == version),
        ):
            response = request(url)
            try:
                matches = response["status"] == 200 and identity(json.loads(response["body"]))
            except (ValueError, TypeError, AttributeError):
                matches = False
            record(label, url, response, matches,
                   f"Registry serves {version}" if matches else f"Registry does not serve the expected {version}")

    return {"schema": 1, "stage": stage, "version": version, "npmTag": npm_tag,
            "checkedAt": datetime.now(timezone.utc).isoformat(),
            "passed": all(check["passed"] for check in checks), "checks": checks}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--stage", choices=("website", "publication", "release"), default="release",
                        help="website permits private/unpublished preparation; publication adds public support; release checks registry versions")
    parser.add_argument("--site", default=SITE, help="Deployed site or local preview URL")
    parser.add_argument("--out", type=Path, help="Save the actual endpoint and content check receipt")
    args = parser.parse_args()
    receipt = audit(args.stage, product_version(), site=args.site)
    for check in receipt["checks"]:
        status = "PASS" if check["passed"] else "FAIL"
        print(f"{status} {check['name']} ({check['status']}): {check['detail']}")
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(receipt, indent=2) + "\n")
    return 0 if receipt["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
