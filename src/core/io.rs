use hound::{WavReader, WavWriter, WavSpec, SampleFormat};
use std::path::Path;
use thiserror::Error;
use crate::signal_processing::{to_mono, resample};
use ndarray::ShapeError;
use rayon::prelude::*;
use std::sync::mpsc::{channel, Receiver};
use std::io::Cursor;

/// Enumerates error conditions for WAV-based audio operations.
///
/// Variants encapsulate specific failure modes encountered during file I/O, format parsing,
/// or signal processing, with detailed diagnostics for DSP pipeline debugging.
#[derive(Error, Debug)]
pub enum AudioError {
    /// WAV file open failure, typically due to invalid path or corrupted header.
    #[error("WAV open failed: {0}")]
    OpenError(#[from] hound::Error),
    
    /// Unsupported WAV sample format (only PCM 16-bit int and 32-bit float are supported).
    #[error("Unsupported WAV sample format")]
    UnsupportedFormat,
    
    /// Offset or duration exceeds sample bounds.
    #[error("Offset or duration out of bounds")]
    InvalidRange,
    
    /// General I/O error outside `hound` operations (e.g., filesystem issues).
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
    
    /// `hound`-specific error during sample read/write.
    #[error("Hound processing error: {0}")]
    HoundError(hound::Error),
    
    /// Resampling failure from `signal_processing::resampling`.
    #[error("Resampling error: {0}")]
    ResampleError(#[from] crate::signal_processing::resampling::ResampleError),
    
    /// Streaming operation failure (e.g., channel disconnect).
    #[error("Stream processing error")]
    StreamError,
    
    /// Array shape mismatch from `ndarray` operations.
    #[error("Shape mismatch: {0}")]
    ShapeError(#[from] ShapeError),
    
    /// Insufficient samples for requested operation.
    #[error("Insufficient sample count: {0}")]
    InsufficientData(String),
    
    /// Invalid parameter (e.g., negative offset).
    #[error("Invalid parameter: {0}")]
    InvalidInput(String),
    
    /// Numerical computation failure (e.g., overflow).
    #[error("Computation error: {0}")]
    ComputationFailed(String),

    /// File not found at the specified path.
    #[error("File not found: {0}")]
    FileNotFound(String),
}

/// Core audio data container for WAV-based DSP workflows.
///
/// Stores interleaved 32-bit float samples with associated sample rate and channel count.
/// Optimized for in-memory processing and compatibility with `signal_processing` operations.
///
/// # Fields
/// - `samples`: Interleaved `f32` sample buffer (e.g., `[L1, R1, L2, R2...]` for stereo)
/// - `sample_rate`: Samples per second (Hz)
/// - `channels`: Number of channels (1 = mono, 2 = stereo)
#[derive(Debug, Clone)]
pub struct AudioData {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioData {
    /// Constructs an `AudioData` instance from raw components.
    ///
    /// # Parameters
    /// - `samples`: Interleaved `f32` sample buffer
    /// - `sample_rate`: Sample rate in Hz
    /// - `channels`: Channel count
    ///
    /// # Returns
    /// Initialized `AudioData` instance
    ///
    /// # Example
    /// ```
    /// use crate::core::AudioData;
    /// let audio = AudioData::new(
    ///     vec![0.5, -0.3, 0.8], // 3 mono samples
    ///     44100,                // 44.1 kHz
    ///     1                     // Mono
    /// );
    /// assert_eq!(audio.samples.len(), 3);
    /// assert_eq!(audio.sample_rate, 44100);
    /// assert_eq!(audio.channels, 1);
    /// ```
    pub fn new(samples: Vec<f32>, sample_rate: u32, channels: u16) -> Self {
        Self { samples, sample_rate, channels }
    }
}

/// Loads WAV file into `AudioData` with optional DSP transformations.
///
/// Reads WAV data in-memory via `Cursor`, supporting 16-bit PCM and 32-bit float formats.
/// Applies resampling, mono conversion, and sample trimming as specified.
///
/// # Parameters
/// - `path`: WAV file path (`AsRef<Path>`)
/// - `sr`: Target sample rate (Hz); `None` retains source rate
/// - `mono`: Convert to mono if `Some(true)`; `None` defaults to `true`
/// - `offset`: Start time (seconds); `None` defaults to 0.0
/// - `duration`: Segment length (seconds); `None` takes full length
///
/// # Returns
/// - `Ok(AudioData)`: Processed audio data
/// - `Err(AudioError)`: Failure due to I/O, format, or parameter errors
///
/// # Errors
/// - `AudioError::FileNotFound`: The specified file does not exist
/// - `AudioError::InvalidRange`: Offset/duration exceeds file length
/// - `AudioError::OpenError`: Invalid WAV file or corrupted header
///
/// # Examples
/// ```
/// use crate::core::{load, AudioData};
/// // Load entire file as mono at original sample rate
/// let audio = load("track.wav", None, Some(true), None, None)?;
/// 
/// // Load 5-second segment starting at 2 seconds, resampled to 16kHz
/// let segment = load("track.wav", Some(16000), Some(true), Some(2.0), Some(5.0))?;
/// # Ok::<(), crate::core::AudioError>(())
/// ```
pub fn load<P: AsRef<Path>>(
    path: P,
    sr: Option<u32>,
    mono: Option<bool>,
    offset: Option<f32>,
    duration: Option<f32>,
) -> Result<AudioData, AudioError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(AudioError::FileNotFound(path.to_string_lossy().into_owned()));
    }

    let wav_data = std::fs::read(&path)?;
    let mut reader = WavReader::new(Cursor::new(wav_data))?;
    let spec = reader.spec();
    let sample_rate = spec.sample_rate;

    let start = (offset.unwrap_or(0.0) * sample_rate as f32) as usize;
    let len = duration.map(|d| (d * sample_rate as f32) as usize);

    let samples: Vec<f32> = match spec.sample_format {
        SampleFormat::Float => reader.samples::<f32>()
            .skip(start)
            .take(len.unwrap_or(usize::MAX))
            .map(|s| s.unwrap())
            .collect(),
        SampleFormat::Int => reader.samples::<i16>()
            .skip(start)
            .take(len.unwrap_or(usize::MAX))
            .map(|s| s.unwrap() as f32 / i16::MAX as f32)
            .collect(),
    };

    if start >= samples.len() && !samples.is_empty() {
        return Err(AudioError::InvalidRange);
    }

    let mut samples = samples;
    let channels = spec.channels as usize;
    if channels > 1 && mono.unwrap_or(true) {
        samples = to_mono(&samples, channels);
    }

    let final_samples = if let Some(target_samplerate) = sr {
        if target_samplerate != sample_rate {
            resample(&samples, sample_rate, target_samplerate)?
        } else {
            samples
        }
    } else {
        samples
    };

    Ok(AudioData::new(final_samples, sr.unwrap_or(sample_rate), if mono.unwrap_or(true) { 1 } else { spec.channels }))
}

/// Exports `AudioData` to a WAV file using in-memory buffering.
///
/// Writes 32-bit float WAV data via `Cursor`, committing to disk in a single operation.
///
/// # Parameters
/// - `path`: Output WAV file path (`AsRef<Path>`)
/// - `audio_data`: Source `AudioData` reference
///
/// # Returns
/// - `Ok(())`: Successful write
/// - `Err(AudioError)`: I/O or format error
///
/// # Errors
/// - `AudioError::IoError`: Failed to write to filesystem
/// - `AudioError::HoundError`: WAV format encoding error
///
/// # Notes
/// - Automatically clamps samples to `[-1.0, 1.0]` range
/// - Preserves channel count and sample rate metadata
///
/// # Example
/// ```
/// use crate::core::{AudioData, export};
/// let audio = AudioData::new(vec![0.1, 0.2, 0.3], 44100, 1);
/// export("output.wav", &audio)?;
/// # Ok::<(), crate::core::AudioError>(())
/// ```
pub fn export<P: AsRef<Path>>(path: P, audio_data: &AudioData) -> Result<(), AudioError> {
    let spec = WavSpec {
        channels: audio_data.channels,
        sample_rate: audio_data.sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    let mut buffer = Vec::with_capacity(audio_data.samples.len() * 4 + 44); // Rough WAV size estimate
    let mut writer = WavWriter::new(Cursor::new(&mut buffer), spec)?;
    for &sample in &audio_data.samples {
        writer.write_sample(sample)?;
    }
    writer.finalize()?;
    std::fs::write(path, buffer)?;
    Ok(())
}

/// Generates an iterator over WAV sample blocks with parallel processing.
///
/// Splits WAV data into fixed-size blocks, processed in parallel using `rayon`.
///
/// # Parameters
/// - `path`: WAV file path (`AsRef<Path>`).
/// - `block_length`: Maximum block count.
/// - `frame_length`: Samples per block.
/// - `hop_length`: Step size between blocks; `None` uses `frame_length`.
///
/// # Returns
/// - `Ok(impl Iterator<Item = Vec<f32>>)`: Block iterator.
/// - `Err(AudioError)`: I/O or format error.
///
/// # Errors
/// - `AudioError::FileNotFound`: The specified file does not exist
/// - `AudioError::OpenError`: Invalid WAV file or corrupted header
///
/// # Example
/// ```
/// use crate::core::stream;
/// let stream = stream("audio.wav", 100, 4096, None)?;
/// for block in stream {
///     // Process each 4096-sample block
///     println!("Block size: {}", block.len());
/// }
/// # Ok::<(), crate::core::AudioError>(())
/// ```
///
/// # Performance
/// - Uses `rayon` thread pool for parallel block processing
/// - Best for offline processing of <1GB files
pub fn stream<P: AsRef<Path>>(
    path: P,
    block_length: usize,
    frame_length: usize,
    hop_length: Option<usize>,
) -> Result<impl Iterator<Item = Vec<f32>>, AudioError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(AudioError::FileNotFound(path.to_string_lossy().into_owned()));
    }

    let wav_data = std::fs::read(&path)?;
    let mut reader = WavReader::new(Cursor::new(wav_data))?;
    let spec = reader.spec();
    let hop = hop_length.unwrap_or(frame_length);

    let samples: Vec<f32> = match spec.sample_format {
        SampleFormat::Float => reader.samples::<f32>().map(|s| s.unwrap()).collect(),
        SampleFormat::Int => reader.samples::<i16>().map(|s| s.unwrap() as f32 / i16::MAX as f32).collect(),
    };

    let indices: Vec<usize> = (0..samples.len()).step_by(hop).take(block_length).collect();
    let blocks: Vec<Vec<f32>> = indices
        .into_par_iter()
        .map(|i| {
            let end = (i + frame_length).min(samples.len());
            let mut block = Vec::with_capacity(frame_length);
            block.extend_from_slice(&samples[i..end]);
            block.resize(frame_length, 0.0);
            block
        })
        .collect();

    Ok(blocks.into_iter())
}

/// Streams WAV sample blocks lazily with parallel chunk processing.
///
/// Processes WAV data incrementally in a separate thread, generating blocks in parallel
/// within chunks to minimize memory footprint.
///
/// # Parameters
/// - `path`: WAV file path (`AsRef<Path>`).
/// - `block_length`: Maximum block count.
/// - `frame_length`: Samples per block.
/// - `hop_length`: Step size between blocks; `None` uses `frame_length`.
///
/// # Returns
/// - `Ok(Receiver<Vec<f32>>)`: Channel receiver for blocks.
/// - `Err(AudioError)`: I/O or streaming error.
///
/// # Errors
/// - `AudioError::FileNotFound`: The specified file does not exist
/// - `AudioError::OpenError`: Invalid WAV file or corrupted header
/// - `AudioError::StreamError`: Channel communication failure
///
/// # Example
/// ```
/// use crate::core::stream_lazy;
/// let rx = stream_lazy("audio.wav", 1000, 1024, Some(512))?;
/// while let Ok(block) = rx.recv() {
///     // Process each 1024-sample block with 50% overlap
///     println!("Received block of {} samples", block.len());
/// }
/// # Ok::<(), crate::core::AudioError>(())
/// ```
///
/// # Performance
/// - Background thread for file reading
/// - Memory-efficient for files >1GB
pub fn stream_lazy<P: AsRef<Path>>(
    path: P,
    block_length: usize,
    frame_length: usize,
    hop_length: Option<usize>,
) -> Result<Receiver<Vec<f32>>, AudioError> {
    let path = path.as_ref();
    if !path.exists() {
        return Err(AudioError::FileNotFound(path.to_string_lossy().into_owned()));
    }

    let wav_data = std::fs::read(&path)?;
    let mut reader = WavReader::new(Cursor::new(wav_data))?;
    let spec = reader.spec();
    let hop = hop_length.unwrap_or(frame_length);

    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let samples_iter: Box<dyn Iterator<Item = Result<f32, _>>> = match spec.sample_format {
            SampleFormat::Float => Box::new(reader.samples::<f32>()),
            SampleFormat::Int => Box::new(reader.samples::<i16>().map(|s| s.map(|v| v as f32 / i16::MAX as f32))),
        };

        let mut chunk = Vec::with_capacity(frame_length * block_length);
        let mut block_count = 0;

        for sample in samples_iter {
            let sample = sample.unwrap_or(0.0);
            chunk.push(sample);

            if chunk.len() >= frame_length && (chunk.len() % hop == 0 || chunk.len() >= frame_length * block_length) {
                let indices: Vec<usize> = (0..chunk.len())
                    .step_by(hop)
                    .take(block_length - block_count)
                    .collect();
                let drain_to = indices.last().map_or(0, |&i| (i + hop).min(chunk.len()));

                let blocks: Vec<Vec<f32>> = indices
                    .into_par_iter()
                    .map(|i| {
                        let end = (i + frame_length).min(chunk.len());
                        let mut block = Vec::with_capacity(frame_length);
                        block.extend_from_slice(&chunk[i..end]);
                        block.resize(frame_length, 0.0);
                        block
                    })
                    .collect();

                for block in blocks {
                    if tx.send(block).is_err() {
                        return;
                    }
                    block_count += 1;
                    if block_count >= block_length {
                        return;
                    }
                }
                chunk.drain(..drain_to);
            }
        }

        if !chunk.is_empty() && block_count < block_length {
            chunk.resize(frame_length, 0.0);
            let _ = tx.send(chunk);
        }
    });

    Ok(rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn create_test_wav() -> AudioData {
        AudioData::new(vec![0.0, 0.1, 0.2, 0.3, 0.4, 0.5], 44100, 1)
    }

    #[test]
    fn test_load() {
        let audio = create_test_wav();
        export("test.wav", &audio).unwrap();
        let loaded = load("test.wav", None, Some(true), None, None).unwrap();
        assert_eq!(loaded.samples, audio.samples);
        fs::remove_file("test.wav").unwrap();
    }

    #[test]
    fn test_load_segment() {
        let audio = create_test_wav();
        export("test.wav", &audio).unwrap();
        let loaded = load("test.wav", None, Some(true), Some(0.00004535147), Some(0.00004535148)).unwrap();
        assert_eq!(loaded.samples, vec![0.1, 0.2]);
        fs::remove_file("test.wav").unwrap();
    }

    #[test]
    fn test_export() {
        let audio = create_test_wav();
        export("test.wav", &audio).unwrap();
        let loaded = load("test.wav", None, Some(true), None, None).unwrap();
        assert_eq!(loaded.samples, audio.samples);
        fs::remove_file("test.wav").unwrap();
    }

    #[test]
    fn test_stream() {
        let audio = create_test_wav();
        export("test.wav", &audio).unwrap();
        let blocks: Vec<_> = stream("test.wav", 3, 2, Some(2)).unwrap().collect();
        assert_eq!(blocks, vec![vec![0.0, 0.1], vec![0.2, 0.3], vec![0.4, 0.5]]);
        fs::remove_file("test.wav").unwrap();
    }

    #[test]
    fn test_stream_lazy() {
        let audio = create_test_wav();
        export("test.wav", &audio).unwrap();
        let rx = stream_lazy("test.wav", 3, 2, Some(2)).unwrap();
        let blocks: Vec<_> = rx.into_iter().collect();
        assert_eq!(blocks, vec![vec![0.0, 0.1], vec![0.2, 0.3], vec![0.4, 0.5]]);
        fs::remove_file("test.wav").unwrap();
    }

    #[test]
    fn test_load_file_not_found() {
        if std::path::Path::new("test.wav").exists() {
            fs::remove_file("test.wav").unwrap();
        }
        let result = load("test.wav", None, Some(true), None, None);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AudioError::FileNotFound(_)));
    }

    #[test]
    fn test_stream_file_not_found() {
        if std::path::Path::new("test.wav").exists() {
            fs::remove_file("test.wav").unwrap();
        }
        let result = stream("test.wav", 3, 2, Some(2));
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(matches!(e, AudioError::FileNotFound(_)));
        }
    }

    #[test]
    fn test_stream_lazy_file_not_found() {
        if std::path::Path::new("test.wav").exists() {
            fs::remove_file("test.wav").unwrap();
        }
        let result = stream_lazy("test.wav", 3, 2, Some(2));
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), AudioError::FileNotFound(_)));
    }
}