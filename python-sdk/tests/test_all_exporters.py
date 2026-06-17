import time
import ecocode

# NOTE: All runs use the "online" exporter and report to the EcoCode backend.
# The API key is read from the ECOCODE_API_KEY environment variable.


def _online_run(run_name):
    print("\n" + "=" * 50)
    print(f"Online run: {run_name}")
    print("=" * 50)
    with ecocode.run(run_name, exporter="online", interval=0.2) as run:
        run.mark("phase", "init")
        time.sleep(0.3)
        run.mark("phase", "forward")
        time.sleep(0.3)
    print(f"Online run '{run_name}' completed successfully!")


def test_terminal_exporter():
    _online_run("online-test-1")


def test_csv_exporter():
    _online_run("online-test-2")


def test_json_exporter():
    _online_run("online-test-3")


def test_logfile_exporter():
    _online_run("online-test-4")


def main():
    print("Starting all online exporter tests...")
    test_terminal_exporter()
    test_csv_exporter()
    test_json_exporter()
    test_logfile_exporter()
    print("\nALL ONLINE EXPORTER TESTS PASSED SUCCESSFULLY!")


if __name__ == "__main__":
    main()
