"""App-home operations anchored to no-follow directory descriptors."""
import hashlib
import os
import secrets
import stat
from contextlib import contextmanager
from pathlib import PurePosixPath
from publish import rename_new


def digest(data):
    return hashlib.sha256(data).hexdigest()


def parts(path):
    p = PurePosixPath(path)
    if not path or p.is_absolute() or str(p) != path or any(x in ('.', '..') for x in p.parts):
        raise ValueError(f'Unsafe relative path: {path}')
    return p.parts


class Home:
    def __init__(self, path):
        if not os.path.isabs(path):
            raise ValueError('--app-home must be absolute and already exist')
        fd = os.open('/', os.O_RDONLY | os.O_DIRECTORY)
        try:
            for component in parts(path.removeprefix('/')):
                next_fd = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
                os.close(fd)
                fd = next_fd
            self.fd = fd
        except BaseException:
            os.close(fd)
            raise

    @contextmanager
    def parent(self, path):
        components = parts(path)
        fd = os.dup(self.fd)
        try:
            for component in components[:-1]:
                next_fd = os.open(component, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
                os.close(fd)
                fd = next_fd
            yield fd, components[-1]
        finally:
            os.close(fd)

    def read(self, path, limit=None):
        try:
            with self.parent(path) as (fd, name):
                child = os.open(name, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK, dir_fd=fd)
                with os.fdopen(child, 'rb') as stream:
                    info = os.fstat(stream.fileno())
                    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
                        raise ValueError(f'Not a private regular file: {path}')
                    if limit is not None and info.st_size > limit:
                        raise ValueError(f'File exceeds read limit: {path}')
                    data = stream.read() if limit is None else stream.read(limit + 1)
                    if limit is not None and len(data) > limit:
                        raise ValueError(f'File exceeds read limit: {path}')
                    return data, stat.S_IMODE(info.st_mode)
        except FileNotFoundError:
            return None

    def exists(self, path):
        try:
            with self.parent(path) as (fd, name):
                os.stat(name, dir_fd=fd, follow_symlinks=False)
            return True
        except FileNotFoundError:
            return False

    def mkdir(self, path):
        with self.parent(path) as (fd, name):
            try:
                os.mkdir(name, 0o755, dir_fd=fd)
                return True
            except FileExistsError:
                check = os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
                os.close(check)
                return False

    def write(self, path, data, mode, expected=None, on_publish=None):
        if self.read(path) != expected:
            raise ValueError(f'File changed or destination occupied: {path}')
        with self.parent(path) as (fd, name):
            temp = '.panorama-' + secrets.token_hex(12)
            out = os.open(temp, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, mode, dir_fd=fd)
            try:
                with os.fdopen(out, 'wb') as stream:
                    stream.write(data)
                    stream.flush()
                    os.fchmod(stream.fileno(), mode)
                    os.fsync(stream.fileno())
                if self.read(path) != expected:
                    raise ValueError(f'File changed: {path}')
                if expected is None:
                    rename_new(fd, temp, name)
                else:
                    os.replace(temp, name, src_dir_fd=fd, dst_dir_fd=fd)
                # Publication precedes durability. Register ownership even if the
                # following fsync or temporary-file cleanup fails.
                if on_publish is not None:
                    on_publish()
                os.fsync(fd)
            finally:
                try:
                    os.unlink(temp, dir_fd=fd)
                except FileNotFoundError:
                    pass

    def remove(self, path, expected):
        # A file can grow after preflight. Never allocate beyond the bytes
        # already validated merely to recheck them before deletion.
        if self.read(path, len(expected[0])) != expected:
            return False
        with self.parent(path) as (fd, name):
            os.unlink(name, dir_fd=fd)
        return True

    def rmdir(self, path):
        with self.parent(path) as (fd, name):
            os.rmdir(name, dir_fd=fd)

    def list(self, path):
        with self.parent(path + '/.probe') as (fd, _name):
            return os.listdir(fd)

    def replace_temp(self, path, temp, expected):
        if self.read(path) != expected:
            raise ValueError(f'File changed: {path}')
        if parts(path)[:-1] != parts(temp)[:-1]:
            raise ValueError('Runtime staging must be in the same directory')
        with self.parent(path) as (fd, name):
            os.replace(parts(temp)[-1], name, src_dir_fd=fd, dst_dir_fd=fd)
            os.fsync(fd)
