use super::*;
use std::io::{Error, ErrorKind};

fn busy() -> Error {
    Error::from(ErrorKind::ExecutableFileBusy)
}

#[test]
fn a_binary_that_stays_busy_is_retried_a_bounded_number_of_times() {
    // fed ETXTBSY on every platform (macOS never returns it for a file open for writing)
    let mut attempts = 0;
    let got: std::io::Result<()> = retry_busy(|| {
        attempts += 1;
        assert!(attempts <= 100, "the retries never end");
        Err(busy())
    });
    assert_eq!(got.unwrap_err().kind(), ErrorKind::ExecutableFileBusy);
    assert_eq!(attempts, SPAWN_TRIES + 1);
}

#[test]
fn a_binary_busy_for_a_moment_is_run_and_other_errors_return_at_once() {
    let mut attempts = 0;
    let got = retry_busy(|| {
        attempts += 1;
        if attempts < 3 {
            Err(busy())
        } else {
            Ok(attempts)
        }
    });
    assert_eq!(got.unwrap(), 3);
    let mut attempts = 0;
    let got: std::io::Result<()> = retry_busy(|| {
        attempts += 1;
        Err(Error::from(ErrorKind::NotFound))
    });
    assert_eq!(
        (got.unwrap_err().kind(), attempts),
        (ErrorKind::NotFound, 1)
    );
}
