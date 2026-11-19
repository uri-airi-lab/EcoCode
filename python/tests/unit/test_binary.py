import os
import stat
import sysconfig

import pytest

from ecocode import _binary


def _fake_binary(directory):
    path = directory / _binary.EXE_NAME
    path.write_text("#!/bin/sh\n")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)
    return str(path)


def test_bundled_binary_in_scripts_dir_wins(tmp_path, monkeypatch):
    scripts = tmp_path / "scripts"
    scripts.mkdir()
    bundled = _fake_binary(scripts)
    on_path = tmp_path / "path"
    on_path.mkdir()
    _fake_binary(on_path)
    monkeypatch.setattr(sysconfig, "get_path", lambda name, *a: str(scripts))
    monkeypatch.setenv("PATH", str(on_path))
    assert _binary.find_ecocode_binary() == bundled


def test_falls_back_to_path(tmp_path, monkeypatch):
    monkeypatch.setattr(sysconfig, "get_path", lambda name, *a: str(tmp_path / "empty"))
    on_path = tmp_path / "path"
    on_path.mkdir()
    found = _fake_binary(on_path)
    monkeypatch.setenv("PATH", str(on_path))
    assert os.path.samefile(_binary.find_ecocode_binary(), found)


def test_missing_binary_explains_how_to_install(tmp_path, monkeypatch):
    monkeypatch.setattr(sysconfig, "get_path", lambda name, *a: str(tmp_path))
    monkeypatch.setenv("PATH", str(tmp_path))
    with pytest.raises(RuntimeError, match="pip install ecocodes"):
        _binary.resolve_ecocode_binary()


def test_unexpected_location_warns(tmp_path, monkeypatch):
    monkeypatch.setattr(sysconfig, "get_path", lambda name, *a: str(tmp_path / "empty"))
    monkeypatch.setattr(_binary, "_trusted_prefixes", lambda: [os.path.normcase(str(tmp_path / "trusted"))])
    elsewhere = tmp_path / "elsewhere"
    elsewhere.mkdir()
    _fake_binary(elsewhere)
    monkeypatch.setenv("PATH", str(elsewhere))
    with pytest.warns(UserWarning, match="unexpected path"):
        _binary.resolve_ecocode_binary()
