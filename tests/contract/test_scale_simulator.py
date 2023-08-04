import os
import signal
import subprocess
import time
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class ScaleSimulatorTest(unittest.TestCase):
    def test_rust_probe_accepts_rust_scale_simulator_port(self) -> None:
        subprocess.run(["cargo", "build", "--quiet", "--bins"], cwd=ROOT, check=True)

        simulator = subprocess.Popen(
            [
                str(ROOT / "target/debug/rp-scale-sim-scale"),
                "--weight",
                "1.25",
                "--stable",
                "true",
                "--interval-ms",
                "40",
            ],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.addCleanup(stop_process, simulator)

        assert simulator.stdout is not None
        first_line = simulator.stdout.readline().strip()
        self.assertTrue(first_line.startswith("device="), first_line)
        device = first_line.removeprefix("device=")
        self.assertTrue(device.startswith("/dev/"), device)

        time.sleep(0.1)
        result = subprocess.run(
            [
                str(ROOT / "target/debug/rp-scale-probe-serial"),
                device,
                "9600",
                "1200",
                "kg",
            ],
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=True,
            timeout=10,
        )

        self.assertIn("parsed_weight=true", result.stdout)
        self.assertIn("has_data=true", result.stdout)


def stop_process(process: subprocess.Popen[str]) -> None:
    try:
        if process.poll() is None:
            process.send_signal(signal.SIGTERM)
            try:
                process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait(timeout=2)
    finally:
        if process.stdout is not None:
            process.stdout.close()
        if process.stderr is not None:
            process.stderr.close()


if __name__ == "__main__":
    unittest.main()
