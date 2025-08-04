use crate::utils;
use ndarray::{Array, ArrayBase, ArrayD, Dim, IxDynImpl, OwnedRepr};
use std::path::Path;

#[derive(Debug)]
pub struct Silero {
    _model_path: String, // Store for future use
    sample_rate: i64,
    state: ArrayBase<OwnedRepr<f32>, Dim<IxDynImpl>>,
}

impl Silero {
    pub fn new(
        sample_rate: utils::SampleRate,
        model_path: impl AsRef<Path>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let state = ArrayD::<f32>::zeros([2, 1, 128].as_slice());
        Ok(Self {
            _model_path: model_path.as_ref().to_string_lossy().to_string(),
            sample_rate: sample_rate.into(),
            state,
        })
    }

    pub fn reset(&mut self) {
        self.state = ArrayD::<f32>::zeros([2, 1, 128].as_slice());
    }

    pub fn calc_level(&mut self, _audio_frame: &[i16]) -> Result<f32, Box<dyn std::error::Error>> {
        // For now, return a mock value
        // TODO: Implement actual ONNX inference when the API is stable
        Ok(0.5) // Mock speech probability
    }
}