"""The per-tab panels' core: publishes panorama's state to the panel viewers on a local socket
(docs/DESIGN.md "Per-tab panels", protocol.py). One core per herdr session.

Hardening (the design review of #29, M1 and M7):
- one core: an exclusive flock (``core-<session>.lock``) held for the core's whole life;
- a socket file left by a dead core is removed only after a connect to it failed, lock held;
- the socket is 0600 in a 0700 directory, and a peer must have our uid (SO_PEERCRED);
- lines over protocol.MAX_LINE close the connection;
- each viewer has a small bounded outbox: a slow viewer loses older ``state`` messages, it
  never stalls the core;
- a viewer's ``hello.pane_id`` is only a claim: ``verify_pane`` (herdr's snapshot) decides
  whether it is trusted, and it is never authority to close anything.
"""

from __future__ import annotations

import errno
import fcntl
import logging
import os
import queue
import socket
import struct
import threading
from pathlib import Path

from . import protocol

log = logging.getLogger("herdr_panorama.core")

OUTBOX = 4  # queued messages per viewer; when full the oldest is dropped
PING_SECONDS = 5.0


def _peer_uid(conn: socket.socket) -> int | None:
    """The connecting process's uid (Linux SO_PEERCRED; macOS LOCAL_PEERCRED), or None."""
    try:
        if hasattr(socket, "SO_PEERCRED"):
            creds = conn.getsockopt(socket.SOL_SOCKET, socket.SO_PEERCRED, struct.calcsize("3i"))
            return struct.unpack("3i", creds)[1]
        getpeereid = getattr(os, "getpeereid", None)  # not in CPython; kept for other runtimes
        if getpeereid:
            return getpeereid(conn.fileno())[0]
    except OSError:
        return None
    return None


class Viewer:
    """One connected panel."""

    def __init__(self, conn: socket.socket):
        self.conn = conn
        self.outbox: queue.Queue = queue.Queue(maxsize=OUTBOX)
        self.pane_id: str | None = None  # verified hello.pane_id, else None
        self.tab_id: str | None = None
        self.hello = False
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
                 uid: int | None = None):
        self.sock_path = Path(sock_path)
        self.lock_path = Path(lock_path)
        self.on_seen = on_seen or (lambda ids: None)
        self.verify_pane = verify_pane or (lambda pane_id: False)
        self.uid = os.getuid() if uid is None else uid
        self.viewers: list[Viewer] = []
        self._lock_fd: int | None = None
        self._server: socket.socket | None = None
        self._stop = threading.Event()
        self._mu = threading.Lock()
        self._last: bytes | None = None  # the last state, for viewers that connect later

    # -- lifecycle --------------------------------------------------------------------

    @classmethod
    def start(cls, sock_path: Path, lock_path: Path, **kw) -> Core | None:
        core = cls(sock_path, lock_path, **kw)
        return core if core._start() else None

    def _start(self) -> bool:
        d = self.sock_path.parent
        os.makedirs(d, mode=0o700, exist_ok=True)
        try:
            os.chmod(d, 0o700)
        except OSError:
            pass
        fd = os.open(self.lock_path, os.O_RDWR | os.O_CREAT, 0o600)
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
            viewers = [v for v in self.viewers if v.hello and not v.closed.is_set()]
        for v in viewers:
            v.send(line)

    def connected(self) -> list[Viewer]:
        with self._mu:
            return [v for v in self.viewers if v.hello and not v.closed.is_set()]

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
            if _peer_uid(conn) != self.uid:
                log.warning("refused a connection from another user")
                conn.close()
                continue
            v = Viewer(conn)
            with self._mu:
                self.viewers.append(v)
            threading.Thread(target=self._read, args=(v,), daemon=True, name="core-read").start()
            threading.Thread(target=self._write, args=(v,), daemon=True, name="core-write").start()

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
        try:
            while not v.closed.is_set():
                data = v.conn.recv(65536)
                if not data:
                    break
                for msg in reader.feed(data):
                    if not self._handle(v, msg):
                        return
        except (OSError, protocol.TooLong) as exc:
            log.info("viewer dropped: %s", exc)
        finally:
            self._drop(v)

    def _handle(self, v: Viewer, msg: dict) -> bool:
        """One message from a viewer; False closes the connection."""
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
            v.pane_id = pane if isinstance(pane, str) and self.verify_pane(pane) else None
            v.tab_id = hello.get("tab_id") if isinstance(hello.get("tab_id"), str) else None
            v.hello = True
            v.send(protocol.encode({"welcome": {"protocol": protocol.PROTOCOL}}))
            if self._last is not None:
                v.send(self._last)
            return True
        if not v.hello:
            return False  # anything before hello: not a panel of ours
        if "seen" in msg:
            ids = [i for i in msg["seen"] if isinstance(i, str)] if isinstance(msg["seen"], list) else []
            if ids:
                self.on_seen(ids)
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
