"""Offline pdfcraft glue tests (no CLI / WIA)."""

from fakescan.pdfcraft import (
    find_device_row,
    handling_status,
    looks_like_fakescan,
    pick_fakescan_device,
)


def test_looks_like_fakescan():
    assert looks_like_fakescan(name="Fake WSD scanner (lite caps)")
    assert looks_like_fakescan(port="http://192.168.1.10:3920/ScannerService")
    assert not looks_like_fakescan(name="Brother MFC-9330CDW")


def test_pick_fakescan_device_prefers_wsd_name():
    scanners = [
        {"name": "Brother MFC", "id": "usb", "connection": "usb", "port": ""},
        {
            "name": "Fake WSD scanner (lite caps)",
            "id": "wsd-1",
            "connection": "wsd",
            "port": "http://127.0.0.1:3920/",
        },
    ]
    assert pick_fakescan_device(scanners) == "Fake WSD scanner (lite caps)"


def test_pick_fakescan_ignores_brother_scanner_service_path():
    scanners = [
        {
            "name": "Brother MFC-9330CDW",
            "id": "x",
            "connection": "wsd",
            "port": "http://192.168.1.5/ScannerService",
        }
    ]
    assert pick_fakescan_device(scanners) is None


def test_handling_status_jam_bits():
    row = {
        "name": "Fake WSD scanner (lite caps)",
        "handling_status": {
            "feeder_ready": True,
            "jam": True,
            "cover_open": False,
        },
    }
    assert handling_status(row)["jam"] is True
    assert find_device_row([row], "Fake WSD scanner (lite caps)") is row
