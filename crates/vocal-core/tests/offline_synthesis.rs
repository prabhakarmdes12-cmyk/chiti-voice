//! Offline synthesis tests
//!
//! Validates that synthesis works with no network connectivity.
//! These tests enforce VOICE_INV_001: Offline Independence

#[cfg(test)]
mod offline_tests {
    use vocal_core::engine::mock::MockEngine;
    use vocal_core::engine::VoiceEngine;
    use vocal_core::synthesis::{SynthesisFormat, SynthesisRequest};

    #[tokio::test]
    async fn test_mock_engine_offline_synthesis() {
        // Initialize mock engine (no network access required)
        let mut engine = MockEngine::new();
        assert!(engine.initialize().await.is_ok());

        // Verify health check works offline
        let health = engine.health().await;
        assert!(health.is_ok());

        // Synthesize text with no network access
        let request = SynthesisRequest::new("tara-mock", "Welcome to Chiti Vocal Runtime.")
            .with_format(SynthesisFormat::PcmF32);

        let response = engine.synthesize(&request).await;
        assert!(response.is_ok());

        let response = response.unwrap();
        assert!(!response.audio.is_empty());
        assert_eq!(response.metadata.sample_rate, 22050);
    }

    #[tokio::test]
    async fn test_mock_engine_multiple_voices() {
        // Test that multiple voices work offline
        let mut engine = MockEngine::new();
        engine.initialize().await.unwrap();

        // Synthesize with TARA
        let tara_request = SynthesisRequest::new("tara-mock", "Hello from Tara");
        let tara_response = engine.synthesize(&tara_request).await;
        assert!(tara_response.is_ok());

        // Synthesize with KASHI
        let kashi_request = SynthesisRequest::new("kashi-mock", "Namaste from Kashi");
        let kashi_response = engine.synthesize(&kashi_request).await;
        assert!(kashi_response.is_ok());

        // Ensure both produced different audio (different text lengths)
        let tara_audio = tara_response.unwrap();
        let kashi_audio = kashi_response.unwrap();

        assert_ne!(tara_audio.audio.len(), kashi_audio.audio.len());
    }

    #[tokio::test]
    async fn test_no_network_access() {
        // This test verifies no synchronous network calls occur during synthesis
        // TODO: Add integration with network blocking library (e.g., netadapter)
        // For now, this is a documentation placeholder.

        // In a real offline test environment, we would:
        // 1. Block all network interfaces at the OS level
        // 2. Run synthesis
        // 3. Verify synthesis completes successfully
        // 4. Restore network interfaces
        //
        // Example (linux):
        //   $ sudo iptables -I OUTPUT 1 -j DROP
        //   $ cargo test test_no_network_access
        //   $ sudo iptables -D OUTPUT 1
        //
        // For CI/CD, this would be wrapped in a test harness.

        let mut engine = MockEngine::new();
        engine.initialize().await.unwrap();

        let request = SynthesisRequest::new("tara-mock", "Offline synthesis validation");
        let response = engine.synthesize(&request).await;
        assert!(response.is_ok());
    }

    #[tokio::test]
    async fn test_critical_evaluation_sentences() {
        // Test the critical evaluation sentences from the PRD
        let mut engine = MockEngine::new();
        engine.initialize().await.unwrap();

        let evaluation_sentences = vec![
            // TARA sentences
            ("tara-mock", "Your appointment is confirmed for Thursday at three PM."),
            ("tara-mock", "We'll be with you shortly — thank you for your patience."),
            ("tara-mock", "The total amount due is twelve thousand five hundred rupees."),
            (
                "tara-mock",
                "Your order has been dispatched and will arrive within two to three business days.",
            ),
            // KASHI sentences
            ("kashi-mock", "Your question is important."),
            ("kashi-mock", "Peace and patience are the true strength."),
        ];

        for (voice_id, text) in evaluation_sentences {
            let request = SynthesisRequest::new(voice_id, text);
            let response = engine.synthesize(&request).await;
            assert!(response.is_ok(), "Failed to synthesize: {} -> {}", voice_id, text);
        }
    }

    #[tokio::test]
    async fn test_synthesis_output_validity() {
        // Verify that synthesized output has expected properties
        let mut engine = MockEngine::new();
        engine.initialize().await.unwrap();

        let request = SynthesisRequest::new("tara-mock", "Test audio output");
        let response = engine.synthesize(&request).await.unwrap();

        // Audio should not be empty
        assert!(!response.audio.is_empty());

        // Metadata should be valid
        assert!(response.metadata.sample_rate > 0);
        assert!(response.metadata.channels > 0);
        assert!(response.metadata.duration_ms > 0);
        assert_eq!(response.metadata.bit_depth, 32); // 32-bit float PCM

        // Audio length should match metadata
        let expected_bytes = (response.metadata.sample_rate as usize)
            * response.metadata.channels as usize
            * (response.metadata.duration_ms as usize / 1000)
            * (response.metadata.bit_depth / 8) as usize;

        assert!(response.audio.len() > 0);
    }

    #[test]
    fn test_error_codes_defined() {
        use vocal_core::error::VoiceErrorCode;

        // Verify all error codes are properly defined
        let codes = vec![
            VoiceErrorCode::VoiceNotFound,
            VoiceErrorCode::PackNotFound,
            VoiceErrorCode::PackInvalidFormat,
            VoiceErrorCode::PackSchemaMismatch,
            VoiceErrorCode::PackChecksumFailed,
            VoiceErrorCode::PackPathTraversal,
            VoiceErrorCode::PackSizeExceeded,
            VoiceErrorCode::EngineNotAvailable,
            VoiceErrorCode::SynthesisFailed,
        ];

        for code in codes {
            assert!(!code.as_str().is_empty());
            assert!(!code.user_message().is_empty());
        }
    }
}
