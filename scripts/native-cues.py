#!/usr/bin/env python3
"""Bounded, opt-in native pipe/receipt/capture driver; does not change displays.

No input/focus events. X11 captures are cropped from root to this child's client;
Windows captures BitBlt this child's visible client area from the screen.
Every capture is associated with an observed receipt and independently checked.
"""
import argparse
import hashlib
import json
import os
import pathlib
import queue
import secrets
import struct
import subprocess
import sys
import tempfile
import threading
import time

WINDOWS = sys.platform == "win32"
if WINDOWS:
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    import native_win as nw


def run():
    root = pathlib.Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=pathlib.Path, default=pathlib.Path(
        os.environ.get("CARGO_TARGET_DIR", root / "target")) / "debug/examples"
        / ("native_cues.exe" if WINDOWS else "native_cues"))
    parser.add_argument("--backend", choices=["gl", "vulkan", "dx12", "metal"], default="gl")
    parser.add_argument("--out", type=pathlib.Path, default=root / ".amp/in/artifacts/native-cues")
    args = parser.parse_args()
    if not WINDOWS and not os.environ.get("DISPLAY"):
        parser.error("requires an existing authorized X11 DISPLAY; driver never starts one")
    if not args.binary.is_file():
        parser.error(f"build native_cues first: {args.binary}")
    args.out.mkdir(parents=True, exist_ok=True)
    # Never leave a prior success summary after a failed rerun.
    (args.out / "summary.json").unlink(missing_ok=True)
    history = []
    children = []
    readers = {}
    epoch = int.from_bytes(secrets.token_bytes(16), "little") or 1

    with tempfile.TemporaryDirectory(prefix="sela-native-cues-") as scratch:
        env = dict(os.environ, XDG_RUNTIME_DIR=scratch)

        def cmd(*argv):
            return subprocess.check_output(argv, env=env, timeout=5, text=True).strip()

        def pump(stream, sink):
            # Portable bounded-wait reads: Windows pipes do not support select().
            while chunk := stream.read(65536):
                sink.put(chunk)
            sink.put(b"")

        def read_exact(child, size, deadline):
            pending = readers[child.pid]
            data = pending["buffer"]
            while len(data) < size:
                remain = deadline - time.monotonic()
                try:
                    chunk = pending["queue"].get(timeout=max(0, remain)) if remain > 0 else None
                except queue.Empty:
                    chunk = None
                if chunk is None:
                    raise TimeoutError("receipt missing; output unknown, do not replay")
                if not chunk:
                    raise RuntimeError("child disconnected; output unknown, do not replay")
                data.extend(chunk)
            result = bytes(data[:size])
            del data[:size]
            return result

        def receive(child, kind, timeout=3):
            deadline = time.monotonic() + timeout
            header = read_exact(child, 8, deadline)
            magic, version, actual_kind, size = struct.unpack("<4sBBH", header)
            assert (magic, version, actual_kind) == (b"SCUE", 1, kind)
            assert size == {2: 25, 3: 20}[kind]
            body = read_exact(child, size, deadline)
            history.append({"pid": child.pid, "kind": kind, "body_hex": body.hex(),
                            "received_unix_ns": time.time_ns(), "received_monotonic_ns": time.monotonic_ns()})
            return body

        def launch(session, label):
            with (args.out / f"{label}.stderr.log").open("wb") as logs:
                child = subprocess.Popen([str(args.binary), f"{session:x}", args.backend],
                                         stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                         stderr=logs, env=env, bufsize=0)
            children.append(child)
            readers[child.pid] = {"queue": queue.Queue(), "buffer": bytearray()}
            threading.Thread(target=pump, args=(child.stdout, readers[child.pid]["queue"]),
                             daemon=True).start()
            body = receive(child, 3, timeout=10)
            assert int.from_bytes(body[:16], "little") == session
            assert struct.unpack("<I", body[16:])[0] >= 641
            return child

        def write_all(child, packet, timeout=3):
            # Bounded writer: if a renderer stops consuming, the supervisor fails
            # instead of hanging. A stuck thread is unblocked when the child is killed.
            failure = []

            def writer():
                try:
                    view = memoryview(packet)
                    while view:
                        view = view[child.stdin.write(view[:65536]):]
                except OSError as error:
                    failure.append(error)

            thread = threading.Thread(target=writer, daemon=True)
            thread.start()
            thread.join(timeout)
            if thread.is_alive():
                raise TimeoutError("resource writer blocked; retire child, output unknown")
            if failure:
                raise failure[0]

        def send(child, session, seq, color, lane=0, extent=(641, 360)):
            # Fixed schema, inline owned resource values, no paths/pointers.
            body = (session.to_bytes(16, "little") + struct.pack("<QBI", seq, lane, 2000)
                    + (19).to_bytes(16, "little") + struct.pack("<QII4B", seq + 22, *extent, *color))
            assert len(body) == 65
            packet = struct.pack("<4sBBH", b"SCUE", 1, 1, len(body)) + body
            # One small packet, only in supervisor. Deadline bounds child
            # lifetime; renderer's pipe reader is independent of its frame loop.
            write_all(child, packet)

        def ack(child, session, seq, outcome):
            body = receive(child, 2)
            assert int.from_bytes(body[:16], "little") == session
            assert struct.unpack("<Q", body[16:24])[0] == seq
            assert body[24] == outcome, (body[24], outcome)

        def mask(child, session, seq, layer, lane=1):
            # MASK kind 5: layer 0 None, 1 Clear, 2 Black, 3 Logo; Safety lane.
            body = session.to_bytes(16, "little") + struct.pack("<QBIB", seq, lane, 2000, layer)
            assert len(body) == 30
            write_all(child, struct.pack("<4sBBH", b"SCUE", 1, 5, len(body)) + body)

        def resource(child, session, seq, text="Signal café\nBeacon", font=None, budget=2000):
            font = (root / "tests/fixtures/DejaVuSans.ttf").read_bytes() if font is None else font
            version = (71).to_bytes(16, "little") + struct.pack("<Q", 13)
            pixels = bytes([173, 31, 57, 255, 23, 149, 79, 255, 31, 53, 179, 255,
                            151, 117, 23, 255, 19, 137, 151, 255, 137, 29, 149, 255])
            content = text.encode("utf-8")
            body = (session.to_bytes(16, "little") + struct.pack("<QBI", seq, 0, budget)
                    + (19).to_bytes(16, "little") + struct.pack("<QII", seq + 22, 641, 360)
                    + b"\x01" + version + struct.pack("<III", 3, 2, len(pixels)) + pixels
                    + b"\x01" + version + struct.pack("<HI", 32, len(content)) + content
                    + struct.pack("<I", len(font)) + font)
            packet = struct.pack("<4sBBHI", b"SCUE", 2, 1, 0, len(body)) + body
            write_all(child, packet)

        def window(child):
            end = time.monotonic() + 5
            while time.monotonic() < end:
                if child.poll() is not None:
                    raise RuntimeError("child exited before capture")
                if WINDOWS:
                    found = nw.top_windows(child.pid)
                    if found:
                        return found[0]
                else:
                    try:
                        return cmd("xdotool", "search", "--onlyvisible", "--pid", str(child.pid)).splitlines()[0]
                    except subprocess.CalledProcessError:
                        pass
                time.sleep(0.05)
            raise TimeoutError("owned window not found")

        def grab(target, output):
            """Returns (width, height, packed 8-bit RGB) and writes the PNG."""
            if WINDOWS:
                width, height, bgra = nw.capture_client(target)
                nw.write_png(output, width, height, bgra)
                return width, height, nw.bgra_to_rgb(bgra)
            # xdotool's absolute origin can include a decoration offset under a
            # reparenting WM. Use actual client coordinates, as the GPUI drivers do.
            geom = {}
            for line in cmd("xwininfo", "-id", target).splitlines():
                for key, field in [("X", "Absolute upper-left X:"),
                                   ("Y", "Absolute upper-left Y:"),
                                   ("WIDTH", "Width:"), ("HEIGHT", "Height:")]:
                    if field in line:
                        geom[key] = int(line.split(":")[-1].strip())
            image = pathlib.Path(scratch) / "root.png"
            subprocess.run(["import", "-window", "root", str(image)], env=env, check=True, timeout=5)
            bounds = "{WIDTH}x{HEIGHT}+{X}+{Y}".format(**geom)
            subprocess.run(["convert", str(image), "-crop", bounds, "+repage", str(output)],
                           env=env, check=True, timeout=5)
            raw = subprocess.check_output(["convert", str(output), "-depth", "8", "rgb:-"], env=env, timeout=5)
            return int(geom["WIDTH"]), int(geom["HEIGHT"]), raw

        def capture(child, label, expected, receipt_seq, text=False, cleared=False):
            # Allow the compositor to sample the submitted frame; this is not
            # scanout timing, a GPU wait, or evidence of every frame's visibility.
            time.sleep(0.15)
            output = args.out / f"{label}.png"
            width, height, raw = grab(window(child), output)
            assert (width, height) == (641, 360), (width, height)
            assert len(raw) == width * height * 3
            # Check multiple interior points; center-only could miss a stale scene.
            pixels = []
            points = [(32, 32), (320, 180), (608, 327)]
            if text or cleared:
                points = [(100, 160), (320, 160), (540, 160), (100, 280), (320, 280), (540, 280)]
            for index, (x, y) in enumerate(points):
                offset = (y * width + x) * 3
                pixel = raw[offset:offset + 3]
                wanted = expected[index] if text or cleared else expected
                assert all(abs(a - b) <= 2 for a, b in zip(pixel, wanted)), (label, list(pixel), wanted)
                pixels.append(list(pixel))
            white = sum(min(raw[i:i + 3]) >= 245 for i in range(0, len(raw), 3))
            if text:
                assert white > 100, ("no actual text coverage", white)
            if cleared:
                assert white == 0, ("text visible under Clear", white)
            history.append({"pid": child.pid, "capture": output.name, "confirmed_receipt_sequence": receipt_seq,
                            "expected_rgb": expected, "observed_rgb": pixels, "captured_unix_ns": time.time_ns(),
                            "rgb_sha256": hashlib.sha256(raw).hexdigest()})
            return hashlib.sha256(raw).digest()

        def retire(child):
            child.stdin.close()  # Disconnect, not implicit replay or service reset.
            try:
                child.wait(timeout=3)
            except subprocess.TimeoutExpired:
                child.terminate()
                try:
                    child.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    child.kill()
                    child.wait(timeout=2)
            assert child.returncode == 0, ("audience did not retire cleanly", child.returncode)

        try:
            child = launch(epoch, "first")
            red, green = (173, 31, 57, 255), (23, 149, 79, 255)
            send(child, epoch, 1, red)
            ack(child, epoch, 1, 0)  # Accepted != live.
            ack(child, epoch, 1, 1)  # Actual queue.submit + native present call.
            capture(child, "applied-red", red[:3], 1)
            send(child, epoch, 2, (255, 255, 0, 0))  # Invalid prepared alpha.
            ack(child, epoch, 2, 7)
            capture(child, "invalid-retains-red", red[:3], 1)
            send(child, epoch, 3, green, extent=(640, 360))
            ack(child, epoch, 3, 0)
            ack(child, epoch, 3, 7)  # Valid CPU color, renderer rejects extent.
            capture(child, "render-failure-retains-red", red[:3], 1)
            send(child, epoch, 1, red)  # Currently applied duplicate, no reapply.
            ack(child, epoch, 1, 1)
            send(child, epoch, 4, green, lane=1)  # Reserved lane, no mask semantics.
            ack(child, epoch, 4, 0)
            ack(child, epoch, 4, 1)
            capture(child, "applied-green", green[:3], 4)
            send(child, epoch, 3, red)
            ack(child, epoch, 3, 6)  # Old sequence rejected.
            capture(child, "stale-retains-green", green[:3], 4)
            resource(child, epoch, 5)
            ack(child, epoch, 5, 0)
            ack(child, epoch, 5, 1)
            asymmetric = [(173, 31, 57), (23, 149, 79), (31, 53, 179),
                          (151, 117, 23), (19, 137, 151), (137, 29, 149)]
            retained = capture(child, "text-asymmetric-image", asymmetric, 5, text=True)
            for seq, label, content, font in [
                (6, "missing-glyph", "\U0010ffff", None),
                (7, "overflow", "Overflow " * 100, None),
                (8, "invalid-font", "Do not replace", b"OTTOgarbage"),
                (9, "oversize-text", "A" * 65537, None),
            ]:
                resource(child, epoch, seq, text=content, font=font)
                ack(child, epoch, seq, 7)
                assert capture(child, f"{label}-retains-text-image", asymmetric, 5, text=True) == retained
            resource(child, epoch, 4)
            ack(child, epoch, 4, 6)
            assert capture(child, "stale-retains-text-image", asymmetric, 5, text=True) == retained
            resource(child, epoch, 10, budget=1)
            ack(child, epoch, 10, 4)  # Transfer/queued preparation consumed budget.
            assert capture(child, "expired-retains-text-image", asymmetric, 5, text=True) == retained
            # M0-08a masks over the text/image slide; Safety lane order is its own.
            mask(child, epoch, 11, 1)
            ack(child, epoch, 11, 0)
            ack(child, epoch, 11, 1)
            capture(child, "clear-keeps-image-background", asymmetric, 11, cleared=True)
            mask(child, epoch, 12, 2)
            ack(child, epoch, 12, 0)
            ack(child, epoch, 12, 1)
            capture(child, "black-mask", (0, 0, 0), 12)
            mask(child, epoch, 13, 3)  # No logo was ever sent.
            ack(child, epoch, 13, 0)
            ack(child, epoch, 13, 7)
            capture(child, "logo-without-logo-keeps-black", (0, 0, 0), 12)
            mask(child, epoch, 12, 0)
            ack(child, epoch, 12, 1)  # Applied duplicate, not a new unmask.
            capture(child, "duplicate-mask-keeps-black", (0, 0, 0), 12)
            mask(child, epoch, 14, 0)
            ack(child, epoch, 14, 0)
            ack(child, epoch, 14, 1)
            assert capture(child, "unmask-restores-text-image", asymmetric, 14, text=True) == retained
            retire(child)
            new_epoch = epoch ^ 1  # Distinct within run; old child fully retired.
            if new_epoch == 0:
                new_epoch = epoch ^ 2
            child = launch(new_epoch, "restarted")
            capture(child, "fresh-session-unconfirmed", (0, 0, 0), None)
            resource(child, epoch, 11)
            ack(child, epoch, 11, 5)  # Retired resource epoch cannot replace anything.
            capture(child, "old-epoch-rejected", (0, 0, 0), None)
            send(child, new_epoch, 1, green)
            ack(child, new_epoch, 1, 0)
            ack(child, new_epoch, 1, 1)
            capture(child, "fresh-explicit-green", green[:3], 1)
            # Both legal lanes arrive without awaiting the first acknowledgment.
            # Accepted may interleave, but terminal presentation remains FIFO.
            blue = (31, 53, 179, 255)
            send(child, new_epoch, 2, red)
            send(child, new_epoch, 3, blue, lane=1)
            outcomes = {2: [], 3: []}
            applied = []
            for _ in range(4):
                body = receive(child, 2)
                assert int.from_bytes(body[:16], "little") == new_epoch
                sequence = struct.unpack("<Q", body[16:24])[0]
                outcomes[sequence].append(body[24])
                if body[24] == 1:
                    applied.append(sequence)
            assert outcomes == {2: [0, 1], 3: [0, 1]}, outcomes
            assert applied == [2, 3], applied
            capture(child, "consecutive-lanes-blue", blue[:3], 3)
            retire(child)
            (args.out / "summary.json").write_text(json.dumps({"status": "PASS",
                "backend": args.backend, "platform": sys.platform,
                "qualification": "native submission/capture only, not physical scanout timing",
                "epoch_hex": f"{epoch:x}", "new_epoch_hex": f"{new_epoch:x}", "events": history}, indent=2) + "\n")
            print("PASS: native command/receipt/pixel retention/restart; physical scanout unqualified")
        finally:
            for child in children:
                if child.poll() is None:
                    child.kill()
                child.wait(timeout=3)
                if child.stdin and not child.stdin.closed:
                    child.stdin.close()


if __name__ == "__main__":
    run()
