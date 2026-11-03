"""Atomic create-only rename; never introduce a recoverability-breaking hardlink."""
import ctypes
import os
import sys


def rename_new(fd, source, destination):
    # Both primitives preserve an existing destination atomically. Do not fall
    # back to replace(), or link()+unlink() with a process-interruption window.
    # Linux: include/uapi/linux/fs.h RENAME_NOREPLACE = 1.
    # macOS: apple-oss-distributions/xnu bsd/sys/stdio.h RENAME_EXCL = 4.
    libc = ctypes.CDLL(None, use_errno=True)
    if sys.platform == 'linux':
        name, flag = 'renameat2', 1
    elif sys.platform == 'darwin':
        name, flag = 'renameatx_np', 4
    else:
        raise ValueError('Atomic create-only installation requires Linux or macOS')
    function = getattr(libc, name, None)
    if function is None:
        raise ValueError(f'Atomic create-only rename is unavailable: {name}')
    function.argtypes = [ctypes.c_int, ctypes.c_char_p, ctypes.c_int, ctypes.c_char_p, ctypes.c_uint]
    function.restype = ctypes.c_int
    if function(fd, os.fsencode(source), fd, os.fsencode(destination), flag) != 0:
        error = ctypes.get_errno()
        raise OSError(error, os.strerror(error), destination)
