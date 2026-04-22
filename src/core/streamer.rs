use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use rodio::{Decoder, Source};
use crate::core::remote::{RemoteManager, Command, pb};
use crate::core::remote::pb::{StreamSetup, StreamData, AudioFormat};
use tokio::sync::mpsc;

pub struct AudioStreamer {
    cancel_tx: Option<mpsc::Sender<()>>,
}

impl AudioStreamer {
    pub fn new() -> Self {
        Self { cancel_tx: None }
    }

    pub async fn stop(&mut self) {
        if let Some(tx) = self.cancel_tx.take() {
            let _ = tx.send(()).await;
        }
    }

    pub async fn stream_file(
        &mut self,
        path: &Path,
        mut stream: TcpStream,
    ) -> anyhow::Result<()> {
        self.stop().await;
        
        let (tx, mut rx) = mpsc::channel(1);
        self.cancel_tx = Some(tx);

        let path_owned = path.to_path_buf();

        tokio::spawn(async move {
            tracing::info!(path = ?path_owned, "Starting audio stream task");
            
            let file = match File::open(&path_owned) {
                Ok(f) => f,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to open file for streaming");
                    return;
                }
            };

            let source = match Decoder::new(BufReader::new(file)) {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!(error = %e, "Failed to decode file for streaming");
                    return;
                }
            };

            let sample_rate = source.sample_rate();
            let channels = source.channels();
            
            // 1. Send StreamSetup
            let setup = StreamSetup {
                stream_id: 1,
                sample_rate,
                channels: channels as u32,
                format: AudioFormat::Pcm as i32,
                bit_depth: 32, // f32
            };

            if let Err(e) = RemoteManager::send_command(&mut stream, Command::StreamSetup(setup)).await {
                tracing::error!(error = %e, "Failed to send StreamSetup");
                return;
            }

            let mut sequence = 0;
            let chunk_size = 1024 * channels as usize;
            let mut samples = source.convert_samples::<f32>();
            
            let start_time = Instant::now();
            let mut total_samples_sent = 0u64;

            loop {
                if rx.try_recv().is_ok() {
                    tracing::info!("Streaming cancelled");
                    break;
                }

                let mut chunk = Vec::with_capacity(chunk_size);
                for _ in 0..chunk_size {
                    if let Some(s) = samples.next() {
                        chunk.push(s);
                    } else {
                        break;
                    }
                }

                if chunk.is_empty() {
                    tracing::info!("Streaming finished: end of file");
                    break;
                }

                let data: Vec<u8> = chunk.iter().flat_map(|s| s.to_le_bytes().to_vec()).collect();
                let packet = StreamData {
                    stream_id: 1,
                    sequence,
                    timestamp_us: (total_samples_sent * 1_000_000 / (sample_rate as u64 * channels as u64)),
                    data,
                };

                if let Err(e) = RemoteManager::send_command(&mut stream, Command::StreamPacket(packet)).await {
                    tracing::error!(error = %e, "Failed to send StreamPacket");
                    break;
                }

                sequence += 1;
                total_samples_sent += chunk.len() as u64;

                // Simple flow control: wait until we should have played this much
                let elapsed = start_time.elapsed();
                let expected_elapsed = Duration::from_secs_f64(total_samples_sent as f64 / (sample_rate as f64 * channels as f64));
                
                if expected_elapsed > elapsed {
                    tokio::time::sleep(expected_elapsed - elapsed).await;
                }
            }
        });

        Ok(())
    }
}
