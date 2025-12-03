#[cfg(test)]
mod tests {
    use crate::perception::audio::{AudioProcessor, AudioEvent};
    use std::vec::Vec;

    #[test]
    fn test_silence_detection() {
        // Generate silence (all zeros)
        let samples = vec![0i16; 1000];
        let event = AudioProcessor::process(&samples);
        assert!(event.is_none(), "Silence should not trigger an event");
    }

    #[test]
    fn test_speech_detection() {
        // Generate "speech-like" signal (low frequency sine wave)
        // 100Hz sine wave at 16kHz sample rate
        let mut samples = Vec::new();
        for i in 0..1000 {
            let val = (i as f32 * 0.1).sin() * 20000.0;
            samples.push(val as i16);
        }
        
        let event = AudioProcessor::process(&samples);
        assert!(event.is_some(), "Speech-like signal should trigger event");
        
        let e = event.unwrap();
        assert_eq!(e.class_id, 1, "Should be classified as Speech (1)");
        assert!(e.energy > 0.1, "Energy should be significant");
        assert!(e.zcr < 0.3, "ZCR should be low for sine wave");
    }

    #[test]
    fn test_noise_detection() {
        // Generate "noise" (random high frequency)
        // Alternating +max, -max
        let mut samples = Vec::new();
        for i in 0..1000 {
            let val = if i % 2 == 0 { 20000 } else { -20000 };
            samples.push(val);
        }
        
        let event = AudioProcessor::process(&samples);
        assert!(event.is_some(), "Noise should trigger event");
        
        let e = event.unwrap();
        assert_eq!(e.class_id, 2, "Should be classified as Noise (2)");
        assert!(e.zcr > 0.9, "ZCR should be very high for alternating signal");
    }
}
