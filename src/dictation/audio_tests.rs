use super::{
    f32_interleaved_to_mono, i16_interleaved_to_mono, resample_to_target_rate,
    u16_interleaved_to_mono, BoundedAudioBuffer, TARGET_SAMPLE_RATE,
};

#[test]
fn bounded_audio_buffer_caps_recording_length() {
    let mut buffer = BoundedAudioBuffer::new(5);

    assert!(!buffer.push_samples(&[0.1, 0.2, 0.3]));
    assert!(buffer.push_samples(&[0.4, 0.5, 0.6]));

    assert_eq!(buffer.len(), 5);
    assert_eq!(buffer.finish(), vec![0.1, 0.2, 0.3, 0.4, 0.5]);
}

#[test]
fn bounded_audio_buffer_reports_empty_state() {
    let mut buffer = BoundedAudioBuffer::new(2);

    assert!(buffer.is_empty());
    buffer.push_samples(&[0.25]);
    assert!(!buffer.is_empty());
}

#[test]
fn interleaved_f32_audio_is_mixed_to_clamped_mono() {
    let stereo = [1.2, -0.2, 0.25, 0.75];

    let mono = f32_interleaved_to_mono(&stereo, 2).unwrap();

    assert_eq!(mono, vec![0.4, 0.5]);
}

#[test]
fn interleaved_i16_audio_is_normalized_to_mono() {
    let stereo = [32_767, -32_768, 16_384, 16_384];

    let mono = i16_interleaved_to_mono(&stereo, 2).unwrap();

    assert!((mono[0] - -0.000015258789).abs() < 0.000001);
    assert_eq!(mono[1], 0.5);
}

#[test]
fn interleaved_u16_audio_is_normalized_to_mono() {
    let stereo = [65_535, 0, 49_152, 49_152];

    let mono = u16_interleaved_to_mono(&stereo, 2).unwrap();

    assert!((mono[0] - -0.000015258789).abs() < 0.000001);
    assert_eq!(mono[1], 0.5);
}

#[test]
fn interleaved_audio_rejects_invalid_channel_layouts() {
    assert!(f32_interleaved_to_mono(&[0.0], 0).is_err());
    assert!(f32_interleaved_to_mono(&[0.0, 1.0, 2.0], 2).is_err());
}

#[test]
fn resampling_same_rate_preserves_samples() {
    let samples = vec![0.0, 0.5, -0.5];

    let resampled = resample_to_target_rate(&samples, TARGET_SAMPLE_RATE).unwrap();

    assert_eq!(resampled, samples);
}

#[test]
fn resampling_changes_sample_count_for_common_input_rate() {
    let samples = vec![0.0; 48_000];

    let resampled = resample_to_target_rate(&samples, 48_000).unwrap();

    assert_eq!(resampled.len(), TARGET_SAMPLE_RATE as usize);
}
