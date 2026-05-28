import os
import json
import csv
import time
import ecocode

def test_terminal_exporter():
    print("\n" + "="*50)
    print("Testing Exporter: terminal")
    print("="*50)
    # The terminal exporter writes to stdout. It requires no output_file or db_path.
    with ecocode.run("terminal-test", exporter="terminal", interval=0.2) as run:
        run.mark("phase", "init")
        time.sleep(0.3)
        run.mark("phase", "forward")
        time.sleep(0.3)
    print("Terminal exporter test completed successfully!")

def test_csv_exporter():
    print("\n" + "="*50)
    print("Testing Exporter: csv")
    print("="*50)
    csv_file = "test_run.csv"
    if os.path.exists(csv_file):
        os.remove(csv_file)

    with ecocode.run("csv-test", exporter="csv", output_file=csv_file, interval=0.2) as run:
        run.mark("phase", "init")
        time.sleep(0.3)
        run.mark("phase", "forward")
        time.sleep(0.3)

    if not os.path.exists(csv_file):
        raise AssertionError("CSV file was not created!")
    
    # Read the CSV to verify it contains records
    with open(csv_file, mode='r') as f:
        reader = csv.reader(f)
        rows = list(reader)
        print(f"CSV Header: {rows[0] if rows else 'None'}")
        print(f"CSV Total Rows: {len(rows)}")
        if len(rows) < 2:
            raise AssertionError("CSV file does not contain enough records!")

    os.remove(csv_file)
    print("CSV exporter test passed!")

def test_json_exporter():
    print("\n" + "="*50)
    print("Testing Exporter: json")
    print("="*50)
    json_file = "test_run.json"
    if os.path.exists(json_file):
        os.remove(json_file)

    with ecocode.run("json-test", exporter="json", output_file=json_file, interval=0.2) as run:
        run.mark("phase", "init")
        time.sleep(0.3)
        run.mark("phase", "forward")
        time.sleep(0.3)

    if not os.path.exists(json_file):
        raise AssertionError("JSON file was not created!")
    
    with open(json_file, mode='r') as f:
        data = json.load(f)
        print(f"JSON Total Records: {len(data)}")
        if not isinstance(data, list) or len(data) == 0:
            raise AssertionError("JSON file does not contain valid records array!")
        print(f"First JSON Record Sample: {data[0]}")

    os.remove(json_file)
    print("JSON exporter test passed!")


def test_logfile_exporter():
    print("\n" + "="*50)
    print("Testing Exporter: logfile")
    print("="*50)
    log_file = "test_run.log"
    if os.path.exists(log_file):
        os.remove(log_file)

    with ecocode.run("logfile-test", exporter="logfile", output_file=log_file, interval=0.2) as run:
        run.mark("phase", "init")
        time.sleep(0.3)
        run.mark("phase", "forward")
        time.sleep(0.3)

    if not os.path.exists(log_file):
        raise AssertionError("Log file was not created!")
    
    with open(log_file, mode='r') as f:
        lines = f.readlines()
        print(f"Logfile Total Lines: {len(lines)}")
        if len(lines) < 2:
            raise AssertionError("Logfile does not contain enough records!")
        print(f"Logfile sample line: {lines[0].strip()}")

    os.remove(log_file)
    print("Logfile exporter test passed!")

def main():
    print("Starting all local exporter tests...")
    test_terminal_exporter()
    test_csv_exporter()
    test_json_exporter()
    test_logfile_exporter()
    print("\nALL LOCAL EXPORTER TESTS PASSED SUCCESSFULLY!")

if __name__ == "__main__":
    main()
