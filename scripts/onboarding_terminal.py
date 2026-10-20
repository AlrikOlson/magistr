"""Drive the actual setup interview through a background pseudo-terminal."""
import errno
import os
import pty
import select
import subprocess
import time


class Terminal:
    def __init__(self, command, cwd, env):
        self.master, slave = pty.openpty()
        try:
            self.process = subprocess.Popen(command, cwd=cwd, env=env,
                stdin=slave, stdout=slave, stderr=slave, start_new_session=True)
        finally:
            os.close(slave)
        self.output = b""
        self.cursor = 0
        self.deadline = time.monotonic() + 60

    def read(self):
        remaining = self.deadline - time.monotonic()
        if remaining <= 0:
            raise AssertionError("Setup interview timed out:\n" + self.output.decode(errors="replace"))
        if select.select([self.master], [], [], min(remaining, 0.25))[0]:
            try:
                data = os.read(self.master, 65536)
            except OSError as error:
                if error.errno != errno.EIO:
                    raise
                data = b""
            self.output += data
            if not data:
                return False
        return True

    def expect(self, *prompts):
        while True:
            for prompt in prompts:
                position = self.output.find(prompt.encode(), self.cursor)
                if position >= 0:
                    self.cursor = position + len(prompt.encode())
                    return prompt
            if not self.read():
                raise AssertionError("Interview ended before " + repr(prompts)
                    + ":\n" + self.output.decode(errors="replace"))

    def answer(self, value=""):
        os.write(self.master, (value + "\n").encode())

    def finish(self):
        while self.read():
            pass
        status = self.process.wait(timeout=5)
        text = self.output.decode(errors="replace")
        if status != 0:
            raise AssertionError(f"Setup exited {status}:\n{text}")
        return text

    def close(self):
        # Only the disposable process created above is stopped on failure.
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
        os.close(self.master)


def interview(binary, root, env, approve, on_preview):
    terminal = Terminal([str(binary), "init"], root, dict(env, NO_COLOR="1", TERM="dumb"))
    try:
        terminal.expect("Use this detected stack?")
        terminal.answer()
        terminal.expect("[1-6, default=1]:")
        terminal.answer("4")
        terminal.expect("Description (one line):")
        terminal.answer("Interactive setup acceptance")
        terminal.expect("Enter for defaults]:")
        terminal.answer()
        # Some detected stacks offer optional external servers.
        if terminal.expect("Enter for defaults]:", "[Y/n]:") == "Enter for defaults]:":
            terminal.answer()
            terminal.expect("[Y/n]:")
        terminal.answer()
        terminal.expect("Customize gate commands?")
        terminal.answer("y")
        for name, command in (("check", "cargo check --locked"),
                              ("test", "cargo test --lib"),
                              ("lint", "cargo clippy -- -D warnings")):
            terminal.expect(name + ":")
            terminal.answer(command)
        terminal.expect("Apply these changes?")
        on_preview(terminal.output.decode(errors="replace"))
        terminal.answer("y" if approve else "n")
        return terminal.finish()
    finally:
        terminal.close()
