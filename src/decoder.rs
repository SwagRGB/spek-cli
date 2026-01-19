use anyhow::{anyhow, Result, Context};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;
use symphonia::core::codecs::{CodecType, DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::audio::{AudioBufferRef, Channels};
use symphonia::core::conv::FromSample;
use symphonia::core::audio::Signal;
use std::fs::File;
use std::path::Path;
use indicatif::{ProgressBar, ProgressStyle};

pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u32,
    pub duration_secs: f64,
    pub metadata: AudioMetadata,
}

#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub codec: String,
    pub bits_per_sample: Option<u32>,
    pub bit_rate: Option<u64>,
    pub channel_layout: String,
}

macro_rules! process_buffer {
    ($buf:expr, $samples:expr) => {
        for i in 0..$buf.frames() {
            let mut sum = 0.0;
            for c in 0..$buf.spec().channels.count() {
                sum += f32::from_sample($buf.chan(c)[i]);
            }
            $samples.push(sum / $buf.spec().channels.count() as f32);
        }
    };
}

pub fn decode_file(path: &Path, quiet: bool) -> Result<AudioData> {
    let file = File::open(path).with_context(|| format!("failed to open audio file: {:?}", path))?;
    let file_size = file.metadata()?.len();
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let hint = Hint::new();
    let format_opts: FormatOptions = Default::default();
    let metadata_opts: MetadataOptions = Default::default();
    let decoder_opts: DecoderOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .context("unsupported format")?;

    let mut format = probed.format;
    let track = format.tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow!("no supported audio tracks found"))?;

    // Extract metadata
    let codec_name = codec_to_string(track.codec_params.codec);
    let bits_per_sample = track.codec_params.bits_per_sample;
    let bit_rate = None;
    let channel_layout = channels_to_string(track.codec_params.channels);

    let metadata = AudioMetadata {
        codec: codec_name,
        bits_per_sample,
        bit_rate,
        channel_layout,
    };

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &decoder_opts)
        .context("unsupported codec")?;

    let track_id = track.id;
    let sample_rate = track.codec_params.sample_rate.unwrap_or(44100);
    let mut samples: Vec<f32> = Vec::new();

    // Setup progress bar (only if not quiet)
    let pb = if quiet {
        ProgressBar::hidden()
    } else {
        let pb = ProgressBar::new(file_size);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} {msg} [{bar:40.cyan/blue}] {percent}% ({eta})")
                .unwrap()
                .progress_chars("━━╸")
        );
        pb.set_message("Decoding");
        pb
    };

    let mut bytes_read = 0u64;

    // Decode all packets
    loop {
        let packet = match format.next_packet() {
            Ok(packet) => {
                bytes_read += packet.buf().len() as u64;
                pb.set_position(bytes_read.min(file_size));
                packet
            },
            Err(symphonia::core::errors::Error::IoError(err)) => {
                 if err.kind() == std::io::ErrorKind::UnexpectedEof {
                     break;
                 }
                 return Err(anyhow::Error::new(err));
            }
            Err(symphonia::core::errors::Error::ResetRequired) => {
                continue;
            }
            Err(err) => return Err(anyhow::Error::new(err)),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                match decoded {
                    AudioBufferRef::F32(buf) => process_buffer!(buf, samples),
                    AudioBufferRef::U8(buf) => process_buffer!(buf, samples),
                    AudioBufferRef::S16(buf) => process_buffer!(buf, samples),
                    AudioBufferRef::S24(buf) => process_buffer!(buf, samples),
                    AudioBufferRef::S32(buf) => process_buffer!(buf, samples),
                    _ => return Err(anyhow!("unsupported sample format")),
                }
            }
            Err(symphonia::core::errors::Error::DecodeError(_)) => {
                continue;
            }
            Err(err) => return Err(anyhow::Error::new(err)),
        }
    }

    if !quiet {
        pb.finish_with_message("Decoded ✓");
    }

    let duration_secs = samples.len() as f64 / sample_rate as f64;

    Ok(AudioData {
        samples,
        sample_rate,
        channels: 1, // We mixed down to mono
        duration_secs,
        metadata,
    })
}

fn codec_to_string(codec: CodecType) -> String {
    // Check against known codec types
    use symphonia::core::codecs::*;
    
    match codec {
        CODEC_TYPE_FLAC => "FLAC".to_string(),
        CODEC_TYPE_MP3 => "MP3".to_string(),
        CODEC_TYPE_AAC => "AAC".to_string(),
        CODEC_TYPE_ALAC => "ALAC".to_string(),
        CODEC_TYPE_VORBIS => "Vorbis".to_string(),
        CODEC_TYPE_OPUS => "Opus".to_string(),
        CODEC_TYPE_PCM_S16LE | CODEC_TYPE_PCM_S16BE => "PCM (16-bit)".to_string(),
        CODEC_TYPE_PCM_S24LE | CODEC_TYPE_PCM_S24BE => "PCM (24-bit)".to_string(),
        CODEC_TYPE_PCM_S32LE | CODEC_TYPE_PCM_S32BE => "PCM (32-bit)".to_string(),
        CODEC_TYPE_PCM_F32LE | CODEC_TYPE_PCM_F32BE => "PCM (32-bit float)".to_string(),
        _ => format!("Unknown ({:?})", codec),
    }
}

fn channels_to_string(channels: Option<Channels>) -> String {
    match channels {
        Some(ch) => {
            let count = ch.count();
            if ch.contains(Channels::FRONT_LEFT) && ch.contains(Channels::FRONT_RIGHT) && count == 2 {
                "Stereo".to_string()
            } else if ch.contains(Channels::FRONT_CENTRE) && count == 1 {
                "Mono".to_string()
            } else if count == 6 {
                "5.1 Surround".to_string()
            } else if count == 8 {
                "7.1 Surround".to_string()
            } else {
                format!("{} channels", count)
            }
        }
        None => "Unknown".to_string(),
    }
}
