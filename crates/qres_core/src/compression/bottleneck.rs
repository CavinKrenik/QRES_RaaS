//! 4-Layer Bottleneck Autoencoder for Summary Gene Compression
//!
//! Learns the most common sensor patterns and uses the bottleneck
//! representation as a compressed encoding. The architecture is:
//!
//! ```text
//! Input (D) -> Encoder Layer 1 (D -> D/2) -> Encoder Layer 2 (D/2 -> B)
//!     -> [Bottleneck B bytes] ->
//! Decoder Layer 3 (B -> D/2) -> Decoder Layer 4 (D/2 -> D) -> Output (D)
//! ```
//!
//! Training uses simple gradient descent with MSE loss.

use std::vec::Vec;

/// 4-layer bottleneck autoencoder.
///
/// Layers:
/// 1. Encoder: input_dim -> hidden_dim (with ReLU)
/// 2. Encoder: hidden_dim -> bottleneck_dim (with ReLU)
/// 3. Decoder: bottleneck_dim -> hidden_dim (with ReLU)
/// 4. Decoder: hidden_dim -> input_dim (linear output)
pub struct BottleneckAutoencoder {
    pub input_dim: usize,
    pub hidden_dim: usize,
    pub bottleneck_dim: usize,

    // Layer 1: input -> hidden
    pub w1: Vec<Vec<f32>>, // [hidden_dim x input_dim]
    pub b1: Vec<f32>,      // [hidden_dim]

    // Layer 2: hidden -> bottleneck
    pub w2: Vec<Vec<f32>>, // [bottleneck_dim x hidden_dim]
    pub b2: Vec<f32>,      // [bottleneck_dim]

    // Layer 3: bottleneck -> hidden
    pub w3: Vec<Vec<f32>>, // [hidden_dim x bottleneck_dim]
    pub b3: Vec<f32>,      // [hidden_dim]

    // Layer 4: hidden -> input
    pub w4: Vec<Vec<f32>>, // [input_dim x hidden_dim]
    pub b4: Vec<f32>,      // [input_dim]
}

impl BottleneckAutoencoder {
    /// Create a new autoencoder with Xavier-initialized weights.
    pub fn new(input_dim: usize, hidden_dim: usize, bottleneck_dim: usize) -> Self {
        let w1 = xavier_init(hidden_dim, input_dim);
        let b1 = vec![0.0; hidden_dim];
        let w2 = xavier_init(bottleneck_dim, hidden_dim);
        let b2 = vec![0.0; bottleneck_dim];
        let w3 = xavier_init(hidden_dim, bottleneck_dim);
        let b3 = vec![0.0; hidden_dim];
        let w4 = xavier_init(input_dim, hidden_dim);
        let b4 = vec![0.0; input_dim];

        Self {
            input_dim,
            hidden_dim,
            bottleneck_dim,
            w1,
            b1,
            w2,
            b2,
            w3,
            b3,
            w4,
            b4,
        }
    }

    /// Forward pass through the encoder only (compress).
    /// Returns the bottleneck representation.
    pub fn encode(&self, input: &[f32]) -> Vec<f32> {
        let h1 = relu(&affine(&self.w1, &self.b1, input));
        relu(&affine(&self.w2, &self.b2, &h1))
    }

    /// Forward pass through the decoder only (decompress).
    /// Takes bottleneck representation, returns reconstruction.
    pub fn decode(&self, bottleneck: &[f32]) -> Vec<f32> {
        let h3 = relu(&affine(&self.w3, &self.b3, bottleneck));
        affine(&self.w4, &self.b4, &h3) // Linear output
    }

    /// Full forward pass: encode then decode.
    /// Returns (bottleneck, reconstruction).
    pub fn forward(&self, input: &[f32]) -> (Vec<f32>, Vec<f32>) {
        let bottleneck = self.encode(input);
        let reconstruction = self.decode(&bottleneck);
        (bottleneck, reconstruction)
    }

    /// Train on a batch of samples using simple gradient descent.
    /// Returns the average MSE loss over the batch.
    pub fn train_batch(&mut self, samples: &[Vec<f32>], learning_rate: f32) -> f32 {
        if samples.is_empty() {
            return 0.0;
        }

        let mut total_loss = 0.0;

        for sample in samples {
            // Forward pass with intermediate activations
            let z1 = affine(&self.w1, &self.b1, sample);
            let h1 = relu(&z1);
            let z2 = affine(&self.w2, &self.b2, &h1);
            let h2 = relu(&z2);
            let z3 = affine(&self.w3, &self.b3, &h2);
            let h3 = relu(&z3);
            let output = affine(&self.w4, &self.b4, &h3);

            // MSE loss
            let loss: f32 = sample
                .iter()
                .zip(output.iter())
                .map(|(a, b)| (a - b) * (a - b))
                .sum::<f32>()
                / sample.len() as f32;
            total_loss += loss;

            // Backpropagation
            // dL/doutput = 2/N * (output - sample)
            let n = sample.len() as f32;
            let d_output: Vec<f32> = output
                .iter()
                .zip(sample.iter())
                .map(|(o, s)| 2.0 * (o - s) / n)
                .collect();

            // Layer 4 gradients
            let d_h3 = backward_affine_input(&self.w4, &d_output);
            update_weights(&mut self.w4, &mut self.b4, &h3, &d_output, learning_rate);

            // Layer 3 gradients (ReLU)
            let d_z3: Vec<f32> = d_h3
                .iter()
                .zip(z3.iter())
                .map(|(d, z)| if *z > 0.0 { *d } else { 0.0 })
                .collect();
            let d_h2 = backward_affine_input(&self.w3, &d_z3);
            update_weights(&mut self.w3, &mut self.b3, &h2, &d_z3, learning_rate);

            // Layer 2 gradients (ReLU)
            let d_z2: Vec<f32> = d_h2
                .iter()
                .zip(z2.iter())
                .map(|(d, z)| if *z > 0.0 { *d } else { 0.0 })
                .collect();
            let d_h1 = backward_affine_input(&self.w2, &d_z2);
            update_weights(&mut self.w2, &mut self.b2, &h1, &d_z2, learning_rate);

            // Layer 1 gradients (ReLU)
            let d_z1: Vec<f32> = d_h1
                .iter()
                .zip(z1.iter())
                .map(|(d, z)| if *z > 0.0 { *d } else { 0.0 })
                .collect();
            update_weights(&mut self.w1, &mut self.b1, sample, &d_z1, learning_rate);
        }

        total_loss / samples.len() as f32
    }

    /// Compress data to bytes: encode to bottleneck, then quantize to u8.
    pub fn compress_to_bytes(&self, input: &[f32]) -> Vec<u8> {
        let bottleneck = self.encode(input);
        // Find range for quantization
        let min_val = bottleneck.iter().cloned().fold(f32::INFINITY, f32::min);
        let max_val = bottleneck.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let range = (max_val - min_val).max(1e-10);

        let mut bytes = Vec::with_capacity(2 + 4 + 4 + bottleneck.len());
        // Header: bottleneck_dim (u16) + min (f32) + range (f32)
        bytes.extend_from_slice(&(bottleneck.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&min_val.to_le_bytes());
        bytes.extend_from_slice(&range.to_le_bytes());

        // Quantized bottleneck values
        for &v in &bottleneck {
            let normalized = ((v - min_val) / range * 255.0).clamp(0.0, 255.0);
            bytes.push(normalized as u8);
        }

        bytes
    }

    /// Decompress bytes back to original space.
    pub fn decompress_from_bytes(&self, bytes: &[u8]) -> Option<Vec<f32>> {
        if bytes.len() < 10 {
            return None;
        }

        let dim = u16::from_le_bytes([bytes[0], bytes[1]]) as usize;
        let min_val = f32::from_le_bytes([bytes[2], bytes[3], bytes[4], bytes[5]]);
        let range = f32::from_le_bytes([bytes[6], bytes[7], bytes[8], bytes[9]]);

        if bytes.len() < 10 + dim {
            return None;
        }

        // Dequantize
        let bottleneck: Vec<f32> = bytes[10..10 + dim]
            .iter()
            .map(|&b| min_val + (b as f32 / 255.0) * range)
            .collect();

        Some(self.decode(&bottleneck))
    }
}

/// Affine transformation: `output[i] = sum(w[i][j] * x[j]) + b[i]`
fn affine(w: &[Vec<f32>], b: &[f32], x: &[f32]) -> Vec<f32> {
    w.iter()
        .zip(b.iter())
        .map(|(row, &bias)| {
            let dot: f32 = row.iter().zip(x.iter()).map(|(&wi, &xi)| wi * xi).sum();
            dot + bias
        })
        .collect()
}

/// ReLU activation
fn relu(x: &[f32]) -> Vec<f32> {
    x.iter().map(|&v| v.max(0.0)).collect()
}

/// Compute dL/dx given dL/dy and W (for y = Wx + b)
fn backward_affine_input(w: &[Vec<f32>], d_output: &[f32]) -> Vec<f32> {
    if w.is_empty() {
        return Vec::new();
    }
    let input_dim = w[0].len();
    let mut d_input = vec![0.0f32; input_dim];
    for (row, &dy) in w.iter().zip(d_output.iter()) {
        for (j, &wij) in row.iter().enumerate() {
            d_input[j] += wij * dy;
        }
    }
    d_input
}

/// Update weights via SGD: W -= lr * dL/dW, b -= lr * dL/db
fn update_weights(w: &mut [Vec<f32>], b: &mut [f32], input: &[f32], d_output: &[f32], lr: f32) {
    for (i, (row, &dy)) in w.iter_mut().zip(d_output.iter()).enumerate() {
        for (j, wij) in row.iter_mut().enumerate() {
            if j < input.len() {
                *wij -= lr * dy * input[j];
            }
        }
        b[i] -= lr * dy;
    }
}

/// Xavier initialization for weight matrix
fn xavier_init(rows: usize, cols: usize) -> Vec<Vec<f32>> {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let scale = (2.0 / (rows + cols) as f64).sqrt() as f32;

    (0..rows)
        .map(|_| (0..cols).map(|_| rng.gen_range(-scale..scale)).collect())
        .collect()
}

/// Compute MSE between two vectors
pub fn mse(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return f32::MAX;
    }
    let sum: f32 = a.iter().zip(b.iter()).map(|(x, y)| (x - y) * (x - y)).sum();
    sum / a.len() as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_autoencoder_learns_identity() {
        // Train a small autoencoder to learn near-identity mapping
        let input_dim = 8;
        let hidden_dim = 6;
        let bottleneck_dim = 4;

        let mut ae = BottleneckAutoencoder::new(input_dim, hidden_dim, bottleneck_dim);

        // Generate training data: simple patterns
        let samples: Vec<Vec<f32>> = (0..100)
            .map(|i| {
                let phase = i as f32 * 0.1;
                (0..input_dim)
                    .map(|j| (phase + j as f32 * 0.5).sin() * 0.5 + 0.5)
                    .collect()
            })
            .collect();

        // Train for many epochs
        let mut last_loss = f32::MAX;
        for epoch in 0..200 {
            let loss = ae.train_batch(&samples, 0.05);
            if epoch % 50 == 0 {
                println!("Epoch {}: loss = {:.6}", epoch, loss);
            }
            last_loss = loss;
        }

        println!("Final loss: {:.6}", last_loss);
        assert!(
            last_loss < 0.1,
            "Autoencoder should converge (loss={:.6})",
            last_loss
        );
    }

    #[test]
    fn test_compression_size_reduction() {
        let input_dim = 37; // ~74 bytes as f32 pairs (matching Summary Gene)
        let hidden_dim = 20;
        let bottleneck_dim = 12; // Target: <50 bytes

        let mut ae = BottleneckAutoencoder::new(input_dim, hidden_dim, bottleneck_dim);

        // Train on representative data
        let samples: Vec<Vec<f32>> = (0..200)
            .map(|i| {
                let phase = i as f32 * 0.05;
                (0..input_dim)
                    .map(|j| (phase + j as f32 * 0.3).sin() * 0.3 + 0.5)
                    .collect()
            })
            .collect();

        for _ in 0..300 {
            ae.train_batch(&samples, 0.005);
        }

        // Test compression
        let test_input = &samples[50];
        let compressed = ae.compress_to_bytes(test_input);
        let compressed_size = compressed.len();

        // Original size: input_dim * 4 bytes (f32) = 148 bytes
        let original_size = input_dim * 4;

        println!("Original size: {} bytes", original_size);
        println!("Compressed size: {} bytes", compressed_size);
        println!(
            "Compression ratio: {:.1}x",
            original_size as f32 / compressed_size as f32
        );

        // Header (10 bytes) + bottleneck_dim quantized values
        let expected_size = 10 + bottleneck_dim;
        assert_eq!(
            compressed_size, expected_size,
            "Compressed size should be header + bottleneck"
        );

        // Verify it's smaller than 50 bytes
        assert!(
            compressed_size < 50,
            "Compressed size {} should be < 50 bytes",
            compressed_size
        );

        // Verify reconstruction quality
        let reconstructed = ae.decompress_from_bytes(&compressed).unwrap();
        let reconstruction_error = mse(test_input, &reconstructed);
        println!("Reconstruction MSE: {:.6}", reconstruction_error);

        // Note: with u8 quantization we accept higher error than pure float bottleneck
        assert!(
            reconstruction_error < 0.5,
            "Reconstruction error too high: {:.6}",
            reconstruction_error
        );
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let ae = BottleneckAutoencoder::new(8, 6, 4);
        let input = vec![0.5; 8];

        let (bottleneck, reconstruction) = ae.forward(&input);
        assert_eq!(bottleneck.len(), 4);
        assert_eq!(reconstruction.len(), 8);
    }
}
