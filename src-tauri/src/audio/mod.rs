// AUTHR_OS — audio studio (Phase 4, build plan Sprints 9–11).
//
// Layout:
//   wav.rs       — WAV read/write (hound, 16-bit PCM mono)
//   dsp.rs       — DSP kernels: K-weighting/BS.1770 loudness, RMS/peak
//                  measurement, sidechain ducking, lookahead limiter
//   cache.rs     — SHA-256 content-addressable chunk cache (`audio/segments/`)
//   g2p.rs       — proper-noun IPA phonetics (lexicon + rule fallback)
//   scheduler.rs — topological DAG lookahead scheduler (acoustic continuity)
//   tts.rs       — multi-engine TTS connectors (Edge/ElevenLabs/Cartesia/
//                  OpenAI/HF-MMS) + deterministic offline `Scripted` engine
//   mixer.rs     — 3-layer mixer (Speech + Spot-SFX + Ambient) with resampling
//   master.rs    — two-pass normalize → lookahead limiter → ACX validation

pub mod cache;
pub mod dsp;
pub mod g2p;
pub mod master;
pub mod mixer;
pub mod scheduler;
pub mod tts;
pub mod wav;
