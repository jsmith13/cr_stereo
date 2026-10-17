#!/usr/bin/env python3

import random
import struct


def dsp_average(left: int, right: int) -> int:
    total = left + right
    return total // 2


def transform(words: list[int], mode: str) -> list[int]:
    if len(words) % 2:
        raise ValueError("interleaved capture has an odd word count")
    if mode not in {"mono", "left", "right"}:
        raise ValueError("invalid transform")

    output = []
    for offset in range(0, len(words), 2):
        left, right = words[offset : offset + 2]
        if mode == "mono":
            output.append(dsp_average(left, right))
        elif mode == "left":
            output.append(left)
        else:
            output.append(right)
    return output


def transform_chunked(words: list[int], mode: str, frames_per_chunk: int) -> list[int]:
    output = []
    words_per_chunk = frames_per_chunk * 2
    for offset in range(0, len(words), words_per_chunk):
        output.extend(transform(words[offset : offset + words_per_chunk], mode))
    return output


def pcm16(values: list[int]) -> bytes:
    return struct.pack(f"<{len(values)}h", *values)


def main() -> None:
    vectors = [
        -32768,
        -32767,
        -1,
        0,
        1,
        32766,
        32767,
    ]
    pairs = [(left, right) for left in vectors for right in vectors]
    rng = random.Random(0x5695)
    pairs.extend(
        (rng.randint(-32768, 32767), rng.randint(-32768, 32767))
        for _ in range(100_000)
    )
    words = [sample for pair in pairs for sample in pair]

    mono = transform(words, "mono")
    left = transform(words, "left")
    right = transform(words, "right")

    assert left == [pair[0] for pair in pairs]
    assert right == [pair[1] for pair in pairs]
    assert mono == [dsp_average(*pair) for pair in pairs]
    assert all(-32768 <= sample <= 32767 for sample in mono)
    assert transform_chunked(words, "mono", 1) == mono
    assert transform_chunked(words, "mono", 256) == mono
    assert transform_chunked(words, "left", 256) == left
    assert transform_chunked(words, "right", 256) == right
    assert len(pcm16(mono)) == len(pairs) * 2

    try:
        transform([1], "mono")
    except ValueError:
        pass
    else:
        raise AssertionError("odd word count was accepted")

    print(f"validated {len(pairs):,} stereo frames")


if __name__ == "__main__":
    main()
