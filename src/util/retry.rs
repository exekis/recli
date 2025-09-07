use rand::{rngs::StdRng, Rng, SeedableRng};
use std::time::Duration;
use tokio::time::sleep;

/// retry an async operation with exponential backoff and jitter
pub async fn retry<F, Fut, T, E>(
    mut op: F,
    max_retries: usize,
    base_delay_ms: u64,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
{
    let mut rng = StdRng::from_entropy();
    let mut attempt = 0usize;
    loop {
        match op().await {
            Ok(v) => return Ok(v),
            Err(e) => {
                if attempt >= max_retries {
                    return Err(e);
                }
                let expo = base_delay_ms.saturating_mul(1u64 << attempt.min(10));
                let jitter: u64 = rng.gen_range(0..(expo / 2 + 1));
                let delay = Duration::from_millis(expo + jitter);
                sleep(delay).await;
                attempt += 1;
            }
        }
    }
}
