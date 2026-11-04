"""Check actual worker Dock policy and EOF cleanup without invoking providers."""

import concurrent.futures
import json
from pathlib import Path
import subprocess
import sys
import tempfile


def line(process):
    with concurrent.futures.ThreadPoolExecutor() as executor:
        pending = executor.submit(process.stdout.readline)
        try:
            return pending.result(timeout=30)
        except concurrent.futures.TimeoutError:
            process.kill()
            raise AssertionError("Process did not become ready") from None


def launch(command):
    return subprocess.Popen(
        command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, text=True,
    )


def policy(probe, process):
    return int(subprocess.check_output([str(probe), str(process.pid)], text=True, timeout=15))


def stop(process):
    process.stdin.close()
    try:
        assert process.wait(timeout=20) == 0, "Process did not exit cleanly on EOF"
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


providers = [
    "claudecode", "claudedesktop", "dola", "doubaowork",
    "trae", "traecn", "workbuddy", "workbuddyai",
]
with tempfile.TemporaryDirectory(prefix="echobird-worker-policy-") as directory:
    probe = Path(directory) / "policy-probe"
    subprocess.run(
        ["swiftc", str(Path(__file__).with_suffix(".swift")), "-o", str(probe)],
        check=True, timeout=120,
    )
    control = launch([str(probe), "control"])
    try:
        assert int(line(control)) == control.pid
        assert policy(probe, control) == 0, "Probe must recognize a regular Dock application"
    finally:
        stop(control)
    # Two passes detect changes after earlier workers exit; hold all eight alive
    # together to reproduce visiting every tool before returning to the first.
    for _ in range(2):
        workers = []
        try:
            for provider in providers:
                worker = launch([str(Path(sys.argv[1]).resolve())])
                workers.append(worker)
                worker.stdin.write(json.dumps({
                    "provider": provider, "secret": "fixture-worker-secret-0123456789abcdef", "tools": {},
                }) + "\n")
                worker.stdin.flush()
                ready = json.loads(line(worker))
                assert ready["protocol"] == 1 and 0 < ready["port"] < 65536
            for worker in workers:
                assert worker.poll() is None, "Worker exited before policy verification"
                assert policy(probe, worker) == 1, "Worker must use Accessory policy, without a Dock icon"
        finally:
            for worker in workers:
                stop(worker)
print("All eight macOS workers use Accessory policy and exit on EOF (two passes); regular-app control passed")
