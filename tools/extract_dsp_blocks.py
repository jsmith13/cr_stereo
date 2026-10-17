#!/usr/bin/env python3
"""Extract the packed 24-bit DSP blocks embedded in known Rhythm firmware."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


IMAGE_BASE = 0x08010000
KNOWN_IMAGES = {
    "9B3B76CFB80570F9622CC9D5BA395CAC228862D87782F55B4857D0E095E320EF": {
        "build": 4799,
        "blocks": {
            "common_a": 0x0802F5C4,
            "common_b": 0x0805ECC8,
            "primary_a": 0x08019F00,
            "primary_b": 0x080101F0,
            "alternate_a": 0x0805ABFC,
            "alternate_b": 0x0805B9C0,
        },
    },
    "F00BAD6FFBEB08A551021CA656E04CA8FDA1975116EF1A2FEF38EE316AA3A5DB": {
        "build": 5695,
        "blocks": {
            "common_a": 0x08031638,
            "common_b": 0x08066244,
            "primary_a": 0x0801A838,
            "primary_b": 0x080101F0,
            "alternate_a": 0x0806211C,
            "alternate_b": 0x08062F0C,
        },
    },
    "D5B9FF5DADF2E284F80D69ECB7087486CF0DF315A697BED8272AF9DE0E90DF15": {
        "build": 5706,
        "blocks": {
            "common_a": 0x08031638,
            "common_b": 0x080662CC,
            "primary_a": 0x0801A838,
            "primary_b": 0x080101F0,
            "alternate_a": 0x08062130,
            "alternate_b": 0x08062F20,
        },
    },
}


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("image", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()

    image = args.image.read_bytes()
    image_sha256 = hashlib.sha256(image).hexdigest().upper()
    try:
        known = KNOWN_IMAGES[image_sha256]
    except KeyError as error:
        raise ValueError(f"Unsupported image SHA-256: {image_sha256}") from error

    args.output.mkdir(parents=True, exist_ok=True)
    manifest: dict[str, object] = {
        "source": str(args.image),
        "source_sha256": image_sha256,
        "build": known["build"],
        "packing": "three bytes per DSP word; first three bytes are a block header",
        "blocks": {},
    }

    for name, address in known["blocks"].items():
        offset = address - IMAGE_BASE
        if offset < 0 or offset + 3 > len(image):
            raise ValueError(f"{name} address is outside the image")

        header = image[offset : offset + 3]
        word_count = (header[1] << 8) | header[2]
        size = (word_count + 2) * 3
        block = image[offset : offset + size]
        if len(block) != size:
            raise ValueError(f"{name} is truncated")

        output = args.output / f"{name}.dsp24"
        output.write_bytes(block)
        manifest["blocks"][name] = {
            "address": f"0x{address:08X}",
            "header": header.hex(" ").upper(),
            "payload_words": word_count,
            "size_bytes": size,
            "end_address_exclusive": f"0x{address + size:08X}",
            "sha256": hashlib.sha256(block).hexdigest().upper(),
            "filename": output.name,
        }

    manifest_path = args.output / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="ascii")
    print(manifest_path)


if __name__ == "__main__":
    main()
