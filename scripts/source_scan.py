"""Read each Rust source once for the architecture gates."""
from functools import lru_cache
import os
from pathlib import Path
import re


class SourceTree:
    def __init__(self, root):
        self.root = Path(root).resolve()

    def read(self, relative):
        return self._read(Path(relative).as_posix())

    @lru_cache(maxsize=None)
    def _read(self, relative):
        path = self.root / relative
        return path.read_text(encoding="utf-8") if path.is_file() else ""

    def rust(self):
        sources = {}
        for directory, children, files in os.walk(self.root / "crates"):
            children[:] = sorted(name for name in children if name != "target")
            for name in sorted(files):
                if name.endswith(".rs"):
                    relative = (Path(directory) / name).relative_to(self.root).as_posix()
                    sources[relative] = self.read(relative)
        return sources

    def allowlist(self, name):
        return set(self.read(f".magistr/{name}").splitlines())


@lru_cache(maxsize=None)
def pattern(expression):
    return re.compile(expression, re.MULTILINE)


def hits(text, expression, comments=True):
    compiled = pattern(expression)
    return [f"{number}:{line}" for number, line in enumerate(text.splitlines(), 1)
            if (comments or not line.lstrip().startswith("//")) and compiled.search(line)]
