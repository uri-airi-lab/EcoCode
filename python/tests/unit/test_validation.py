import os

import pytest

from ecocode._validation import (
    validate_api_key,
    validate_marker_fd,
    validate_marker_handle,
    validate_metric_key,
    validate_run_id,
)


def test_api_key_format():
    assert validate_api_key("ecocode_abcdefgh12") == "ecocode_abcdefgh12"
    assert validate_api_key("eco_abcdefgh") == "eco_abcdefgh"
    for bad in ["", "abc_12345678", "ecocode_short", "ecocode_abc\ndefghij"]:
        with pytest.raises(ValueError):
            validate_api_key(bad)


def test_run_id_range():
    assert validate_run_id("42") == 42
    for bad in ["0", "-1", "x", str(2**53 + 1)]:
        with pytest.raises(ValueError):
            validate_run_id(bad)


@pytest.mark.skipif(os.name == "nt", reason="file descriptors are POSIX")
def test_marker_fd_must_be_open_and_not_stdio():
    r, w = os.pipe()
    try:
        assert validate_marker_fd(str(w)) == w
    finally:
        os.close(r)
        os.close(w)
    for bad in ["1", "2", "abc", str(w)]:  # w is closed now
        with pytest.raises(ValueError):
            validate_marker_fd(bad)


def test_marker_handle_value():
    assert validate_marker_handle("1234") == 1234
    for bad in ["0", "-5", "0x10", "abc", str(2**64)]:
        with pytest.raises(ValueError):
            validate_marker_handle(bad)


def test_metric_key():
    assert validate_metric_key("grad_norm-1") == "grad_norm-1"
    for bad in ["", "has space", "x" * 129]:
        with pytest.raises(ValueError):
            validate_metric_key(bad)
