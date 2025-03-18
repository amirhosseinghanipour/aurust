use crate::io::AudioData;
use ndarray::Array2;

/// Calculates the duration of an audio signal in seconds.
///
/// # Arguments
/// * `audio` - Reference to an `AudioData` struct containing samples and sample rate
///
/// # Returns
/// Returns a `f32` representing the duration in seconds.
///
/// # Examples
/// ```
/// let audio = AudioData { samples: vec![0.0; 44100], sample_rate: 44100 };
/// let duration = get_duration(&audio);
/// assert_eq!(duration, 1.0); // 1 second
/// ```
pub fn get_duration(audio: &AudioData) -> f32 {
    audio.samples.len() as f32 / audio.sample_rate as f32
}

/// Calculates the duration of an audio file from its path.
///
/// # Arguments
/// * `path` - Path to the audio file, implementing `AsRef<Path>`
///
/// # Returns
/// Returns a `Result<f32, AudioError>` containing the duration in seconds or an error if loading fails.
///
/// # Errors
/// Returns `AudioError` if the audio file cannot be loaded.
///
/// # Examples
/// ```
/// let duration = get_duration_from_path("test.wav");
/// // Assuming test.wav is 2 seconds long at 44100 Hz
/// assert!(duration.is_ok_and(|d| d == 2.0));
/// ```
pub fn get_duration_from_path<P: AsRef<std::path::Path>>(path: P) -> Result<f32, crate::io::AudioError> {
    let audio = crate::io::load(path, None, None, None, None)?;
    Ok(get_duration(&audio))
}

/// Converts frame indices to sample indices.
///
/// # Arguments
/// * `frames` - Array of frame indices
/// * `hop_length` - Optional hop length in samples (defaults to 512)
/// * `_n_fft` - Optional FFT size (unused, defaults to None)
///
/// # Returns
/// Returns a `Vec<usize>` containing corresponding sample indices.
///
/// # Examples
/// ```
/// let frames = vec![0, 1, 2];
/// let samples = frames_to_samples(&frames, None, None);
/// assert_eq!(samples, vec![0, 512, 1024]);
/// ```
pub fn frames_to_samples(frames: &[usize], hop_length: Option<usize>, _n_fft: Option<usize>) -> Vec<usize> {
    let hop = hop_length.unwrap_or(512);
    frames.iter().map(|&f| f * hop).collect()
}

/// Converts frame indices to time values in seconds.
///
/// # Arguments
/// * `frames` - Array of frame indices
/// * `sr` - Optional sample rate in Hz (defaults to 44100)
/// * `hop_length` - Optional hop length in samples (defaults to 512)
///
/// # Returns
/// Returns a `Vec<f32>` containing corresponding time values in seconds.
///
/// # Examples
/// ```
/// let frames = vec![0, 1, 2];
/// let times = frames_to_time(&frames, None, None);
/// assert_eq!(times, vec![0.0, 0.011609977, 0.023219954]); // Approx at 44100 Hz, hop 512
/// ```
pub fn frames_to_time(frames: &[usize], sr: Option<u32>, hop_length: Option<usize>) -> Vec<f32> {
    let sample_rate = sr.unwrap_or(44100);
    let hop = hop_length.unwrap_or(512);
    frames.iter().map(|&f| f as f32 * hop as f32 / sample_rate as f32).collect()
}

/// Converts sample indices to frame indices.
///
/// # Arguments
/// * `samples` - Array of sample indices
/// * `hop_length` - Optional hop length in samples (defaults to 512)
///
/// # Returns
/// Returns a `Vec<usize>` containing corresponding frame indices (integer division).
///
/// # Examples
/// ```
/// let samples = vec![0, 512, 1024];
/// let frames = samples_to_frames(&samples, None);
/// assert_eq!(frames, vec![0, 1, 2]);
/// ```
pub fn samples_to_frames(samples: &[usize], hop_length: Option<usize>) -> Vec<usize> {
    let hop = hop_length.unwrap_or(512);
    samples.iter().map(|&s| s / hop).collect()
}

/// Converts sample indices to time values in seconds.
///
/// # Arguments
/// * `samples` - Array of sample indices
/// * `sr` - Optional sample rate in Hz (defaults to 44100)
///
/// # Returns
/// Returns a `Vec<f32>` containing corresponding time values in seconds.
///
/// # Examples
/// ```
/// let samples = vec![0, 44100];
/// let times = samples_to_time(&samples, None);
/// assert_eq!(times, vec![0.0, 1.0]);
/// ```
pub fn samples_to_time(samples: &[usize], sr: Option<u32>) -> Vec<f32> {
    let sample_rate = sr.unwrap_or(44100);
    samples.iter().map(|&s| s as f32 / sample_rate as f32).collect()
}

/// Converts time values in seconds to frame indices.
///
/// # Arguments
/// * `times` - Array of time values in seconds
/// * `sr` - Optional sample rate in Hz (defaults to 44100)
/// * `hop_length` - Optional hop length in samples (defaults to 512)
/// * `_n_fft` - Optional FFT size (unused, defaults to None)
///
/// # Returns
/// Returns a `Vec<usize>` containing corresponding frame indices.
///
/// # Examples
/// ```
/// let times = vec![0.0, 0.011609977];
/// let frames = time_to_frames(&times, None, None, None);
/// assert_eq!(frames, vec![0, 1]);
/// ```
pub fn time_to_frames(times: &[f32], sr: Option<u32>, hop_length: Option<usize>, _n_fft: Option<usize>) -> Vec<usize> {
    let sample_rate = sr.unwrap_or(44100);
    let hop = hop_length.unwrap_or(512);
    times.iter().map(|&t| (t * sample_rate as f32 / hop as f32) as usize).collect()
}

/// Converts time values in seconds to sample indices.
///
/// # Arguments
/// * `times` - Array of time values in seconds
/// * `sr` - Optional sample rate in Hz (defaults to 44100)
///
/// # Returns
/// Returns a `Vec<usize>` containing corresponding sample indices.
///
/// # Examples
/// ```
/// let times = vec![0.0, 1.0];
/// let samples = time_to_samples(&times, None);
/// assert_eq!(samples, vec![0, 44100]);
/// ```
pub fn time_to_samples(times: &[f32], sr: Option<u32>) -> Vec<usize> {
    let sample_rate = sr.unwrap_or(44100);
    times.iter().map(|&t| (t * sample_rate as f32) as usize).collect()
}

/// Converts block indices to frame indices.
///
/// # Arguments
/// * `blocks` - Array of block indices
/// * `block_length` - Number of frames per block
///
/// # Returns
/// Returns a `Vec<usize>` containing corresponding frame indices.
///
/// # Examples
/// ```
/// let blocks = vec![0, 1, 2];
/// let frames = blocks_to_frames(&blocks, 10);
/// assert_eq!(frames, vec![0, 10, 20]);
/// ```
pub fn blocks_to_frames(blocks: &[usize], block_length: usize) -> Vec<usize> {
    blocks.iter().map(|&b| b * block_length).collect()
}

/// Converts block indices to sample indices.
///
/// # Arguments
/// * `blocks` - Array of block indices
/// * `block_length` - Number of frames per block
/// * `hop_length` - Optional hop length in samples (defaults to 512)
///
/// # Returns
/// Returns a `Vec<usize>` containing corresponding sample indices.
///
/// # Examples
/// ```
/// let blocks = vec![0, 1];
/// let samples = blocks_to_samples(&blocks, 2, None);
/// assert_eq!(samples, vec![0, 1024]); // 2 frames * 512 hop
/// ```
pub fn blocks_to_samples(blocks: &[usize], block_length: usize, hop_length: Option<usize>) -> Vec<usize> {
    let hop = hop_length.unwrap_or(512);
    blocks.iter().map(|&b| b * block_length * hop).collect()
}

/// Converts block indices to time values in seconds.
///
/// # Arguments
/// * `blocks` - Array of block indices
/// * `block_length` - Number of frames per block
/// * `hop_length` - Optional hop length in samples (defaults to 512)
/// * `sr` - Optional sample rate in Hz (defaults to 44100)
///
/// # Returns
/// Returns a `Vec<f32>` containing corresponding time values in seconds.
///
/// # Examples
/// ```
/// let blocks = vec![0, 1];
/// let times = blocks_to_time(&blocks, 2, None, None);
/// assert_eq!(times, vec![0.0, 0.023219954]); // 2 frames * 512 hop / 44100 Hz
/// ```
pub fn blocks_to_time(blocks: &[usize], block_length: usize, hop_length: Option<usize>, sr: Option<u32>) -> Vec<f32> {
    let hop = hop_length.unwrap_or(512);
    let sample_rate = sr.unwrap_or(44100);
    blocks.iter().map(|&b| b as f32 * block_length as f32 * hop as f32 / sample_rate as f32).collect()
}

/// Generates sample indices corresponding to the columns of a 2D array.
///
/// # Arguments
/// * `X` - 2D array (typically a spectrogram)
/// * `hop_length` - Optional hop length in samples (defaults to 512)
/// * `_n_fft` - Optional FFT size (unused, defaults to None)
/// * `_axis` - Optional axis (unused, defaults to None)
///
/// # Returns
/// Returns a `Vec<usize>` containing sample indices for each column of `X`.
///
/// # Examples
/// ```
/// use ndarray::arr2;
/// let X = arr2(&[[1.0, 2.0], [3.0, 4.0]]);
/// let samples = samples_like(&X, None, None, None);
/// assert_eq!(samples, vec![0, 512]);
/// ```
pub fn samples_like(X: &Array2<f32>, hop_length: Option<usize>, _n_fft: Option<usize>, _axis: Option<isize>) -> Vec<usize> {
    let hop = hop_length.unwrap_or(512);
    (0..X.shape()[1]).map(|i| i * hop).collect()
}

/// Generates time values corresponding to the columns of a 2D array.
///
/// # Arguments
/// * `X` - 2D array (typically a spectrogram)
/// * `sr` - Optional sample rate in Hz (defaults to 44100)
/// * `hop_length` - Optional hop length in samples (defaults to 512)
/// * `_n_fft` - Optional FFT size (unused, defaults to None)
/// * `_axis` - Optional axis (unused, defaults to None)
///
/// # Returns
/// Returns a `Vec<f32>` containing time values in seconds for each column of `X`.
///
/// # Examples
/// ```
/// use ndarray::arr2;
/// let X = arr2(&[[1.0, 2.0], [3.0, 4.0]]);
/// let times = times_like(&X, None, None, None, None);
/// assert_eq!(times, vec![0.0, 0.011609977]); // 512 hop / 44100 Hz
/// ```
pub fn times_like(X: &Array2<f32>, sr: Option<u32>, hop_length: Option<usize>, _n_fft: Option<usize>, _axis: Option<isize>) -> Vec<f32> {
    let sample_rate = sr.unwrap_or(44100);
    let hop = hop_length.unwrap_or(512);
    (0..X.shape()[1]).map(|i| i as f32 * hop as f32 / sample_rate as f32).collect()
}