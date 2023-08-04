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

    def test_simulator_emits_realistic_moving_and_stable_frames(self) -> None:
        subprocess.run(["cargo", "build", "--quiet", "--bins"], cwd=ROOT, check=True)

        simulator = subprocess.Popen(
            [
                str(ROOT / "target/debug/rp-scale-sim-scale"),
                "--scenario",
                "batch",
                "--interval-ms",
                "20",
            ],
            cwd=ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        self.addCleanup(stop_process, simulator)

        assert simulator.stdout is not None
        first_line = simulator.stdout.readline().strip()
        device = first_line.removeprefix("device=")

        frames = read_serial_lines(device, limit=80, timeout=5.0)

        self.assertGreaterEqual(len(frames), 40)
        self.assertTrue(any(frame.endswith(" US") for frame in frames), frames)
        self.assertTrue(any(frame.endswith(" ST") for frame in frames), frames)
        self.assertTrue(any(frame.startswith("0.000 kg ST") for frame in frames), frames)
        self.assertTrue(any(frame.startswith("1.250 kg ST") for frame in frames), frames)
        self.assertGreaterEqual(count_repeated(frames, "1.250 kg ST"), 10)


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


def read_serial_lines(device: str, *, limit: int, timeout: float) -> list[str]:
    fd = os.open(device, os.O_RDONLY | os.O_NONBLOCK)
    try:
        deadline = time.time() + timeout
        raw = b""
        lines: list[str] = []
        while time.time() < deadline and len(lines) < limit:
            try:
                chunk = os.read(fd, 4096)
            except BlockingIOError:
                time.sleep(0.02)
                continue
            if not chunk:
                time.sleep(0.02)
                continue
            raw += chunk
            while b"\r" in raw or b"\n" in raw:
                split_at = min(
                    [idx for idx in [raw.find(b"\r"), raw.find(b"\n")] if idx >= 0]
                )
                frame = raw[:split_at].decode("utf-8", errors="replace").strip()
                raw = raw[split_at + 1 :]
                if frame:
                    lines.append(frame)
        return lines
    finally:
        os.close(fd)


def count_repeated(frames: list[str], value: str) -> int:
    best = 0
    current = 0
    for frame in frames:
        if frame == value:
            current += 1
            best = max(best, current)
        else:
            current = 0
    return best


if __name__ == "__main__":
    unittest.main()
