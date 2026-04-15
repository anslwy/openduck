use ndarray::{Array, Array2, ArrayBase, ArrayD, Dim, IxDynImpl, OwnedRepr};
use ort::session::Session;
use ort::value::Value;
use std::mem::take;
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub enum SampleRate {
    EightkHz,
    SixteenkHz,
}

impl From<SampleRate> for i64 {
    fn from(value: SampleRate) -> Self {
        match value {
            SampleRate::EightkHz => 8000,
            SampleRate::SixteenkHz => 16000,
        }
    }
}

pub struct Silero {
    session: Session,
    sample_rate: ArrayBase<OwnedRepr<i64>, Dim<[usize; 1]>>,
    state: ArrayBase<OwnedRepr<f32>, Dim<IxDynImpl>>,
    buffer: Vec<f32>,
    last_prob: f32,
}

impl Silero {
    pub fn new(sample_rate_enum: SampleRate, model_path: impl AsRef<Path>) -> Result<Self, String> {
        let session = Session::builder()
            .map_err(|e| e.to_string())?
            .commit_from_file(model_path)
            .map_err(|e| e.to_string())?;

        let state = ArrayD::<f32>::zeros([2, 1, 128].as_slice());
        let sample_rate_val: i64 = sample_rate_enum.into();
        let sample_rate = Array::from_shape_vec([1], vec![sample_rate_val]).unwrap();
        Ok(Self {
            session,
            sample_rate,
            state,
            buffer: Vec::new(),
            last_prob: 0.0,
        })
    }

    pub fn reset(&mut self) {
        self.state = ArrayD::<f32>::zeros([2, 1, 128].as_slice());
        self.buffer.clear();
        self.last_prob = 0.0;
    }

    pub fn calc_level(&mut self, audio_frame: &[f32]) -> Result<f32, String> {
        self.buffer.extend_from_slice(audio_frame);

        let frame_size = 512; // Silero VAD v4 expects 512 samples for 16kHz

        while self.buffer.len() >= frame_size {
            let data: Vec<f32> = self.buffer.drain(..frame_size).collect();

            let frame =
                Array2::<f32>::from_shape_vec([1, data.len()], data).map_err(|e| e.to_string())?;

            let frame_value = Value::from_array(frame).map_err(|e| e.to_string())?;
            let state_value =
                Value::from_array(take(&mut self.state)).map_err(|e| e.to_string())?;
            let sr_value =
                Value::from_array(self.sample_rate.clone()).map_err(|e| e.to_string())?;

            // Using positional inputs: [input, state, sr]
            let res = self
                .session
                .run([
                    (&frame_value).into(),
                    (&state_value).into(),
                    (&sr_value).into(),
                ])
                .map_err(|e| e.to_string())?;

            let (shape, state_data) = res["stateN"]
                .try_extract_tensor::<f32>()
                .map_err(|e| e.to_string())?;
            let shape_usize: Vec<usize> = shape.as_ref().iter().map(|&d| d as usize).collect();
            self.state =
                ArrayD::from_shape_vec(shape_usize.as_slice(), state_data.to_vec()).unwrap();

            self.last_prob = *res["output"]
                .try_extract_tensor::<f32>()
                .map_err(|e| e.to_string())?
                .1
                .first()
                .ok_or_else(|| "No output from VAD model".to_string())?;
        }

        Ok(self.last_prob)
    }
}

pub fn resample_to_16k(samples: &[f32], from_rate: u32) -> Vec<f32> {
    if from_rate == 16000 {
        return samples.to_vec();
    }
    let mut resampled = Vec::new();
    let ratio = 16000.0 / from_rate as f32;
    let mut current_pos = 0.0;
    while current_pos < samples.len() as f32 {
        let index = current_pos as usize;
        let frac = current_pos - index as f32;
        if index + 1 < samples.len() {
            resampled.push(samples[index] * (1.0 - frac) + samples[index + 1] * frac);
        } else {
            resampled.push(samples[index]);
        }
        current_pos += 1.0 / ratio;
    }
    resampled
}
