"""The per-tab panels' core: publishes panorama's state to the panel viewers on a local socket
(docs/DESIGN.md "Per-tab panels", protocol.py). One core per herdr session.

Hardening (the design review of #29, M1 and M7):
- one core: an exclusive flock (``core-<session>.lock``) held for the core's whole life;
- a socket file left by a dead core is removed only after a connect to it failed, lock held;
- the socket is 0600 in a directory that must be a real 0700 directory of ours (not a
  symlink, not another user's, not group/world-accessible), and a peer must have our uid;
- a connection must say hello within HELLO_SECONDS, at most MAX_VIEWERS are open at once, and
  a send that does not finish in SEND_SECONDS drops the viewer: no connection holds a thread
  forever;
- lines over protocol.MAX_LINE close the connection;
- each viewer has a small bounded outbox: a slow viewer loses older ``state`` messages, it
  never stalls the core;
- a viewer's ``hello.pane_id`` is only a claim: ``verify_pane(pane_id, pid)`` decides whether
  it is trusted (the home panorama asks herdr whether that pane's foreground runs the peer's
  own pid as ``panorama panel``), and it is never authority to close anything.
"""

from __future__ import annotations

import errno
import fcntl
import logging
import os
import queue
import socket
import stat
import threading
import time
from pathlib import Path

from . import protocol

log = logging.getLogger("herdr_panorama.core")

OUTBOX = 4  # queued messages per viewer; when full the oldest is dropped
PING_SECONDS = 5.0
HELLO_SECONDS = 5.0  # a connection that has not said hello by then is closed
SEND_SECONDS = 10.0  # a viewer that does not take a message in this long is dropped
MAX_VIEWERS = 64  # open connections at once (a viewer per tab); more are refused


def private_dir(d: Path, uid: int) -> str | None:
    """Why ``d`` is not a safe home for the socket, or None: it must be a real directory (not a
    symlink), owned by ``uid``, with no group or world access, in a parent that no other user
    can rename it out of: one owned by ``uid`` or root, either not writable by others or
    sticky (``/tmp``, the macOS fallback's parent)."""
    try:
        up = os.stat(Path(d).parent)
    except OSError as exc:
        return f"its parent: {exc}"
    if up.st_uid not in (uid, 0):
        return f"in a directory of uid {up.st_uid}"
    if up.st_mode & 0o022 and not up.st_mode & stat.S_ISVTX:
        return f"in a directory others can write in (mode {stat.S_IMODE(up.st_mode):o}, not sticky)"
    try:
        st = os.lstat(d)
    except OSError as exc:
        return str(exc)
    if stat.S_ISLNK(st.st_mode):
        return "a symlink"
    if not stat.S_ISDIR(st.st_mode):
        return "not a directory"
    if st.st_uid != uid:
        return f"owned by uid {st.st_uid}"
    if st.st_mode & 0o077:
        return f"mode {stat.S_IMODE(st.st_mode):o}, not 700"
    return None


class Viewer:
    """One connected panel."""

    def __init__(self, conn: socket.socket):
        self.conn = conn
        self.outbox: queue.Queue = queue.Queue(maxsize=OUTBOX)
        self.pid, _ = protocol.peer_cred(conn)
        self.pane_id: str | None = None  # verified hello.pane_id, else None
        self.tab_id: str | None = None
        self.hello = False
        self.role = "panel"  # "key": the open key's one request, no state
        self.closed = threading.Event()

    def send(self, line: bytes) -> None:
        """Queue without blocking; a full outbox loses its oldest message (a stale state)."""
        while True:
            try:
                self.outbox.put_nowait(line)
                return
            except queue.Full:
                try:
                    self.outbox.get_nowait()
                except queue.Empty:
                    pass

    def close(self) -> None:
        if not self.closed.is_set():
            self.closed.set()
            try:
                self.conn.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            self.conn.close()


class Core:
    """``start()`` returns the running core, or None when another core holds the lock (or the
    socket cannot be made). ``publish(msg)`` sends a message to every viewer."""

    def __init__(self, sock_path: Path, lock_path: Path, on_seen=None, verify_pane=None,
                 uid: int | None = None, on_open=None):
        self.sock_path = Path(sock_path)
        self.lock_path = Path(lock_path)
        self.on_seen = on_seen or (lambda ids: None)
        self.verify_pane = verify_pane or (lambda pane_id, pid: False)
        self.on_open = on_open or (lambda req: "home")  # the open key (role "key"): what it did
        self.uid = os.getuid() if uid is None else uid
        self.viewers: list[Viewer] = []
        self._lock_fd: int | None = None
        self._server: socket.socket | None = None
        self._stop = threading.Event()
        self._mu = threading.Lock()
        self._refusing = False  # at MAX_VIEWERS, already logged
        self._last: bytes | None = None  # the last state, for viewers that connect later

    # -- lifecycle --------------------------------------------------------------------

    @classmethod
    def start(cls, sock_path: Path, lock_path: Path, **kw) -> Core | None:
        core = cls(sock_path, lock_path, **kw)
        return core if core._start() else None

    def _start(self) -> bool:
        d = self.sock_path.parent
        try:
            os.makedirs(d.parent, exist_ok=True)
            os.mkdir(d, 0o700)
        except FileExistsError:
            pass
        except OSError as exc:
            log.warning("no socket directory %s: %s", d, exc)
            return False
        why = private_dir(d, self.uid)
        if why is not None:
            log.warning("not starting: the socket directory %s is %s", d, why)
            return False
        try:
            fd = os.open(self.lock_path, os.O_RDWR | os.O_CREAT | getattr(os, "O_NOFOLLOW", 0), 0o600)
        except OSError as exc:
            log.warning("no lock file %s: %s", self.lock_path, exc)
            return False
        try:
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        except OSError:
            os.close(fd)
            return False  # another core is running
        self._lock_fd = fd
        if self.sock_path.exists():
            probe = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            try:
                probe.settimeout(0.5)
                probe.connect(str(self.sock_path))
                probe.close()
                log.warning("a core answers on %s without its lock: not starting", self.sock_path)
                self._release()
                return False
            except OSError:
                probe.close()
                try:
                    self.sock_path.unlink()  # a dead core's leftover, removed under our lock
                except FileNotFoundError:
                    pass
        server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        old = os.umask(0o177)
        try:
            server.bind(str(self.sock_path))
        except OSError as exc:
            log.warning("could not bind %s: %s", self.sock_path, exc)
            server.close()
            self._release()
            return False
        finally:
            os.umask(old)
        os.chmod(self.sock_path, 0o600)
        server.listen(16)
        self._server = server
        threading.Thread(target=self._accept, daemon=True, name="core-accept").start()
        threading.Thread(target=self._ping, daemon=True, name="core-ping").start()
        return True

    def _release(self) -> None:
        if self._lock_fd is not None:
            try:
                fcntl.flock(self._lock_fd, fcntl.LOCK_UN)
            finally:
                os.close(self._lock_fd)
            self._lock_fd = None

    def stop(self) -> None:
        self._stop.set()
        if self._server is not None:
            try:
                self._server.close()
            except OSError:
                pass
            self._server = None
        with self._mu:
            viewers, self.viewers = list(self.viewers), []
        for v in viewers:
            v.close()
        try:
            self.sock_path.unlink()
        except FileNotFoundError:
            pass
        self._release()

    # -- publishing -------------------------------------------------------------------

    def publish(self, msg: dict) -> None:
        line = protocol.encode(msg)
        if "state" in msg:
            self._last = line
        with self._mu:
            viewers = [v for v in self.viewers if v.hello and v.role == "panel" and not v.closed.is_set()]
        for v in viewers:
            v.send(line)

    def connected(self) -> list[Viewer]:
        with self._mu:
            return [v for v in self.viewers if v.hello and not v.closed.is_set()]

    def verified_panes(self) -> set[str]:
        """The panes whose connected viewers said a verified hello."""
        return {v.pane_id for v in self.connected() if v.pane_id}

    # -- connections ------------------------------------------------------------------

    def _accept(self) -> None:
        while not self._stop.is_set():
            server = self._server
            if server is None:
                return
            try:
                conn, _ = server.accept()
            except OSError as exc:
                if self._stop.is_set() or exc.errno in (errno.EBADF, errno.EINVAL):
                    return
                continue
            if protocol.peer_cred(conn)[1] != self.uid:
                log.warning("refused a connection from another user")
                conn.close()
                continue
            with self._mu:
                full = len(self.viewers) >= MAX_VIEWERS
                v = None if full else Viewer(conn)
                if v is not None:
                    self.viewers.append(v)
            if v is None:
                if not self._refusing:  # once per full spell, not once per connection
                    log.warning("refusing connections: %d open already", MAX_VIEWERS)
                self._refusing = True
                conn.close()
                continue
            self._refusing = False
            # the writer starts at hello: until then a connection holds one thread, for at
            # most HELLO_SECONDS
            threading.Thread(target=self._read, args=(v,), daemon=True, name="core-read").start()

    def _write(self, v: Viewer) -> None:
        while not v.closed.is_set():
            try:
                line = v.outbox.get(timeout=0.5)
            except queue.Empty:
                continue
            try:
                v.conn.sendall(line)
            except OSError:
                break
        self._drop(v)

    def _read(self, v: Viewer) -> None:
        reader = protocol.LineReader()
        deadline = time.monotonic() + HELLO_SECONDS
        try:
            while not v.closed.is_set():
                if not v.hello:
                    left = deadline - time.monotonic()
                    if left <= 0:
                        log.info("viewer dropped: no hello in %ss", HELLO_SECONDS)
                        return
                    v.conn.settimeout(left)
                try:
                    data = v.conn.recv(65536)
                except TimeoutError:
                    if v.hello:
                        continue  # a quiet viewer is fine; only the hello has a deadline
                    log.info("viewer dropped: no hello in %ss", HELLO_SECONDS)
                    return
                if not data:
                    break
                seen: list[str] = []
                for msg in reader.feed(data):
                    said_hello = v.hello
                    if not self._handle(v, msg, seen):
                        return
                    if v.hello and not said_hello:  # the writer's sends time out; recv retries
                        v.conn.settimeout(SEND_SECONDS)
                        threading.Thread(target=self._write, args=(v,), daemon=True, name="core-write").start()
                if seen:
                    self.on_seen(list(dict.fromkeys(seen)))  # one call per read
        except (OSError, protocol.TooLong) as exc:
            log.info("viewer dropped: %s", exc)
        finally:
            self._drop(v)

    def _handle(self, v: Viewer, msg: dict, seen: list[str]) -> bool:
        """One message from a viewer; False closes the connection. ``seen`` ids are collected
        into ``seen`` (the caller reports a read's worth at once)."""
        if "hello" in msg:
            hello = msg["hello"] if isinstance(msg["hello"], dict) else {}
            if not protocol.compatible(hello.get("protocol")):
                try:  # said directly, before the close (the writer thread may not get to it)
                    v.conn.sendall(protocol.encode({"error": {"message": "protocol",
                                                              "protocol": protocol.PROTOCOL}}))
                except OSError:
                    pass
                return False
            pane = hello.get("pane_id")
            v.pane_id = pane if isinstance(pane, str) and v.pid and self.verify_pane(pane, v.pid) else None
            v.tab_id = hello.get("tab_id") if isinstance(hello.get("tab_id"), str) else None
            v.role = "key" if hello.get("role") == "key" else "panel"
            v.hello = True
            v.send(protocol.encode({"welcome": {"protocol": protocol.PROTOCOL}}))
            if self._last is not None and v.role == "panel":
                v.send(self._last)
            return True
        if not v.hello:
            return False  # anything before hello: not a panel of ours
        if "seen" in msg:
            ids = [i for i in msg["seen"] if isinstance(i, str)] if isinstance(msg["seen"], list) else []
            seen += ids
            return True
        if "open" in msg:  # the open key: answered on this connection, which then closes
            req = msg["open"] if isinstance(msg["open"], dict) else {}
            try:
                did = self.on_open(req)
            except Exception as exc:  # noqa: BLE001 - a broken request never takes the core down
                log.warning("open request failed: %s", exc)
                did = "failed"
            v.send(protocol.encode({"opened": {"result": did}}))
            return True
        if "bye" in msg:
            return False
        return True  # unknown: ignored (a newer minor)

    def _drop(self, v: Viewer) -> None:
        v.close()
        with self._mu:
            if v in self.viewers:
                self.viewers.remove(v)

    def _ping(self) -> None:
        n = 0
        while not self._stop.wait(PING_SECONDS):
            n += 1
            self.publish({"ping": n})
