"""AUTHR_OS — Phase 4/5 audio & AI test fixtures (WAV generation, seeded chapters)."""

import math
import struct
import wave
from pathlib import Path


def make_wav(path: Path, seconds: float = 0.2, freq: float = 440.0, amp: float = 0.4, rate: int = 24000):
    """Write a 16-bit PCM mono WAV — deterministic sine fixture."""
    path.parent.mkdir(parents=True, exist_ok=True)
    n = int(seconds * rate)
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        frames = b"".join(
            struct.pack("<h", int(amp * math.sin(2 * math.pi * freq * i / rate) * 32767))
            for i in range(n)
        )
        w.writeframes(frames)
    return path


def make_spike_wav(path: Path, seconds: float = 0.4, rate: int = 24000, density: float = 0.01,
                   tail_silence: float = 0.0):
    """Full-scale 1-sample spikes at `density` — crest factor ≈ 20 dB, so the
    mastering chain MUST engage the lookahead limiter (RMS target −20 dB pushes
    the peaks far above the −3.1 dBFS ceiling). `tail_silence` appends digital
    silence so ACX noise-floor checks have a clean measurement window."""
    path.parent.mkdir(parents=True, exist_ok=True)
    n = int(seconds * rate)
    tail = int(tail_silence * rate)
    step = max(1, int(1.0 / density))
    with wave.open(str(path), "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(rate)
        frames = b"".join(
            struct.pack("<h", 32767 if i % step == 0 else 0) for i in range(n)
        )
        w.writeframes(frames)
        w.writeframes(b"\x00\x00" * tail)
    return path


def read_wav_stats(path: Path):
    """Return (frames, rate) of a WAV file via stdlib wave."""
    with wave.open(str(path), "rb") as w:
        return w.getnframes(), w.getframerate()


def seed_audio_chapter(bundle, cli, speakers_and_texts=None):
    """Create book/chapter/characters + 5 segments across 3 speakers.

    Returns (book_id, chapter_id, segment_ids).
    """
    b = bundle
    cli("character_upsert", bundle=b, args={
        "character_key": "CHAR_KIRA", "display_name": "Kira Voss",
        "role": "PROTAGONIST", "voice_engine": "EDGE_TTS", "voice_id": "voice_kira_bright",
    })
    cli("character_upsert", bundle=b, args={
        "character_key": "CHAR_ORIN", "display_name": "Orin",
        "role": "SUPPORTING", "voice_engine": "EDGE_TTS", "voice_id": "voice_orin_low",
    })
    r = cli("chapter_create", bundle=b, args={
        "book_id": 1, "chapter_number": 1, "title": "Signal Bloom", "act_number": 1,
    })
    assert r.ok, r.payload
    chapter_id = r.payload["data"]["id"]
    default = [
        ("NARRATION", "NARRATOR", "Dawn broke over the relay array in pale amber bands."),
        ("DIALOGUE", "CHAR_KIRA", "The signal is older than the colony itself, Orin."),
        ("NARRATION", "NARRATOR", "Wind dragged static curtains across the ridge."),
        ("DIALOGUE", "CHAR_ORIN", "Then we answer it before it stops asking."),
        ("INTERNAL_MONOLOGUE", "CHAR_KIRA", "Every answer we send is a door we cannot close."),
    ]
    seq = speakers_and_texts or default
    ids = []
    for i, (seg_type, speaker, text) in enumerate(seq, start=1):
        r = cli("segment_append", bundle=b, args={
            "chapter_id": chapter_id, "segment_type": seg_type, "speaker_id": speaker,
            "text_content": text, "sequence_order": i,
        })
        assert r.ok, r.payload
        ids.append(r.payload["data"]["segment_id"])
    return 1, chapter_id, ids
