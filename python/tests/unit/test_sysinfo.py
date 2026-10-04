from ecocode._sysinfo import collect_system_info


def test_reports_this_machine():
    info = collect_system_info()
    assert set(info) == {"cpu_name", "cpu_cores", "ram_gb", "gpu_count", "gpu_names"}
    assert info["cpu_cores"] > 0
    assert info["ram_gb"] > 0, "RAM must be detected on Linux, macOS and Windows"
    assert info["cpu_name"]
    assert info["gpu_count"] == len([g for g in info["gpu_names"].split(", ") if g])
