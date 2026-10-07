import io
import json
import os

import pytest

from ecocode._pipe import (
    MARKER_FD_ENV,
    MARKER_HANDLE_ENV,
    NullWriter,
    PipeWriter,
    marker_pipe_in,
    open_inherited_writer,
)


def test_pipe_writer_sends_json_lines_with_timestamp():
    stream = io.BytesIO()
    w = PipeWriter(stream)
    w.send({"event": "phase", "value": "forward"})
    w.send({"event": "step", "value": 3})
    lines = stream.getvalue().decode().splitlines()
    first = json.loads(lines[0])
    assert first["event"] == "phase" and first["value"] == "forward"
    assert isinstance(first["sent_at_ns"], int)
    assert json.loads(lines[1])["value"] == 3


def test_writing_to_a_closed_pipe_is_silent():
    stream = io.BytesIO()
    w = PipeWriter(stream)
    w.close()
    w.send({"event": "step", "value": 1})  # must not raise
    NullWriter().send({"event": "x"})


def test_marker_pipe_detection_follows_the_os():
    key = MARKER_HANDLE_ENV if os.name == "nt" else MARKER_FD_ENV
    other = MARKER_FD_ENV if os.name == "nt" else MARKER_HANDLE_ENV
    assert marker_pipe_in({key: "7"})
    assert not marker_pipe_in({other: "7"})
    assert not marker_pipe_in({})
    assert open_inherited_writer({}) is None


def _read_all(fd):
    with os.fdopen(fd, "rb") as f:
        return f.read().decode()


@pytest.mark.skipif(os.name == "nt", reason="file descriptors are POSIX")
def test_inherited_fd_writer_round_trip():
    r, w = os.pipe()
    writer = open_inherited_writer({MARKER_FD_ENV: str(w)})
    writer.send({"event": "loss", "value": 0.5})
    writer.close()
    assert json.loads(_read_all(r))["value"] == 0.5


@pytest.mark.skipif(os.name != "nt", reason="handles are Windows")
def test_inherited_handle_writer_round_trip():
    import msvcrt

    r, w = os.pipe()
    handle = msvcrt.get_osfhandle(w)
    writer = open_inherited_writer({MARKER_HANDLE_ENV: str(handle)})
    writer.send({"event": "loss", "value": 0.5})
    writer.close()  # closes the handle, i.e. the pipe's write end
    assert json.loads(_read_all(r))["value"] == 0.5


def test_invalid_inherited_pipe_is_rejected():
    key = MARKER_HANDLE_ENV if os.name == "nt" else MARKER_FD_ENV
    with pytest.raises(ValueError):
        open_inherited_writer({key: "not-a-number"})
